//! FTS5 search index creation, population, segment optimization, and query execution.
//!
//! Generates `rede_search.db` featuring full-text search over company names,
//! trade names (nome fantasia), and partner names/IDs using SQLite's FTS5 engine
//! with unicode61 diacritic-insensitive tokenization.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use tracing::info;

use crate::graph::attachment::{apply_graph_pragmas, finalize_database_to_wal, AttachedDatabase};

/// DDL for the FTS5 virtual table `id_search`.
pub const FTS5_SEARCH_TABLE_DDL: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS id_search USING fts5 (
    id,
    descricao,
    tokenize = 'unicode61 remove_diacritics 2'
);";

/// Population query deriving entity IDs and descriptions from attached `cnpj` and `rede` databases.
pub const FTS5_POPULATE_SQL: &str = "
INSERT INTO id_search (id, descricao)
SELECT id, descricao
FROM (
    -- 1. Matriz CNPJ with Razão Social
    SELECT 'PJ_' || te.cnpj AS id, em.razao_social AS descricao
    FROM cnpj.estabelecimento te
    LEFT JOIN cnpj.empresas em ON em.cnpj_basico = te.cnpj_basico
    WHERE te.matriz_filial = '1' AND em.razao_social IS NOT NULL AND em.razao_social <> ''

    UNION ALL

    -- 2. All establishments with Nome Fantasia (including filiais with empty fantasia for prefix lookup)
    SELECT 'PJ_' || te.cnpj AS id, COALESCE(te.nome_fantasia, '') AS descricao
    FROM cnpj.estabelecimento te

    UNION ALL

    -- 3. Origin non-PJ entities from ligacao (PF, PE, etc.)
    SELECT id1 AS id, '' AS descricao
    FROM rede.ligacao
    WHERE substr(id1, 1, 3) <> 'PJ_'

    UNION ALL

    -- 4. Destination non-PJ entities from ligacao
    SELECT id2 AS id, '' AS descricao
    FROM rede.ligacao
    WHERE substr(id2, 1, 3) <> 'PJ_'
) AS tunion
GROUP BY id, descricao;
";

/// Optimization command to merge FTS5 b-tree segments.
pub const FTS5_OPTIMIZE_SQL: &str = "INSERT INTO id_search(id_search) VALUES('optimize');";

/// Sanitizes an input search term for safe execution against an SQLite FTS5 MATCH expression.
///
/// Prevents SQLite FTS5 syntax errors from:
/// - Formatted CNPJs (`11.223.344/0001-91` -> `"11223344000191"`, `11.223.344` -> `"11223344"*`)
/// - Hyphenated trade names (`ALFA-TECH` -> `"ALFA-TECH"`)
/// - Punctuation / abbreviations (`C&A` -> `"C&A"`, `S.A.` -> `"S.A."`, `D'AVILA` -> `"D'AVILA"`)
/// - Prefix wildcards (`TECNOL*` -> `"TECNOL"*`)
/// - Reserved operators (`AND`, `OR`, `NOT`, `NEAR` -> escaped safely inside double quotes)
/// - Unbalanced quotes, colons (`id:col`), and asterisks
pub fn sanitize_fts5_term(term: &str) -> String {
    let trimmed = term.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // 1. Exact quoted string: if user explicitly provided a fully double-quoted string
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 1 {
        let inner = trimmed[1..trimmed.len() - 1].replace('"', "\"\"");
        let inner_trimmed = inner.trim();
        if !inner_trimmed.is_empty() {
            return format!("\"{inner_trimmed}\"");
        }
        return String::new();
    }

    // 2. Formatted CNPJ: check if input is formatted CNPJ (digits with ., /, -, spaces)
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    let is_cnpj_like = !digits.is_empty()
        && trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '/' || c == '-' || c.is_whitespace());

    if is_cnpj_like && trimmed.contains(['.', '/']) {
        if digits.len() >= 14 {
            return format!("\"{}\"", &digits[..14]);
        } else if digits.len() >= 8 {
            return format!("\"{}\"*", &digits[..8]);
        } else {
            return format!("\"{digits}\"*");
        }
    }

    // 3. Tokenize general words separated by whitespace
    let mut tokens = Vec::new();
    for word in trimmed.split_whitespace() {
        // Strip leading asterisks to avoid invalid FTS5 syntax
        let without_leading_star = word.trim_start_matches('*');
        let has_wildcard = without_leading_star.ends_with('*');
        let clean_word = without_leading_star.trim_end_matches('*').replace('"', "\"\"");

        // Ignore tokens that contain no alphanumeric characters (e.g. lone punctuation)
        if clean_word.is_empty() || !clean_word.chars().any(|c| c.is_alphanumeric()) {
            continue;
        }

        if has_wildcard {
            tokens.push(format!("\"{clean_word}\"*"));
        } else {
            tokens.push(format!("\"{clean_word}\""));
        }
    }

    tokens.join(" ")
}

/// Builds `dados-publicos/rede_search.db` from `cnpj.db` and `rede.db`.
pub fn build_search_database(
    cnpj_db_path: &Path,
    rede_db_path: &Path,
    output_search_path: &Path,
    memory_limit_mb: usize,
    drop_existing: bool,
) -> Result<u64> {
    if output_search_path.exists() && drop_existing {
        info!("Removing existing search database at {:?}", output_search_path);
        let _ = std::fs::remove_file(output_search_path);
        let wal = output_search_path.with_extension("db-wal");
        if wal.exists() {
            let _ = std::fs::remove_file(wal);
        }
        let shm = output_search_path.with_extension("db-shm");
        if shm.exists() {
            let _ = std::fs::remove_file(shm);
        }
    }

    if let Some(parent) = output_search_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory for {:?}", output_search_path))?;
    }

    let conn = Connection::open(output_search_path)
        .with_context(|| format!("Failed to open search database at {:?}", output_search_path))?;

    apply_graph_pragmas(&conn, memory_limit_mb)?;

    // Attach dependencies
    let att_cnpj = AttachedDatabase::attach(&conn, cnpj_db_path, "cnpj")?;
    let att_rede = AttachedDatabase::attach(&conn, rede_db_path, "rede")?;

    info!("Initializing FTS5 virtual table id_search...");
    conn.execute_batch(FTS5_SEARCH_TABLE_DDL)
        .context("Failed to create FTS5 id_search virtual table")?;

    info!("Populating id_search from attached cnpj and rede databases...");
    conn.execute_batch(FTS5_POPULATE_SQL)
        .context("Failed to populate FTS5 id_search virtual table")?;

    // Explicitly detach attached databases prior to post-load PRAGMAs
    att_cnpj.detach()?;
    att_rede.detach()?;

    info!("Optimizing FTS5 index segments...");
    conn.execute_batch(FTS5_OPTIMIZE_SQL)
        .context("Failed to optimize FTS5 id_search segments")?;

    info!("Transitioning rede_search.db to WAL mode...");
    finalize_database_to_wal(&conn)?;

    let count: i64 = conn
        .query_row("SELECT count(*) FROM id_search;", [], |r| r.get(0))
        .unwrap_or(0);

    info!(total_indexed = count, "Successfully generated rede_search.db");
    Ok(count as u64)
}

/// Query helper to search by full-text token matching both id and descricao.
pub fn search_by_term(conn: &Connection, term: &str, limit: usize) -> rusqlite::Result<Vec<(String, String)>> {
    let sanitized = sanitize_fts5_term(term);
    if sanitized.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare_cached(
        "SELECT id, descricao FROM id_search WHERE id_search MATCH ?1 LIMIT ?2;",
    )?;
    let rows = stmt.query_map(params![sanitized, limit as i64], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Query helper to search by CNPJ radical prefix (e.g. '11223344' -> matches 'PJ_11223344*').
pub fn search_by_cnpj_basico(conn: &Connection, cnpj_basico: &str, limit: usize) -> rusqlite::Result<Vec<String>> {
    let digits: String = cnpj_basico.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Ok(Vec::new());
    }
    let radical = if digits.len() >= 8 {
        &digits[..8]
    } else {
        &digits[..]
    };
    let match_pattern = format!("PJ_{radical}*");
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT id FROM id_search WHERE id MATCH ?1 LIMIT ?2;",
    )?;
    let rows = stmt.query_map(params![match_pattern, limit as i64], |r| r.get::<_, String>(0))?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_fts5_virtual_table_and_queries() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(FTS5_SEARCH_TABLE_DDL).unwrap();

        conn.execute_batch(
            "INSERT INTO id_search VALUES ('PJ_11111111000191', 'ALFA TECNOLOGIA LTDA');
             INSERT INTO id_search VALUES ('PJ_11111111000191', 'ALFA TECH');
             INSERT INTO id_search VALUES ('PF_***123456**-JOAO DA SILVA', '');",
        )
        .unwrap();

        let term_results = search_by_term(&conn, "TECNOLOGIA", 10).unwrap();
        assert_eq!(term_results.len(), 1);
        assert_eq!(term_results[0].0, "PJ_11111111000191");

        let cnpj_results = search_by_cnpj_basico(&conn, "11111111", 10).unwrap();
        assert_eq!(cnpj_results.len(), 1);
        assert_eq!(cnpj_results[0], "PJ_11111111000191");

        let pf_results = search_by_term(&conn, "JOAO", 10).unwrap();
        assert_eq!(pf_results.len(), 1);
        assert_eq!(pf_results[0].0, "PF_***123456**-JOAO DA SILVA");
    }

    #[test]
    fn test_fts5_sanitization_formatted_cnpj_and_hyphens() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(FTS5_SEARCH_TABLE_DDL).unwrap();

        conn.execute_batch(
            "INSERT INTO id_search VALUES ('PJ_11223344000191', 'ALFA-TECH LTDA');
             INSERT INTO id_search VALUES ('PJ_11223344000272', 'ALFA FILIAL');
             INSERT INTO id_search VALUES ('PJ_001', 'C&A MODAS S.A.');
             INSERT INTO id_search VALUES ('PF_001', 'PF_***123456**-D''AVILA SILVA');",
        )
        .unwrap();

        // 1. Formatted CNPJ
        let r1 = search_by_term(&conn, "11.223.344/0001-91", 10).unwrap();
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].0, "PJ_11223344000191");

        // 2. Formatted CNPJ radical
        let r2 = search_by_term(&conn, "11.223.344", 10).unwrap();
        assert_eq!(r2.len(), 2);

        // 3. search_by_cnpj_basico with formatted input
        let r3 = search_by_cnpj_basico(&conn, "11.223.344/0001-91", 10).unwrap();
        assert_eq!(r3.len(), 2);

        // 4. Hyphenated trade name
        let r4 = search_by_term(&conn, "ALFA-TECH", 10).unwrap();
        assert_eq!(r4.len(), 1);

        // 5. Special punctuation: &, ., '
        let r5 = search_by_term(&conn, "C&A", 10).unwrap();
        assert_eq!(r5.len(), 1);

        let r6 = search_by_term(&conn, "S.A.", 10).unwrap();
        assert_eq!(r6.len(), 1);

        let r7 = search_by_term(&conn, "D'AVILA", 10).unwrap();
        assert_eq!(r7.len(), 1);

        // 6. Trailing boolean operator and malformed input
        let r8 = search_by_term(&conn, "MODAS OR", 10).unwrap();
        assert_eq!(r8.len(), 0);

        let r9 = search_by_term(&conn, "", 10).unwrap();
        assert_eq!(r9.len(), 0);

        let r10 = search_by_cnpj_basico(&conn, "", 10).unwrap();
        assert_eq!(r10.len(), 0);
    }

    #[test]
    fn test_build_search_database_end_to_end() {
        let cnpj_tmp = NamedTempFile::new().unwrap();
        let rede_tmp = NamedTempFile::new().unwrap();
        let search_tmp = NamedTempFile::new().unwrap();

        // Seed mock cnpj.db
        {
            let conn = Connection::open(cnpj_tmp.path()).unwrap();
            conn.execute_batch(
                "CREATE TABLE estabelecimento (
                    cnpj_basico TEXT,
                    matriz_filial TEXT,
                    nome_fantasia TEXT,
                    cnpj TEXT
                 );
                 CREATE TABLE empresas (
                     cnpj_basico TEXT,
                     razao_social TEXT
                 );
                 INSERT INTO estabelecimento VALUES ('11111111', '1', 'FANTASIA ALFA', '11111111000191');
                 INSERT INTO empresas VALUES ('11111111', 'ALFA RAZAO LTDA');",
            )
            .unwrap();
        }

        // Seed mock rede.db
        {
            let conn = Connection::open(rede_tmp.path()).unwrap();
            conn.execute_batch(
                "CREATE TABLE ligacao (
                    id1 TEXT,
                    id2 TEXT,
                    descricao TEXT,
                    comentario TEXT
                 );
                 INSERT INTO ligacao VALUES ('PF_***123456**-JOAO', 'PJ_11111111000191', 'Sócio', 'socios');",
            )
            .unwrap();
        }

        let count = build_search_database(
            cnpj_tmp.path(),
            rede_tmp.path(),
            search_tmp.path(),
            512,
            true,
        )
        .unwrap();

        assert!(count >= 3);

        let search_conn = Connection::open(search_tmp.path()).unwrap();
        let hits = search_by_term(&search_conn, "ALFA", 10).unwrap();
        assert!(hits.len() >= 2);
    }

    #[test]
    fn test_fts5_portuguese_diacritics_and_special_chars() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(FTS5_SEARCH_TABLE_DDL).unwrap();

        conn.execute_batch(
            "INSERT INTO id_search VALUES ('PJ_1', 'COMÉRCIO E INDÚSTRIA LTDA');
             INSERT INTO id_search VALUES ('PJ_2', 'MINERAÇÃO CONCEIÇÃO S.A.');
             INSERT INTO id_search VALUES ('PJ_3', 'AUTO POSTO SÃO PAULO');",
        )
        .unwrap();

        // Query unaccented matching accented
        let res1 = search_by_term(&conn, "comercio", 10).unwrap();
        assert_eq!(res1.len(), 1);
        assert_eq!(res1[0].0, "PJ_1");

        let res2 = search_by_term(&conn, "industria", 10).unwrap();
        assert_eq!(res2.len(), 1);
        assert_eq!(res2[0].0, "PJ_1");

        let res3 = search_by_term(&conn, "conceicao", 10).unwrap();
        assert_eq!(res3.len(), 1);
        assert_eq!(res3[0].0, "PJ_2");

        let res4 = search_by_term(&conn, "sao", 10).unwrap();
        assert_eq!(res4.len(), 1);
        assert_eq!(res4[0].0, "PJ_3");
    }
}
