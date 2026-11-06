//! SQLite table DDL specifications, staging definitions, and post-load index SQL for `rede.db`.

use anyhow::{Context, Result};
use rusqlite::Connection;
use tracing::info;

/// DDL for the temporary unindexed staging table `ligacao1`.
pub const LIGACAO1_STAGING_DDL: &str = "
CREATE TABLE IF NOT EXISTS ligacao1 (
    origem TEXT,
    destino TEXT,
    tipo TEXT,
    base TEXT
);";

/// DDL for the final `ligacao` table with exact schema expected by `ecd-graph` and `rede-cnpj`.
pub const LIGACAO_FINAL_DDL: &str = "
CREATE TABLE IF NOT EXISTS ligacao (
    id1 TEXT NOT NULL,
    id2 TEXT NOT NULL,
    descricao TEXT NOT NULL DEFAULT 'SOCIO',
    comentario TEXT NOT NULL DEFAULT ''
);";

/// DDL for the `entidades` lookup table with primary key `id`.
pub const ENTIDADES_DDL: &str = "
CREATE TABLE IF NOT EXISTS entidades (
    id TEXT PRIMARY KEY,
    nome TEXT NOT NULL,
    tipo TEXT NOT NULL DEFAULT 'PJ'
);";

/// DDL for the GRC sanctions `links` table compatibility with `links.db`.
pub const LINKS_DDL: &str = "
CREATE TABLE IF NOT EXISTS links (
    id1 TEXT NOT NULL,
    id2 TEXT NOT NULL,
    descricao TEXT NOT NULL DEFAULT '',
    comentario TEXT NOT NULL DEFAULT ''
);";

/// DDL statements for secondary indices created strictly post-load for optimal insert speed.
pub const POST_LOAD_INDICES_SQL: &str = "
CREATE INDEX IF NOT EXISTS idx_ligacao_origem ON ligacao (id1);
CREATE INDEX IF NOT EXISTS idx_ligacao_destino ON ligacao (id2);
CREATE INDEX IF NOT EXISTS idx_entidades_tipo ON entidades (tipo);
CREATE INDEX IF NOT EXISTS idx_links_id1 ON links (id1);
CREATE INDEX IF NOT EXISTS idx_links_id2 ON links (id2);
";

/// Creates initial staging tables (`ligacao1`, `entidades`, `links`) without secondary indices.
pub fn create_graph_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(LIGACAO1_STAGING_DDL)
        .context("Failed to create ligacao1 staging table")?;
    conn.execute_batch(ENTIDADES_DDL)
        .context("Failed to create entidades table")?;
    conn.execute_batch(LINKS_DDL)
        .context("Failed to create links table")?;
    Ok(())
}

/// Creates secondary indices on `ligacao`, `entidades`, and `links` after bulk load completes.
pub fn create_graph_indices(conn: &Connection) -> Result<()> {
    info!("Building secondary indices on ligacao, entidades, and links...");
    conn.execute_batch(POST_LOAD_INDICES_SQL)
        .context("Failed to create post-load indices on rede.db")?;
    info!("Secondary indices created successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_creation_and_indexing() {
        let conn = Connection::open_in_memory().unwrap();
        create_graph_schema(&conn).unwrap();

        // Check tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert!(tables.contains(&"ligacao1".to_string()));
        assert!(tables.contains(&"entidades".to_string()));
        assert!(tables.contains(&"links".to_string()));

        // Create ligacao final table and indices
        conn.execute_batch(LIGACAO_FINAL_DDL).unwrap();
        create_graph_indices(&conn).unwrap();

        let indices: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' ORDER BY name;")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert!(indices.contains(&"idx_ligacao_origem".to_string()));
        assert!(indices.contains(&"idx_ligacao_destino".to_string()));
        assert!(indices.contains(&"idx_entidades_tipo".to_string()));
    }
}
