//! High-performance SQLite batch loader with prepared statement caching,
//! transactional chunking, cache materialization, and August 2026 radical expansion.

use crossbeam_channel::Receiver;
use rusqlite::{params, Connection, Result};
use std::path::Path;

use crate::ingestion::schema::{CNPJ_BASE2MATRIZ_DDL, SOCIOS_FINAL_DDL};
use crate::ingestion::transforms::{decode_trimmed_str, parse_brl_float, synthesize_cnpj};
use crate::ingestion::zip_stream::process_zip_byte_records;

/// In-memory representation of an `empresas` row for batch insertion.
#[derive(Debug, Clone)]
pub struct EmpresaBatchRow {
    pub cnpj_basico: String,
    pub razao_social: String,
    pub natureza_juridica: String,
    pub qualificacao_responsavel: String,
    pub capital_social_str: String,
    pub porte_empresa: String,
    pub ente_federativo_responsavel: String,
    pub capital_social: f64,
}

impl EmpresaBatchRow {
    pub fn from_byte_record(rec: &csv::ByteRecord) -> Self {
        let cap_bytes = rec.get(4).unwrap_or(b"");
        Self {
            cnpj_basico: decode_trimmed_str(rec.get(0).unwrap_or(b"")),
            razao_social: decode_trimmed_str(rec.get(1).unwrap_or(b"")),
            natureza_juridica: decode_trimmed_str(rec.get(2).unwrap_or(b"")),
            qualificacao_responsavel: decode_trimmed_str(rec.get(3).unwrap_or(b"")),
            capital_social_str: decode_trimmed_str(cap_bytes),
            porte_empresa: decode_trimmed_str(rec.get(5).unwrap_or(b"")),
            ente_federativo_responsavel: decode_trimmed_str(rec.get(6).unwrap_or(b"")),
            capital_social: parse_brl_float(cap_bytes),
        }
    }
}

/// In-memory representation of an `estabelecimento` row for batch insertion.
#[derive(Debug, Clone)]
pub struct EstabelecimentoBatchRow {
    pub cnpj_basico: String,
    pub cnpj_ordem: String,
    pub cnpj_dv: String,
    pub matriz_filial: String,
    pub nome_fantasia: String,
    pub situacao_cadastral: String,
    pub data_situacao_cadastral: String,
    pub motivo_situacao_cadastral: String,
    pub nome_cidade_exterior: String,
    pub pais: String,
    pub data_inicio_atividades: String,
    pub cnae_fiscal: String,
    pub cnae_fiscal_secundaria: String,
    pub tipo_logradouro: String,
    pub logradouro: String,
    pub numero: String,
    pub complemento: String,
    pub bairro: String,
    pub cep: String,
    pub uf: String,
    pub municipio: String,
    pub ddd1: String,
    pub telefone1: String,
    pub ddd2: String,
    pub telefone2: String,
    pub ddd_fax: String,
    pub fax: String,
    pub correio_eletronico: String,
    pub situacao_especial: String,
    pub data_situacao_especial: String,
    pub cnpj: String,
}

impl EstabelecimentoBatchRow {
    pub fn from_byte_record(rec: &csv::ByteRecord) -> Self {
        let basico_bytes = rec.get(0).unwrap_or(b"");
        let ordem_bytes = rec.get(1).unwrap_or(b"");
        let dv_bytes = rec.get(2).unwrap_or(b"");
        let cnpj = synthesize_cnpj(basico_bytes, ordem_bytes, dv_bytes);

        Self {
            cnpj_basico: decode_trimmed_str(basico_bytes),
            cnpj_ordem: decode_trimmed_str(ordem_bytes),
            cnpj_dv: decode_trimmed_str(dv_bytes),
            matriz_filial: decode_trimmed_str(rec.get(3).unwrap_or(b"")),
            nome_fantasia: decode_trimmed_str(rec.get(4).unwrap_or(b"")),
            situacao_cadastral: decode_trimmed_str(rec.get(5).unwrap_or(b"")),
            data_situacao_cadastral: decode_trimmed_str(rec.get(6).unwrap_or(b"")),
            motivo_situacao_cadastral: decode_trimmed_str(rec.get(7).unwrap_or(b"")),
            nome_cidade_exterior: decode_trimmed_str(rec.get(8).unwrap_or(b"")),
            pais: decode_trimmed_str(rec.get(9).unwrap_or(b"")),
            data_inicio_atividades: decode_trimmed_str(rec.get(10).unwrap_or(b"")),
            cnae_fiscal: decode_trimmed_str(rec.get(11).unwrap_or(b"")),
            cnae_fiscal_secundaria: decode_trimmed_str(rec.get(12).unwrap_or(b"")),
            tipo_logradouro: decode_trimmed_str(rec.get(13).unwrap_or(b"")),
            logradouro: decode_trimmed_str(rec.get(14).unwrap_or(b"")),
            numero: decode_trimmed_str(rec.get(15).unwrap_or(b"")),
            complemento: decode_trimmed_str(rec.get(16).unwrap_or(b"")),
            bairro: decode_trimmed_str(rec.get(17).unwrap_or(b"")),
            cep: decode_trimmed_str(rec.get(18).unwrap_or(b"")),
            uf: decode_trimmed_str(rec.get(19).unwrap_or(b"")),
            municipio: decode_trimmed_str(rec.get(20).unwrap_or(b"")),
            ddd1: decode_trimmed_str(rec.get(21).unwrap_or(b"")),
            telefone1: decode_trimmed_str(rec.get(22).unwrap_or(b"")),
            ddd2: decode_trimmed_str(rec.get(23).unwrap_or(b"")),
            telefone2: decode_trimmed_str(rec.get(24).unwrap_or(b"")),
            ddd_fax: decode_trimmed_str(rec.get(25).unwrap_or(b"")),
            fax: decode_trimmed_str(rec.get(26).unwrap_or(b"")),
            correio_eletronico: decode_trimmed_str(rec.get(27).unwrap_or(b"")),
            situacao_especial: decode_trimmed_str(rec.get(28).unwrap_or(b"")),
            data_situacao_especial: decode_trimmed_str(rec.get(29).unwrap_or(b"")),
            cnpj,
        }
    }
}

/// In-memory representation of a `simples` row for batch insertion.
#[derive(Debug, Clone)]
pub struct SimplesBatchRow {
    pub cnpj_basico: String,
    pub opcao_simples: String,
    pub data_opcao_simples: String,
    pub data_exclusao_simples: String,
    pub opcao_mei: String,
    pub data_opcao_mei: String,
    pub data_exclusao_mei: String,
}

impl SimplesBatchRow {
    pub fn from_byte_record(rec: &csv::ByteRecord) -> Self {
        Self {
            cnpj_basico: decode_trimmed_str(rec.get(0).unwrap_or(b"")),
            opcao_simples: decode_trimmed_str(rec.get(1).unwrap_or(b"")),
            data_opcao_simples: decode_trimmed_str(rec.get(2).unwrap_or(b"")),
            data_exclusao_simples: decode_trimmed_str(rec.get(3).unwrap_or(b"")),
            opcao_mei: decode_trimmed_str(rec.get(4).unwrap_or(b"")),
            data_opcao_mei: decode_trimmed_str(rec.get(5).unwrap_or(b"")),
            data_exclusao_mei: decode_trimmed_str(rec.get(6).unwrap_or(b"")),
        }
    }
}

/// In-memory representation of a raw socio row for staging table insertion.
#[derive(Debug, Clone)]
pub struct SocioStagingBatchRow {
    pub cnpj_basico: String,
    pub identificador_de_socio: String,
    pub nome_socio: String,
    pub cnpj_cpf_socio: String,
    pub qualificacao_socio: String,
    pub data_entrada_sociedade: String,
    pub pais: String,
    pub representante_legal: String,
    pub nome_representante: String,
    pub qualificacao_representante_legal: String,
    pub faixa_etaria: String,
}

impl SocioStagingBatchRow {
    pub fn from_byte_record(rec: &csv::ByteRecord) -> Self {
        Self {
            cnpj_basico: decode_trimmed_str(rec.get(0).unwrap_or(b"")),
            identificador_de_socio: decode_trimmed_str(rec.get(1).unwrap_or(b"")),
            nome_socio: decode_trimmed_str(rec.get(2).unwrap_or(b"")),
            cnpj_cpf_socio: decode_trimmed_str(rec.get(3).unwrap_or(b"")),
            qualificacao_socio: decode_trimmed_str(rec.get(4).unwrap_or(b"")),
            data_entrada_sociedade: decode_trimmed_str(rec.get(5).unwrap_or(b"")),
            pais: decode_trimmed_str(rec.get(6).unwrap_or(b"")),
            representante_legal: decode_trimmed_str(rec.get(7).unwrap_or(b"")),
            nome_representante: decode_trimmed_str(rec.get(8).unwrap_or(b"")),
            qualificacao_representante_legal: decode_trimmed_str(rec.get(9).unwrap_or(b"")),
            faixa_etaria: decode_trimmed_str(rec.get(10).unwrap_or(b"")),
        }
    }
}

/// Batch insert domain lookup records (e.g. cnae, motivo, municipio).
pub fn insert_domain_batch(
    conn: &mut Connection,
    table_name: &str,
    records: &[(String, String)],
) -> Result<usize> {
    if records.is_empty() {
        return Ok(0);
    }
    let sql = format!("INSERT OR REPLACE INTO {table_name} (codigo, descricao) VALUES (?1, ?2)");
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare_cached(&sql)?;
        for (code, desc) in records {
            stmt.execute(params![code, desc])?;
        }
    }
    tx.commit()?;
    Ok(records.len())
}

/// Loads a single domain zip file directly into the target table.
pub fn load_domain_file(
    conn: &mut Connection,
    zip_path: &Path,
    table_name: &str,
    expected_ext: &str,
) -> anyhow::Result<u64> {
    let mut records = Vec::new();
    process_zip_byte_records(zip_path, Some(expected_ext), |rec| {
        let code = decode_trimmed_str(rec.get(0).unwrap_or(b""));
        let desc = decode_trimmed_str(rec.get(1).unwrap_or(b""));
        if !code.is_empty() {
            records.push((code, desc));
        }
        Ok(())
    })?;

    if records.is_empty() {
        return Ok(0);
    }

    let count = insert_domain_batch(conn, table_name, &records)?;
    Ok(count as u64)
}

/// Drains batches of `EmpresaBatchRow` from a channel and inserts them inside transactions.
pub fn consume_empresas_channel<F>(
    conn: &mut Connection,
    rx: Receiver<anyhow::Result<Vec<EmpresaBatchRow>>>,
    mut on_progress: F,
) -> anyhow::Result<u64>
where
    F: FnMut(usize),
{
    let sql = "
        INSERT INTO empresas (
            cnpj_basico, razao_social, natureza_juridica, qualificacao_responsavel,
            capital_social_str, porte_empresa, ente_federativo_responsavel, capital_social
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);";

    let mut total = 0u64;

    while let Ok(batch_res) = rx.recv() {
        let batch = batch_res?;
        if batch.is_empty() {
            continue;
        }

        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(sql)?;
            for row in &batch {
                stmt.execute(params![
                    row.cnpj_basico,
                    row.razao_social,
                    row.natureza_juridica,
                    row.qualificacao_responsavel,
                    row.capital_social_str,
                    row.porte_empresa,
                    row.ente_federativo_responsavel,
                    row.capital_social,
                ])?;
            }
        }
        tx.commit()?;

        let count = batch.len();
        total += count as u64;
        on_progress(count);
    }

    Ok(total)
}

/// Drains batches of `EstabelecimentoBatchRow` from a channel and inserts them inside transactions.
pub fn consume_estabelecimentos_channel<F>(
    conn: &mut Connection,
    rx: Receiver<anyhow::Result<Vec<EstabelecimentoBatchRow>>>,
    mut on_progress: F,
) -> anyhow::Result<u64>
where
    F: FnMut(usize),
{
    let sql = "
        INSERT INTO estabelecimento (
            cnpj_basico, cnpj_ordem, cnpj_dv, matriz_filial, nome_fantasia,
            situacao_cadastral, data_situacao_cadastral, motivo_situacao_cadastral,
            nome_cidade_exterior, pais, data_inicio_atividades, cnae_fiscal,
            cnae_fiscal_secundaria, tipo_logradouro, logradouro, numero,
            complemento, bairro, cep, uf, municipio, ddd1, telefone1,
            ddd2, telefone2, ddd_fax, fax, correio_eletronico,
            situacao_especial, data_situacao_especial, cnpj
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31
        );";

    let mut total = 0u64;

    while let Ok(batch_res) = rx.recv() {
        let batch = batch_res?;
        if batch.is_empty() {
            continue;
        }

        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(sql)?;
            for r in &batch {
                stmt.execute(params![
                    r.cnpj_basico,
                    r.cnpj_ordem,
                    r.cnpj_dv,
                    r.matriz_filial,
                    r.nome_fantasia,
                    r.situacao_cadastral,
                    r.data_situacao_cadastral,
                    r.motivo_situacao_cadastral,
                    r.nome_cidade_exterior,
                    r.pais,
                    r.data_inicio_atividades,
                    r.cnae_fiscal,
                    r.cnae_fiscal_secundaria,
                    r.tipo_logradouro,
                    r.logradouro,
                    r.numero,
                    r.complemento,
                    r.bairro,
                    r.cep,
                    r.uf,
                    r.municipio,
                    r.ddd1,
                    r.telefone1,
                    r.ddd2,
                    r.telefone2,
                    r.ddd_fax,
                    r.fax,
                    r.correio_eletronico,
                    r.situacao_especial,
                    r.data_situacao_especial,
                    r.cnpj,
                ])?;
            }
        }
        tx.commit()?;

        let count = batch.len();
        total += count as u64;
        on_progress(count);
    }

    Ok(total)
}

/// Drains batches of `SimplesBatchRow` from a channel and inserts them inside transactions.
pub fn consume_simples_channel<F>(
    conn: &mut Connection,
    rx: Receiver<anyhow::Result<Vec<SimplesBatchRow>>>,
    mut on_progress: F,
) -> anyhow::Result<u64>
where
    F: FnMut(usize),
{
    let sql = "
        INSERT INTO simples (
            cnpj_basico, opcao_simples, data_opcao_simples,
            data_exclusao_simples, opcao_mei, data_opcao_mei, data_exclusao_mei
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);";

    let mut total = 0u64;

    while let Ok(batch_res) = rx.recv() {
        let batch = batch_res?;
        if batch.is_empty() {
            continue;
        }

        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(sql)?;
            for r in &batch {
                stmt.execute(params![
                    r.cnpj_basico,
                    r.opcao_simples,
                    r.data_opcao_simples,
                    r.data_exclusao_simples,
                    r.opcao_mei,
                    r.data_opcao_mei,
                    r.data_exclusao_mei,
                ])?;
            }
        }
        tx.commit()?;

        let count = batch.len();
        total += count as u64;
        on_progress(count);
    }

    Ok(total)
}

/// Drains batches of `SocioStagingBatchRow` from a channel into staging table `socios_original`.
pub fn consume_socios_channel<F>(
    conn: &mut Connection,
    rx: Receiver<anyhow::Result<Vec<SocioStagingBatchRow>>>,
    mut on_progress: F,
) -> anyhow::Result<u64>
where
    F: FnMut(usize),
{
    let sql = "
        INSERT INTO socios_original (
            cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio,
            qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
            nome_representante, qualificacao_representante_legal, faixa_etaria
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);";

    let mut total = 0u64;

    while let Ok(batch_res) = rx.recv() {
        let batch = batch_res?;
        if batch.is_empty() {
            continue;
        }

        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(sql)?;
            for r in &batch {
                stmt.execute(params![
                    r.cnpj_basico,
                    r.identificador_de_socio,
                    r.nome_socio,
                    r.cnpj_cpf_socio,
                    r.qualificacao_socio,
                    r.data_entrada_sociedade,
                    r.pais,
                    r.representante_legal,
                    r.nome_representante,
                    r.qualificacao_representante_legal,
                    r.faixa_etaria,
                ])?;
            }
        }
        tx.commit()?;

        let count = batch.len();
        total += count as u64;
        on_progress(count);
    }

    Ok(total)
}

/// Materialize lookup cache table `cnpj_base2matriz` from `estabelecimento`.
pub fn materialize_cnpj_base2matriz(conn: &mut Connection) -> Result<u64> {
    conn.execute_batch(CNPJ_BASE2MATRIZ_DDL)?;

    let inserted = conn.execute(
        "INSERT OR IGNORE INTO cnpj_base2matriz (cnpj_basico, cnpj)
         SELECT DISTINCT cnpj_basico, cnpj
         FROM estabelecimento
         WHERE matriz_filial = '1';",
        [],
    )?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_cnpj_base2matriz ON cnpj_base2matriz (cnpj_basico);",
    )?;

    Ok(inserted as u64)
}

/// Materializes the final `socios` table from `socios_original` and `cnpj_base2matriz`,
/// performing the August 2026 8-digit radical expansion for corporate partners (identificador_de_socio = '1').
pub fn transform_socios_and_expand_radicals(conn: &mut Connection) -> Result<u64> {
    // Ensure destination table exists
    conn.execute_batch(SOCIOS_FINAL_DDL)?;

    // Index staging table to accelerate the transformation join
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_socios_original_cnpj_basico ON socios_original(cnpj_basico);
         CREATE INDEX IF NOT EXISTS idx_socios_original_cnpj_cpf ON socios_original(cnpj_cpf_socio);",
    )?;

    // Part 1: Non-PJ partners (PF, Foreign/PE): direct left join to company matriz
    conn.execute(
        "INSERT INTO socios (
            cnpj, cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio,
            qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
            nome_representante, qualificacao_representante_legal, faixa_etaria
        )
        SELECT
            te.cnpj AS cnpj,
            ts.cnpj_basico,
            ts.identificador_de_socio,
            ts.nome_socio,
            ts.cnpj_cpf_socio,
            ts.qualificacao_socio,
            ts.data_entrada_sociedade,
            ts.pais,
            ts.representante_legal,
            ts.nome_representante,
            ts.qualificacao_representante_legal,
            ts.faixa_etaria
        FROM socios_original ts
        LEFT JOIN cnpj_base2matriz te ON te.cnpj_basico = ts.cnpj_basico
        WHERE ts.identificador_de_socio <> '1';",
        [],
    )?;

    // Part 2: PJ partners (identificador_de_socio = '1'):
    // Join te for company matriz AND tes for partner's 8-digit radical expansion
    conn.execute(
        "INSERT INTO socios (
            cnpj, cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio,
            qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
            nome_representante, qualificacao_representante_legal, faixa_etaria
        )
        SELECT
            te.cnpj AS cnpj,
            ts.cnpj_basico,
            ts.identificador_de_socio,
            ts.nome_socio,
            COALESCE(tes.cnpj, ts.cnpj_cpf_socio) AS cnpj_cpf_socio,
            ts.qualificacao_socio,
            ts.data_entrada_sociedade,
            ts.pais,
            ts.representante_legal,
            ts.nome_representante,
            ts.qualificacao_representante_legal,
            ts.faixa_etaria
        FROM socios_original ts
        LEFT JOIN cnpj_base2matriz te ON te.cnpj_basico = ts.cnpj_basico
        LEFT JOIN cnpj_base2matriz tes ON tes.cnpj_basico = ts.cnpj_cpf_socio
        WHERE ts.identificador_de_socio = '1';",
        [],
    )?;

    // Drop temporary staging table to reclaim pages
    conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?;

    let total_socios: i64 = conn.query_row("SELECT COUNT(*) FROM socios;", [], |r| r.get(0))?;

    Ok(total_socios as u64)
}

/// Record reference date and total establishment count into `_referencia`.
pub fn insert_audit_metadata(
    conn: &mut Connection,
    ref_date: &str,
    establishment_count: u64,
) -> Result<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt =
            tx.prepare_cached("INSERT INTO _referencia (referencia, valor) VALUES (?1, ?2);")?;
        stmt.execute(params!["CNPJ", ref_date])?;
        stmt.execute(params!["cnpj_qtde", establishment_count.to_string()])?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::IngestionPragmas;
    use crate::ingestion::schema::{create_initial_tables, create_post_load_indices};

    #[test]
    fn test_aug_2026_radical_expansion() {
        let mut conn = Connection::open_in_memory().unwrap();
        IngestionPragmas::default().apply_to(&conn).unwrap();
        create_initial_tables(&conn).unwrap();

        // Seed estabelecimento with Matriz (11223344000199) and Partner Matriz (55667788000100)
        conn.execute(
            "INSERT INTO estabelecimento (cnpj_basico, cnpj_ordem, cnpj_dv, matriz_filial, cnpj)
             VALUES ('11223344', '0001', '99', '1', '11223344000199')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO estabelecimento (cnpj_basico, cnpj_ordem, cnpj_dv, matriz_filial, cnpj)
             VALUES ('55667788', '0001', '00', '1', '55667788000100')",
            [],
        )
        .unwrap();

        // Materialize lookup cache
        let matriz_count = materialize_cnpj_base2matriz(&mut conn).unwrap();
        assert_eq!(matriz_count, 2);

        // Seed socios_original with:
        // 1) PF Partner
        conn.execute(
            "INSERT INTO socios_original (cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio)
             VALUES ('11223344', '2', 'JOAO SILVA', '***123456**')",
            [],
        )
        .unwrap();

        // 2) PJ Partner with August 2026 radical '55667788'
        conn.execute(
            "INSERT INTO socios_original (cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio)
             VALUES ('11223344', '1', 'HOLDING BRASIL LTDA', '55667788')",
            [],
        )
        .unwrap();

        let socios_count = transform_socios_and_expand_radicals(&mut conn).unwrap();
        assert_eq!(socios_count, 2);

        // Verify PJ partner expanded to 14-digit CNPJ
        let expanded_cnpj: String = conn
            .query_row(
                "SELECT cnpj_cpf_socio FROM socios WHERE identificador_de_socio = '1'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(expanded_cnpj, "55667788000100");

        // Verify PF partner kept masked CPF
        let pf_cpf: String = conn
            .query_row(
                "SELECT cnpj_cpf_socio FROM socios WHERE identificador_de_socio = '2'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(pf_cpf, "***123456**");

        create_post_load_indices(&conn).unwrap();
        IngestionPragmas::apply_post_load(&conn).unwrap();
    }
}
