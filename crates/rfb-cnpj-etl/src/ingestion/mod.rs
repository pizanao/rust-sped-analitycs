//! Ingestion orchestrator coordinating streaming decompression, parsing,
//! bounded MPSC channels, SQLite single-writer persistence, and progress tracking.

pub mod loader;
pub mod schema;
pub mod transforms;
pub mod zip_stream;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use crossbeam_channel::bounded;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use rusqlite::Connection;
use tracing::{debug, info, warn};

use crate::cli::BuildCnpjArgs;
use crate::config::{IngestionPragmas, BOUNDED_CHANNEL_CAPACITY};
pub use loader::*;
pub use schema::*;
pub use transforms::*;
pub use zip_stream::*;

/// Summary report for a completed ingestion run.
#[derive(Debug, Clone, Default)]
pub struct IngestionStats {
    pub total_rows: u64,
    pub total_duration: Duration,
    pub stage_reports: Vec<StageReport>,
    pub db_size_bytes: u64,
    pub peak_rss_bytes: u64,
}

/// Statistics for an individual table ingestion stage.
#[derive(Debug, Clone)]
pub struct StageReport {
    pub stage_name: String,
    pub rows_inserted: u64,
    pub duration: Duration,
    pub rows_per_sec: f64,
}

/// The 8 sequential ingestion pipeline stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestionStage {
    DomainTables,
    Empresas,
    Estabelecimentos,
    Simples,
    MaterializeMatriz,
    Socios,
    AuditMetadata,
    PostIndexing,
}

impl IngestionStage {
    pub fn description(&self) -> &'static str {
        match self {
            Self::DomainTables => "Domain Tables (Cnae, Motivo, etc.)",
            Self::Empresas => "Empresas (Enterprises)",
            Self::Estabelecimentos => "Estabelecimentos (Establishments)",
            Self::Simples => "Simples & MEI",
            Self::MaterializeMatriz => "Materialize Base2Matriz Cache",
            Self::Socios => "Socios (Corporate & PF Partners)",
            Self::AuditMetadata => "Audit Metadata (_referencia)",
            Self::PostIndexing => "Post-Load Index Creation",
        }
    }
}

/// Central orchestrator coordinating pipeline stages, Rayon workers,
/// bounded channels, and the dedicated SQLite loader thread.
pub struct IngestionOrchestrator {
    args: BuildCnpjArgs,
    pragmas: IngestionPragmas,
    thread_pool: rayon::ThreadPool,
}

impl IngestionOrchestrator {
    /// Construct a new orchestrator from validated CLI arguments.
    pub fn new(args: BuildCnpjArgs) -> Result<Self> {
        let threads = args.threads.clamp(1, 16);
        let pragmas = IngestionPragmas::from_memory_limit(args.memory_limit_mb, threads);

        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("cnpj-ingest-{i}"))
            .build()
            .context("Failed to initialize Rayon ingestion thread pool")?;

        Ok(Self {
            args,
            pragmas,
            thread_pool,
        })
    }

    /// Execute the complete ingestion pipeline.
    pub fn execute(&self) -> Result<IngestionStats> {
        let start_time = Instant::now();
        info!(
            "Starting RFB CNPJ ingestion pipeline (input: {:?}, output: {:?}, threads: {}, memory limit: {} MB)",
            self.args.input_dir, self.args.output_db, self.args.threads, self.args.memory_limit_mb
        );

        // 1. Validate inputs and environment
        self.validate_environment()?;

        // 2. Prepare database file & directories
        self.prepare_database_destination()?;

        // 3. Connect to SQLite and apply high-throughput ingestion PRAGMAs
        let mut conn = Connection::open(&self.args.output_db).with_context(|| {
            format!(
                "Failed to open SQLite database at {:?}",
                self.args.output_db
            )
        })?;
        self.pragmas.apply_to(&conn)?;

        // 4. Create raw table schemas without indices
        create_initial_tables(&conn)?;

        // 5. Initialize progress tracking
        let (mp, overall_pb) = self.create_progress_bars();

        let mut stats = IngestionStats::default();

        // 6. Execute Stage 1: Domain Tables
        self.run_stage(
            IngestionStage::DomainTables,
            &overall_pb,
            &mut stats,
            || self.ingest_domain_tables(&mut conn),
        )?;

        // 7. Execute Stage 2: Empresas (0..9)
        self.run_stage(IngestionStage::Empresas, &overall_pb, &mut stats, || {
            self.ingest_empresas(&mut conn, &mp)
        })?;

        // 8. Execute Stage 3: Estabelecimentos (0..9)
        self.run_stage(
            IngestionStage::Estabelecimentos,
            &overall_pb,
            &mut stats,
            || self.ingest_estabelecimentos(&mut conn, &mp),
        )?;

        // 9. Execute Stage 4: Simples
        self.run_stage(IngestionStage::Simples, &overall_pb, &mut stats, || {
            self.ingest_simples(&mut conn, &mp)
        })?;

        // 10. Execute Stage 5: Materialize Base2Matriz Cache
        self.run_stage(
            IngestionStage::MaterializeMatriz,
            &overall_pb,
            &mut stats,
            || materialize_cnpj_base2matriz(&mut conn).map_err(Into::into),
        )?;

        // 11. Execute Stage 6: Socios (0..9) with August 2026 radical resolution
        self.run_stage(IngestionStage::Socios, &overall_pb, &mut stats, || {
            self.ingest_socios(&mut conn, &mp)
        })?;

        // 12. Execute Stage 7: Audit Metadata (_referencia)
        self.run_stage(
            IngestionStage::AuditMetadata,
            &overall_pb,
            &mut stats,
            || self.insert_audit_metadata(&mut conn),
        )?;

        // 13. Execute Stage 8: Post-load indexing (if not skipped)
        if !self.args.skip_indices {
            self.run_stage(
                IngestionStage::PostIndexing,
                &overall_pb,
                &mut stats,
                || {
                    create_post_load_indices(&conn)?;
                    Ok(0)
                },
            )?;
        } else {
            warn!("Skipping secondary index creation due to --skip-indices flag");
        }

        // 14. Finalize database with post-load PRAGMAs (WAL mode, optimize)
        IngestionPragmas::apply_post_load(&conn)?;
        drop(conn);

        // 15. Compile final stats
        stats.total_duration = start_time.elapsed();
        stats.db_size_bytes = fs::metadata(&self.args.output_db)
            .map(|m| m.len())
            .unwrap_or(0);
        stats.peak_rss_bytes = get_resident_memory_bytes().unwrap_or(0);

        overall_pb.finish_with_message("Ingestion Complete");

        info!(
            "Pipeline successfully finished! Total rows: {}, Elapsed: {:.2?}, Output DB size: {:.2} MB, Peak RSS: {:.2} MB",
            stats.total_rows,
            stats.total_duration,
            stats.db_size_bytes as f64 / (1024.0 * 1024.0),
            stats.peak_rss_bytes as f64 / (1024.0 * 1024.0),
        );

        Ok(stats)
    }

    /// Helper to execute a single stage with timing, error handling, and progress updating.
    fn run_stage<F>(
        &self,
        stage: IngestionStage,
        overall_pb: &ProgressBar,
        stats: &mut IngestionStats,
        stage_fn: F,
    ) -> Result<()>
    where
        F: FnOnce() -> Result<u64>,
    {
        let desc = stage.description();
        overall_pb.set_message(desc);
        let start = Instant::now();

        let rows = stage_fn().with_context(|| format!("Failed during stage: {desc}"))?;
        let duration = start.elapsed();
        let rows_per_sec = if duration.as_secs_f64() > 0.0 {
            rows as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        stats.total_rows += rows;
        stats.stage_reports.push(StageReport {
            stage_name: desc.to_string(),
            rows_inserted: rows,
            duration,
            rows_per_sec,
        });

        overall_pb.inc(1);
        Ok(())
    }

    /// Stage 1: Ingest 6 small domain tables.
    fn ingest_domain_tables(&self, conn: &mut Connection) -> Result<u64> {
        let domain_files = [
            ("Cnaes", "cnae", ".CNAECSV"),
            ("Motivos", "motivo", ".MOTICSV"),
            ("Municipios", "municipio", ".MUNICCSV"),
            ("Naturezas", "natureza_juridica", ".NATJUCSV"),
            ("Paises", "pais", ".PAISCSV"),
            ("Qualificacoes", "qualificacao_socio", ".QUALSCSV"),
        ];

        let mut total_rows = 0u64;
        for (name, table_name, ext) in domain_files {
            if let Some(zip_path) = self.find_single_zip(name) {
                let rows = load_domain_file(conn, &zip_path, table_name, ext)?;
                total_rows += rows;
            } else {
                debug!("Domain zip file {:?} not found, skipping", name);
            }
        }
        Ok(total_rows)
    }

    /// Stage 2: Ingest 10 Empresas partitions using parallel Rayon parsing and bounded MPSC queue.
    fn ingest_empresas(
        &self,
        conn: &mut Connection,
        mp: &Option<Arc<MultiProgress>>,
    ) -> Result<u64> {
        let files = self.find_partition_files("Empresas", 10);
        if files.is_empty() {
            warn!("No Empresas*.zip files found in input directory");
            return Ok(0);
        }

        let (tx, rx) = bounded::<Result<Vec<EmpresaBatchRow>>>(BOUNDED_CHANNEL_CAPACITY);
        let batch_size = self.args.batch_size;
        let abort_flag = Arc::new(AtomicBool::new(false));

        let spinner = mp.as_ref().map(|m| {
            let pb = m.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner} {msg} [{elapsed}] ({pos} rows)")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            pb.set_message("Ingesting Empresas...");
            pb.enable_steady_tick(Duration::from_millis(100));
            pb
        });

        let abort_flag_clone = Arc::clone(&abort_flag);
        let tx_clone = tx.clone();
        let files_clone = files.clone();

        self.thread_pool.spawn(move || {
            let _ = files_clone
                .par_iter()
                .try_for_each(|file_path| -> Result<()> {
                    if abort_flag_clone.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let res = stream_zip_batches(
                        file_path,
                        Some(".EMPRECSV"),
                        batch_size,
                        &tx_clone,
                        |rec| Ok(EmpresaBatchRow::from_byte_record(rec)),
                    );
                    if let Err(e) = res {
                        abort_flag_clone.store(true, Ordering::Relaxed);
                        let _ = tx_clone.send(Err(anyhow::anyhow!(
                            "Stream error on {:?}: {:#}",
                            file_path,
                            e
                        )));
                        return Err(anyhow::anyhow!("Stream error: {:#}", e));
                    }
                    Ok(())
                });
        });

        drop(tx);

        let pb_ref = spinner.as_ref();
        let rows = consume_empresas_channel(conn, rx, |n| {
            if let Some(pb) = pb_ref {
                pb.inc(n as u64);
            }
        })?;

        if let Some(pb) = spinner {
            pb.finish_with_message(format!("Empresas: {rows} rows inserted"));
        }

        Ok(rows)
    }

    /// Stage 3: Ingest 10 Estabelecimentos partitions using parallel Rayon parsing and bounded MPSC queue.
    fn ingest_estabelecimentos(
        &self,
        conn: &mut Connection,
        mp: &Option<Arc<MultiProgress>>,
    ) -> Result<u64> {
        let files = self.find_partition_files("Estabelecimentos", 10);
        if files.is_empty() {
            warn!("No Estabelecimentos*.zip files found in input directory");
            return Ok(0);
        }

        let (tx, rx) = bounded::<Result<Vec<EstabelecimentoBatchRow>>>(BOUNDED_CHANNEL_CAPACITY);
        let batch_size = self.args.batch_size;
        let abort_flag = Arc::new(AtomicBool::new(false));

        let spinner = mp.as_ref().map(|m| {
            let pb = m.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner} {msg} [{elapsed}] ({pos} rows)")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            pb.set_message("Ingesting Estabelecimentos...");
            pb.enable_steady_tick(Duration::from_millis(100));
            pb
        });

        let abort_flag_clone = Arc::clone(&abort_flag);
        let tx_clone = tx.clone();
        let files_clone = files.clone();

        self.thread_pool.spawn(move || {
            let _ = files_clone
                .par_iter()
                .try_for_each(|file_path| -> Result<()> {
                    if abort_flag_clone.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let res = stream_zip_batches(
                        file_path,
                        Some(".ESTABELE"),
                        batch_size,
                        &tx_clone,
                        |rec| Ok(EstabelecimentoBatchRow::from_byte_record(rec)),
                    );
                    if let Err(e) = res {
                        abort_flag_clone.store(true, Ordering::Relaxed);
                        let _ = tx_clone.send(Err(anyhow::anyhow!(
                            "Stream error on {:?}: {:#}",
                            file_path,
                            e
                        )));
                        return Err(anyhow::anyhow!("Stream error: {:#}", e));
                    }
                    Ok(())
                });
        });

        drop(tx);

        let pb_ref = spinner.as_ref();
        let rows = consume_estabelecimentos_channel(conn, rx, |n| {
            if let Some(pb) = pb_ref {
                pb.inc(n as u64);
            }
        })?;

        if let Some(pb) = spinner {
            pb.finish_with_message(format!("Estabelecimentos: {rows} rows inserted"));
        }

        Ok(rows)
    }

    /// Stage 4: Ingest Simples.zip.
    fn ingest_simples(
        &self,
        conn: &mut Connection,
        mp: &Option<Arc<MultiProgress>>,
    ) -> Result<u64> {
        let files = self.find_partition_files("Simples", 1);
        if files.is_empty() {
            warn!("Simples.zip not found in input directory");
            return Ok(0);
        }

        let (tx, rx) = bounded::<Result<Vec<SimplesBatchRow>>>(BOUNDED_CHANNEL_CAPACITY);
        let batch_size = self.args.batch_size;
        let abort_flag = Arc::new(AtomicBool::new(false));

        let spinner = mp.as_ref().map(|m| {
            let pb = m.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner} {msg} [{elapsed}] ({pos} rows)")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            pb.set_message("Ingesting Simples...");
            pb.enable_steady_tick(Duration::from_millis(100));
            pb
        });

        let abort_flag_clone = Arc::clone(&abort_flag);
        let tx_clone = tx.clone();
        let files_clone = files.clone();

        self.thread_pool.spawn(move || {
            let _ = files_clone
                .par_iter()
                .try_for_each(|file_path| -> Result<()> {
                    if abort_flag_clone.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let res = stream_zip_batches(
                        file_path,
                        Some(".SIMPLES"),
                        batch_size,
                        &tx_clone,
                        |rec| Ok(SimplesBatchRow::from_byte_record(rec)),
                    );
                    if let Err(e) = res {
                        abort_flag_clone.store(true, Ordering::Relaxed);
                        let _ = tx_clone.send(Err(anyhow::anyhow!(
                            "Stream error on {:?}: {:#}",
                            file_path,
                            e
                        )));
                        return Err(anyhow::anyhow!("Stream error: {:#}", e));
                    }
                    Ok(())
                });
        });

        drop(tx);

        let pb_ref = spinner.as_ref();
        let rows = consume_simples_channel(conn, rx, |n| {
            if let Some(pb) = pb_ref {
                pb.inc(n as u64);
            }
        })?;

        if let Some(pb) = spinner {
            pb.finish_with_message(format!("Simples: {rows} rows inserted"));
        }

        Ok(rows)
    }

    /// Stage 6: Ingest 10 Socios partitions with August 2026 14-digit Matriz expansion.
    fn ingest_socios(&self, conn: &mut Connection, mp: &Option<Arc<MultiProgress>>) -> Result<u64> {
        let files = self.find_partition_files("Socios", 10);
        if files.is_empty() {
            warn!("No Socios*.zip files found in input directory");
            conn.execute_batch(SOCIOS_FINAL_DDL)?;
            conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?;
            return Ok(0);
        }

        let (tx, rx) = bounded::<Result<Vec<SocioStagingBatchRow>>>(BOUNDED_CHANNEL_CAPACITY);
        let batch_size = self.args.batch_size;
        let abort_flag = Arc::new(AtomicBool::new(false));

        let spinner = mp.as_ref().map(|m| {
            let pb = m.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner} {msg} [{elapsed}] ({pos} rows)")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            pb.set_message("Ingesting Socios (staging)...");
            pb.enable_steady_tick(Duration::from_millis(100));
            pb
        });

        let abort_flag_clone = Arc::clone(&abort_flag);
        let tx_clone = tx.clone();
        let files_clone = files.clone();

        self.thread_pool.spawn(move || {
            let _ = files_clone
                .par_iter()
                .try_for_each(|file_path| -> Result<()> {
                    if abort_flag_clone.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let res = stream_zip_batches(
                        file_path,
                        Some(".SOCIOCSV"),
                        batch_size,
                        &tx_clone,
                        |rec| Ok(SocioStagingBatchRow::from_byte_record(rec)),
                    );
                    if let Err(e) = res {
                        abort_flag_clone.store(true, Ordering::Relaxed);
                        let _ = tx_clone.send(Err(anyhow::anyhow!(
                            "Stream error on {:?}: {:#}",
                            file_path,
                            e
                        )));
                        return Err(anyhow::anyhow!("Stream error: {:#}", e));
                    }
                    Ok(())
                });
        });

        drop(tx);

        let pb_ref = spinner.as_ref();
        let raw_rows = consume_socios_channel(conn, rx, |n| {
            if let Some(pb) = pb_ref {
                pb.inc(n as u64);
            }
        })?;

        if let Some(pb) = &spinner {
            pb.set_message("Expanding August 2026 Socios radicals...");
        }

        // Execute relational join and 14-digit matrix radical resolution
        let final_rows = transform_socios_and_expand_radicals(conn)?;

        if let Some(pb) = spinner {
            pb.finish_with_message(format!(
                "Socios: {final_rows} rows materialized ({raw_rows} raw)"
            ));
        }

        Ok(final_rows)
    }

    /// Stage 7: Insert reference date and establishment count into _referencia.
    fn insert_audit_metadata(&self, conn: &mut Connection) -> Result<u64> {
        let reference_date = self
            .extract_reference_date()
            .unwrap_or_else(|| chrono::Local::now().format("%d/%m/%Y").to_string());

        let est_count: u64 = conn
            .query_row("SELECT COUNT(*) FROM estabelecimento", [], |row| row.get(0))
            .unwrap_or(0);

        insert_audit_metadata(conn, &reference_date, est_count)?;
        Ok(1)
    }

    /// Finds a single zip file matching `name_without_ext.zip` case-insensitively.
    fn find_single_zip(&self, name_without_ext: &str) -> Option<PathBuf> {
        let lower_target = format!("{}.zip", name_without_ext.to_lowercase());
        if let Ok(entries) = fs::read_dir(&self.args.input_dir) {
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.to_lowercase() == lower_target {
                        return Some(entry.path());
                    }
                }
            }
        }
        None
    }

    /// Search for partitioned files matching `<Prefix>0.zip` .. `<Prefix>9.zip` or direct names.
    fn find_partition_files(&self, prefix: &str, count: usize) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let lower_prefix = prefix.to_lowercase();

        for i in 0..count {
            let expected_name = format!("{}{}.zip", lower_prefix, i);
            if let Ok(entries) = fs::read_dir(&self.args.input_dir) {
                for entry in entries.flatten() {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if file_name.to_lowercase() == expected_name {
                            files.push(entry.path());
                            break;
                        }
                    }
                }
            }
        }

        if files.is_empty() {
            let expected_name = format!("{}.zip", lower_prefix);
            if let Ok(entries) = fs::read_dir(&self.args.input_dir) {
                for entry in entries.flatten() {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if file_name.to_lowercase() == expected_name {
                            files.push(entry.path());
                            break;
                        }
                    }
                }
            }
        }

        files.sort();
        files
    }

    /// Attempt to parse official reference date from internal zip header timestamps.
    fn extract_reference_date(&self) -> Option<String> {
        if let Some(p) = self
            .find_single_zip("Empresas0")
            .or_else(|| self.find_single_zip("Empresas"))
        {
            if let Ok(meta) = inspect_zip(&p, None) {
                if let Some(dt) = meta.rfb_ref_date {
                    return Some(dt);
                }
            }
        }
        // Fallback to any other present zip file
        if let Ok(entries) = fs::read_dir(&self.args.input_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "zip").unwrap_or(false) {
                    if let Ok(meta) = inspect_zip(&path, None) {
                        if let Some(dt) = meta.rfb_ref_date {
                            return Some(dt);
                        }
                    }
                }
            }
        }
        None
    }

    /// Validate that input directory exists and contains target files.
    fn validate_environment(&self) -> Result<()> {
        if !self.args.input_dir.exists() {
            bail!("Input directory does not exist: {:?}", self.args.input_dir);
        }
        Ok(())
    }

    /// Prepare destination directory and handle `--drop-existing`.
    fn prepare_database_destination(&self) -> Result<()> {
        if let Some(parent) = self.args.output_db.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create database parent directory at {:?}", parent)
            })?;
        }

        if self.args.output_db.exists() {
            if self.args.drop_existing {
                info!(
                    "Removing existing output database at {:?}",
                    self.args.output_db
                );
                fs::remove_file(&self.args.output_db)?;
                let wal = self.args.output_db.with_extension("db-wal");
                if wal.exists() {
                    let _ = fs::remove_file(wal);
                }
                let shm = self.args.output_db.with_extension("db-shm");
                if shm.exists() {
                    let _ = fs::remove_file(shm);
                }
            } else {
                warn!(
                    "Output database {:?} already exists. Appending to existing tables.",
                    self.args.output_db
                );
            }
        }
        Ok(())
    }

    /// Create terminal progress bars using `indicatif`.
    fn create_progress_bars(&self) -> (Option<Arc<MultiProgress>>, ProgressBar) {
        let mp = Arc::new(MultiProgress::new());
        let total_stages = if self.args.skip_indices { 7 } else { 8 };

        let overall_style = ProgressStyle::default_bar()
            .template("{prefix:<18} [{elapsed_precise}] [{bar:30.green/black}] {pos}/{len} stages ({percent}%) {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-");

        let pb = mp.add(ProgressBar::new(total_stages));
        pb.set_style(overall_style);
        pb.set_prefix("Ingestion Pipeline");
        pb.set_message("Initializing...");

        (Some(mp), pb)
    }
}

/// Helper function to retrieve process Resident Set Size (RSS) on Linux.
pub fn get_resident_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = fs::read_to_string("/proc/self/statm") {
            let mut parts = content.split_whitespace();
            if let Some(_total) = parts.next() {
                if let Some(resident) = parts.next() {
                    if let Ok(pages) = resident.parse::<u64>() {
                        let page_size = 4096u64;
                        return Some(pages * page_size);
                    }
                }
            }
        }
    }
    None
}

/// Public CLI entry point called from `main.rs`.
pub fn run(args: BuildCnpjArgs) -> Result<()> {
    let orchestrator = IngestionOrchestrator::new(args)?;
    let stats = orchestrator.execute()?;

    println!("\n📊 RFB CNPJ Ingestion Summary:");
    println!("  Total Records Inserted: {}", stats.total_rows);
    println!("  Execution Time:         {:.2?}", stats.total_duration);
    println!(
        "  Database Size:          {:.2} MB",
        stats.db_size_bytes as f64 / (1024.0 * 1024.0)
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
            "  - {:<40} {:>10} rows ({:>10.0} rows/s) [{:.2?}]",
            r.stage_name, r.rows_inserted, r.rows_per_sec, r.duration
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_orchestrator_initialization_and_thread_clamp() {
        let args = BuildCnpjArgs {
            input_dir: PathBuf::from("dados-publicos-zip"),
            output_db: PathBuf::from("dados-publicos/cnpj.db"),
            memory_limit_mb: 1536,
            batch_size: 5000,
            threads: 32, // Should be clamped
            drop_existing: true,
            skip_indices: false,
        };
        let orchestrator = IngestionOrchestrator::new(args).expect("initialize");
        assert_eq!(orchestrator.thread_pool.current_num_threads(), 16);
    }

    #[test]
    fn test_find_partition_files_format() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path();
        for i in 0..3 {
            fs::File::create(path.join(format!("Empresas{i}.zip"))).unwrap();
        }
        let args = BuildCnpjArgs {
            input_dir: path.to_path_buf(),
            output_db: path.join("test.db"),
            memory_limit_mb: 1024,
            batch_size: 1000,
            threads: 2,
            drop_existing: true,
            skip_indices: true,
        };
        let orchestrator = IngestionOrchestrator::new(args).unwrap();
        let files = orchestrator.find_partition_files("Empresas", 10);
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_linux_rss_reporting_non_zero_or_none() {
        let rss = get_resident_memory_bytes();
        #[cfg(target_os = "linux")]
        {
            assert!(rss.is_some());
            assert!(rss.unwrap() > 0);
        }
    }

    fn create_test_zip(zip_path: &std::path::Path, entry_name: &str, content: &[u8]) {
        use std::io::Write;
        use zip::write::{SimpleFileOptions, ZipWriter};
        let file = fs::File::create(zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(entry_name, options).unwrap();
        zip.write_all(content).unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn test_end_to_end_ingestion_with_synthetic_fixtures() {
        let temp = tempfile::tempdir().unwrap();
        let input_dir = temp.path().join("zip");
        fs::create_dir_all(&input_dir).unwrap();
        let output_db = temp.path().join("cnpj.db");

        // 1. Cnaes.zip
        create_test_zip(
            &input_dir.join("Cnaes.zip"),
            "F.K03200$Z.D61024.CNAECSV",
            b"6201501;Desenvolvimento de software\r\n",
        );

        // 2. Empresas0.zip
        let emp_csv = "\
11111111;ALFA TECNOLOGIA LTDA;2062;49;150000,50;01;\r\n\
22222222;BETA COMERCIO;2054;16;2500000,00;05;\r\n";
        create_test_zip(
            &input_dir.join("Empresas0.zip"),
            "F.K03200$Z.D61024.EMPRECSV",
            emp_csv.as_bytes(),
        );

        // 3. Estabelecimentos0.zip
        let estab_csv = "\
11111111;0001;91;1;ALFA TECH;02;20210101;00;;105;20200101;6201501;;RUA A;1;;CENTRO;79002000;MS;9051;67;33334444;;;;;contato@alfa.com;;\r\n\
22222222;0001;82;1;BETA STORE;02;20210101;00;;105;20200101;4711301;;AV B;2;;CENTRO;01310100;SP;7107;11;30004000;;;;;sac@beta.com;;\r\n\
11111111;0002;72;2;ALFA FILIAL;02;20210601;00;;105;20210601;6201501;;RUA C;3;;CENTRO;79002000;MS;9051;67;33334445;;;;;filial@alfa.com;;\r\n";
        create_test_zip(
            &input_dir.join("Estabelecimentos0.zip"),
            "F.K03200$Z.D61024.ESTABELE",
            estab_csv.as_bytes(),
        );

        // 4. Simples.zip
        create_test_zip(
            &input_dir.join("Simples.zip"),
            "F.K03200$Z.D61024.SIMPLES.CSV",
            b"11111111;S;20210101;;N;;\r\n",
        );

        // 5. Socios0.zip (August 2026 format: 8-digit radical for PJ partner)
        let socios_csv = "\
11111111;2;JOAO DA SILVA;***123456**;49;20200101;105;;;;5\r\n\
22222222;1;ALFA TECNOLOGIA LTDA;11111111;22;20200601;105;***555444**;ROBERTO REP;05;0\r\n";
        create_test_zip(
            &input_dir.join("Socios0.zip"),
            "F.K03200$Z.D61024.SOCIOCSV",
            socios_csv.as_bytes(),
        );

        let args = BuildCnpjArgs {
            input_dir,
            output_db: output_db.clone(),
            memory_limit_mb: 512,
            batch_size: 1000,
            threads: 2,
            drop_existing: true,
            skip_indices: false,
        };

        let orchestrator = IngestionOrchestrator::new(args).unwrap();
        let stats = orchestrator.execute().unwrap();
        assert!(stats.total_rows > 0);

        // Inspect output SQLite database
        let conn = Connection::open(&output_db).unwrap();

        // 1. Table existence & counts
        let cnae_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM cnae", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnae_count, 1);

        let emp_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM empresas", [], |r| r.get(0))
            .unwrap();
        assert_eq!(emp_count, 2);

        let cap: f64 = conn
            .query_row(
                "SELECT capital_social FROM empresas WHERE cnpj_basico = '11111111'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cap, 150000.50);

        let estab_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM estabelecimento", [], |r| r.get(0))
            .unwrap();
        assert_eq!(estab_count, 3);

        // Verify synthesized 14-char CNPJs
        let cnpj1: String = conn
            .query_row(
                "SELECT cnpj FROM estabelecimento WHERE cnpj_basico = '11111111' AND matriz_filial = '1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnpj1, "11111111000191");
        let cnpj2: String = conn
            .query_row(
                "SELECT cnpj FROM estabelecimento WHERE cnpj_basico = '11111111' AND matriz_filial = '2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnpj2, "11111111000272");

        // 2. Materialized cnpj_base2matriz
        let matriz_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM cnpj_base2matriz", [], |r| r.get(0))
            .unwrap();
        assert_eq!(matriz_count, 2); // 11111111 and 22222222 (filial excluded)

        // 3. August 2026 PJ radical expansion
        let socios_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM socios", [], |r| r.get(0))
            .unwrap();
        assert_eq!(socios_count, 2);

        let expanded: String = conn
            .query_row(
                "SELECT cnpj_cpf_socio FROM socios WHERE cnpj_basico = '22222222' AND identificador_de_socio = '1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(expanded, "11111111000191"); // 8-digit '11111111' expanded to 14-digit '11111111000191'!

        // 4. Staging table was dropped
        let staging_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='socios_original'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!staging_exists);

        // 5. Audit metadata (_referencia)
        let ref_date: String = conn
            .query_row(
                "SELECT valor FROM _referencia WHERE referencia = 'CNPJ'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ref_date, "24/10/2026");
        let qtde: String = conn
            .query_row(
                "SELECT valor FROM _referencia WHERE referencia = 'cnpj_qtde'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(qtde, "3");

        // 6. PRAGMAs: WAL mode
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_lowercase(), "wal");

        // 7. Indices
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index'")
            .unwrap();
        let indices: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(indices.contains(&"idx_empresas_cnpj_basico".to_string()));
        assert!(indices.contains(&"idx_estabelecimento_cnpj".to_string()));
        assert!(indices.contains(&"idx_socios_cnpj".to_string()));
        assert!(indices.contains(&"idx_cnpj_base2matriz".to_string()));
    }
}
