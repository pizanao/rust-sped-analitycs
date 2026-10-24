//! # ecd-storage
//!
//! Camada de persistência e acesso a dados:
//! - PostgreSQL via SQLx (schema core_* preservado e tabelas de apoio otimizadas)
//! - SQLite via Rusqlite + R2D2 pool (leitura somente-leitura das bases da Receita Federal)

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Erro no PostgreSQL: {0}")]
    Postgres(#[from] sqlx::Error),
    #[error("Erro no SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub async fn create_postgres_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(50)
        .connect(database_url)
        .await
}
