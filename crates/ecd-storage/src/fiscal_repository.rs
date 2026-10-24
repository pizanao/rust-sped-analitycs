//! Repositório de consultas fiscais, DRE, Balanço e controle de importações.
//! Escopo exclusivo do Agente responsável pelo Servidor HTTP (`apps/server`).

use crate::StorageError;
use ecd_domain::{
    ImportacaoEcd, RegI355, RegJ150, Registro0000, RegistroI155, RegistroJ100, ResultadoExercicio,
    StatusImportacao,
};
use sqlx::postgres::PgRow;
use sqlx::types::{Decimal, Uuid};
use sqlx::{PgPool, Row};

/// Converte [`StatusImportacao`] para string persistida na coluna `status` de `importacoes_ecd`.
pub fn status_to_str(status: StatusImportacao) -> &'static str {
    match status {
        StatusImportacao::Aguardando => "Aguardando",
        StatusImportacao::EmProcessamento => "EmProcessamento",
        StatusImportacao::Sucesso => "Sucesso",
        StatusImportacao::Erro => "Erro",
        StatusImportacao::Substituida => "Substituida",
    }
}

/// Converte string da coluna `status` de `importacoes_ecd` para [`StatusImportacao`].
pub fn status_from_str(raw: &str) -> StatusImportacao {
    match raw.trim() {
        "EmProcessamento" | "Processando" => StatusImportacao::EmProcessamento,
        "Sucesso" | "Concluido" => StatusImportacao::Sucesso,
        "Erro" | "Falha" => StatusImportacao::Erro,
        "Substituida" => StatusImportacao::Substituida,
        _ => StatusImportacao::Aguardando,
    }
}

/// Dados consolidados de um CNPJ abrangendo cabeçalho (`0000`), Balanço (`J100`),
/// DRE (`J150`), Saldos Periódicos (`I155`), Encerramento (`I355`) e apuração fiscal.
#[derive(Clone, Debug, Default)]
pub struct ConsultaCnpjConsolidada {
    pub cnpj: String,
    pub razao_social: String,
    pub uf: Option<String>,
    pub reg_0000: Vec<Registro0000>,
    pub reg_j100: Vec<RegistroJ100>,
    pub reg_j150: Vec<RegJ150>,
    pub reg_i155: Vec<RegistroI155>,
    pub reg_i355: Vec<RegI355>,
    pub resultado_exercicio: Option<ResultadoExercicio>,
}

fn map_importacao_row(row: PgRow) -> Result<ImportacaoEcd, sqlx::Error> {
    let status_str: String = row.try_get("status")?;
    Ok(ImportacaoEcd {
        id: row.try_get("id")?,
        hash_arquivo: row.try_get("hash_arquivo")?,
        cnpj: row.try_get::<String, _>("cnpj")?.trim().to_string(),
        nome_arquivo_original: row.try_get("nome_arquivo_original")?,
        caminho_armazenamento: row.try_get("caminho_armazenamento")?,
        tamanho_bytes: row.try_get("tamanho_bytes")?,
        status: status_from_str(&status_str),
        linhas_processadas: row.try_get("linhas_processadas")?,
        tempo_processamento_ms: row.try_get("tempo_processamento_ms")?,
        mensagem_erro: row.try_get("mensagem_erro")?,
        criado_em: row.try_get("criado_em")?,
        atualizado_em: row.try_get("atualizado_em")?,
    })
}

fn map_resultado_exercicio_row(row: PgRow) -> Result<ResultadoExercicio, sqlx::Error> {
    Ok(ResultadoExercicio {
        id: row.try_get("id")?,
        cnpj: row.try_get::<String, _>("cnpj")?.trim().to_string(),
        razao_social: row.try_get("razao_social")?,
        periodo_inicial: row.try_get("periodo_inicial")?,
        periodo_final: row.try_get("periodo_final")?,
        cnpj_scp: row
            .try_get::<Option<String>, _>("cnpj_scp")?
            .map(|s| s.trim().to_string()),
        receita_bruta: row.try_get("receita_bruta")?,
        receita_servico: row.try_get("receita_servico")?,
        outras_receitas: row.try_get("outras_receitas")?,
        receita_acumulada: row.try_get("receita_acumulada")?,
        total_nota: row.try_get("total_nota")?,
        hash_arquivo: row.try_get("hash_arquivo")?,
        cod_hash_sub: row.try_get("cod_hash_sub")?,
        situacao: row.try_get("situacao")?,
        detalhes_j150: row.try_get("detalhes_j150")?,
        detalhes_i355: row.try_get("detalhes_i355")?,
        criado_em: row.try_get("criado_em")?,
    })
}

fn map_reg0000_row(row: PgRow) -> Result<Registro0000, sqlx::Error> {
    Ok(Registro0000 {
        id: Some(row.try_get("id")?),
        dt_ini: row.try_get("dt_ini")?,
        dt_fim: row.try_get("dt_fim")?,
        nome_razao: row.try_get("nome_razao")?,
        cnpj: row.try_get("cnpj")?,
        uf: row.try_get("uf")?,
        ie: row.try_get("ie")?,
        cod_mun: row.try_get("cod_mun")?,
        im: row.try_get("im")?,
        ind_sit_esp: row.try_get("ind_sit_esp")?,
        ind_sit_ini_per: row.try_get("ind_sit_ini_per")?,
        ind_nire: row.try_get("ind_nire")?,
        ind_fin_esc: row.try_get("ind_fin_esc")?,
        cod_hash_sub: row.try_get("cod_hash_sub")?,
        ind_grande_porte: row.try_get("ind_grande_porte")?,
        tip_ecd: row.try_get("tip_ecd")?,
        cod_scp: row.try_get("cod_scp")?,
        ident_mf: row.try_get("ident_mf")?,
        ind_esc_cons: row.try_get("ind_esc_cons")?,
        ind_centralizada: row.try_get("ind_centralizada")?,
        ind_mudanc_pc: row.try_get("ind_mudanc_pc")?,
        cod_plan_ref: row.try_get("cod_plan_ref")?,
        i990: row.try_get("i990")?,
        j990: row.try_get("j990")?,
        hash_arquivo_id: row.try_get("hash_arquivo_id")?,
    })
}

fn map_regj100_row(row: PgRow) -> Result<RegistroJ100, sqlx::Error> {
    Ok(RegistroJ100 {
        id: Some(row.try_get("id")?),
        cod_agl: row.try_get("cod_agl")?,
        ind_cod_agl: row.try_get("ind_cod_agl")?,
        nivel_agl: row.try_get("nivel_agl")?,
        cod_agl_sup: row.try_get("cod_agl_sup")?,
        ind_grp_bal: row.try_get("ind_grp_bal")?,
        descr_cod_agl: row.try_get("descr_cod_agl")?,
        vl_cta_ini: row.try_get("vl_cta_ini")?,
        ind_dc_cta_ini: row.try_get("ind_dc_cta_ini")?,
        vl_cta_fin: row.try_get("vl_cta_fin")?,
        ind_dc_cta_fin: row.try_get("ind_dc_cta_fin")?,
        nota_exp_ref: row.try_get("nota_exp_ref")?,
        cnpj: row.try_get("cnpj")?,
        hash_arquivo_id: row.try_get("hash_arquivo_id")?,
        reg_0000_id: row.try_get("reg_0000_id")?,
        reg_j005_id: row.try_get("reg_j005_id")?,
    })
}

fn map_regj150_row(row: PgRow) -> Result<RegJ150, sqlx::Error> {
    Ok(RegJ150 {
        id: Some(row.try_get("id")?),
        nu_ordem: row.try_get("nu_ordem")?,
        cod_agl: row.try_get("cod_agl")?,
        ind_cod_agl: row.try_get("ind_cod_agl")?,
        nivel_agl: row.try_get("nivel_agl")?,
        cod_agl_sup: row.try_get("cod_agl_sup")?,
        descr_cod_agl: row.try_get("descr_cod_agl")?,
        vl_cta_ini: row.try_get("vl_cta_ini")?,
        ind_dc_cta_ini: row.try_get("ind_dc_cta_ini")?,
        vl_cta_fin: row.try_get("vl_cta_fin")?,
        ind_dc_cta_fin: row.try_get("ind_dc_cta_fin")?,
        ind_grp_dre: row.try_get("ind_grp_dre")?,
        nota_exp_ref: row.try_get("nota_exp_ref")?,
        cnpj: row.try_get("cnpj")?,
        dt_ini: row.try_get("dt_ini")?,
        dt_fim: row.try_get("dt_fim")?,
        hash_arquivo: row.try_get("hash_arquivo")?,
    })
}

fn map_regi155_row(row: PgRow) -> Result<RegistroI155, sqlx::Error> {
    Ok(RegistroI155 {
        id: Some(row.try_get("id")?),
        cod_cta: row.try_get("cod_cta")?,
        cod_ccus: row.try_get("cod_ccus")?,
        vl_sld_ini: row.try_get("vl_sld_ini")?,
        ind_dc_ini: row.try_get("ind_dc_ini")?,
        vl_deb: row.try_get("vl_deb")?,
        vl_cred: row.try_get("vl_cred")?,
        vl_sld_fim: row.try_get("vl_sld_fim")?,
        ind_dc_fim: row.try_get("ind_dc_fim")?,
        reg_0000_id: row.try_get("reg_0000_id")?,
        reg_i150_id: row.try_get("reg_i150_id")?,
    })
}

fn map_regi355_row(row: PgRow) -> Result<RegI355, sqlx::Error> {
    Ok(RegI355 {
        id: row.try_get("id")?,
        cnpj: row.try_get("cnpj")?,
        cod_cta: row.try_get("cod_cta")?,
        nome_cta_referencia: row.try_get("nome_cta_referencia")?,
        cod_ccus: row.try_get("cod_ccus")?,
        vl_cta: row.try_get("vl_cta")?,
        ind_dc: row.try_get("ind_dc")?,
        vl_cta_mf: row.try_get("vl_cta_mf")?,
        ind_dc_mf: row.try_get("ind_dc_mf")?,
        dt_res: row.try_get("dt_res")?,
        cod_agl: row.try_get("cod_agl")?,
        cod_cta_ref: row.try_get("cod_cta_ref")?,
        nome_aglutinador: row.try_get("nome_aglutinador")?,
        hash_arquivo: row.try_get("hash_arquivo")?,
    })
}

/// Repositório PostgreSQL para consultas contábeis, DRE e jobs de importação SPED.
#[derive(Clone, Debug)]
pub struct FiscalRepository {
    pool: PgPool,
}

impl FiscalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Registra ou atualiza um job de importação SPED em `importacoes_ecd`.
    pub async fn registrar_importacao(
        &self,
        imp: &ImportacaoEcd,
    ) -> Result<ImportacaoEcd, StorageError> {
        let row = sqlx::query(
            r#"
            INSERT INTO importacoes_ecd (
                id, hash_arquivo, cnpj, nome_arquivo_original, caminho_armazenamento,
                tamanho_bytes, status, linhas_processadas, tempo_processamento_ms,
                mensagem_erro, criado_em, atualizado_em
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (hash_arquivo) DO UPDATE
            SET id = EXCLUDED.id,
                cnpj = EXCLUDED.cnpj,
                nome_arquivo_original = EXCLUDED.nome_arquivo_original,
                caminho_armazenamento = EXCLUDED.caminho_armazenamento,
                tamanho_bytes = EXCLUDED.tamanho_bytes,
                status = EXCLUDED.status,
                mensagem_erro = NULL,
                atualizado_em = NOW()
            RETURNING id, hash_arquivo, cnpj, nome_arquivo_original, caminho_armazenamento,
                      tamanho_bytes, status, linhas_processadas, tempo_processamento_ms,
                      mensagem_erro, criado_em, atualizado_em
            "#,
        )
        .bind(imp.id)
        .bind(&imp.hash_arquivo)
        .bind(&imp.cnpj)
        .bind(&imp.nome_arquivo_original)
        .bind(&imp.caminho_armazenamento)
        .bind(imp.tamanho_bytes)
        .bind(status_to_str(imp.status))
        .bind(imp.linhas_processadas)
        .bind(imp.tempo_processamento_ms)
        .bind(&imp.mensagem_erro)
        .bind(imp.criado_em)
        .bind(imp.atualizado_em)
        .fetch_one(&self.pool)
        .await?;

        map_importacao_row(row).map_err(StorageError::from)
    }

    /// Obtém um job de importação pelo seu UUID.
    pub async fn obter_importacao(&self, id: Uuid) -> Result<Option<ImportacaoEcd>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, hash_arquivo, cnpj, nome_arquivo_original, caminho_armazenamento,
                   tamanho_bytes, status, linhas_processadas, tempo_processamento_ms,
                   mensagem_erro, criado_em, atualizado_em
            FROM importacoes_ecd
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_importacao_row)
            .transpose()
            .map_err(StorageError::from)
    }

    /// Obtém um job de importação pelo hash do arquivo.
    pub async fn obter_importacao_por_hash(
        &self,
        hash_arquivo: &str,
    ) -> Result<Option<ImportacaoEcd>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, hash_arquivo, cnpj, nome_arquivo_original, caminho_armazenamento,
                   tamanho_bytes, status, linhas_processadas, tempo_processamento_ms,
                   mensagem_erro, criado_em, atualizado_em
            FROM importacoes_ecd
            WHERE hash_arquivo = $1
            LIMIT 1
            "#,
        )
        .bind(hash_arquivo.trim())
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_importacao_row)
            .transpose()
            .map_err(StorageError::from)
    }

    /// Lista os jobs de importação mais recentes.
    pub async fn listar_importacoes(&self, limit: i64) -> Result<Vec<ImportacaoEcd>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT id, hash_arquivo, cnpj, nome_arquivo_original, caminho_armazenamento,
                   tamanho_bytes, status, linhas_processadas, tempo_processamento_ms,
                   mensagem_erro, criado_em, atualizado_em
            FROM importacoes_ecd
            ORDER BY criado_em DESC
            LIMIT $1
            "#,
        )
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(map_importacao_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Atualiza o status de uma importação em andamento ou concluída.
    pub async fn atualizar_status_importacao(
        &self,
        id: Uuid,
        status: StatusImportacao,
        linhas_processadas: i32,
        tempo_processamento_ms: i32,
        mensagem_erro: Option<&str>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            UPDATE importacoes_ecd
            SET status = $2,
                linhas_processadas = $3,
                tempo_processamento_ms = $4,
                mensagem_erro = $5,
                atualizado_em = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status_to_str(status))
        .bind(linhas_processadas)
        .bind(tempo_processamento_ms)
        .bind(mensagem_erro)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Busca os registros `0000` ativos de um CNPJ (excluindo escriturações substituídas via `cod_hash_sub`).
    pub async fn buscar_reg0000_por_cnpj(
        &self,
        cnpj: &str,
        ano: Option<i32>,
    ) -> Result<Vec<Registro0000>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT r.id, r.dt_ini, r.dt_fim, r.nome_razao, r.cnpj, r.uf, r.ie, r.cod_mun,
                   r.im, r.ind_sit_esp, r.ind_sit_ini_per, r.ind_nire, r.ind_fin_esc,
                   r.cod_hash_sub, r.ind_grande_porte, r.tip_ecd, r.cod_scp, r.ident_mf,
                   r.ind_esc_cons, r.ind_centralizada, r.ind_mudanc_pc, r.cod_plan_ref,
                   r.i990, r.j990, r.hash_arquivo_id
            FROM core_registro0000 r
            WHERE r.cnpj = $1
              AND ($2::int IS NULL OR EXTRACT(YEAR FROM r.dt_fim)::int = $2)
              AND NOT EXISTS (
                  SELECT 1 FROM core_registro0000 sub
                  WHERE sub.cod_hash_sub = r.hash_arquivo_id
              )
            ORDER BY r.dt_fim DESC, r.id DESC
            "#,
        )
        .bind(cnpj)
        .bind(ano)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(map_reg0000_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Busca registros do Balanço Patrimonial (`core_registroj100`) de um CNPJ.
    pub async fn buscar_regj100_por_cnpj(
        &self,
        cnpj: &str,
        reg_0000_ids: &[i64],
    ) -> Result<Vec<RegistroJ100>, StorageError> {
        let rows = if reg_0000_ids.is_empty() {
            sqlx::query(
                r#"
                SELECT id, cod_agl, ind_cod_agl, nivel_agl, cod_agl_sup, ind_grp_bal,
                       descr_cod_agl, vl_cta_ini, ind_dc_cta_ini, vl_cta_fin, ind_dc_cta_fin,
                       nota_exp_ref, cnpj, hash_arquivo_id, reg_0000_id, reg_j005_id
                FROM core_registroj100
                WHERE cnpj = $1
                ORDER BY id ASC
                "#,
            )
            .bind(cnpj)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, cod_agl, ind_cod_agl, nivel_agl, cod_agl_sup, ind_grp_bal,
                       descr_cod_agl, vl_cta_ini, ind_dc_cta_ini, vl_cta_fin, ind_dc_cta_fin,
                       nota_exp_ref, cnpj, hash_arquivo_id, reg_0000_id, reg_j005_id
                FROM core_registroj100
                WHERE cnpj = $1 AND reg_0000_id = ANY($2)
                ORDER BY id ASC
                "#,
            )
            .bind(cnpj)
            .bind(reg_0000_ids)
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter()
            .map(map_regj100_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Busca registros da DRE (`regj150_demonstracao_resultado`) de um CNPJ,
    /// excluindo escriturações substituídas.
    pub async fn buscar_regj150_por_cnpj(
        &self,
        cnpj: &str,
        hashes_ativos: &[String],
    ) -> Result<Vec<RegJ150>, StorageError> {
        let rows = if hashes_ativos.is_empty() {
            sqlx::query(
                r#"
                SELECT r.id, r.nu_ordem, r.cod_agl, r.ind_cod_agl, r.nivel_agl, r.cod_agl_sup,
                       r.descr_cod_agl, r.vl_cta_ini, r.ind_dc_cta_ini, r.vl_cta_fin,
                       r.ind_dc_cta_fin, r.ind_grp_dre, r.nota_exp_ref, r.cnpj,
                       r.dt_ini, r.dt_fim, r.hash_arquivo
                FROM regj150_demonstracao_resultado r
                WHERE r.cnpj = $1
                  AND NOT EXISTS (
                      SELECT 1 FROM core_registro0000 sub
                      WHERE sub.cod_hash_sub = r.hash_arquivo
                  )
                ORDER BY r.dt_fim DESC NULLS LAST, r.nu_ordem ASC, r.id ASC
                "#,
            )
            .bind(cnpj)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT r.id, r.nu_ordem, r.cod_agl, r.ind_cod_agl, r.nivel_agl, r.cod_agl_sup,
                       r.descr_cod_agl, r.vl_cta_ini, r.ind_dc_cta_ini, r.vl_cta_fin,
                       r.ind_dc_cta_fin, r.ind_grp_dre, r.nota_exp_ref, r.cnpj,
                       r.dt_ini, r.dt_fim, r.hash_arquivo
                FROM regj150_demonstracao_resultado r
                WHERE r.cnpj = $1 AND r.hash_arquivo = ANY($2)
                ORDER BY r.dt_fim DESC NULLS LAST, r.nu_ordem ASC, r.id ASC
                "#,
            )
            .bind(cnpj)
            .bind(hashes_ativos)
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter()
            .map(map_regj150_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Busca saldos periódicos (`core_registroi155`) vinculados às escriturações do CNPJ.
    pub async fn buscar_regi155_por_cnpj(
        &self,
        cnpj: &str,
        reg_0000_ids: &[i64],
        limit: i64,
    ) -> Result<Vec<RegistroI155>, StorageError> {
        let clamped_limit = limit.clamp(1, 50_000);
        let rows = if reg_0000_ids.is_empty() {
            sqlx::query(
                r#"
                SELECT i.id, i.cod_cta, i.cod_ccus, i.vl_sld_ini, i.ind_dc_ini,
                       i.vl_deb, i.vl_cred, i.vl_sld_fim, i.ind_dc_fim,
                       i.reg_0000_id, i.reg_i150_id
                FROM core_registroi155 i
                INNER JOIN core_registro0000 r0 ON r0.id = i.reg_0000_id
                WHERE r0.cnpj = $1
                  AND NOT EXISTS (
                      SELECT 1 FROM core_registro0000 sub
                      WHERE sub.cod_hash_sub = r0.hash_arquivo_id
                  )
                ORDER BY i.id ASC
                LIMIT $2
                "#,
            )
            .bind(cnpj)
            .bind(clamped_limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT i.id, i.cod_cta, i.cod_ccus, i.vl_sld_ini, i.ind_dc_ini,
                       i.vl_deb, i.vl_cred, i.vl_sld_fim, i.ind_dc_fim,
                       i.reg_0000_id, i.reg_i150_id
                FROM core_registroi155 i
                WHERE i.reg_0000_id = ANY($1)
                ORDER BY i.id ASC
                LIMIT $2
                "#,
            )
            .bind(reg_0000_ids)
            .bind(clamped_limit)
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter()
            .map(map_regi155_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Busca contas de resultado antes do encerramento (`regi355_detalhes_contas_resultado`).
    pub async fn buscar_regi355_por_cnpj(
        &self,
        cnpj: &str,
        hashes_ativos: &[String],
    ) -> Result<Vec<RegI355>, StorageError> {
        let rows = if hashes_ativos.is_empty() {
            sqlx::query(
                r#"
                SELECT r.id, r.cnpj, r.cod_cta, r.nome_cta_referencia, r.cod_ccus,
                       r.vl_cta, r.ind_dc, r.vl_cta_mf, r.ind_dc_mf, r.dt_res,
                       r.cod_agl, r.cod_cta_ref, r.nome_aglutinador, r.hash_arquivo
                FROM regi355_detalhes_contas_resultado r
                WHERE r.cnpj = $1
                  AND NOT EXISTS (
                      SELECT 1 FROM core_registro0000 sub
                      WHERE sub.cod_hash_sub = r.hash_arquivo
                  )
                ORDER BY r.dt_res DESC, r.cod_cta ASC
                "#,
            )
            .bind(cnpj)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT r.id, r.cnpj, r.cod_cta, r.nome_cta_referencia, r.cod_ccus,
                       r.vl_cta, r.ind_dc, r.vl_cta_mf, r.ind_dc_mf, r.dt_res,
                       r.cod_agl, r.cod_cta_ref, r.nome_aglutinador, r.hash_arquivo
                FROM regi355_detalhes_contas_resultado r
                WHERE r.cnpj = $1 AND r.hash_arquivo = ANY($2)
                ORDER BY r.dt_res DESC, r.cod_cta ASC
                "#,
            )
            .bind(cnpj)
            .bind(hashes_ativos)
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter()
            .map(map_regi355_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Busca o resultado consolidado do exercício (`resultados_exercicio`) para um CNPJ.
    pub async fn buscar_resultado_exercicio(
        &self,
        cnpj: &str,
        ano: Option<i32>,
    ) -> Result<Option<ResultadoExercicio>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, cnpj, razao_social, periodo_inicial, periodo_final, cnpj_scp,
                   receita_bruta, receita_servico, outras_receitas, receita_acumulada,
                   total_nota, hash_arquivo, cod_hash_sub, situacao,
                   detalhes_j150, detalhes_i355, criado_em
            FROM resultados_exercicio
            WHERE cnpj = $1
              AND situacao != 'X'
              AND ($2::int IS NULL OR EXTRACT(YEAR FROM periodo_final)::int = $2)
            ORDER BY periodo_final DESC, criado_em DESC
            LIMIT 1
            "#,
        )
        .bind(cnpj)
        .bind(ano)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_resultado_exercicio_row)
            .transpose()
            .map_err(StorageError::from)
    }

    /// Soma a receita bruta total apurada em `resultados_exercicio` (para KPI do dashboard).
    pub async fn somar_receita_bruta_apurada(&self) -> Result<Decimal, StorageError> {
        let total: Option<Decimal> = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(receita_bruta), 0)
            FROM resultados_exercicio
            WHERE situacao != 'X'
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(total.unwrap_or(Decimal::ZERO))
    }

    /// Executa a consulta consolidada de auditoria para um CNPJ, reunindo `0000`, `J100`, `J150`, `I155`, `I355` e `resultados_exercicio`.
    pub async fn consultar_cnpj_consolidado(
        &self,
        cnpj: &str,
        ano: Option<i32>,
    ) -> Result<ConsultaCnpjConsolidada, StorageError> {
        let clean: String = cnpj.chars().filter(|c| c.is_ascii_digit()).collect();
        let reg_0000 = self.buscar_reg0000_por_cnpj(&clean, ano).await?;

        let reg_0000_ids: Vec<i64> = reg_0000.iter().filter_map(|r| r.id).collect();
        let hashes_ativos: Vec<String> =
            reg_0000.iter().map(|r| r.hash_arquivo_id.clone()).collect();

        let reg_j100 = self.buscar_regj100_por_cnpj(&clean, &reg_0000_ids).await?;
        let reg_j150 = self.buscar_regj150_por_cnpj(&clean, &hashes_ativos).await?;
        let reg_i155 = self
            .buscar_regi155_por_cnpj(&clean, &reg_0000_ids, 5_000)
            .await?;
        let reg_i355 = self.buscar_regi355_por_cnpj(&clean, &hashes_ativos).await?;
        let resultado_exercicio = self.buscar_resultado_exercicio(&clean, ano).await?;

        let razao_social = reg_0000
            .first()
            .map(|r| r.nome_razao.clone())
            .or_else(|| {
                resultado_exercicio
                    .as_ref()
                    .map(|res| res.razao_social.clone())
            })
            .unwrap_or_default();

        let uf = reg_0000.first().map(|r| r.uf.clone());

        Ok(ConsultaCnpjConsolidada {
            cnpj: clean,
            razao_social,
            uf,
            reg_0000,
            reg_j100,
            reg_j150,
            reg_i155,
            reg_i355,
            resultado_exercicio,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_importacao_conversao_roundtrip() {
        let statuses = [
            StatusImportacao::Aguardando,
            StatusImportacao::EmProcessamento,
            StatusImportacao::Sucesso,
            StatusImportacao::Erro,
            StatusImportacao::Substituida,
        ];
        for st in statuses {
            let s = status_to_str(st);
            assert_eq!(status_from_str(s), st);
        }
        assert_eq!(
            status_from_str("desconhecido"),
            StatusImportacao::Aguardando
        );
    }
}
