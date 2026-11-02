//! SQLite table schemas, DDL definitions, and post-load index specifications.

use rusqlite::{Connection, Result};

/// Canonical list of all 6 domain lookup tables.
pub const DOMAIN_TABLES: [&str; 6] = [
    "cnae",
    "motivo",
    "municipio",
    "natureza_juridica",
    "pais",
    "qualificacao_socio",
];

/// SQL DDL for domain lookup tables (codigo, descricao).
pub fn domain_table_ddl(table_name: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {table_name} (
            codigo TEXT PRIMARY KEY,
            descricao TEXT
        );"
    )
}

/// SQL DDL for `empresas` (7 base columns + capital_social REAL).
pub const EMPRESAS_DDL: &str = "
CREATE TABLE IF NOT EXISTS empresas (
    cnpj_basico TEXT,
    razao_social TEXT,
    natureza_juridica TEXT,
    qualificacao_responsavel TEXT,
    capital_social_str TEXT,
    porte_empresa TEXT,
    ente_federativo_responsavel TEXT,
    capital_social REAL
);";

/// SQL DDL for `estabelecimento` (30 original columns + synthesized 14-char cnpj).
pub const ESTABELECIMENTO_DDL: &str = "
CREATE TABLE IF NOT EXISTS estabelecimento (
    cnpj_basico TEXT,
    cnpj_ordem TEXT,
    cnpj_dv TEXT,
    matriz_filial TEXT,
    nome_fantasia TEXT,
    situacao_cadastral TEXT,
    data_situacao_cadastral TEXT,
    motivo_situacao_cadastral TEXT,
    nome_cidade_exterior TEXT,
    pais TEXT,
    data_inicio_atividades TEXT,
    cnae_fiscal TEXT,
    cnae_fiscal_secundaria TEXT,
    tipo_logradouro TEXT,
    logradouro TEXT,
    numero TEXT,
    complemento TEXT,
    bairro TEXT,
    cep TEXT,
    uf TEXT,
    municipio TEXT,
    ddd1 TEXT,
    telefone1 TEXT,
    ddd2 TEXT,
    telefone2 TEXT,
    ddd_fax TEXT,
    fax TEXT,
    correio_eletronico TEXT,
    situacao_especial TEXT,
    data_situacao_especial TEXT,
    cnpj TEXT
);";

/// SQL DDL for `simples` (7 columns).
pub const SIMPLES_DDL: &str = "
CREATE TABLE IF NOT EXISTS simples (
    cnpj_basico TEXT,
    opcao_simples TEXT,
    data_opcao_simples TEXT,
    data_exclusao_simples TEXT,
    opcao_mei TEXT,
    data_opcao_mei TEXT,
    data_exclusao_mei TEXT
);";

/// SQL DDL for unindexed staging table `socios_original`.
pub const SOCIOS_ORIGINAL_DDL: &str = "
CREATE TABLE IF NOT EXISTS socios_original (
    cnpj_basico TEXT,
    identificador_de_socio TEXT,
    nome_socio TEXT,
    cnpj_cpf_socio TEXT,
    qualificacao_socio TEXT,
    data_entrada_sociedade TEXT,
    pais TEXT,
    representante_legal TEXT,
    nome_representante TEXT,
    qualificacao_representante_legal TEXT,
    faixa_etaria TEXT
);";

/// SQL DDL for final `socios` table (12 columns: cnpj TEXT + 11 socio columns).
pub const SOCIOS_FINAL_DDL: &str = "
CREATE TABLE IF NOT EXISTS socios (
    cnpj TEXT,
    cnpj_basico TEXT,
    identificador_de_socio TEXT,
    nome_socio TEXT,
    cnpj_cpf_socio TEXT,
    qualificacao_socio TEXT,
    data_entrada_sociedade TEXT,
    pais TEXT,
    representante_legal TEXT,
    nome_representante TEXT,
    qualificacao_representante_legal TEXT,
    faixa_etaria TEXT
);";

/// SQL DDL for `cnpj_base2matriz` lookup cache table.
pub const CNPJ_BASE2MATRIZ_DDL: &str = "
CREATE TABLE IF NOT EXISTS cnpj_base2matriz (
    cnpj_basico TEXT PRIMARY KEY,
    cnpj TEXT
);";

/// SQL DDL for `_referencia` audit metadata table.
pub const REFERENCIA_DDL: &str = "
CREATE TABLE IF NOT EXISTS _referencia (
    referencia TEXT,
    valor TEXT
);";

/// Create all initial tables without secondary indices for fast bulk loading.
pub fn create_initial_tables(conn: &Connection) -> Result<()> {
    for domain in DOMAIN_TABLES {
        conn.execute_batch(&domain_table_ddl(domain))?;
    }
    conn.execute_batch(EMPRESAS_DDL)?;
    conn.execute_batch(ESTABELECIMENTO_DDL)?;
    conn.execute_batch(SIMPLES_DDL)?;
    conn.execute_batch(SOCIOS_ORIGINAL_DDL)?;
    conn.execute_batch(SOCIOS_FINAL_DDL)?;
    conn.execute_batch(CNPJ_BASE2MATRIZ_DDL)?;
    conn.execute_batch(REFERENCIA_DDL)?;
    Ok(())
}

/// DDL statements for secondary indices created strictly post-load.
pub const POST_LOAD_INDICES_SQL: &str = "
-- Empresas
CREATE INDEX IF NOT EXISTS idx_empresas_cnpj_basico ON empresas (cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_empresas_razao_social ON empresas (razao_social);

-- Estabelecimento
CREATE INDEX IF NOT EXISTS idx_estabelecimento_cnpj_basico ON estabelecimento (cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_estabelecimento_cnpj ON estabelecimento (cnpj);
CREATE INDEX IF NOT EXISTS idx_estabelecimento_nomefantasia ON estabelecimento (nome_fantasia);

-- Simples
CREATE INDEX IF NOT EXISTS idx_simples_cnpj_basico ON simples (cnpj_basico);

-- cnpj_base2matriz
CREATE INDEX IF NOT EXISTS idx_cnpj_base2matriz ON cnpj_base2matriz (cnpj_basico);

-- Socios
CREATE INDEX IF NOT EXISTS idx_socios_cnpj ON socios (cnpj);
CREATE INDEX IF NOT EXISTS idx_socios_cnpj_cpf_socio ON socios (cnpj_cpf_socio);
CREATE INDEX IF NOT EXISTS idx_socios_nome_socio ON socios (nome_socio);
CREATE INDEX IF NOT EXISTS idx_socios_representante ON socios (representante_legal);
CREATE INDEX IF NOT EXISTS idx_socios_representante_nome ON socios (nome_representante);

-- Domain tables
CREATE INDEX IF NOT EXISTS idx_cnae ON cnae (codigo);
CREATE INDEX IF NOT EXISTS idx_motivo ON motivo (codigo);
CREATE INDEX IF NOT EXISTS idx_municipio ON municipio (codigo);
CREATE INDEX IF NOT EXISTS idx_natureza_juridica ON natureza_juridica (codigo);
CREATE INDEX IF NOT EXISTS idx_pais ON pais (codigo);
CREATE INDEX IF NOT EXISTS idx_qualificacao_socio ON qualificacao_socio (codigo);
";

/// Create all secondary indices post-load.
pub fn create_post_load_indices(conn: &Connection) -> Result<()> {
    conn.execute_batch(POST_LOAD_INDICES_SQL)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_initial_tables_and_indices() {
        let conn = Connection::open_in_memory().unwrap();
        create_initial_tables(&conn).unwrap();
        create_post_load_indices(&conn).unwrap();

        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(tables.contains(&"cnae".to_string()));
        assert!(tables.contains(&"motivo".to_string()));
        assert!(tables.contains(&"municipio".to_string()));
        assert!(tables.contains(&"natureza_juridica".to_string()));
        assert!(tables.contains(&"pais".to_string()));
        assert!(tables.contains(&"qualificacao_socio".to_string()));
        assert!(tables.contains(&"empresas".to_string()));
        assert!(tables.contains(&"estabelecimento".to_string()));
        assert!(tables.contains(&"simples".to_string()));
        assert!(tables.contains(&"socios".to_string()));
        assert!(tables.contains(&"cnpj_base2matriz".to_string()));
        assert!(tables.contains(&"_referencia".to_string()));
    }
}
