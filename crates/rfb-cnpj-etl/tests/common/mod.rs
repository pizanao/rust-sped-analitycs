//! Common test helpers, SQLite schema assertion utilities, and CLI process runner.
#![allow(dead_code)]

use anyhow::{Context, Result};
use assert_cmd::Command;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Output;
use tempfile::TempDir;

use crate::fixtures::generator::{
    generate_synthetic_rfb_dataset, GeneratorOptions, SyntheticDatasetSummary,
};

#[allow(dead_code)]
/// Isolated test sandbox containing a unique temporary directory and standard output paths.
pub struct TestEnvironment {
    pub dir: TempDir,
    pub zip_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cnpj_db_path: PathBuf,
    pub rede_db_path: PathBuf,
    pub search_db_path: PathBuf,
}

impl TestEnvironment {
    /// Creates a fresh, isolated test environment.
    pub fn new() -> Result<Self> {
        let dir =
            TempDir::new().context("Failed to create temporary directory for test environment")?;
        let zip_dir = dir.path().join("dados-publicos-zip");
        let data_dir = dir.path().join("dados-publicos");
        let cnpj_db_path = data_dir.join("cnpj.db");
        let rede_db_path = data_dir.join("rede.db");
        let search_db_path = data_dir.join("rede_search.db");

        std::fs::create_dir_all(&zip_dir)?;
        std::fs::create_dir_all(&data_dir)?;

        Ok(Self {
            dir,
            zip_dir,
            data_dir,
            cnpj_db_path,
            rede_db_path,
            search_db_path,
        })
    }

    /// Populates `zip_dir` with the full 37-file synthetic dataset.
    pub fn populate_fixtures(
        &self,
        options: Option<GeneratorOptions>,
    ) -> Result<SyntheticDatasetSummary> {
        generate_synthetic_rfb_dataset(&self.zip_dir, options)
    }

    /// Executes `rfb-cnpj-etl` CLI binary with arguments in this test environment.
    pub fn run_cli(&self, args: &[&str]) -> Result<Output> {
        let mut cmd =
            Command::cargo_bin("rfb-cnpj-etl").context("Failed to locate rfb-cnpj-etl binary")?;
        cmd.current_dir(self.dir.path());
        cmd.args(args);
        let output = cmd
            .output()
            .context("Failed to execute rfb-cnpj-etl process")?;
        Ok(output)
    }

    /// Opens a read-only SQLite connection to `cnpj.db`.
    pub fn open_cnpj_db(&self) -> Result<Connection> {
        open_sqlite_readonly(&self.cnpj_db_path)
    }

    /// Opens a read-only SQLite connection to `rede.db`.
    pub fn open_rede_db(&self) -> Result<Connection> {
        open_sqlite_readonly(&self.rede_db_path)
    }

    /// Opens a read-only SQLite connection to `rede_search.db`.
    pub fn open_search_db(&self) -> Result<Connection> {
        open_sqlite_readonly(&self.search_db_path)
    }
}

/// Opens SQLite connection in read-only mode.
pub fn open_sqlite_readonly(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("Failed to open SQLite database at {:?}", path))
}

/// Checks if a table exists in the SQLite database.
pub fn table_exists(conn: &Connection, table_name: &str) -> bool {
    let sql = "SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1 LIMIT 1;";
    conn.query_row(sql, [table_name], |_| Ok(())).is_ok()
}

/// Returns list of all table and view names in SQLite database.
pub fn list_tables(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'view');")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut set = HashSet::new();
    for row in rows {
        set.insert(row?);
    }
    Ok(set)
}

/// Returns list of columns (name, type) for a table via `PRAGMA table_info`.
pub fn get_table_columns(conn: &Connection, table_name: &str) -> Result<Vec<(String, String)>> {
    let pragma_sql = format!("PRAGMA table_info({});", table_name);
    let mut stmt = conn.prepare(&pragma_sql)?;
    let rows = stmt.query_map([], |row| {
        let name: String = row.get(1)?;
        let col_type: String = row.get(2)?;
        Ok((name, col_type.to_uppercase()))
    })?;

    let mut cols = Vec::new();
    for row in rows {
        cols.push(row?);
    }
    Ok(cols)
}

/// Returns set of index names registered on a table via `PRAGMA index_list`.
pub fn get_table_indices(conn: &Connection, table_name: &str) -> Result<HashSet<String>> {
    let pragma_sql = format!("PRAGMA index_list({});", table_name);
    let mut stmt = conn.prepare(&pragma_sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;

    let mut indices = HashSet::new();
    for row in rows {
        indices.insert(row?);
    }
    Ok(indices)
}

/// Returns the exact row count of a table.
pub fn get_row_count(conn: &Connection, table_name: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {};", table_name);
    let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
    Ok(count as usize)
}

/// Checks if an edge (id1 -> id2) exists in `ligacao`.
pub fn edge_exists(conn: &Connection, id1: &str, id2: &str) -> Result<bool> {
    let sql = "SELECT 1 FROM ligacao WHERE id1 = ?1 AND id2 = ?2 LIMIT 1;";
    let exists = conn.query_row(sql, [id1, id2], |_| Ok(())).is_ok();
    Ok(exists)
}

/// Retrieves details `(descricao, comentario)` for an edge (id1 -> id2) in `ligacao`.
pub fn get_edge_details(
    conn: &Connection,
    id1: &str,
    id2: &str,
) -> Result<Option<(String, String)>> {
    let sql = "SELECT descricao, comentario FROM ligacao WHERE id1 = ?1 AND id2 = ?2 LIMIT 1;";
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([id1, id2])?;
    if let Some(row) = rows.next()? {
        let desc: String = row.get(0)?;
        let comment: String = row.get(1)?;
        Ok(Some((desc, comment)))
    } else {
        Ok(None)
    }
}

/// Counts matching rows in FTS5 `id_search` virtual table.
pub fn count_fts5_matches(conn: &Connection, term: &str) -> Result<usize> {
    let sql = "SELECT COUNT(*) FROM id_search WHERE id_search MATCH ?1;";
    let count: i64 = conn.query_row(sql, [term], |row| row.get(0))?;
    Ok(count as usize)
}
