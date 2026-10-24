//! Pool de conexões SQLite thread-safe (`r2d2_sqlite`) configurado para alta performance
//! em modo estrito somente-leitura (`query_only = ON`) com mapeamento de memória (`mmap_size`)
//! e cache de páginas (`cache_size`).

use crate::{GraphError, Result};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 256 MB de mapeamento de memória por conexão SQLite.
pub const DEFAULT_MMAP_SIZE: i64 = 268_435_456;

/// 64 MB de cache de páginas em memória (`-64000` KiB).
pub const DEFAULT_CACHE_SIZE: i64 = -64_000;

/// Tamanho máximo padrão do pool de conexões de leitura.
pub const DEFAULT_POOL_MAX_SIZE: u32 = 16;

/// Estado efetivo dos PRAGMAs de uma conexão do pool SQLite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PragmaStatus {
    pub query_only: bool,
    pub mmap_size: i64,
    pub cache_size: i64,
}

/// Configuração do store SQLite para a Rede CNPJ (`rede.db`, `cnpj.db`, `links.db`).
#[derive(Clone, Debug)]
pub struct SqliteGraphConfig {
    pub rede_db_path: PathBuf,
    pub cnpj_db_path: Option<PathBuf>,
    pub links_db_path: Option<PathBuf>,
    pub max_pool_size: u32,
    pub mmap_size: i64,
    pub cache_size: i64,
}

impl SqliteGraphConfig {
    pub fn new<P: AsRef<Path>>(rede_db_path: P) -> Self {
        Self {
            rede_db_path: rede_db_path.as_ref().to_path_buf(),
            cnpj_db_path: None,
            links_db_path: None,
            max_pool_size: DEFAULT_POOL_MAX_SIZE,
            mmap_size: DEFAULT_MMAP_SIZE,
            cache_size: DEFAULT_CACHE_SIZE,
        }
    }

    pub fn with_cnpj_db<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.cnpj_db_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_links_db<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.links_db_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_max_pool_size(mut self, max_pool_size: u32) -> Self {
        self.max_pool_size = max_pool_size;
        self
    }
}

/// Store thread-safe de grafos societários apoiado em `r2d2_sqlite`.
#[derive(Clone, Debug)]
pub struct SqliteGraphStore {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteGraphStore {
    /// Cria um novo `SqliteGraphStore` sobre o arquivo SQLite informado com os PRAGMAs padrão:
    /// - `PRAGMA mmap_size = 268435456;`
    /// - `PRAGMA cache_size = -64000;`
    /// - `PRAGMA query_only = ON;`
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        Self::from_config(SqliteGraphConfig::new(db_path))
    }

    /// Cria um `SqliteGraphStore` com tamanho máximo de pool customizado.
    pub fn new_with_pool_size<P: AsRef<Path>>(db_path: P, max_size: u32) -> Result<Self> {
        Self::from_config(SqliteGraphConfig::new(db_path).with_max_pool_size(max_size))
    }

    /// Cria um `SqliteGraphStore` anexando opcionalmente `cnpj.db` e `links.db` em cada conexão.
    pub fn with_attached_dbs<P: AsRef<Path>>(
        rede_db_path: P,
        cnpj_db_path: Option<P>,
        links_db_path: Option<P>,
    ) -> Result<Self> {
        let mut cfg = SqliteGraphConfig::new(rede_db_path);
        if let Some(cnpj) = cnpj_db_path {
            cfg = cfg.with_cnpj_db(cnpj);
        }
        if let Some(links) = links_db_path {
            cfg = cfg.with_links_db(links);
        }
        Self::from_config(cfg)
    }

    /// Constrói o pool `r2d2` a partir de uma [`SqliteGraphConfig`].
    pub fn from_config(config: SqliteGraphConfig) -> Result<Self> {
        let cnpj_db = config.cnpj_db_path.clone();
        let links_db = config.links_db_path.clone();
        let mmap_size = config.mmap_size;
        let cache_size = config.cache_size;

        let manager = SqliteConnectionManager::file(&config.rede_db_path).with_init(move |conn| {
            if let Some(ref cnpj_path) = cnpj_db {
                let escaped = cnpj_path.to_string_lossy().replace('\'', "''");
                conn.execute_batch(&format!("ATTACH DATABASE '{escaped}' AS cnpj_db;"))?;
            }
            if let Some(ref links_path) = links_db {
                let escaped = links_path.to_string_lossy().replace('\'', "''");
                conn.execute_batch(&format!("ATTACH DATABASE '{escaped}' AS links_db;"))?;
            }
            let pragma_sql = format!(
                "PRAGMA mmap_size = {mmap_size};
                 PRAGMA cache_size = {cache_size};
                 PRAGMA temp_store = MEMORY;
                 PRAGMA query_only = ON;"
            );
            conn.execute_batch(&pragma_sql)
        });

        let pool = Pool::builder()
            .max_size(config.max_pool_size)
            .build(manager)?;

        Ok(Self { pool })
    }

    /// Cria um `SqliteGraphStore` sobre um banco SQLite em memória (`:memory:`), executando
    /// a função `setup` para carga inicial de dados antes de ativar `PRAGMA query_only = ON`.
    pub fn new_in_memory_with_setup<F>(setup: F) -> Result<Self>
    where
        F: Fn(&Connection) -> Result<()> + Send + Sync + 'static,
    {
        let mmap_size = DEFAULT_MMAP_SIZE;
        let cache_size = DEFAULT_CACHE_SIZE;

        let manager = SqliteConnectionManager::memory().with_init(move |conn| {
            Self::init_schema(conn).map_err(|e| match e {
                GraphError::Sqlite(err) => err,
                _ => rusqlite::Error::InvalidQuery,
            })?;
            setup(conn).map_err(|e| match e {
                GraphError::Sqlite(err) => err,
                _ => rusqlite::Error::InvalidQuery,
            })?;
            let pragma_sql = format!(
                "PRAGMA mmap_size = {mmap_size};
                 PRAGMA cache_size = {cache_size};
                 PRAGMA temp_store = MEMORY;
                 PRAGMA query_only = ON;"
            );
            conn.execute_batch(&pragma_sql)
        });

        let pool = Pool::builder().max_size(1).build(manager)?;
        Ok(Self { pool })
    }

    /// Obtém uma conexão emprestada do pool `r2d2`.
    pub fn get_conn(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    /// Retorna referência ao pool `r2d2` subjacente.
    pub fn pool(&self) -> &Pool<SqliteConnectionManager> {
        &self.pool
    }

    /// Lê os PRAGMAs ativos na conexão para verificação e diagnóstico.
    pub fn verificar_pragmas(&self) -> Result<PragmaStatus> {
        use rusqlite::OptionalExtension;

        let conn = self.get_conn()?;
        let query_only_int: i64 = conn.query_row("PRAGMA query_only;", [], |row| row.get(0))?;
        let mmap_size: i64 = conn
            .query_row("PRAGMA mmap_size;", [], |row| row.get(0))
            .optional()?
            .unwrap_or(DEFAULT_MMAP_SIZE);
        let cache_size: i64 = conn.query_row("PRAGMA cache_size;", [], |row| row.get(0))?;

        Ok(PragmaStatus {
            query_only: query_only_int != 0,
            mmap_size,
            cache_size,
        })
    }

    /// Inicializa o schema das tabelas da Rede CNPJ (`entidades`, `ligacao`, `links`)
    /// em uma conexão SQLite gravável (útil para criação de bancos de teste e fixtures).
    pub fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entidades (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                tipo TEXT NOT NULL DEFAULT 'PJ'
            );

            CREATE TABLE IF NOT EXISTS ligacao (
                id1 TEXT NOT NULL,
                id2 TEXT NOT NULL,
                descricao TEXT NOT NULL DEFAULT 'SOCIO',
                comentario TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_ligacao_id1 ON ligacao(id1);
            CREATE INDEX IF NOT EXISTS idx_ligacao_id2 ON ligacao(id2);

            CREATE TABLE IF NOT EXISTS links (
                id1 TEXT NOT NULL,
                id2 TEXT NOT NULL,
                descricao TEXT NOT NULL DEFAULT '',
                comentario TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_links_id1 ON links(id1);
            CREATE INDEX IF NOT EXISTS idx_links_id2 ON links(id2);",
        )
        .map_err(GraphError::from)
    }
}
