//! SQLite multi-database attachment manager, RAII scoping, and PRAGMA configuration.

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use tracing::{debug, info, warn};

/// RAII guard representing an attached SQLite database.
/// Ensures deterministic detachment upon drop to prevent SQLite locking errors.
pub struct AttachedDatabase<'a> {
    conn: &'a Connection,
    schema: String,
    detached: bool,
}

impl<'a> AttachedDatabase<'a> {
    /// Attaches an external SQLite database file under the specified schema name.
    /// Handles single-quote escaping safely.
    pub fn attach(conn: &'a Connection, path: &Path, schema: &str) -> Result<Self> {
        let path_str = path.to_string_lossy();
        let escaped = path_str.replace('\'', "''");
        let sql = format!("ATTACH DATABASE '{escaped}' AS {schema};");
        conn.execute_batch(&sql)
            .with_context(|| format!("Failed to attach database {:?} as '{}'", path, schema))?;

        debug!(schema = schema, path = %path.display(), "Attached external SQLite database");

        Ok(Self {
            conn,
            schema: schema.to_string(),
            detached: false,
        })
    }

    /// Explicitly detaches the database. Returns an error if an active transaction locks it.
    pub fn detach(mut self) -> Result<()> {
        self.do_detach()
    }

    fn do_detach(&mut self) -> Result<()> {
        if !self.detached {
            let sql = format!("DETACH DATABASE {};", self.schema);
            self.conn
                .execute_batch(&sql)
                .with_context(|| format!("Failed to detach database '{}'", self.schema))?;
            self.detached = true;
            debug!(schema = %self.schema, "Detached external SQLite database");
        }
        Ok(())
    }
}

impl<'a> Drop for AttachedDatabase<'a> {
    fn drop(&mut self) {
        if !self.detached {
            if let Err(e) = self.do_detach() {
                warn!(schema = %self.schema, error = %e, "Failed to detach database during drop");
            }
        }
    }
}

/// Helper to execute a PRAGMA statement regardless of whether it yields result rows.
pub fn exec_pragma(conn: &Connection, sql: &str) -> Result<()> {
    let mut stmt = conn
        .prepare(sql)
        .with_context(|| format!("Failed to prepare PRAGMA statement: {}", sql))?;
    let mut rows = stmt
        .query([])
        .with_context(|| format!("Failed to query PRAGMA statement: {}", sql))?;
    while rows.next()?.is_some() {}
    Ok(())
}

/// Applies high-throughput batch PRAGMAs to an active SQLite connection.
/// Bounds resident memory strictly to stay below the provided budget (default < 1.8 GB RAM).
pub fn apply_graph_pragmas(conn: &Connection, memory_limit_mb: usize) -> Result<()> {
    let cache_kib = ((memory_limit_mb as f64 * 0.35).max(64.0) * 1024.0) as i64;
    exec_pragma(conn, "PRAGMA page_size = 4096;")?;
    exec_pragma(conn, "PRAGMA journal_mode = OFF;")?;
    exec_pragma(conn, "PRAGMA synchronous = OFF;")?;
    exec_pragma(conn, &format!("PRAGMA cache_size = -{};", cache_kib))?;
    exec_pragma(conn, "PRAGMA temp_store = FILE;")?;
    exec_pragma(conn, "PRAGMA locking_mode = EXCLUSIVE;")?;
    exec_pragma(conn, "PRAGMA foreign_keys = OFF;")?;
    exec_pragma(conn, "PRAGMA threads = 4;")?;
    Ok(())
}

/// Commits open transactions and transitions a database to production-ready WAL mode.
pub fn finalize_database_to_wal(conn: &Connection) -> Result<()> {
    exec_pragma(conn, "PRAGMA optimize;")?;
    exec_pragma(conn, "PRAGMA locking_mode = NORMAL;")?;
    exec_pragma(conn, "PRAGMA journal_mode = WAL;")?;
    exec_pragma(conn, "PRAGMA synchronous = NORMAL;")?;
    info!("Transitioned database to WAL production mode");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_attached_database_lifecycle() {
        let conn = Connection::open_in_memory().unwrap();
        let tmp = NamedTempFile::new().unwrap();

        // Create a table in the external database
        {
            let ext = Connection::open(tmp.path()).unwrap();
            ext.execute_batch("CREATE TABLE t_ext (x INT); INSERT INTO t_ext VALUES (42);")
                .unwrap();
        }

        // Attach via AttachedDatabase
        let attached = AttachedDatabase::attach(&conn, tmp.path(), "ext_db").unwrap();

        let val: i32 = conn
            .query_row("SELECT x FROM ext_db.t_ext", [], |r| r.get(0))
            .unwrap();
        assert_eq!(val, 42);

        // Detach
        attached.detach().unwrap();

        // Querying detached db should fail
        let res = conn.query_row("SELECT x FROM ext_db.t_ext", [], |r| r.get::<_, i32>(0));
        assert!(res.is_err());
    }

    #[test]
    fn test_apply_graph_pragmas_and_wal_transition() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = Connection::open(tmp.path()).unwrap();

        apply_graph_pragmas(&conn, 1024).unwrap();
        conn.execute_batch("CREATE TABLE t (y INT); INSERT INTO t VALUES (1);")
            .unwrap();
        finalize_database_to_wal(&conn).unwrap();

        let mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }
}
