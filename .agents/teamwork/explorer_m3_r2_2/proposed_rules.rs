//! Societary graph derivation queries and entity materialization.
//!
//! Implements the exact 7 derivation rules connecting corporate partners,
//! individual partners, foreign partners, legal representatives, and branch-headquarter
//! relationships from `cnpj.db` into `rede.db`.

use anyhow::{Context, Result};
use rusqlite::Connection;
use tracing::info;

/// Rule 1: Filial -> Matriz link using `cnpj_base2matriz` fast lookup index.
pub const DERIVE_FILIAL_MATRIZ_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || tf.cnpj AS origem,
    'PJ_' || tm.cnpj AS destino,
    'filial' AS tipo,
    'estabelecimento' AS base
FROM cnpj.estabelecimento tf
INNER JOIN cnpj.cnpj_base2matriz tm ON tm.cnpj_basico = tf.cnpj_basico
WHERE tf.matriz_filial = '2'
  AND tf.cnpj IS NOT NULL AND length(tf.cnpj) = 14
  AND tm.cnpj IS NOT NULL AND length(tm.cnpj) = 14
  AND tf.cnpj <> tm.cnpj;
";

/// Fallback for Rule 1 if `cnpj_base2matriz` table is empty or unavailable.
pub const DERIVE_FILIAL_MATRIZ_FALLBACK_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || tf.cnpj AS origem,
    'PJ_' || tm.cnpj AS destino,
    'filial' AS tipo,
    'estabelecimento' AS base
FROM cnpj.estabelecimento tf
INNER JOIN cnpj.estabelecimento tm ON tm.cnpj_basico = tf.cnpj_basico AND tm.matriz_filial = '1'
WHERE tf.matriz_filial = '2'
  AND tf.cnpj IS NOT NULL AND length(tf.cnpj) = 14
  AND tm.cnpj IS NOT NULL AND length(tm.cnpj) = 14
  AND tf.cnpj <> tm.cnpj;
";

/// Rule 2: Corporate partner (PJ -> PJ, length 14).
pub const DERIVE_SOCIO_PJ_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || t.cnpj_cpf_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) = 14
  AND t.cnpj IS NOT NULL
  AND length(t.cnpj) = 14;
";

/// Rule 3: Individual partner (PF -> PJ, length 11).
pub const DERIVE_SOCIO_PF_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) = 11
  AND t.nome_socio IS NOT NULL
  AND trim(t.nome_socio) <> ''
  AND t.cnpj IS NOT NULL
  AND length(t.cnpj) = 14;
";

/// Rule 4: Foreign partner (PE -> PJ, empty/null CPF).
pub const DERIVE_SOCIO_ESTRANGEIRO_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PE_' || t.nome_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE (t.cnpj_cpf_socio = '' OR t.cnpj_cpf_socio IS NULL)
  AND t.nome_socio IS NOT NULL
  AND trim(t.nome_socio) <> ''
  AND t.cnpj IS NOT NULL
  AND length(t.cnpj) = 14;
";

/// Rule 5a: Legal representative of foreign partner (PF -> PE).
pub const DERIVE_REP_LEGAL_PE_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
    'PE_' || t.nome_socio AS destino,
    'rep-sócio-' || COALESCE(sq.descricao, '') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE (t.cnpj_cpf_socio = '' OR t.cnpj_cpf_socio IS NULL)
  AND t.nome_socio IS NOT NULL
  AND trim(t.nome_socio) <> ''
  AND t.representante_legal IS NOT NULL
  AND trim(t.representante_legal) <> ''
  AND t.representante_legal <> '***000000**'
  AND t.nome_representante IS NOT NULL
  AND trim(t.nome_representante) <> '';
";

/// Rule 5b: Legal representative of corporate partner (PF -> PJ).
pub const DERIVE_REP_LEGAL_PJ_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
    'PJ_' || t.cnpj_cpf_socio AS destino,
    'rep-sócio-' || COALESCE(sq.descricao, '') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE length(t.cnpj_cpf_socio) = 14
  AND t.representante_legal IS NOT NULL
  AND trim(t.representante_legal) <> ''
  AND t.representante_legal <> '***000000**'
  AND t.nome_representante IS NOT NULL
  AND trim(t.nome_representante) <> '';
";

/// Rule 5c: Legal representative of individual partner (PF -> PF).
pub const DERIVE_REP_LEGAL_PF_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
    'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS destino,
    'rep-sócio-' || COALESCE(sq.descricao, '') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE length(t.cnpj_cpf_socio) = 11
  AND t.nome_socio IS NOT NULL
  AND trim(t.nome_socio) <> ''
  AND t.representante_legal IS NOT NULL
  AND trim(t.representante_legal) <> ''
  AND t.representante_legal <> '***000000**'
  AND t.nome_representante IS NOT NULL
  AND trim(t.nome_representante) <> '';
";

/// Deduplicate staging table into final `ligacao` table.
pub const DEDUPLICATE_LIGACAO_SQL: &str = "
CREATE TABLE IF NOT EXISTS ligacao (
    id1 TEXT NOT NULL,
    id2 TEXT NOT NULL,
    descricao TEXT NOT NULL DEFAULT 'SOCIO',
    comentario TEXT NOT NULL DEFAULT ''
);

INSERT INTO ligacao (id1, id2, descricao, comentario)
SELECT
    origem AS id1,
    destino AS id2,
    COALESCE(tipo, 'Sócio') AS descricao,
    COALESCE(base, '') AS comentario
FROM ligacao1
WHERE origem IS NOT NULL
  AND destino IS NOT NULL
  AND trim(origem) <> ''
  AND trim(destino) <> ''
  AND origem NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
  AND destino NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
GROUP BY origem, destino, tipo, base;

DROP TABLE IF EXISTS ligacao1;
";

/// Materialize corporate entities (PJ) into `entidades`.
pub const MATERIALIZE_ENTIDADES_PJ_SQL: &str = "
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT
    'PJ_' || te.cnpj AS id,
    COALESCE(NULLIF(em.razao_social, ''), NULLIF(te.nome_fantasia, ''), 'EMPRESA') AS nome,
    CASE WHEN te.matriz_filial = '2' THEN 'FILIAL' ELSE 'PJ' END AS tipo
FROM cnpj.estabelecimento te
LEFT JOIN cnpj.empresas em ON em.cnpj_basico = te.cnpj_basico
WHERE te.cnpj IS NOT NULL AND length(te.cnpj) = 14;
";

/// Materialize individuals (PF) and foreign partners (PE) into `entidades`.
pub const MATERIALIZE_ENTIDADES_PF_PE_SQL: &str = "
-- PF Socios
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT DISTINCT
    'PF_' || ts.cnpj_cpf_socio || '-' || ts.nome_socio AS id,
    ts.nome_socio AS nome,
    'PF' AS tipo
FROM cnpj.socios ts
WHERE length(ts.cnpj_cpf_socio) = 11
  AND ts.nome_socio IS NOT NULL
  AND trim(ts.nome_socio) <> '';

-- PE Foreign Partners
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT DISTINCT
    'PE_' || ts.nome_socio AS id,
    ts.nome_socio AS nome,
    'PE' AS tipo
FROM cnpj.socios ts
WHERE (ts.cnpj_cpf_socio = '' OR ts.cnpj_cpf_socio IS NULL)
  AND ts.nome_socio IS NOT NULL
  AND trim(ts.nome_socio) <> '';

-- PF Legal Representatives
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT DISTINCT
    'PF_' || ts.representante_legal || '-' || ts.nome_representante AS id,
    ts.nome_representante AS nome,
    'PF' AS tipo
FROM cnpj.socios ts
WHERE ts.representante_legal IS NOT NULL
  AND trim(ts.representante_legal) <> ''
  AND ts.representante_legal <> '***000000**'
  AND ts.nome_representante IS NOT NULL
  AND trim(ts.nome_representante) <> '';

-- Fallback for any non-PJ node in ligacao
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT id1 AS id, replace(replace(id1, 'PF_', ''), 'PE_', '') AS nome, substr(id1, 1, 2) AS tipo
FROM ligacao
WHERE substr(id1, 1, 3) <> 'PJ_'
  AND id1 IS NOT NULL AND trim(id1) <> '' AND id1 NOT IN ('PF_', 'PE_', 'PF_-')
UNION
SELECT id2 AS id, replace(replace(id2, 'PF_', ''), 'PE_', '') AS nome, substr(id2, 1, 2) AS tipo
FROM ligacao
WHERE substr(id2, 1, 3) <> 'PJ_'
  AND id2 IS NOT NULL AND trim(id2) <> '' AND id2 NOT IN ('PF_', 'PE_', 'PF_-');
";

/// Executes Rule 1 (Filial -> Matriz) with automatic index lookup or fallback.
pub fn derive_filial_matriz(conn: &Connection) -> Result<u64> {
    // Check if cnpj_base2matriz has rows (qualified with cnpj.sqlite_master)
    let has_b2m: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type='table' AND name='cnpj_base2matriz') AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false);

    let sql = if has_b2m {
        DERIVE_FILIAL_MATRIZ_SQL
    } else {
        DERIVE_FILIAL_MATRIZ_FALLBACK_SQL
    };

    let before = count_staging(conn);
    conn.execute_batch(sql)
        .context("Failed executing Rule 1: Filial -> Matriz links")?;
    let after = count_staging(conn);
    let count = after.saturating_sub(before);
    info!(count = count, "Rule 1 (Filial -> Matriz) completed");
    Ok(count)
}

/// Executes Rule 2: Corporate partner (PJ -> PJ).
pub fn derive_socio_pj(conn: &Connection) -> Result<u64> {
    let before = count_staging(conn);
    conn.execute_batch(DERIVE_SOCIO_PJ_SQL)
        .context("Failed executing Rule 2: Corporate partner links")?;
    let after = count_staging(conn);
    let count = after.saturating_sub(before);
    info!(count = count, "Rule 2 (Corporate Partner PJ -> PJ) completed");
    Ok(count)
}

/// Executes Rule 3: Individual partner (PF -> PJ).
pub fn derive_socio_pf(conn: &Connection) -> Result<u64> {
    let before = count_staging(conn);
    conn.execute_batch(DERIVE_SOCIO_PF_SQL)
        .context("Failed executing Rule 3: Individual partner links")?;
    let after = count_staging(conn);
    let count = after.saturating_sub(before);
    info!(count = count, "Rule 3 (Individual Partner PF -> PJ) completed");
    Ok(count)
}

/// Executes Rule 4: Foreign partner (PE -> PJ).
pub fn derive_socio_estrangeiro(conn: &Connection) -> Result<u64> {
    let before = count_staging(conn);
    conn.execute_batch(DERIVE_SOCIO_ESTRANGEIRO_SQL)
        .context("Failed executing Rule 4: Foreign partner links")?;
    let after = count_staging(conn);
    let count = after.saturating_sub(before);
    info!(count = count, "Rule 4 (Foreign Partner PE -> PJ) completed");
    Ok(count)
}

/// Executes Rule 5: Legal representative links (5a, 5b, 5c).
pub fn derive_rep_legal(conn: &Connection) -> Result<u64> {
    let before = count_staging(conn);
    conn.execute_batch(DERIVE_REP_LEGAL_PE_SQL)
        .context("Failed executing Rule 5a: Legal representative of foreign partner")?;
    conn.execute_batch(DERIVE_REP_LEGAL_PJ_SQL)
        .context("Failed executing Rule 5b: Legal representative of corporate partner")?;
    conn.execute_batch(DERIVE_REP_LEGAL_PF_SQL)
        .context("Failed executing Rule 5c: Legal representative of individual partner")?;
    let after = count_staging(conn);
    let count = after.saturating_sub(before);
    info!(count = count, "Rule 5 (Legal Representatives 5a-5c) completed");
    Ok(count)
}

/// Deduplicates staging table `ligacao1` into final `ligacao` table.
pub fn deduplicate_ligacao(conn: &Connection) -> Result<u64> {
    info!("Deduplicating staging links into ligacao table...");
    conn.execute_batch(DEDUPLICATE_LIGACAO_SQL)
        .context("Failed deduplicating ligacao staging table")?;
    let total: i64 = conn
        .query_row("SELECT count(*) FROM ligacao;", [], |r| r.get(0))
        .unwrap_or(0);
    info!(total = total, "Deduplication into ligacao complete");
    Ok(total as u64)
}

/// Materializes all distinct PJ, PF, and PE entities into `entidades`.
pub fn materialize_entidades(conn: &Connection) -> Result<u64> {
    info!("Materializing corporate entities (PJ) into entidades...");
    conn.execute_batch(MATERIALIZE_ENTIDADES_PJ_SQL)
        .context("Failed materializing PJ entities")?;

    info!("Materializing individual (PF) and foreign (PE) entities into entidades...");
    conn.execute_batch(MATERIALIZE_ENTIDADES_PF_PE_SQL)
        .context("Failed materializing PF and PE entities")?;

    let total: i64 = conn
        .query_row("SELECT count(*) FROM entidades;", [], |r| r.get(0))
        .unwrap_or(0);
    info!(total = total, "Materialization of entidades complete");
    Ok(total as u64)
}

fn count_staging(conn: &Connection) -> u64 {
    conn.query_row("SELECT count(*) FROM ligacao1;", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::schema::create_graph_schema;

    #[test]
    fn test_derivation_rules_in_memory() {
        let conn = Connection::open_in_memory().unwrap();
        create_graph_schema(&conn).unwrap();

        // Attach memory as 'cnpj'
        conn.execute_batch(
            "ATTACH DATABASE ':memory:' AS cnpj;

             CREATE TABLE cnpj.qualificacao_socio (codigo TEXT PRIMARY KEY, descricao TEXT);
             INSERT INTO cnpj.qualificacao_socio VALUES ('10', 'Diretor');
             INSERT INTO cnpj.qualificacao_socio VALUES ('22', 'Sócio');
             INSERT INTO cnpj.qualificacao_socio VALUES ('05', 'Administrador');

             CREATE TABLE cnpj.socios (
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
             );

             CREATE TABLE cnpj.estabelecimento (
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
             );

             CREATE TABLE cnpj.empresas (
                 cnpj_basico TEXT,
                 razao_social TEXT,
                 natureza_juridica TEXT,
                 qualificacao_responsavel TEXT,
                 capital_social_str TEXT,
                 porte_empresa TEXT,
                 ente_federativo_responsavel TEXT,
                 capital_social REAL
             );

             CREATE TABLE cnpj.cnpj_base2matriz (
                 cnpj_basico TEXT PRIMARY KEY,
                 cnpj TEXT
             );

             -- Seed test data
             INSERT INTO cnpj.estabelecimento (cnpj_basico, matriz_filial, cnpj) VALUES ('33333333', '1', '33333333000173');
             INSERT INTO cnpj.estabelecimento (cnpj_basico, matriz_filial, cnpj) VALUES ('33333333', '2', '33333333000254');
             INSERT INTO cnpj.cnpj_base2matriz VALUES ('33333333', '33333333000173');

             -- Corporate partner: Company A (11111111000191) is socio in Company B (22222222000182)
             INSERT INTO cnpj.socios (cnpj, cnpj_cpf_socio, qualificacao_socio, nome_socio)
             VALUES ('22222222000182', '11111111000191', '22', 'ALFA TECNOLOGIA');

             -- Individual partner: JOAO is socio in Company A
             INSERT INTO cnpj.socios (cnpj, cnpj_cpf_socio, qualificacao_socio, nome_socio)
             VALUES ('11111111000191', '***123456**', '22', 'JOAO DA SILVA');

             -- Foreign partner: GLOBAL VENTURES
             INSERT INTO cnpj.socios (cnpj, cnpj_cpf_socio, qualificacao_socio, nome_socio, representante_legal, nome_representante, qualificacao_representante_legal)
             VALUES ('44444444000164', '', '22', 'GLOBAL VENTURES LLC', '***987654**', 'MARIA REP PE', '05');
            ",
        )
        .unwrap();

        derive_filial_matriz(&conn).unwrap();
        derive_socio_pj(&conn).unwrap();
        derive_socio_pf(&conn).unwrap();
        derive_socio_estrangeiro(&conn).unwrap();
        derive_rep_legal(&conn).unwrap();

        deduplicate_ligacao(&conn).unwrap();
        materialize_entidades(&conn).unwrap();

        let edge_count: i64 = conn.query_row("SELECT count(*) FROM ligacao;", [], |r| r.get(0)).unwrap();
        assert_eq!(edge_count, 5);

        // Verify specific edges
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM ligacao WHERE id1 = 'PJ_33333333000254' AND id2 = 'PJ_33333333000173' AND descricao = 'filial');",
            [],
            |r| r.get(0),
        ).unwrap();
        assert!(exists);

        let exists_rep: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM ligacao WHERE id1 = 'PF_***987654**-MARIA REP PE' AND id2 = 'PE_GLOBAL VENTURES LLC');",
            [],
            |r| r.get(0),
        ).unwrap();
        assert!(exists_rep);
    }

    #[test]
    fn test_rule1_fallback_without_cnpj_base2matriz() {
        let conn = Connection::open_in_memory().unwrap();
        create_graph_schema(&conn).unwrap();

        conn.execute_batch(
            "ATTACH DATABASE ':memory:' AS cnpj;

             CREATE TABLE cnpj.estabelecimento (
                 cnpj_basico TEXT,
                 matriz_filial TEXT,
                 cnpj TEXT
             );

             INSERT INTO cnpj.estabelecimento VALUES ('55555555', '1', '55555555000100');
             INSERT INTO cnpj.estabelecimento VALUES ('55555555', '2', '55555555000288');
            ",
        )
        .unwrap();

        let count = derive_filial_matriz(&conn).unwrap();
        assert_eq!(count, 1);

        deduplicate_ligacao(&conn).unwrap();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM ligacao WHERE id1 = 'PJ_55555555000288' AND id2 = 'PJ_55555555000100');",
            [],
            |r| r.get(0),
        ).unwrap();
        assert!(exists);
    }

    #[test]
    fn test_rule5_dummy_cpf_filtering_and_all_targets() {
        let conn = Connection::open_in_memory().unwrap();
        create_graph_schema(&conn).unwrap();

        conn.execute_batch(
            "ATTACH DATABASE ':memory:' AS cnpj;

             CREATE TABLE cnpj.qualificacao_socio (codigo TEXT PRIMARY KEY, descricao TEXT);
             INSERT INTO cnpj.qualificacao_socio VALUES ('05', 'Administrador');

             CREATE TABLE cnpj.socios (
                 cnpj TEXT,
                 nome_socio TEXT,
                 cnpj_cpf_socio TEXT,
                 qualificacao_socio TEXT,
                 representante_legal TEXT,
                 nome_representante TEXT,
                 qualificacao_representante_legal TEXT
             );

             -- 1. Dummy representative CPF: should be excluded
             INSERT INTO cnpj.socios VALUES ('11111111000191', 'JOAO', '***123456**', '05', '***000000**', 'DUMMY REP', '05');

             -- 2. Empty representative CPF: should be excluded
             INSERT INTO cnpj.socios VALUES ('11111111000191', 'PEDRO', '***234567**', '05', '', 'EMPTY REP', '05');

             -- 3. Rep of corporate partner (5b)
             INSERT INTO cnpj.socios VALUES ('11111111000191', 'CORP', '22222222000182', '05', '***555444**', 'ROBERTO REP PJ', '05');

             -- 4. Rep of individual partner (5c)
             INSERT INTO cnpj.socios VALUES ('11111111000191', 'PEDRO MENOR', '***333222**', '05', '***111222**', 'PAULO PAI', '05');
            ",
        )
        .unwrap();

        let count = derive_rep_legal(&conn).unwrap();
        assert_eq!(count, 2);

        deduplicate_ligacao(&conn).unwrap();

        let count_rep_pj: i64 = conn.query_row(
            "SELECT count(*) FROM ligacao WHERE id1 = 'PF_***555444**-ROBERTO REP PJ' AND id2 = 'PJ_22222222000182';",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count_rep_pj, 1);

        let count_rep_pf: i64 = conn.query_row(
            "SELECT count(*) FROM ligacao WHERE id1 = 'PF_***111222**-PAULO PAI' AND id2 = 'PF_***333222**-PEDRO MENOR';",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count_rep_pf, 1);
    }

    #[test]
    fn test_deduplication_removes_duplicate_edges() {
        let conn = Connection::open_in_memory().unwrap();
        create_graph_schema(&conn).unwrap();

        conn.execute_batch(
            "INSERT INTO ligacao1 VALUES ('PJ_1', 'PJ_2', 'filial', 'estabelecimento');
             INSERT INTO ligacao1 VALUES ('PJ_1', 'PJ_2', 'filial', 'estabelecimento');
             INSERT INTO ligacao1 VALUES ('PJ_1', 'PJ_2', 'filial', 'estabelecimento');",
        )
        .unwrap();

        let total = deduplicate_ligacao(&conn).unwrap();
        assert_eq!(total, 1);

        let count: i64 = conn.query_row("SELECT count(*) FROM ligacao;", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_derivation_orphan_and_corrupt_partner_resilience() {
        let conn = Connection::open_in_memory().unwrap();
        create_graph_schema(&conn).unwrap();

        conn.execute_batch(
            "ATTACH DATABASE ':memory:' AS cnpj;

             CREATE TABLE cnpj.qualificacao_socio (codigo TEXT PRIMARY KEY, descricao TEXT);
             INSERT INTO cnpj.qualificacao_socio VALUES ('22', 'Sócio');
             INSERT INTO cnpj.qualificacao_socio VALUES ('05', 'Administrador');

             CREATE TABLE cnpj.socios (
                 cnpj TEXT,
                 nome_socio TEXT,
                 cnpj_cpf_socio TEXT,
                 qualificacao_socio TEXT,
                 representante_legal TEXT,
                 nome_representante TEXT,
                 qualificacao_representante_legal TEXT
             );

             CREATE TABLE cnpj.estabelecimento (
                 cnpj_basico TEXT,
                 matriz_filial TEXT,
                 cnpj TEXT
             );

             CREATE TABLE cnpj.cnpj_base2matriz (
                 cnpj_basico TEXT PRIMARY KEY,
                 cnpj TEXT
             );

             -- 1. Valid company & branch
             INSERT INTO cnpj.estabelecimento VALUES ('11111111', '1', '11111111000191');
             INSERT INTO cnpj.estabelecimento VALUES ('11111111', '2', '11111111000272');
             INSERT INTO cnpj.cnpj_base2matriz VALUES ('11111111', '11111111000191');

             -- 2. Corrupt branch (NULL cnpj)
             INSERT INTO cnpj.estabelecimento VALUES ('22222222', '2', NULL);
             INSERT INTO cnpj.cnpj_base2matriz VALUES ('22222222', '22222222000180');

             -- 3. Orphan partner records (cnpj is NULL or empty)
             INSERT INTO cnpj.socios VALUES (NULL, 'ORPHAN CORP', '33333333000170', '22', '', '', '');
             INSERT INTO cnpj.socios VALUES ('', 'ORPHAN CORP 2', '44444444000160', '22', '', '', '');
             INSERT INTO cnpj.socios VALUES (NULL, 'ORPHAN PF', '***222333**', '22', '', '', '');
             INSERT INTO cnpj.socios VALUES ('', 'ORPHAN PE', '', '22', '', '', '');

             -- 4. Partner records with NULL or empty nome_socio
             INSERT INTO cnpj.socios VALUES ('11111111000191', NULL, '***444555**', '22', '', '', '');
             INSERT INTO cnpj.socios VALUES ('11111111000191', '   ', '***555666**', '22', '', '', '');
             INSERT INTO cnpj.socios VALUES ('11111111000191', NULL, '', '22', '', '', '');

             -- 5. Legal rep with NULL nome_socio (Rules 5a, 5c)
             INSERT INTO cnpj.socios VALUES ('11111111000191', NULL, '', '22', '***999888**', 'REP PE NULL SOCIO', '05');
             INSERT INTO cnpj.socios VALUES ('11111111000191', NULL, '***333444**', '22', '***555666**', 'REP PF NULL SOCIO', '05');

             -- 6. Valid partner record
             INSERT INTO cnpj.socios VALUES ('11111111000191', 'VALDIR SILVA', '***777888**', '22', '', '', '');
            ",
        )
        .unwrap();

        derive_filial_matriz(&conn).unwrap();
        derive_socio_pj(&conn).unwrap();
        derive_socio_pf(&conn).unwrap();
        derive_socio_estrangeiro(&conn).unwrap();
        derive_rep_legal(&conn).unwrap();

        // Must NOT fail with SQLite IntegrityError NOT NULL constraint failed: ligacao.id2
        let total = deduplicate_ligacao(&conn).expect("Deduplication must succeed without constraint violations");
        assert_eq!(total, 2); // 1 filial + 1 valid PF partner

        // Verify no NULL or corrupt node IDs exist in ligacao
        let corrupt_count: i64 = conn.query_row(
            "SELECT count(*) FROM ligacao WHERE id1 IS NULL OR id2 IS NULL OR id1 = 'PJ_' OR id2 = 'PJ_' OR id1 = 'PE_' OR id2 = 'PE_';",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(corrupt_count, 0, "No corrupt or prefix-only IDs allowed in ligacao");
    }
}
