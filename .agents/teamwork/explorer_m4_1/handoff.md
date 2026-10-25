# Handoff Report — Milestone 4: CLI Status Subcommand & Diagnostics Architect

- **Agent**: `explorer_m4_1`
- **Role**: CLI Status Subcommand & Diagnostics Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_1`
- **Date**: 2026-10-25T01:55:00Z
- **Milestone**: Milestone 4 (CLI Status & Root Makefile Automation)

---

## 1. Observation

### 1.1 Existing Target Codebase Inspection
Direct inspection of `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas` revealed:
1. **`Cargo.toml` Dependencies**:
   - `clap = { version = "4.5", features = ["derive", "env"] }` (line 21)
   - `rusqlite = { version = "0.32", features = ["bundled-full", "backup", "blob"] }` (line 32)
   - `serde = { version = "1.0", features = ["derive"] }` (line 45)
   - `serde_json = "1.0"` (line 46)
   - `chrono = { version = "0.4", features = ["serde"] }` (line 47)
   - `indicatif = { version = "0.17", features = ["rayon"] }` (line 29)
   - Compiler version is `rustc 1.98.0 (88d9e12ae 2026-08-18)`. `std::io::IsTerminal` is part of Rust standard library.

2. **Existing CLI Definition (`src/cli.rs`)**:
   - Lines 58–61:
     ```rust
     /// Inspect database sizes, table row counts, and indexing status
     #[command(name = "status", alias = "info")]
     Status(StatusArgs),
     ```
   - Lines 244–295:
     ```rust
     #[derive(Args, Debug, Clone)]
     pub struct StatusArgs {
         #[arg(long = "cnpj-db", default_value = "dados-publicos/cnpj.db", value_name = "PATH", env = "RFB_CNPJ_DB")]
         pub cnpj_db: PathBuf,
         #[arg(long = "rede-db", default_value = "dados-publicos/rede.db", value_name = "PATH", env = "RFB_REDE_DB")]
         pub rede_db: PathBuf,
         #[arg(long = "search-db", default_value = "dados-publicos/rede_search.db", value_name = "PATH", env = "RFB_SEARCH_DB")]
         pub search_db: PathBuf,
         #[arg(long = "zip-dir", default_value = "dados-publicos-zip", value_name = "DIR", env = "RFB_ZIP_DIR")]
         pub zip_dir: PathBuf,
         #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text, value_name = "FORMAT")]
         pub format: OutputFormat,
         #[arg(long = "json", help = "Shortcut for --format json")]
         pub json: bool,
     }
     ```
   - Lines 297–303:
     ```rust
     #[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
     #[serde(rename_all = "lowercase")]
     pub enum OutputFormat {
         Text,
         Json,
     }
     ```

3. **Existing Main & Lib Status Wiring**:
   - `src/lib.rs` (lines 16–23):
     ```rust
     /// Inspection & status reporter stubs for Milestone 4.
     pub mod status {
         use crate::cli::StatusArgs;

         pub fn run(_args: StatusArgs) -> anyhow::Result<()> {
             anyhow::bail!("status will be implemented in Milestone 4")
         }
     }
     ```
   - `src/main.rs` (lines 37–43):
     ```rust
     Commands::Status(mut args) => {
         if args.json {
             args.format = OutputFormat::Json;
         }
         rfb_cnpj_etl::status::run(args)?;
     }
     ```

4. **Database Schemas & Tables Across Milestones 2 & 3**:
   - `dados-publicos/cnpj.db` (`src/ingestion/schema.rs`):
     - 12 tables: `empresas`, `estabelecimento`, `socios`, `simples`, `cnae`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio`, `cnpj_base2matriz`, `_referencia`.
     - 18 post-load indices:
       - `idx_empresas_cnpj_basico`, `idx_empresas_razao_social`
       - `idx_estabelecimento_cnpj_basico`, `idx_estabelecimento_cnpj`, `idx_estabelecimento_nomefantasia`
       - `idx_simples_cnpj_basico`
       - `idx_cnpj_base2matriz`
       - `idx_socios_cnpj`, `idx_socios_cnpj_cpf_socio`, `idx_socios_nome_socio`, `idx_socios_representante`, `idx_socios_representante_nome`
       - `idx_cnae`, `idx_motivo`, `idx_municipio`, `idx_natureza_juridica`, `idx_pais`, `idx_qualificacao_socio`
     - Audit metadata in `_referencia`:
       - `referencia = 'CNPJ'`: reference date (e.g., `"01/10/2026"`)
       - `referencia = 'cnpj_qtde'`: total establishment count (e.g., `"16"`)
   - `dados-publicos/rede.db` (`src/graph/schema.rs`, `src/graph/rules.rs`):
     - Tables: `ligacao` (`id1`, `id2`, `descricao`, `comentario`), `entidades` (`id`, `nome`, `tipo`), `links` (`id1`, `id2`, `descricao`, `comentario`).
     - Indices: `idx_ligacao_origem`, `idx_ligacao_destino`, `idx_entidades_tipo`, `idx_links_id1`, `idx_links_id2`.
     - Edge category in `ligacao.comentario`:
       - `'estabelecimento'` (Rule 1: Filial -> Matriz links)
       - `'socios'` (Rules 2–8: Corporate & individual partner links)
     - Entity category in `entidades.tipo`:
       - `'PJ'`, `'FILIAL'`, `'PF'`, `'PE'`
   - `dados-publicos/rede_search.db` (`src/graph/search.rs`):
     - Virtual table: `id_search` using `fts5(id, descricao, tokenize = 'unicode61 remove_diacritics 2')`.
   - `dados-publicos-zip`:
     - 37 canonical RFB `.zip` files.

5. **Test Assertions in `tests/e2e_pipeline_test.rs`**:
   - Lines 697–708 (`test_tier1_f15_status_command_interface`):
     Invokes `status --help`, asserts exit code 0 and presence of `--format` and `--json`.
   - Lines 1039–1060 (`test_tier4_scenario_1_full_cold_pipeline_execution`):
     Invokes `status --cnpj-db ... --rede-db ... --search-db ... --zip-dir ... --json`. Asserts exit code 0.
   - Lines 1202–1221 (`test_tier4_scenario_5_status_inspection_and_empty_directory`):
     Invokes `status --cnpj-db ... --rede-db ... --json` on non-existent database paths in an empty sandbox. Asserts command succeeds without panic or crash.

---

## 2. Logic Chain

1. **Safety on Non-Existent Files (Graceful Absence)**:
   - Observation: In a fresh clone or before running `build-cnpj` / `build-rede`, user or tests (e.g. `test_tier4_scenario_5`) invoke `status`.
   - Deduction: Attempting `Connection::open_with_flags(..., SQLITE_OPEN_READ_ONLY)` on a non-existent file path will return an `IoError` / `CantOpen`. The diagnostics engine must first check `path.exists()`. If `false`, it must record `exists: false` and `integrity_check: "not_found"` without bubbling an error or panicking.
   - Conclusion: The status subcommand exits with code 0 in both healthy and empty/pre-build environments.

2. **Integrity Verification Method**:
   - Observation: `PRAGMA integrity_check;` on a multi-gigabyte database can take several minutes because it reads every B-tree cell and verifies indices against row data.
   - Deduction: `PRAGMA quick_check(1);` performs fast B-tree structure validation in milliseconds on small files and < 2 seconds on multi-GB files, stopping at the first error if corrupted.
   - Conclusion: Use `PRAGMA quick_check(1);` for fast, non-blocking CLI status checks. If result is `"ok"`, record clean integrity.

3. **Row Counting Strategy**:
   - Observation: SQLite stores tables as B-trees. A table might not exist if a stage failed or was omitted.
   - Deduction: Query `SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1;` before attempting `SELECT COUNT(*) FROM <table>;`. If the table does not exist, return `None` rather than aborting. If it exists, execute `SELECT COUNT(*) FROM <table>;`.
   - Conclusion: Tables are mapped to `Option<u64>` (Some(count) if present, None if absent).

4. **Formatting Architecture (Dual Representation)**:
   - Observation: Requirements state: "ANSI-colored if terminal, plain if piped" and "Structured JSON output when `--json` is specified".
   - Deduction: Rust's standard library `std::io::IsTerminal` (`std::io::stdout().is_terminal()`) accurately detects interactive TTYs vs pipes (redirecting to file or test capture).
   - Conclusion:
     - When `format == OutputFormat::Json`: serialize `StatusReport` via `serde_json::to_string_pretty`.
     - When `format == OutputFormat::Text`: print aligned, structured dashboard tables. If `is_terminal() == true`, apply ANSI green/cyan/yellow/red styling; otherwise, output clean plain text.

---

## 3. Caveats

1. **Concurrent Database Access**:
   - Inspection opens SQLite connections with `rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX`. In WAL mode (`journal_mode = wal`), concurrent read-only status checks do not block ongoing ingestion workers or readers.
2. **Missing Filesystem Permissions**:
   - If a file exists but cannot be read due to OS permissions, the error is captured in `integrity_check: "error: <msg>"` rather than causing a CLI panic.
3. **No External Table Dependencies**:
   - Table formatting is implemented using Rust standard library string formatting without adding extra third-party crates (like `comfy-table`), keeping binary size lean, compile times fast, and zero transitive vulnerability risks.

---

## 4. Conclusion & Complete Design Specification

### 4.1 Module Architecture
Target project directory: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
```
src/
├── status/
│   ├── mod.rs           # Orchestrator & entry point run(StatusArgs)
│   ├── models.rs        # Strongly-typed serializable status data models
│   ├── diagnostics.rs   # SQLite PRAGMAs, row counts, indices & filesystem metrics
│   └── reporter.rs      # ANSI-styled text table renderer & JSON pretty printer
├── lib.rs               # Wire `pub mod status;`
└── main.rs              # Subcommand routing to status::run(args)
```

---

### 4.2 Proposed Code Implementation

#### Component 1: `src/status/models.rs`
```rust
//! Strongly typed serializable data models for status reporting and diagnostics.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Complete status report for the RFB CNPJ open data pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReport {
    pub timestamp: String,
    pub version: String,
    pub overall_status: PipelineOverallStatus,
    pub databases: DatabaseStatuses,
    pub zip_archives: ZipArchivesStatus,
}

/// Overall pipeline health and progress classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineOverallStatus {
    /// All 3 databases exist, integrity checks pass, and all canonical tables are populated.
    Complete,
    /// At least one database or archive exists, but pipeline is partially built.
    Partial,
    /// No database files or downloaded archives found.
    Empty,
    /// A database is corrupted or an integrity check failed.
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatuses {
    pub cnpj: CnpjDbStatus,
    pub rede: RedeDbStatus,
    pub rede_search: SearchDbStatus,
}

/// Common SQLite database file metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCommonMetadata {
    pub path: PathBuf,
    pub exists: bool,
    pub size_bytes: u64,
    pub size_formatted: String,
    pub journal_mode: Option<String>,
    pub user_version: Option<i64>,
    pub page_count: Option<i64>,
    pub page_size: Option<i64>,
    pub integrity_check: String,
}

/// Status and table row counts for `cnpj.db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnpjDbStatus {
    #[serde(flatten)]
    pub common: DbCommonMetadata,
    pub reference_date: Option<String>,
    pub establishment_count_ref: Option<u64>,
    pub tables: BTreeMap<String, Option<u64>>,
    pub indices: IndexStatus,
}

/// Status and graph metrics for `rede.db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeDbStatus {
    #[serde(flatten)]
    pub common: DbCommonMetadata,
    pub tables: BTreeMap<String, Option<u64>>,
    pub edge_breakdown: BTreeMap<String, u64>,
    pub entity_breakdown: BTreeMap<String, u64>,
    pub indices: IndexStatus,
}

/// Status and FTS5 search index metrics for `rede_search.db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDbStatus {
    #[serde(flatten)]
    pub common: DbCommonMetadata,
    pub tables: BTreeMap<String, Option<u64>>,
}

/// Index verification results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub total_expected: usize,
    pub total_found: usize,
    pub all_present: bool,
    pub missing: Vec<String>,
}

/// Downloaded zip archives directory status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipArchivesStatus {
    pub path: PathBuf,
    pub exists: bool,
    pub total_expected: usize,
    pub total_found: usize,
    pub total_size_bytes: u64,
    pub total_size_formatted: String,
    pub missing_files: Vec<String>,
}
```

---

#### Component 2: `src/status/diagnostics.rs`
```rust
//! Database inspection, SQLite PRAGMA execution, table counting, and filesystem metrics.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, OpenFlags};

use super::models::*;
use crate::cli::StatusArgs;

/// Expected 12 tables in `cnpj.db`.
pub const CNPJ_TABLES: [&str; 12] = [
    "cnae",
    "motivo",
    "municipio",
    "natureza_juridica",
    "pais",
    "qualificacao_socio",
    "empresas",
    "estabelecimento",
    "simples",
    "socios",
    "cnpj_base2matriz",
    "_referencia",
];

/// Expected 18 post-load indices in `cnpj.db`.
pub const CNPJ_INDICES: [&str; 18] = [
    "idx_empresas_cnpj_basico",
    "idx_empresas_razao_social",
    "idx_estabelecimento_cnpj_basico",
    "idx_estabelecimento_cnpj",
    "idx_estabelecimento_nomefantasia",
    "idx_simples_cnpj_basico",
    "idx_cnpj_base2matriz",
    "idx_socios_cnpj",
    "idx_socios_cnpj_cpf_socio",
    "idx_socios_nome_socio",
    "idx_socios_representante",
    "idx_socios_representante_nome",
    "idx_cnae",
    "idx_motivo",
    "idx_municipio",
    "idx_natureza_juridica",
    "idx_pais",
    "idx_qualificacao_socio",
];

/// Expected tables in `rede.db`.
pub const REDE_TABLES: [&str; 3] = ["ligacao", "entidades", "links"];

/// Expected 5 indices in `rede.db`.
pub const REDE_INDICES: [&str; 5] = [
    "idx_ligacao_origem",
    "idx_ligacao_destino",
    "idx_entidades_tipo",
    "idx_links_id1",
    "idx_links_id2",
];

/// 37 canonical zip filenames.
pub const CANONICAL_ZIP_FILES: [&str; 37] = [
    "Cnaes.zip",
    "Motivos.zip",
    "Municipios.zip",
    "Naturezas.zip",
    "Paises.zip",
    "Qualificacoes.zip",
    "Simples.zip",
    "Empresas0.zip", "Empresas1.zip", "Empresas2.zip", "Empresas3.zip", "Empresas4.zip",
    "Empresas5.zip", "Empresas6.zip", "Empresas7.zip", "Empresas8.zip", "Empresas9.zip",
    "Estabelecimentos0.zip", "Estabelecimentos1.zip", "Estabelecimentos2.zip", "Estabelecimentos3.zip", "Estabelecimentos4.zip",
    "Estabelecimentos5.zip", "Estabelecimentos6.zip", "Estabelecimentos7.zip", "Estabelecimentos8.zip", "Estabelecimentos9.zip",
    "Socios0.zip", "Socios1.zip", "Socios2.zip", "Socios3.zip", "Socios4.zip",
    "Socios5.zip", "Socios6.zip", "Socios7.zip", "Socios8.zip", "Socios9.zip",
];

/// Formats byte quantities into readable units (B, KB, MB, GB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Formats numbers with thousands comma separators.
pub fn format_number(val: u64) -> String {
    let s = val.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();

    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
}

/// Inspects common SQLite metadata and PRAGMAs.
pub fn inspect_common_db(path: &Path) -> (DbCommonMetadata, Option<Connection>) {
    if !path.exists() {
        return (
            DbCommonMetadata {
                path: path.to_path_buf(),
                exists: false,
                size_bytes: 0,
                size_formatted: "0 B".to_string(),
                journal_mode: None,
                user_version: None,
                page_count: None,
                page_size: None,
                integrity_check: "not_found".to_string(),
            },
            None,
        );
    }

    let size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let size_formatted = format_bytes(size_bytes);

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = match Connection::open_with_flags(path, flags) {
        Ok(c) => c,
        Err(e) => {
            return (
                DbCommonMetadata {
                    path: path.to_path_buf(),
                    exists: true,
                    size_bytes,
                    size_formatted,
                    journal_mode: None,
                    user_version: None,
                    page_count: None,
                    page_size: None,
                    integrity_check: format!("open_error: {}", e),
                },
                None,
            );
        }
    };

    let journal_mode: Option<String> = conn
        .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
        .ok();
    let user_version: Option<i64> = conn
        .query_row("PRAGMA user_version;", [], |r| r.get(0))
        .ok();
    let page_count: Option<i64> = conn
        .query_row("PRAGMA page_count;", [], |r| r.get(0))
        .ok();
    let page_size: Option<i64> = conn
        .query_row("PRAGMA page_size;", [], |r| r.get(0))
        .ok();

    let integrity_check: String = conn
        .query_row("PRAGMA quick_check(1);", [], |r| r.get(0))
        .unwrap_or_else(|e| format!("error: {}", e));

    (
        DbCommonMetadata {
            path: path.to_path_buf(),
            exists: true,
            size_bytes,
            size_formatted,
            journal_mode,
            user_version,
            page_count,
            page_size,
            integrity_check,
        },
        Some(conn),
    )
}

/// Helper to count rows in a SQLite table if it exists.
pub fn query_table_row_count(conn: &Connection, table: &str) -> Option<u64> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1 LIMIT 1;",
            [table],
            |_| Ok(()),
        )
        .is_ok();

    if !exists {
        return None;
    }

    conn.query_row(&format!("SELECT COUNT(*) FROM \"{}\";", table), [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|cnt| if cnt >= 0 { cnt as u64 } else { 0 })
    .ok()
}

/// Verifies presence of expected secondary indices.
pub fn inspect_indices(conn: &Connection, expected: &[&str]) -> IndexStatus {
    let mut existing: HashSet<String> = HashSet::new();
    if let Ok(mut stmt) = conn.prepare("SELECT name FROM sqlite_master WHERE type='index';") {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
            for row in rows.flatten() {
                existing.insert(row);
            }
        }
    }

    let mut missing = Vec::new();
    let mut found = 0;

    for &idx in expected {
        if existing.contains(idx) {
            found += 1;
        } else {
            missing.push(idx.to_string());
        }
    }

    IndexStatus {
        total_expected: expected.len(),
        total_found: found,
        all_present: missing.is_empty(),
        missing,
    }
}

/// Collects full status for `cnpj.db`.
pub fn collect_cnpj_status(path: &Path) -> CnpjDbStatus {
    let (common, conn_opt) = inspect_common_db(path);
    let mut tables = BTreeMap::new();
    let mut ref_date = None;
    let mut estab_count_ref = None;
    let mut indices = IndexStatus {
        total_expected: CNPJ_INDICES.len(),
        total_found: 0,
        all_present: false,
        missing: CNPJ_INDICES.iter().map(|s| s.to_string()).collect(),
    };

    if let Some(conn) = conn_opt {
        for &t in &CNPJ_TABLES {
            tables.insert(t.to_string(), query_table_row_count(&conn, t));
        }

        // Query _referencia audit metadata if table exists
        if tables.get("_referencia").and_then(|v| *v).is_some() {
            ref_date = conn
                .query_row("SELECT valor FROM _referencia WHERE referencia = 'CNPJ' LIMIT 1;", [], |r| r.get(0))
                .ok();
            estab_count_ref = conn
                .query_row("SELECT valor FROM _referencia WHERE referencia = 'cnpj_qtde' LIMIT 1;", [], |r| {
                    let s: String = r.get(0)?;
                    Ok(s.parse::<u64>().unwrap_or(0))
                })
                .ok();
        }

        indices = inspect_indices(&conn, &CNPJ_INDICES);
    } else {
        for &t in &CNPJ_TABLES {
            tables.insert(t.to_string(), None);
        }
    }

    CnpjDbStatus {
        common,
        reference_date: ref_date,
        establishment_count_ref: estab_count_ref,
        tables,
        indices,
    }
}

/// Collects full status for `rede.db`.
pub fn collect_rede_status(path: &Path) -> RedeDbStatus {
    let (common, conn_opt) = inspect_common_db(path);
    let mut tables = BTreeMap::new();
    let mut edge_breakdown = BTreeMap::new();
    let mut entity_breakdown = BTreeMap::new();
    let mut indices = IndexStatus {
        total_expected: REDE_INDICES.len(),
        total_found: 0,
        all_present: false,
        missing: REDE_INDICES.iter().map(|s| s.to_string()).collect(),
    };

    if let Some(conn) = conn_opt {
        for &t in &REDE_TABLES {
            tables.insert(t.to_string(), query_table_row_count(&conn, t));
        }

        // Breakdown of ligacao by comentario
        if tables.get("ligacao").and_then(|v| *v).is_some() {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT COALESCE(NULLIF(trim(comentario), ''), 'outro') AS cat, COUNT(*) \
                 FROM ligacao GROUP BY cat ORDER BY COUNT(*) DESC;"
            ) {
                if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))) {
                    for row in rows.flatten() {
                        edge_breakdown.insert(row.0, row.1);
                    }
                }
            }
        }

        // Breakdown of entidades by tipo
        if tables.get("entidades").and_then(|v| *v).is_some() {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT COALESCE(NULLIF(trim(tipo), ''), 'outro') AS cat, COUNT(*) \
                 FROM entidades GROUP BY cat ORDER BY COUNT(*) DESC;"
            ) {
                if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))) {
                    for row in rows.flatten() {
                        entity_breakdown.insert(row.0, row.1);
                    }
                }
            }
        }

        indices = inspect_indices(&conn, &REDE_INDICES);
    } else {
        for &t in &REDE_TABLES {
            tables.insert(t.to_string(), None);
        }
    }

    RedeDbStatus {
        common,
        tables,
        edge_breakdown,
        entity_breakdown,
        indices,
    }
}

/// Collects full status for `rede_search.db`.
pub fn collect_search_status(path: &Path) -> SearchDbStatus {
    let (common, conn_opt) = inspect_common_db(path);
    let mut tables = BTreeMap::new();

    if let Some(conn) = conn_opt {
        tables.insert("id_search".to_string(), query_table_row_count(&conn, "id_search"));
    } else {
        tables.insert("id_search".to_string(), None);
    }

    SearchDbStatus { common, tables }
}

/// Collects status of downloaded zip archives directory.
pub fn collect_zip_status(dir: &Path) -> ZipArchivesStatus {
    if !dir.exists() {
        return ZipArchivesStatus {
            path: dir.to_path_buf(),
            exists: false,
            total_expected: CANONICAL_ZIP_FILES.len(),
            total_found: 0,
            total_size_bytes: 0,
            total_size_formatted: "0 B".to_string(),
            missing_files: CANONICAL_ZIP_FILES.iter().map(|s| s.to_string()).collect(),
        };
    }

    let mut found_count = 0;
    let mut total_size = 0u64;
    let mut missing = Vec::new();

    for &name in &CANONICAL_ZIP_FILES {
        let p = dir.join(name);
        if p.exists() {
            found_count += 1;
            total_size += fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        } else {
            missing.push(name.to_string());
        }
    }

    ZipArchivesStatus {
        path: dir.to_path_buf(),
        exists: true,
        total_expected: CANONICAL_ZIP_FILES.len(),
        total_found: found_count,
        total_size_bytes: total_size,
        total_size_formatted: format_bytes(total_size),
        missing_files: missing,
    }
}

/// Collects the entire pipeline status report.
pub fn collect_status_report(args: &StatusArgs) -> StatusReport {
    let cnpj = collect_cnpj_status(&args.cnpj_db);
    let rede = collect_rede_status(&args.rede_db);
    let search = collect_search_status(&args.search_db);
    let zips = collect_zip_status(&args.zip_dir);

    // Evaluate overall status
    let has_corruption = cnpj.common.integrity_check != "ok" && cnpj.common.integrity_check != "not_found"
        || rede.common.integrity_check != "ok" && rede.common.integrity_check != "not_found"
        || search.common.integrity_check != "ok" && search.common.integrity_check != "not_found";

    let all_complete = cnpj.common.exists
        && rede.common.exists
        && search.common.exists
        && cnpj.common.integrity_check == "ok"
        && rede.common.integrity_check == "ok"
        && search.common.integrity_check == "ok"
        && cnpj.indices.all_present
        && rede.indices.all_present;

    let any_exists = cnpj.common.exists || rede.common.exists || search.common.exists || zips.exists;

    let overall_status = if has_corruption {
        PipelineOverallStatus::Degraded
    } else if all_complete {
        PipelineOverallStatus::Complete
    } else if any_exists {
        PipelineOverallStatus::Partial
    } else {
        PipelineOverallStatus::Empty
    };

    StatusReport {
        timestamp: Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        overall_status,
        databases: DatabaseStatuses {
            cnpj,
            rede,
            rede_search: search,
        },
        zip_archives: zips,
    }
}
```

---

#### Component 3: `src/status/reporter.rs`
```rust
//! Output rendering: ANSI-styled terminal dashboards and formatted JSON.

use std::borrow::Cow;
use std::io::{stdout, IsTerminal, Write};

use anyhow::Result;

use super::diagnostics::format_number;
use super::models::*;
use crate::cli::OutputFormat;

/// ANSI terminal color styler with automated TTY detection.
pub struct Styler {
    is_tty: bool,
}

impl Styler {
    pub fn new() -> Self {
        Self {
            is_tty: stdout().is_terminal(),
        }
    }

    pub fn bold<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.is_tty {
            format!("\x1b[1m{}\x1b[0m", text).into()
        } else {
            text.into()
        }
    }

    pub fn green<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.is_tty {
            format!("\x1b[1;32m{}\x1b[0m", text).into()
        } else {
            text.into()
        }
    }

    pub fn red<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.is_tty {
            format!("\x1b[1;31m{}\x1b[0m", text).into()
        } else {
            text.into()
        }
    }

    pub fn yellow<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.is_tty {
            format!("\x1b[1;33m{}\x1b[0m", text).into()
        } else {
            text.into()
        }
    }

    pub fn cyan<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.is_tty {
            format!("\x1b[1;36m{}\x1b[0m", text).into()
        } else {
            text.into()
        }
    }
}

/// Dispatches rendering based on requested output format.
pub fn render_report(report: &StatusReport, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => render_json(report),
        OutputFormat::Text => render_text(report),
    }
}

/// Renders JSON representation to stdout.
pub fn render_json(report: &StatusReport) -> Result<()> {
    let json_str = serde_json::to_string_pretty(report)?;
    let mut out = stdout().lock();
    writeln!(out, "{}", json_str)?;
    out.flush()?;
    Ok(())
}

/// Renders human-readable aligned dashboard to stdout.
pub fn render_text(report: &StatusReport) -> Result<()> {
    let s = Styler::new();
    let mut out = stdout().lock();

    let status_badge = match report.overall_status {
        PipelineOverallStatus::Complete => s.green("[COMPLETE]"),
        PipelineOverallStatus::Partial => s.yellow("[PARTIAL]"),
        PipelineOverallStatus::Empty => s.yellow("[EMPTY]"),
        PipelineOverallStatus::Degraded => s.red("[DEGRADED / CORRUPT]"),
    };

    writeln!(out, "{}", "=".repeat(80))?;
    writeln!(
        out,
        " {} — RFB CNPJ Open Data Pipeline Status",
        s.bold("rfb-cnpj-etl")
    )?;
    writeln!(
        out,
        " Timestamp: {} | Version: {} | Status: {}",
        report.timestamp, report.version, status_badge
    )?;
    writeln!(out, "{}", "=".repeat(80))?;

    // 1. CNPJ DATABASE
    writeln!(out, "\n{}", s.bold("1. CNPJ DATABASE (cnpj.db)"))?;
    let cnpj = &report.databases.cnpj;
    writeln!(out, "   Path:           {}", cnpj.common.path.display())?;
    if cnpj.common.exists {
        let integ = if cnpj.common.integrity_check == "ok" {
            s.green("[OK]")
        } else {
            s.red(&format!("[{}]", cnpj.common.integrity_check))
        };
        writeln!(
            out,
            "   Status:         {}  Size: {} ({} bytes)",
            s.green("[EXISTS]"),
            cnpj.common.size_formatted,
            format_number(cnpj.common.size_bytes)
        )?;
        writeln!(
            out,
            "   Journal Mode:   {:<8}  Integrity: {}",
            cnpj.common.journal_mode.as_deref().unwrap_or("unknown"),
            integ
        )?;
        if let Some(ref d) = cnpj.reference_date {
            let estab_str = cnpj
                .establishment_count_ref
                .map(format_number)
                .unwrap_or_else(|| "none".to_string());
            writeln!(
                out,
                "   Audit Metadata: Reference: {} (Estabelecimentos: {})",
                s.cyan(d),
                estab_str
            )?;
        }
        let idx_str = if cnpj.indices.all_present {
            s.green(&format!("{}/{} [ALL PRESENT]", cnpj.indices.total_found, cnpj.indices.total_expected))
        } else {
            s.yellow(&format!("{}/{} [MISSING: {:?}]", cnpj.indices.total_found, cnpj.indices.total_expected, cnpj.indices.missing))
        };
        writeln!(out, "   Indices:        {}", idx_str)?;
        writeln!(out, "   {}", "-".repeat(75))?;
        writeln!(out, "   {:<35} {:>15}   {:<10}", "Table Name", "Row Count", "Status")?;
        writeln!(out, "   {}", "-".repeat(75))?;

        let mut total_rows = 0u64;
        for (tbl, cnt_opt) in &cnpj.tables {
            if let Some(cnt) = cnt_opt {
                total_rows += cnt;
                writeln!(out, "   {:<35} {:>15}   {}", tbl, format_number(*cnt), s.green("[OK]"))?;
            } else {
                writeln!(out, "   {:<35} {:>15}   {}", tbl, "-", s.red("[MISSING]"))?;
            }
        }
        writeln!(out, "   {}", "-".repeat(75))?;
        writeln!(out, "   {:<35} {:>15}", s.bold("Total Rows (cnpj.db):"), format_number(total_rows))?;
    } else {
        writeln!(out, "   Status:         {}", s.yellow("[NOT FOUND] (Not generated yet)"))?;
        writeln!(out, "   Action:         Run 'make build-cnpj' or 'rfb-cnpj-etl build-cnpj'")?;
    }

    // 2. REDE DATABASE
    writeln!(out, "\n{}", s.bold("2. RELATIONAL NETWORK GRAPH (rede.db)"))?;
    let rede = &report.databases.rede;
    writeln!(out, "   Path:           {}", rede.common.path.display())?;
    if rede.common.exists {
        let integ = if rede.common.integrity_check == "ok" {
            s.green("[OK]")
        } else {
            s.red(&format!("[{}]", rede.common.integrity_check))
        };
        writeln!(
            out,
            "   Status:         {}  Size: {} ({} bytes)",
            s.green("[EXISTS]"),
            rede.common.size_formatted,
            format_number(rede.common.size_bytes)
        )?;
        writeln!(
            out,
            "   Journal Mode:   {:<8}  Integrity: {}",
            rede.common.journal_mode.as_deref().unwrap_or("unknown"),
            integ
        )?;
        let idx_str = if rede.indices.all_present {
            s.green(&format!("{}/{} [ALL PRESENT]", rede.indices.total_found, rede.indices.total_expected))
        } else {
            s.yellow(&format!("{}/{} [MISSING: {:?}]", rede.indices.total_found, rede.indices.total_expected, rede.indices.missing))
        };
        writeln!(out, "   Indices:        {}", idx_str)?;
        writeln!(out, "   {}", "-".repeat(75))?;
        writeln!(out, "   {:<35} {:>15}   {:<10}", "Table Name", "Row Count", "Status")?;
        writeln!(out, "   {}", "-".repeat(75))?;
        for (tbl, cnt_opt) in &rede.tables {
            if let Some(cnt) = cnt_opt {
                writeln!(out, "   {:<35} {:>15}   {}", tbl, format_number(*cnt), s.green("[OK]"))?;
            } else {
                writeln!(out, "   {:<35} {:>15}   {}", tbl, "-", s.red("[MISSING]"))?;
            }
        }
        if !rede.edge_breakdown.is_empty() {
            writeln!(out, "   Edge Breakdown (ligacao.comentario):")?;
            for (cat, count) in &rede.edge_breakdown {
                writeln!(out, "     - {:<28} {:>15}", cat, format_number(*count))?;
            }
        }
        if !rede.entity_breakdown.is_empty() {
            writeln!(out, "   Entity Breakdown (entidades.tipo):")?;
            for (cat, count) in &rede.entity_breakdown {
                writeln!(out, "     - {:<28} {:>15}", cat, format_number(*count))?;
            }
        }
    } else {
        writeln!(out, "   Status:         {}", s.yellow("[NOT FOUND] (Not generated yet)"))?;
        writeln!(out, "   Action:         Run 'make build-rede' or 'rfb-cnpj-etl build-rede'")?;
    }

    // 3. FTS5 SEARCH INDEX
    writeln!(out, "\n{}", s.bold("3. FTS5 SEARCH INDEX (rede_search.db)"))?;
    let search = &report.databases.rede_search;
    writeln!(out, "   Path:           {}", search.common.path.display())?;
    if search.common.exists {
        let integ = if search.common.integrity_check == "ok" {
            s.green("[OK]")
        } else {
            s.red(&format!("[{}]", search.common.integrity_check))
        };
        writeln!(
            out,
            "   Status:         {}  Size: {} ({} bytes)",
            s.green("[EXISTS]"),
            search.common.size_formatted,
            format_number(search.common.size_bytes)
        )?;
        writeln!(
            out,
            "   Journal Mode:   {:<8}  Integrity: {}",
            search.common.journal_mode.as_deref().unwrap_or("unknown"),
            integ
        )?;
        writeln!(out, "   {}", "-".repeat(75))?;
        writeln!(out, "   {:<35} {:>15}   {:<10}", "Virtual Table", "Row Count", "Status")?;
        writeln!(out, "   {}", "-".repeat(75))?;
        for (tbl, cnt_opt) in &search.tables {
            if let Some(cnt) = cnt_opt {
                writeln!(out, "   {:<35} {:>15}   {}", tbl, format_number(*cnt), s.green("[OK]"))?;
            } else {
                writeln!(out, "   {:<35} {:>15}   {}", tbl, "-", s.red("[MISSING]"))?;
            }
        }
    } else {
        writeln!(out, "   Status:         {}", s.yellow("[NOT FOUND] (Not generated yet)"))?;
        writeln!(out, "   Action:         Run 'make build-rede' or 'rfb-cnpj-etl build-rede'")?;
    }

    // 4. ZIP ARCHIVES
    writeln!(out, "\n{}", s.bold("4. OFFICIAL ZIP ARCHIVES (dados-publicos-zip)"))?;
    let zips = &report.zip_archives;
    writeln!(out, "   Path:           {}", zips.path.display())?;
    if zips.exists {
        let z_badge = if zips.total_found == zips.total_expected {
            s.green(&format!("[{}/{} ARCHIVES PRESENT]", zips.total_found, zips.total_expected))
        } else {
            s.yellow(&format!("[{}/{} ARCHIVES PRESENT]", zips.total_found, zips.total_expected))
        };
        writeln!(out, "   Status:         {}", z_badge)?;
        writeln!(
            out,
            "   Total Size:     {} ({} bytes)",
            zips.total_size_formatted,
            format_number(zips.total_size_bytes)
        )?;
        if !zips.missing_files.is_empty() && zips.missing_files.len() <= 5 {
            writeln!(out, "   Missing:        {:?}", zips.missing_files)?;
        }
    } else {
        writeln!(out, "   Status:         {}", s.yellow("[DIRECTORY NOT FOUND]"))?;
        writeln!(out, "   Action:         Run 'make download' or 'rfb-cnpj-etl download'")?;
    }

    writeln!(out, "\n{}", "=".repeat(80))?;
    out.flush()?;
    Ok(())
}
```

---

#### Component 4: `src/status/mod.rs`
```rust
//! Subcommand entry point, orchestration, and comprehensive unit tests.

pub mod diagnostics;
pub mod models;
pub mod reporter;

pub use diagnostics::*;
pub use models::*;
pub use reporter::*;

use anyhow::Result;
use crate::cli::StatusArgs;

/// Executes the `status` subcommand: collects metrics and renders report.
pub fn run(args: StatusArgs) -> Result<StatusReport> {
    let report = collect_status_report(&args);
    render_report(&report, args.format)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::TempDir;

    #[test]
    fn test_format_bytes_scaling() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_format_number_commas() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1000000), "1,000,000");
        assert_eq!(format_number(123456789), "123,456,789");
    }

    #[test]
    fn test_status_on_empty_nonexistent_paths() {
        let temp = TempDir::new().unwrap();
        let args = StatusArgs {
            cnpj_db: temp.path().join("nonexistent_cnpj.db"),
            rede_db: temp.path().join("nonexistent_rede.db"),
            search_db: temp.path().join("nonexistent_search.db"),
            zip_dir: temp.path().join("nonexistent_zips"),
            format: crate::cli::OutputFormat::Text,
            json: false,
        };

        let report = collect_status_report(&args);
        assert_eq!(report.overall_status, PipelineOverallStatus::Empty);
        assert!(!report.databases.cnpj.common.exists);
        assert!(!report.databases.rede.common.exists);
        assert!(!report.databases.rede_search.common.exists);
        assert!(!report.zip_archives.exists);

        // Assert JSON formatting succeeds without panic
        assert!(render_report(&report, crate::cli::OutputFormat::Json).is_ok());
        // Assert Text formatting succeeds without panic
        assert!(render_report(&report, crate::cli::OutputFormat::Text).is_ok());
    }

    #[test]
    fn test_status_with_synthetic_cnpj_and_rede_dbs() {
        let temp = TempDir::new().unwrap();
        let cnpj_path = temp.path().join("cnpj.db");
        let rede_path = temp.path().join("rede.db");
        let search_path = temp.path().join("rede_search.db");
        let zip_dir = temp.path().join("dados-publicos-zip");

        // Seed synthetic cnpj.db
        {
            let conn = Connection::open(&cnpj_path).unwrap();
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE empresas (cnpj_basico TEXT);
                 INSERT INTO empresas VALUES ('11111111');
                 INSERT INTO empresas VALUES ('22222222');
                 CREATE TABLE _referencia (referencia TEXT, valor TEXT);
                 INSERT INTO _referencia VALUES ('CNPJ', '01/10/2026');
                 INSERT INTO _referencia VALUES ('cnpj_qtde', '2');
                 CREATE INDEX idx_empresas_cnpj_basico ON empresas(cnpj_basico);"
            ).unwrap();
        }

        // Seed synthetic rede.db
        {
            let conn = Connection::open(&rede_path).unwrap();
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE ligacao (id1 TEXT, id2 TEXT, descricao TEXT, comentario TEXT);
                 INSERT INTO ligacao VALUES ('PJ_1', 'PJ_2', 'filial', 'estabelecimento');
                 INSERT INTO ligacao VALUES ('PF_1', 'PJ_1', 'socio', 'socios');
                 CREATE TABLE entidades (id TEXT, nome TEXT, tipo TEXT);
                 INSERT INTO entidades VALUES ('PJ_1', 'Empresa 1', 'PJ');
                 INSERT INTO entidades VALUES ('PF_1', 'Sócio 1', 'PF');
                 CREATE INDEX idx_ligacao_origem ON ligacao(id1);"
            ).unwrap();
        }

        // Seed synthetic rede_search.db
        {
            let conn = Connection::open(&search_path).unwrap();
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE VIRTUAL TABLE id_search USING fts5(id, descricao);
                 INSERT INTO id_search VALUES ('PJ_1', 'Empresa 1');"
            ).unwrap();
        }

        // Seed 1 zip file
        std::fs::create_dir_all(&zip_dir).unwrap();
        std::fs::write(zip_dir.join("Empresas0.zip"), b"dummy").unwrap();

        let args = StatusArgs {
            cnpj_db: cnpj_path,
            rede_db: rede_path,
            search_db: search_path,
            zip_dir,
            format: crate::cli::OutputFormat::Json,
            json: true,
        };

        let report = collect_status_report(&args);
        assert_eq!(report.overall_status, PipelineOverallStatus::Partial);
        assert!(report.databases.cnpj.common.exists);
        assert_eq!(report.databases.cnpj.common.journal_mode.as_deref(), Some("wal"));
        assert_eq!(report.databases.cnpj.reference_date.as_deref(), Some("01/10/2026"));
        assert_eq!(report.databases.cnpj.establishment_count_ref, Some(2));
        assert_eq!(report.databases.cnpj.tables.get("empresas"), Some(&Some(2)));

        assert!(report.databases.rede.common.exists);
        assert_eq!(report.databases.rede.edge_breakdown.get("estabelecimento"), Some(&1));
        assert_eq!(report.databases.rede.edge_breakdown.get("socios"), Some(&1));
        assert_eq!(report.databases.rede.entity_breakdown.get("PJ"), Some(&1));
        assert_eq!(report.databases.rede.entity_breakdown.get("PF"), Some(&1));

        assert!(report.databases.rede_search.common.exists);
        assert_eq!(report.databases.rede_search.tables.get("id_search"), Some(&Some(1)));

        assert!(report.zip_archives.exists);
        assert_eq!(report.zip_archives.total_found, 1);
        assert_eq!(report.zip_archives.missing_files.len(), 36);
    }
}
```

---

#### Component 5: Wiring in `src/lib.rs` and `src/main.rs`
1. In `src/lib.rs`:
   Replace lines 16–23:
   ```rust
   // BEFORE:
   /// Inspection & status reporter stubs for Milestone 4.
   pub mod status {
       use crate::cli::StatusArgs;

       pub fn run(_args: StatusArgs) -> anyhow::Result<()> {
           anyhow::bail!("status will be implemented in Milestone 4")
       }
   }

   // AFTER:
   /// Database inspection, status reporting, and diagnostics for Milestone 4.
   pub mod status;
   ```

2. In `src/main.rs`:
   Lines 37–43 already invoke `rfb_cnpj_etl::status::run(args)?;`:
   ```rust
   Commands::Status(mut args) => {
       if args.json {
           args.format = OutputFormat::Json;
       }
       rfb_cnpj_etl::status::run(args)?;
   }
   ```
   No modifications needed in `src/main.rs`!

---

## 5. Verification Method

Once implemented by `worker_m4`, verify the solution independently with these steps:

### Step 1: Unit & Integration Test Execution
```bash
cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml status::
```
Expected: All status unit tests (`test_format_bytes_scaling`, `test_format_number_commas`, `test_status_on_empty_nonexistent_paths`, `test_status_with_synthetic_cnpj_and_rede_dbs`) pass.

### Step 2: E2E Pipeline Tests (Tiers 1 to 4)
```bash
cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --test e2e_pipeline_test test_tier1_f15_status_command_interface
cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --test e2e_pipeline_test test_tier4_scenario_5_status_inspection_and_empty_directory
cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --test e2e_pipeline_test test_tier4_scenario_1_full_cold_pipeline_execution
```
Expected: All 3 tests pass without skipping stubs or panicking.

### Step 3: Clippy & Code Hygiene Check
```bash
cargo clippy --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --all-targets -- -D warnings
```
Expected: 0 warnings, 0 errors.

### Step 4: CLI Manual Spot-Check
```bash
# Check help
cargo run --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --bin rfb-cnpj-etl -- status --help

# Run status in empty / non-existent directory
cargo run --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --bin rfb-cnpj-etl -- status

# Run status with JSON output
cargo run --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --bin rfb-cnpj-etl -- status --json
```
Expected: Exit code 0, formatted table and valid JSON schema.
