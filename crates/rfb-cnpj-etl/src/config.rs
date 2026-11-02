//! Central configuration constants, SQLite PRAGMA presets, and canonical file catalogs.

/// Default RFB WebDAV share token for Nextcloud repository.
pub const DEFAULT_SHARE_TOKEN: &str = "YggdBLfdninEJX9";

/// Default RFB WebDAV base URL.
pub const DEFAULT_WEBDAV_BASE_URL: &str =
    "https://arquivos.receitafederal.gov.br/public.php/webdav";

/// Default browser User-Agent header used to bypass F5 BIG-IP WAF challenge cookies.
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Default directory for downloaded zip archives.
pub const DEFAULT_ZIP_DIR: &str = "dados-publicos-zip";

/// Default directory for output SQLite databases.
pub const DEFAULT_DATA_DIR: &str = "dados-publicos";

/// Default output filename for `cnpj.db`.
pub const DEFAULT_CNPJ_DB: &str = "dados-publicos/cnpj.db";

/// Default output filename for `rede.db`.
pub const DEFAULT_REDE_DB: &str = "dados-publicos/rede.db";

/// Default output filename for `rede_search.db`.
pub const DEFAULT_SEARCH_DB: &str = "dados-publicos/rede_search.db";

/// Bounded MPSC channel capacity (chunks in flight).
pub const BOUNDED_CHANNEL_CAPACITY: usize = 16;

/// Default number of rows per insert transaction batch.
pub const DEFAULT_BATCH_SIZE: usize = 5000;

/// Canonical list of all 37 official RFB CNPJ open data zip files.
pub const CANONICAL_ZIP_FILES: [&str; 37] = [
    // 6 Domain lookup tables
    "Cnaes.zip",
    "Motivos.zip",
    "Municipios.zip",
    "Naturezas.zip",
    "Paises.zip",
    "Qualificacoes.zip",
    // 1 Special table
    "Simples.zip",
    // 10 Enterprises partitions
    "Empresas0.zip",
    "Empresas1.zip",
    "Empresas2.zip",
    "Empresas3.zip",
    "Empresas4.zip",
    "Empresas5.zip",
    "Empresas6.zip",
    "Empresas7.zip",
    "Empresas8.zip",
    "Empresas9.zip",
    // 10 Establishments partitions
    "Estabelecimentos0.zip",
    "Estabelecimentos1.zip",
    "Estabelecimentos2.zip",
    "Estabelecimentos3.zip",
    "Estabelecimentos4.zip",
    "Estabelecimentos5.zip",
    "Estabelecimentos6.zip",
    "Estabelecimentos7.zip",
    "Estabelecimentos8.zip",
    "Estabelecimentos9.zip",
    // 10 Partners partitions
    "Socios0.zip",
    "Socios1.zip",
    "Socios2.zip",
    "Socios3.zip",
    "Socios4.zip",
    "Socios5.zip",
    "Socios6.zip",
    "Socios7.zip",
    "Socios8.zip",
    "Socios9.zip",
];

/// SQLite PRAGMA configuration tailored for high-speed batch insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestionPragmas {
    pub page_size: u32,
    pub cache_size_kib: i64,
    pub threads: u32,
    pub temp_store_memory: bool,
}

impl Default for IngestionPragmas {
    fn default() -> Self {
        Self {
            page_size: 4096,
            cache_size_kib: -500_000, // 500 MB
            threads: 4,
            temp_store_memory: true,
        }
    }
}

impl IngestionPragmas {
    /// Calculate PRAGMAs dynamically from a given total memory limit in MB.
    pub fn from_memory_limit(memory_limit_mb: usize, worker_threads: usize) -> Self {
        // Allocate approximately 40% of the budget to SQLite page cache (min 128 MB)
        let cache_mb = (memory_limit_mb as f64 * 0.40).max(128.0) as i64;
        Self {
            page_size: 4096,
            cache_size_kib: -cache_mb * 1024,
            threads: worker_threads.clamp(1, 8) as u32,
            temp_store_memory: true,
        }
    }

    /// Apply high-throughput ingestion PRAGMAs to an active SQLite connection.
    pub fn apply_to(&self, conn: &rusqlite::Connection) -> rusqlite::Result<()> {
        exec_pragma(conn, &format!("PRAGMA page_size = {};", self.page_size))?;
        exec_pragma(conn, "PRAGMA journal_mode = OFF;")?;
        exec_pragma(conn, "PRAGMA synchronous = OFF;")?;
        exec_pragma(
            conn,
            &format!("PRAGMA cache_size = {};", self.cache_size_kib),
        )?;
        exec_pragma(
            conn,
            &format!(
                "PRAGMA temp_store = {};",
                if self.temp_store_memory {
                    "MEMORY"
                } else {
                    "FILE"
                }
            ),
        )?;
        exec_pragma(conn, "PRAGMA locking_mode = EXCLUSIVE;")?;
        exec_pragma(conn, "PRAGMA foreign_keys = OFF;")?;
        exec_pragma(conn, "PRAGMA auto_vacuum = NONE;")?;
        exec_pragma(conn, &format!("PRAGMA threads = {};", self.threads))?;
        Ok(())
    }

    /// Apply production post-load PRAGMAs (WAL mode, normal synchronous).
    pub fn apply_post_load(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
        exec_pragma(conn, "PRAGMA optimize;")?;
        exec_pragma(conn, "PRAGMA journal_mode = WAL;")?;
        exec_pragma(conn, "PRAGMA synchronous = NORMAL;")?;
        exec_pragma(conn, "PRAGMA locking_mode = NORMAL;")?;
        Ok(())
    }
}

/// Helper to execute a PRAGMA statement regardless of whether it yields result rows.
fn exec_pragma(conn: &rusqlite::Connection, sql: &str) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_canonical_files_count_and_uniqueness() {
        assert_eq!(CANONICAL_ZIP_FILES.len(), 37);
        let set: HashSet<&str> = CANONICAL_ZIP_FILES.iter().copied().collect();
        assert_eq!(set.len(), 37);
    }

    #[test]
    fn test_ingestion_pragmas_calculation() {
        let pragmas = IngestionPragmas::from_memory_limit(1536, 4);
        assert_eq!(pragmas.page_size, 4096);
        assert_eq!(pragmas.threads, 4);
        assert!(pragmas.cache_size_kib < 0); // negative means KiB
        assert!(pragmas.temp_store_memory);
    }

    #[test]
    fn test_ingestion_pragmas_apply() {
        let conn = rusqlite::Connection::open_in_memory().expect("open memory db");
        let pragmas = IngestionPragmas::default();
        pragmas.apply_to(&conn).expect("apply pragmas");
        IngestionPragmas::apply_post_load(&conn).expect("apply post load");
    }
}
