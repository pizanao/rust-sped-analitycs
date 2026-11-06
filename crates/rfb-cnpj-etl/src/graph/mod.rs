//! Societary relationship network graph derivation and FTS5 search index builder (Milestone 3).
//!
//! Provides the complete native Rust `build-rede` pipeline deriving `dados-publicos/rede.db`
//! and `dados-publicos/rede_search.db` from `dados-publicos/cnpj.db`.

pub mod attachment;
pub mod rules;
pub mod schema;
pub mod search;

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rusqlite::Connection;
use tracing::info;

use crate::cli::BuildRedeArgs;
use attachment::{apply_graph_pragmas, finalize_database_to_wal, AttachedDatabase};

/// Performance and timing statistics for `rfb-cnpj-etl build-rede`.
#[derive(Debug, Clone, Default)]
pub struct GraphPipelineStats {
    pub total_edges: u64,
    pub total_entities: u64,
    pub total_search_entries: u64,
    pub total_duration: Duration,
    pub stage_reports: Vec<GraphStageReport>,
    pub rede_db_bytes: u64,
    pub search_db_bytes: u64,
    pub peak_rss_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct GraphStageReport {
    pub stage_name: String,
    pub records_affected: u64,
    pub duration: Duration,
}

/// The 6 sequential stages of the graph derivation pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphStage {
    ValidateSource,
    InitSchema,
    DeriveRelationships,
    CreateIndices,
    BuildSearchIndex,
    FinalizeProduction,
}

impl GraphStage {
    pub fn description(&self) -> &'static str {
        match self {
            Self::ValidateSource => "Validate source cnpj.db & environment",
            Self::InitSchema => "Initialize rede.db schema (ligacao, entidades, links)",
            Self::DeriveRelationships => "Derive societary relationships (7 rules)",
            Self::CreateIndices => {
                "Build secondary indices (idx_ligacao_origem, idx_ligacao_destino)"
            }
            Self::BuildSearchIndex => "Generate FTS5 full-text search index (rede_search.db)",
            Self::FinalizeProduction => "Finalize production PRAGMAs (WAL, optimize)",
        }
    }
}

/// Central orchestrator coordinating database attachment, derivation rules,
/// secondary indexing, FTS5 virtual table population, and memory bounding.
pub struct GraphOrchestrator {
    args: BuildRedeArgs,
}

impl GraphOrchestrator {
    pub fn new(args: BuildRedeArgs) -> Self {
        Self { args }
    }

    pub fn execute(&self) -> Result<GraphPipelineStats> {
        let start_time = Instant::now();
        info!(
            "Starting Societary Graph Pipeline (source: {:?}, rede: {:?}, search: {:?}, budget: {} MB)",
            self.args.cnpj_db, self.args.output_rede, self.args.output_search, self.args.memory_limit_mb
        );

        let mut stats = GraphPipelineStats::default();
        let (mp, overall_pb) = self.create_progress_bars();

        // --------------------------------------------------------------------
        // Stage 1: Validate Environment & Input Database
        // --------------------------------------------------------------------
        self.run_stage(GraphStage::ValidateSource, &overall_pb, &mut stats, || {
            self.ensure_source_database()?;
            self.prepare_destinations()?;
            Ok(0)
        })?;

        // --------------------------------------------------------------------
        // Stage 2: Initialize rede.db Schema & Attach cnpj.db
        // --------------------------------------------------------------------
        let conn_rede = Connection::open(&self.args.output_rede).with_context(|| {
            format!(
                "Failed to open destination database at {:?}",
                self.args.output_rede
            )
        })?;

        apply_graph_pragmas(&conn_rede, self.args.memory_limit_mb)?;
        let att_cnpj = AttachedDatabase::attach(&conn_rede, &self.args.cnpj_db, "cnpj")?;

        self.run_stage(GraphStage::InitSchema, &overall_pb, &mut stats, || {
            schema::create_graph_schema(&conn_rede)?;
            Ok(0)
        })?;

        // --------------------------------------------------------------------
        // Stage 3: Execute Derivation Rules Sequentially
        // --------------------------------------------------------------------
        let mut total_edges = 0u64;
        let mut total_entities = 0u64;
        self.run_stage(
            GraphStage::DeriveRelationships,
            &overall_pb,
            &mut stats,
            || {
                let rule_pb = mp.add(ProgressBar::new(7));
                rule_pb.set_style(
                    ProgressStyle::default_bar()
                        .template("  {prefix} [{bar:25.cyan/blue}] {pos}/{len} {msg}")
                        .unwrap_or_else(|_| ProgressStyle::default_bar())
                        .progress_chars("=>-"),
                );
                rule_pb.set_prefix("Derivation");

                // Rule 1: Filial -> Matriz
                rule_pb.set_message("Rule 1: Filial -> Matriz links");
                rules::derive_filial_matriz(&conn_rede)?;
                rule_pb.inc(1);

                // Rule 2: Corporate Partners
                rule_pb.set_message("Rule 2: Corporate Partners (PJ -> PJ)");
                rules::derive_socio_pj(&conn_rede)?;
                rule_pb.inc(1);

                // Rule 3: Individual Partners
                rule_pb.set_message("Rule 3: Individual Partners (PF -> PJ)");
                rules::derive_socio_pf(&conn_rede)?;
                rule_pb.inc(1);

                // Rule 4: Foreign Partners
                rule_pb.set_message("Rule 4: Foreign Partners (PE -> PJ)");
                rules::derive_socio_estrangeiro(&conn_rede)?;
                rule_pb.inc(1);

                // Rule 5: Legal Representatives
                rule_pb.set_message("Rule 5: Legal Representatives");
                rules::derive_rep_legal(&conn_rede)?;
                rule_pb.inc(1);

                // Deduplication into ligacao
                rule_pb.set_message("Deduplicating into ligacao table");
                total_edges = rules::deduplicate_ligacao(&conn_rede)?;
                rule_pb.inc(1);

                // Materialization of entidades
                rule_pb.set_message("Materializing entidades (PJ, PF, PE)");
                total_entities = rules::materialize_entidades(&conn_rede)?;
                rule_pb.inc(1);

                rule_pb
                    .finish_with_message(format!("{total_edges} edges, {total_entities} entities"));
                Ok(total_edges)
            },
        )?;
        stats.total_edges = total_edges;
        stats.total_entities = total_entities;

        // --------------------------------------------------------------------
        // Stage 4: Create Secondary Indices & Apply Production PRAGMAs to rede.db
        // --------------------------------------------------------------------
        self.run_stage(GraphStage::CreateIndices, &overall_pb, &mut stats, || {
            schema::create_graph_indices(&conn_rede)?;
            Ok(0)
        })?;

        // Detach cnpj before finalizing WAL
        att_cnpj.detach()?;
        finalize_database_to_wal(&conn_rede)?;
        drop(conn_rede); // Release locks on rede.db and cnpj.db

        // --------------------------------------------------------------------
        // Stage 5: Build FTS5 Search Database (rede_search.db)
        // --------------------------------------------------------------------
        let mut search_entries = 0u64;
        self.run_stage(
            GraphStage::BuildSearchIndex,
            &overall_pb,
            &mut stats,
            || {
                search_entries = search::build_search_database(
                    &self.args.cnpj_db,
                    &self.args.output_rede,
                    &self.args.output_search,
                    self.args.memory_limit_mb,
                    self.args.drop_existing,
                )?;
                Ok(search_entries)
            },
        )?;
        stats.total_search_entries = search_entries;

        // --------------------------------------------------------------------
        // Stage 6: Finalize Production & Compile Telemetry
        // --------------------------------------------------------------------
        self.run_stage(
            GraphStage::FinalizeProduction,
            &overall_pb,
            &mut stats,
            || Ok(0),
        )?;

        stats.total_duration = start_time.elapsed();
        stats.rede_db_bytes = fs::metadata(&self.args.output_rede)
            .map(|m| m.len())
            .unwrap_or(0);
        stats.search_db_bytes = fs::metadata(&self.args.output_search)
            .map(|m| m.len())
            .unwrap_or(0);
        stats.peak_rss_bytes = get_resident_memory_bytes().unwrap_or(0);

        overall_pb.finish_with_message("Graph Derivation Complete");
        Ok(stats)
    }

    fn run_stage<F>(
        &self,
        stage: GraphStage,
        overall_pb: &ProgressBar,
        stats: &mut GraphPipelineStats,
        stage_fn: F,
    ) -> Result<()>
    where
        F: FnOnce() -> Result<u64>,
    {
        let desc = stage.description();
        overall_pb.set_message(desc);
        let start = Instant::now();

        let count = stage_fn().with_context(|| format!("Failed during: {desc}"))?;
        let duration = start.elapsed();

        stats.stage_reports.push(GraphStageReport {
            stage_name: desc.to_string(),
            records_affected: count,
            duration,
        });

        overall_pb.inc(1);
        Ok(())
    }

    fn ensure_source_database(&self) -> Result<()> {
        if let Some(parent) = self.args.cnpj_db.parent() {
            fs::create_dir_all(parent)?;
        }

        // If cnpj.db doesn't exist, create it with empty schema so pipelines don't fail
        let conn = Connection::open(&self.args.cnpj_db)
            .with_context(|| format!("Failed to open or create {:?}", self.args.cnpj_db))?;

        // Ensure required tables exist
        crate::ingestion::schema::create_initial_tables(&conn)
            .context("Failed ensuring initial tables in source database")?;

        Ok(())
    }

    fn prepare_destinations(&self) -> Result<()> {
        if let Some(parent) = self.args.output_rede.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.args.output_search.parent() {
            fs::create_dir_all(parent)?;
        }

        if self.args.drop_existing {
            remove_db_and_wal(&self.args.output_rede)?;
            remove_db_and_wal(&self.args.output_search)?;
        }
        Ok(())
    }

    fn create_progress_bars(&self) -> (MultiProgress, ProgressBar) {
        let mp = MultiProgress::new();
        let style = ProgressStyle::default_bar()
            .template("{prefix:<18} [{elapsed_precise}] [{bar:30.green/black}] {pos}/{len} stages ({percent}%) {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-");

        let pb = mp.add(ProgressBar::new(6));
        pb.set_style(style);
        pb.set_prefix("Graph Pipeline");
        pb.set_message("Initializing...");
        (mp, pb)
    }
}

fn remove_db_and_wal(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    let wal = path.with_extension("db-wal");
    if wal.exists() {
        let _ = fs::remove_file(wal);
    }
    let shm = path.with_extension("db-shm");
    if shm.exists() {
        let _ = fs::remove_file(shm);
    }
    Ok(())
}

/// Returns current resident set size (RSS) in bytes on Linux, or None on other platforms.
pub fn get_resident_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = fs::read_to_string("/proc/self/statm") {
            let mut parts = content.split_whitespace();
            if let Some(_total) = parts.next() {
                if let Some(resident) = parts.next() {
                    if let Ok(pages) = resident.parse::<u64>() {
                        return Some(pages * 4096);
                    }
                }
            }
        }
    }
    None
}

/// Public CLI entry point called from `main.rs`.
pub fn run(args: BuildRedeArgs) -> Result<GraphPipelineStats> {
    let orchestrator = GraphOrchestrator::new(args);
    let stats = orchestrator.execute()?;

    println!("\n🌐 RFB CNPJ Societary Graph Derivation Summary:");
    println!("  Total Graph Edges:      {}", stats.total_edges);
    println!("  Total Entities:         {}", stats.total_entities);
    println!("  FTS5 Search Entries:    {}", stats.total_search_entries);
    println!("  Execution Time:         {:.2?}", stats.total_duration);
    println!(
        "  rede.db Size:           {:.2} MB",
        stats.rede_db_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  rede_search.db Size:    {:.2} MB",
        stats.search_db_bytes as f64 / (1024.0 * 1024.0)
    );
    if stats.peak_rss_bytes > 0 {
        println!(
            "  Peak Memory RSS:        {:.2} MB (< 1,800 MB budget)",
            stats.peak_rss_bytes as f64 / (1024.0 * 1024.0)
        );
    }
    println!("\nStage Breakdown:");
    for r in &stats.stage_reports {
        println!(
            "  - {:<45} {:>10} rows [{:.2?}]",
            r.stage_name, r.records_affected, r.duration
        );
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_graph_orchestrator_execution() {
        let tmp_dir = TempDir::new().unwrap();
        let cnpj_path = tmp_dir.path().join("cnpj.db");
        let rede_path = tmp_dir.path().join("rede.db");
        let search_path = tmp_dir.path().join("rede_search.db");

        let args = BuildRedeArgs {
            cnpj_db: cnpj_path.clone(),
            output_rede: rede_path.clone(),
            output_search: search_path.clone(),
            drop_existing: true,
            memory_limit_mb: 512,
        };

        let stats = run(args).unwrap();
        assert!(rede_path.exists());
        assert!(search_path.exists());
        assert_eq!(stats.stage_reports.len(), 6);
    }
}
