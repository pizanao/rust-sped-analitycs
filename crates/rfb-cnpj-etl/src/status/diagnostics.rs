//! Database inspection, SQLite PRAGMA execution, table counting, and filesystem metrics.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use chrono::Utc;
use rusqlite::{Connection, OpenFlags};

use super::models::*;
use crate::cli::StatusArgs;
pub use crate::config::CANONICAL_ZIP_FILES;

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
        if i > 0 && (len - i).is_multiple_of(3) {
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
    let page_count: Option<i64> = conn.query_row("PRAGMA page_count;", [], |r| r.get(0)).ok();
    let page_size: Option<i64> = conn.query_row("PRAGMA page_size;", [], |r| r.get(0)).ok();

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
                .query_row(
                    "SELECT valor FROM _referencia WHERE referencia = 'CNPJ' LIMIT 1;",
                    [],
                    |r| r.get(0),
                )
                .ok();
            estab_count_ref = conn
                .query_row(
                    "SELECT valor FROM _referencia WHERE referencia = 'cnpj_qtde' LIMIT 1;",
                    [],
                    |r| {
                        let s: String = r.get(0)?;
                        Ok(s.parse::<u64>().unwrap_or(0))
                    },
                )
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
                 FROM ligacao GROUP BY cat ORDER BY COUNT(*) DESC;",
            ) {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
                }) {
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
                 FROM entidades GROUP BY cat ORDER BY COUNT(*) DESC;",
            ) {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
                }) {
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
        tables.insert(
            "id_search".to_string(),
            query_table_row_count(&conn, "id_search"),
        );
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
    let has_corruption = (cnpj.common.exists && cnpj.common.integrity_check != "ok")
        || (rede.common.exists && rede.common.integrity_check != "ok")
        || (search.common.exists && search.common.integrity_check != "ok");

    let all_complete = cnpj.common.exists
        && rede.common.exists
        && search.common.exists
        && cnpj.common.integrity_check == "ok"
        && rede.common.integrity_check == "ok"
        && search.common.integrity_check == "ok"
        && cnpj.indices.all_present
        && rede.indices.all_present;

    let any_exists = cnpj.common.exists
        || rede.common.exists
        || search.common.exists
        || (zips.exists && zips.total_found > 0);

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
