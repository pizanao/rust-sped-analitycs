//! Repositório de lote (batch inserts) dos registros SPED ECD (`core_*` e `reg*`).
//! Escopo exclusivo do Agente responsável pelo Worker (`apps/worker`).
//!
//! Implementa persistência transacional atômica em lotes de 5.000 linhas via `sqlx::QueryBuilder`,
//! remapeamento seguro de chaves estrangeiras (`reg_0000_id`, `reg_i050_id`, `reg_i150_id`, `reg_j005_id`),
//! exclusão lógica de escriturações substituídas (`cod_hash_sub`) e gravação de `resultados_exercicio`.

use std::collections::{HashMap, HashSet};

use ecd_domain::{
    ImportSpedJobPayload, RegI050, RegI355, RegJ150, Registro0000, RegistroI010, RegistroI030,
    RegistroI050, RegistroI051, RegistroI052, RegistroI053, RegistroI150, RegistroI155,
    RegistroJ005, RegistroJ100, RegistroJ210, RegistroJ215, ResultadoExercicio, StatusImportacao,
};
use sqlx::types::Uuid;
use sqlx::{PgPool, Postgres, QueryBuilder, Row, Transaction};

use crate::StorageError;

/// Tamanho padrão de lote (5.000 linhas) para inserções em massa no PostgreSQL.
pub const BATCH_SIZE: usize = 5_000;

/// Limite máximo de parâmetros bind suportados pelo protocolo de rede do PostgreSQL (`u16::MAX`).
pub const MAX_PG_BIND_PARAMS: usize = 65_535;

/// Calcula o tamanho efetivo de cada fatia enviada ao `QueryBuilder` respeitando
/// tanto `batch_size` (5.000 linhas) quanto o limite de 65.535 parâmetros do PostgreSQL.
#[inline]
pub const fn effective_chunk_size(batch_size: usize, columns_per_row: usize) -> usize {
    if columns_per_row == 0 {
        return batch_size;
    }
    let max_rows_by_params = MAX_PG_BIND_PARAMS / columns_per_row;
    if batch_size <= max_rows_by_params {
        batch_size
    } else {
        max_rows_by_params
    }
}

/// Trunca uma string de forma segura em fronteira de caractere UTF-8 para evitar erro
/// `value too long for type character varying(N)` no PostgreSQL.
#[inline]
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => &s[..byte_idx],
        None => s,
    }
}

/// Trunca um `Option<String>` em fronteira de caractere UTF-8.
#[inline]
pub fn truncate_opt_chars(opt: &Option<String>, max_chars: usize) -> Option<&str> {
    opt.as_deref().map(|s| truncate_chars(s, max_chars))
}

/// Converte `StatusImportacao` para a representação textual gravada em `importacoes_ecd.status`.
pub const fn status_importacao_as_str(status: StatusImportacao) -> &'static str {
    match status {
        StatusImportacao::Aguardando => "Aguardando",
        StatusImportacao::EmProcessamento => "EmProcessamento",
        StatusImportacao::Sucesso => "Sucesso",
        StatusImportacao::Erro => "Erro",
        StatusImportacao::Substituida => "Substituida",
    }
}

/// Pacote completo de registros SPED ECD decodificados para persistência atômica em lote.
#[derive(Clone, Debug)]
pub struct SpedBatchRecords {
    pub reg_0000: Registro0000,
    pub reg_i010: Vec<RegistroI010>,
    pub reg_i030: Vec<RegistroI030>,
    pub reg_i050: Vec<RegistroI050>,
    pub reg_i051: Vec<RegistroI051>,
    pub reg_i052: Vec<RegistroI052>,
    pub reg_i053: Vec<RegistroI053>,
    pub plano_contas: Vec<RegI050>,
    pub reg_i150: Vec<RegistroI150>,
    pub reg_i155: Vec<RegistroI155>,
    pub reg_i355: Vec<RegI355>,
    pub reg_j005: Vec<RegistroJ005>,
    pub reg_j100: Vec<RegistroJ100>,
    pub reg_j150: Vec<RegJ150>,
    pub reg_j210: Vec<RegistroJ210>,
    pub reg_j215: Vec<RegistroJ215>,
}

/// Resumo da operação de persistência em lote no PostgreSQL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchInsertSummary {
    pub reg_0000_id: i64,
    pub total_registros_inseridos: usize,
    pub escrituracao_substituida_hash: Option<String>,
    pub arquivo_atual_ja_substituido: bool,
}

/// Remapeia os IDs locais (`1..=N` atribuídos pelo parser em memória) dos registros filhos
/// (`I051`, `I052`, `I155`, `J100`) para os IDs `BIGSERIAL` reais gerados pelo PostgreSQL.
pub fn resolve_parent_id(raw_id: i64, parent_db_ids: &[i64]) -> Option<i64> {
    if raw_id >= 1 && (raw_id as usize) <= parent_db_ids.len() {
        Some(parent_db_ids[(raw_id as usize) - 1])
    } else if parent_db_ids.contains(&raw_id) {
        Some(raw_id)
    } else {
        parent_db_ids.last().copied()
    }
}

/// Constrói a query parametrizada para inserção do `Registro0000` (`core_registro0000`).
pub fn build_insert_0000_query(r: &Registro0000) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registro0000 (
            dt_ini, dt_fim, nome_razao, cnpj, uf, ie, cod_mun, im,
            ind_sit_esp, ind_sit_ini_per, ind_nire, ind_fin_esc, cod_hash_sub,
            ind_grande_porte, tip_ecd, cod_scp, ident_mf, ind_esc_cons,
            ind_centralizada, ind_mudanc_pc, cod_plan_ref, i990, j990, hash_arquivo_id
        ) ",
    );
    qb.push_values(std::iter::once(r), |mut b, row| {
        b.push_bind(row.dt_ini)
            .push_bind(row.dt_fim)
            .push_bind(truncate_chars(&row.nome_razao, 155).to_string())
            .push_bind(truncate_chars(&row.cnpj, 14).to_string())
            .push_bind(truncate_chars(&row.uf, 2).to_string())
            .push_bind(truncate_opt_chars(&row.ie, 50).map(str::to_string))
            .push_bind(row.cod_mun)
            .push_bind(truncate_opt_chars(&row.im, 20).map(str::to_string))
            .push_bind(row.ind_sit_esp)
            .push_bind(row.ind_sit_ini_per)
            .push_bind(row.ind_nire)
            .push_bind(row.ind_fin_esc)
            .push_bind(truncate_opt_chars(&row.cod_hash_sub, 40).map(str::to_string))
            .push_bind(row.ind_grande_porte)
            .push_bind(row.tip_ecd)
            .push_bind(truncate_opt_chars(&row.cod_scp, 14).map(str::to_string))
            .push_bind(truncate_chars(&row.ident_mf, 1).to_string())
            .push_bind(truncate_chars(&row.ind_esc_cons, 1).to_string())
            .push_bind(row.ind_centralizada)
            .push_bind(row.ind_mudanc_pc)
            .push_bind(truncate_opt_chars(&row.cod_plan_ref, 2).map(str::to_string))
            .push_bind(row.i990)
            .push_bind(row.j990)
            .push_bind(truncate_chars(&row.hash_arquivo_id, 40).to_string());
    });
    qb.push(" RETURNING id");
    qb
}

/// Constrói query em lote para `core_registroi010`.
pub fn build_bulk_insert_i010_query(
    reg_0000_id: i64,
    chunk: &[RegistroI010],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroi010 (ind_esc, cod_ver_lc, reg_0000_id) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(truncate_chars(&row.ind_esc, 1).to_string())
            .push_bind(truncate_chars(&row.cod_ver_lc, 10).to_string())
            .push_bind(fk);
    });
    qb
}

/// Constrói query em lote para `core_registroi030`.
pub fn build_bulk_insert_i030_query(
    reg_0000_id: i64,
    chunk: &[RegistroI030],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroi030 (
            num_ord, nat_livr, qtd_lin, nome, nire, cnpj,
            dt_arq, dt_arq_conv, desc_mun, dt_ex_social, reg_0000_id, hash_arquivo_id
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(row.num_ord)
            .push_bind(truncate_chars(&row.nat_livr, 90).to_string())
            .push_bind(row.qtd_lin)
            .push_bind(truncate_chars(&row.nome, 150).to_string())
            .push_bind(truncate_chars(&row.nire, 20).to_string())
            .push_bind(truncate_chars(&row.cnpj, 14).to_string())
            .push_bind(row.dt_arq)
            .push_bind(row.dt_arq_conv)
            .push_bind(truncate_opt_chars(&row.desc_mun, 55).map(str::to_string))
            .push_bind(row.dt_ex_social)
            .push_bind(fk)
            .push_bind(truncate_opt_chars(&row.hash_arquivo_id, 40).map(str::to_string));
    });
    qb
}

/// Constrói query em lote para `core_registroi050` com `RETURNING id`.
pub fn build_bulk_insert_i050_query(
    reg_0000_id: i64,
    chunk: &[RegistroI050],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroi050 (
            cod_cta, cta, nivel, ind_cta, cod_nat, cod_cta_sup,
            dt_alt, descr_cta, descr_conta, codigo_conta, reg_0000_id
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(truncate_chars(&row.cod_cta, 50).to_string())
            .push_bind(truncate_chars(&row.cta, 120).to_string())
            .push_bind(row.nivel)
            .push_bind(truncate_chars(&row.ind_cta, 1).to_string())
            .push_bind(truncate_chars(&row.cod_nat, 2).to_string())
            .push_bind(truncate_opt_chars(&row.cod_cta_sup, 50).map(str::to_string))
            .push_bind(row.dt_alt)
            .push_bind(truncate_chars(&row.descr_cta, 120).to_string())
            .push_bind(truncate_opt_chars(&row.descr_conta, 120).map(str::to_string))
            .push_bind(truncate_opt_chars(&row.codigo_conta, 50).map(str::to_string))
            .push_bind(fk);
    });
    qb.push(" RETURNING id");
    qb
}

/// Constrói query em lote para `core_registroi051`.
pub fn build_bulk_insert_i051_query(
    reg_0000_id: i64,
    chunk: &[RegistroI051],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroi051 (cod_ccus, cod_cta_ref, reg_0000_id, reg_i050_id) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk_0000 = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(truncate_opt_chars(&row.cod_ccus, 50).map(str::to_string))
            .push_bind(truncate_chars(&row.cod_cta_ref, 50).to_string())
            .push_bind(fk_0000)
            .push_bind(row.reg_i050_id);
    });
    qb
}

/// Constrói query em lote para `core_registroi052`.
pub fn build_bulk_insert_i052_query(
    reg_0000_id: i64,
    chunk: &[RegistroI052],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroi052 (cod_ccus, cod_agl, reg_0000_id, reg_i050_id) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk_0000 = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(truncate_opt_chars(&row.cod_ccus, 50).map(str::to_string))
            .push_bind(truncate_chars(&row.cod_agl, 150).to_string())
            .push_bind(fk_0000)
            .push_bind(row.reg_i050_id);
    });
    qb
}

/// Constrói query em lote para `core_registroi053`.
pub fn build_bulk_insert_i053_query(
    reg_0000_id: i64,
    chunk: &[RegistroI053],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroi053 (cod_idt, cod_cnt_corr, nat_sub_cnt, reg_0000_id) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk_0000 = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(truncate_chars(&row.cod_idt, 6).to_string())
            .push_bind(truncate_chars(&row.cod_cnt_corr, 50).to_string())
            .push_bind(truncate_chars(&row.nat_sub_cnt, 2).to_string())
            .push_bind(fk_0000);
    });
    qb
}

/// Constrói query em lote para `regi050_plano_contas` (`RegI050` consolidado).
pub fn build_bulk_insert_reg_i050_query(chunk: &[RegI050]) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO regi050_plano_contas (
            id, cod_cta, cta, nivel, ind_cta, cod_nat, cod_cta_sup, dt_alt,
            cod_ccus_51, cod_cta_ref, cod_ccus_52, cod_agl, cnpj, data_entrega,
            cta_pai, nome_cta_referencia, reg051_json, reg052_json, hash_arquivo
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let id = if row.id.is_nil() {
            Uuid::now_v7()
        } else {
            row.id
        };
        b.push_bind(id)
            .push_bind(truncate_chars(&row.cod_cta, 50).to_string())
            .push_bind(truncate_chars(&row.cta, 120).to_string())
            .push_bind(row.nivel)
            .push_bind(truncate_chars(&row.ind_cta, 1).to_string())
            .push_bind(truncate_chars(&row.cod_nat, 2).to_string())
            .push_bind(truncate_opt_chars(&row.cod_cta_sup, 50).map(str::to_string))
            .push_bind(row.dt_alt)
            .push_bind(truncate_opt_chars(&row.cod_ccus_51, 50).map(str::to_string))
            .push_bind(truncate_opt_chars(&row.cod_cta_ref, 60).map(str::to_string))
            .push_bind(truncate_opt_chars(&row.cod_ccus_52, 50).map(str::to_string))
            .push_bind(truncate_opt_chars(&row.cod_agl, 60).map(str::to_string))
            .push_bind(truncate_chars(&row.cnpj, 14).to_string())
            .push_bind(row.data_entrega)
            .push_bind(truncate_opt_chars(&row.cta_pai, 120).map(str::to_string))
            .push_bind(truncate_opt_chars(&row.nome_cta_referencia, 255).map(str::to_string))
            .push_bind(row.reg051_json.clone())
            .push_bind(row.reg052_json.clone())
            .push_bind(truncate_chars(&row.hash_arquivo, 40).to_string());
    });
    qb
}

/// Constrói query em lote para `core_registroi150` com `RETURNING id`.
pub fn build_bulk_insert_i150_query(
    reg_0000_id: i64,
    chunk: &[RegistroI150],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroi150 (dt_ini, dt_fim, reg_0000_id) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(row.dt_ini).push_bind(row.dt_fim).push_bind(fk);
    });
    qb.push(" RETURNING id");
    qb
}

/// Constrói query em lote para `core_registroi155`.
pub fn build_bulk_insert_i155_query(
    reg_0000_id: i64,
    chunk: &[RegistroI155],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroi155 (
            cod_cta, cod_ccus, vl_sld_ini, ind_dc_ini,
            vl_deb, vl_cred, vl_sld_fim, ind_dc_fim, reg_0000_id, reg_i150_id
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk_0000 = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(truncate_chars(&row.cod_cta, 50).to_string())
            .push_bind(truncate_opt_chars(&row.cod_ccus, 50).map(str::to_string))
            .push_bind(row.vl_sld_ini)
            .push_bind(truncate_chars(&row.ind_dc_ini, 1).to_string())
            .push_bind(row.vl_deb)
            .push_bind(row.vl_cred)
            .push_bind(row.vl_sld_fim)
            .push_bind(truncate_chars(&row.ind_dc_fim, 1).to_string())
            .push_bind(fk_0000)
            .push_bind(row.reg_i150_id);
    });
    qb
}

/// Constrói query em lote para `regi355_detalhes_contas_resultado` (`RegI355`).
pub fn build_bulk_insert_reg_i355_query(chunk: &[RegI355]) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO regi355_detalhes_contas_resultado (
            id, cnpj, cod_cta, nome_cta_referencia, cod_ccus,
            vl_cta, ind_dc, vl_cta_mf, ind_dc_mf, dt_res,
            cod_agl, cod_cta_ref, nome_aglutinador, hash_arquivo
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let id = if row.id.is_nil() {
            Uuid::now_v7()
        } else {
            row.id
        };
        b.push_bind(id)
            .push_bind(truncate_chars(&row.cnpj, 14).to_string())
            .push_bind(truncate_chars(&row.cod_cta, 150).to_string())
            .push_bind(truncate_opt_chars(&row.nome_cta_referencia, 255).map(str::to_string))
            .push_bind(truncate_opt_chars(&row.cod_ccus, 50).map(str::to_string))
            .push_bind(row.vl_cta)
            .push_bind(truncate_chars(&row.ind_dc, 1).to_string())
            .push_bind(row.vl_cta_mf)
            .push_bind(truncate_opt_chars(&row.ind_dc_mf, 1).map(str::to_string))
            .push_bind(row.dt_res)
            .push_bind(truncate_opt_chars(&row.cod_agl, 50).map(str::to_string))
            .push_bind(truncate_opt_chars(&row.cod_cta_ref, 60).map(str::to_string))
            .push_bind(truncate_opt_chars(&row.nome_aglutinador, 255).map(str::to_string))
            .push_bind(truncate_chars(&row.hash_arquivo, 40).to_string());
    });
    qb
}

/// Constrói query em lote para `core_registroj005` com `RETURNING id`.
pub fn build_bulk_insert_j005_query(
    reg_0000_id: i64,
    chunk: &[RegistroJ005],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroj005 (
            dt_ini, dt_fim, id_dem, cab_dem, cnpj, hash_arquivo_id, reg_0000_id
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(row.dt_ini)
            .push_bind(row.dt_fim)
            .push_bind(row.id_dem)
            .push_bind(row.cab_dem.clone())
            .push_bind(truncate_chars(&row.cnpj, 14).to_string())
            .push_bind(truncate_opt_chars(&row.hash_arquivo_id, 40).map(str::to_string))
            .push_bind(fk);
    });
    qb.push(" RETURNING id");
    qb
}

/// Constrói query em lote para `core_registroj100`.
pub fn build_bulk_insert_j100_query(
    reg_0000_id: i64,
    chunk: &[RegistroJ100],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroj100 (
            cod_agl, ind_cod_agl, nivel_agl, cod_agl_sup, ind_grp_bal, descr_cod_agl,
            vl_cta_ini, ind_dc_cta_ini, vl_cta_fin, ind_dc_cta_fin,
            nota_exp_ref, cnpj, hash_arquivo_id, reg_0000_id, reg_j005_id
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk_0000 = if reg_0000_id > 0 {
            reg_0000_id
        } else {
            row.reg_0000_id
        };
        b.push_bind(truncate_chars(&row.cod_agl, 255).to_string())
            .push_bind(truncate_chars(&row.ind_cod_agl, 1).to_string())
            .push_bind(row.nivel_agl)
            .push_bind(truncate_opt_chars(&row.cod_agl_sup, 255).map(str::to_string))
            .push_bind(truncate_chars(&row.ind_grp_bal, 1).to_string())
            .push_bind(truncate_chars(&row.descr_cod_agl, 255).to_string())
            .push_bind(row.vl_cta_ini)
            .push_bind(truncate_chars(&row.ind_dc_cta_ini, 1).to_string())
            .push_bind(row.vl_cta_fin)
            .push_bind(truncate_chars(&row.ind_dc_cta_fin, 1).to_string())
            .push_bind(truncate_opt_chars(&row.nota_exp_ref, 25).map(str::to_string))
            .push_bind(truncate_chars(&row.cnpj, 14).to_string())
            .push_bind(truncate_opt_chars(&row.hash_arquivo_id, 40).map(str::to_string))
            .push_bind(fk_0000)
            .push_bind(row.reg_j005_id);
    });
    qb
}

/// Constrói query em lote para `regj150_demonstracao_resultado` (`RegJ150`).
pub fn build_bulk_insert_reg_j150_query(chunk: &[RegJ150]) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO regj150_demonstracao_resultado (
            nu_ordem, cod_agl, ind_cod_agl, nivel_agl, cod_agl_sup, descr_cod_agl,
            vl_cta_ini, ind_dc_cta_ini, vl_cta_fin, ind_dc_cta_fin,
            ind_grp_dre, nota_exp_ref, cnpj, dt_ini, dt_fim, hash_arquivo
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        b.push_bind(row.nu_ordem)
            .push_bind(truncate_opt_chars(&row.cod_agl, 60).map(str::to_string))
            .push_bind(truncate_chars(&row.ind_cod_agl, 1).to_string())
            .push_bind(row.nivel_agl)
            .push_bind(truncate_opt_chars(&row.cod_agl_sup, 60).map(str::to_string))
            .push_bind(truncate_chars(&row.descr_cod_agl, 255).to_string())
            .push_bind(row.vl_cta_ini)
            .push_bind(truncate_opt_chars(&row.ind_dc_cta_ini, 1).map(str::to_string))
            .push_bind(row.vl_cta_fin)
            .push_bind(truncate_chars(&row.ind_dc_cta_fin, 1).to_string())
            .push_bind(truncate_chars(&row.ind_grp_dre, 1).to_string())
            .push_bind(truncate_opt_chars(&row.nota_exp_ref, 12).map(str::to_string))
            .push_bind(truncate_chars(&row.cnpj, 14).to_string())
            .push_bind(row.dt_ini)
            .push_bind(row.dt_fim)
            .push_bind(truncate_chars(&row.hash_arquivo, 40).to_string());
    });
    qb
}

/// Constrói query em lote para `core_registroj210`.
pub fn build_bulk_insert_j210_query(
    reg_0000_id: i64,
    chunk: &[RegistroJ210],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroj210 (
            ind_tip, cod_agl, descr_cod_agl, vl_cta_ini, ind_dc_cta_ini,
            vl_cta_fin, ind_dc_cta_fin, nota_exp_ref, cnpj, hash_arquivo_id, reg_0000_id
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk = if reg_0000_id > 0 {
            Some(reg_0000_id)
        } else {
            row.reg_0000_id
        };
        b.push_bind(truncate_chars(&row.ind_tip, 1).to_string())
            .push_bind(truncate_chars(&row.cod_agl, 255).to_string())
            .push_bind(truncate_chars(&row.descr_cod_agl, 255).to_string())
            .push_bind(row.vl_cta_ini)
            .push_bind(truncate_chars(&row.ind_dc_cta_ini, 1).to_string())
            .push_bind(row.vl_cta_fin)
            .push_bind(truncate_chars(&row.ind_dc_cta_fin, 1).to_string())
            .push_bind(truncate_opt_chars(&row.nota_exp_ref, 25).map(str::to_string))
            .push_bind(truncate_chars(&row.cnpj, 14).to_string())
            .push_bind(truncate_opt_chars(&row.hash_arquivo_id, 40).map(str::to_string))
            .push_bind(fk);
    });
    qb
}

/// Constrói query em lote para `core_registroj215`.
pub fn build_bulk_insert_j215_query(
    reg_0000_id: i64,
    chunk: &[RegistroJ215],
) -> QueryBuilder<'static, Postgres> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO core_registroj215 (
            cod_hist_fat, desc_fat, vl_fat_cont, ind_dc_fat, cnpj, hash_arquivo_id, reg_0000_id
        ) ",
    );
    qb.push_values(chunk, |mut b, row| {
        let fk = if reg_0000_id > 0 {
            Some(reg_0000_id)
        } else {
            row.reg_0000_id
        };
        b.push_bind(truncate_chars(&row.cod_hist_fat, 255).to_string())
            .push_bind(truncate_chars(&row.desc_fat, 255).to_string())
            .push_bind(row.vl_fat_cont)
            .push_bind(truncate_chars(&row.ind_dc_fat, 20).to_string())
            .push_bind(truncate_chars(&row.cnpj, 14).to_string())
            .push_bind(truncate_opt_chars(&row.hash_arquivo_id, 40).map(str::to_string))
            .push_bind(fk);
    });
    qb
}

/// Repositório PostgreSQL dedicado à ingestão em massa de arquivos SPED ECD.
#[derive(Clone, Debug)]
pub struct SpedRepository {
    pool: PgPool,
    batch_size: usize,
}

impl SpedRepository {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            batch_size: BATCH_SIZE,
        }
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Carrega o catálogo referencial da RFB (`plano_contas_base`) do PostgreSQL.
    pub async fn carregar_plano_contas_base(
        &self,
    ) -> Result<HashMap<String, String>, StorageError> {
        let rows = sqlx::query(
            "SELECT codigo_conta, COALESCE(nome_conta, '') AS nome_conta FROM plano_contas_base",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut mapa = HashMap::with_capacity(rows.len());
        for row in rows {
            let codigo: String = row.try_get("codigo_conta")?;
            let nome: String = row.try_get("nome_conta")?;
            if !codigo.is_empty() && !nome.is_empty() {
                mapa.insert(codigo, nome);
            }
        }
        Ok(mapa)
    }

    /// Retorna todos os hashes de escriturações substituídas (`cod_hash_sub`) para um CNPJ.
    pub async fn obter_hashes_substituidos(
        &self,
        cnpj: &str,
    ) -> Result<HashSet<String>, StorageError> {
        let rows = sqlx::query(
            "SELECT DISTINCT cod_hash_sub
             FROM core_registro0000
             WHERE cnpj = $1 AND cod_hash_sub IS NOT NULL AND cod_hash_sub <> ''",
        )
        .bind(truncate_chars(cnpj, 14))
        .fetch_all(&self.pool)
        .await?;

        let mut hashes = HashSet::with_capacity(rows.len());
        for row in rows {
            let h: String = row.try_get("cod_hash_sub")?;
            hashes.insert(h);
        }
        Ok(hashes)
    }

    /// Registra ou atualiza o início do processamento do arquivo na tabela `importacoes_ecd`.
    pub async fn registrar_inicio_importacao(
        &self,
        job: &ImportSpedJobPayload,
    ) -> Result<(), StorageError> {
        let id = if job.job_id.is_nil() {
            Uuid::now_v7()
        } else {
            job.job_id
        };
        sqlx::query(
            "INSERT INTO importacoes_ecd (
                id, hash_arquivo, cnpj, nome_arquivo_original,
                caminho_armazenamento, tamanho_bytes, status,
                linhas_processadas, tempo_processamento_ms, mensagem_erro,
                criado_em, atualizado_em
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, 0, 0, NULL, $8, NOW())
            ON CONFLICT (hash_arquivo) DO UPDATE SET
                id = EXCLUDED.id,
                cnpj = EXCLUDED.cnpj,
                nome_arquivo_original = EXCLUDED.nome_arquivo_original,
                caminho_armazenamento = EXCLUDED.caminho_armazenamento,
                tamanho_bytes = EXCLUDED.tamanho_bytes,
                status = EXCLUDED.status,
                mensagem_erro = NULL,
                atualizado_em = NOW()",
        )
        .bind(id)
        .bind(truncate_chars(&job.hash_arquivo, 40))
        .bind(truncate_chars(&job.cnpj, 14))
        .bind(truncate_chars(&job.nome_original, 255))
        .bind(truncate_chars(&job.caminho_arquivo, 500))
        .bind(job.tamanho_bytes)
        .bind(status_importacao_as_str(StatusImportacao::EmProcessamento))
        .bind(job.enfileirado_em)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Atualiza o registro em `importacoes_ecd` após conclusão com sucesso (ou substituição).
    #[allow(clippy::too_many_arguments)]
    pub async fn finalizar_importacao_sucesso(
        &self,
        job_id: Uuid,
        hash_arquivo: &str,
        cnpj: &str,
        caminho_final: &str,
        linhas_processadas: i32,
        tempo_ms: i32,
        ja_substituida: bool,
    ) -> Result<(), StorageError> {
        let status = if ja_substituida {
            StatusImportacao::Substituida
        } else {
            StatusImportacao::Sucesso
        };

        sqlx::query(
            "UPDATE importacoes_ecd
             SET cnpj = $1,
                 caminho_armazenamento = $2,
                 status = $3,
                 linhas_processadas = $4,
                 tempo_processamento_ms = $5,
                 mensagem_erro = NULL,
                 atualizado_em = NOW()
             WHERE id = $6 OR hash_arquivo = $7",
        )
        .bind(truncate_chars(cnpj, 14))
        .bind(truncate_chars(caminho_final, 500))
        .bind(status_importacao_as_str(status))
        .bind(linhas_processadas)
        .bind(tempo_ms)
        .bind(job_id)
        .bind(truncate_chars(hash_arquivo, 40))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Atualiza o registro em `importacoes_ecd` em caso de falha de processamento.
    pub async fn finalizar_importacao_erro(
        &self,
        job_id: Uuid,
        hash_arquivo: &str,
        caminho_final: &str,
        tempo_ms: i32,
        mensagem_erro: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE importacoes_ecd
             SET caminho_armazenamento = $1,
                 status = $2,
                 tempo_processamento_ms = $3,
                 mensagem_erro = $4,
                 atualizado_em = NOW()
             WHERE id = $5 OR hash_arquivo = $6",
        )
        .bind(truncate_chars(caminho_final, 500))
        .bind(status_importacao_as_str(StatusImportacao::Erro))
        .bind(tempo_ms)
        .bind(mensagem_erro)
        .bind(job_id)
        .bind(truncate_chars(hash_arquivo, 40))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Executa a inserção atômica em lote (`BATCH_SIZE = 5_000`) de todas as tabelas `core_*` e `reg*`.
    pub async fn bulk_insert_registros(
        &self,
        batch: &SpedBatchRecords,
    ) -> Result<BatchInsertSummary, StorageError> {
        let mut tx = self.pool.begin().await?;
        let summary = self.bulk_insert_in_tx(&mut tx, batch).await?;
        tx.commit().await?;
        Ok(summary)
    }

    /// Executa em uma única transação atômica:
    /// 1. Inserção em lote de todas as 16 tabelas `core_*` e `reg*`
    /// 2. Exclusão lógica de escriturações substituídas (`cod_hash_sub`)
    /// 3. Persistência da apuração fiscal em `resultados_exercicio`
    pub async fn persistir_ingestao_completa(
        &self,
        batch: &SpedBatchRecords,
        resultado: &ResultadoExercicio,
    ) -> Result<BatchInsertSummary, StorageError> {
        let mut tx = self.pool.begin().await?;
        let summary = self.bulk_insert_in_tx(&mut tx, batch).await?;

        let mut resultado_ajustado = resultado.clone();
        if summary.arquivo_atual_ja_substituido {
            resultado_ajustado.situacao = "X".to_string();
        }
        Self::salvar_resultado_exercicio_tx(&mut tx, &resultado_ajustado).await?;

        tx.commit().await?;
        Ok(summary)
    }

    /// Executa todas as inserções em lote dentro de uma transação SQLx aberta.
    pub async fn bulk_insert_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        batch: &SpedBatchRecords,
    ) -> Result<BatchInsertSummary, StorageError> {
        let hash_arquivo = truncate_chars(&batch.reg_0000.hash_arquivo_id, 40);
        let chunk_cap = self.batch_size.max(1);

        // Limpa eventuais registros anteriores do mesmo hash_arquivo para garantir idempotência
        Self::limpar_registros_por_hash_tx(tx, hash_arquivo).await?;

        // 1. Bloco 0000
        let reg_0000_id = Self::insert_registro_0000_tx(tx, &batch.reg_0000).await?;
        let mut total_inseridos = 1usize;

        // 2. Bloco I010
        total_inseridos +=
            Self::bulk_insert_i010_tx(tx, reg_0000_id, &batch.reg_i010, self.batch_size).await?;

        // 3. Bloco I030
        total_inseridos +=
            Self::bulk_insert_i030_tx(tx, reg_0000_id, &batch.reg_i030, self.batch_size).await?;

        // 4. Bloco I050 relacional (retorna os IDs BIGSERIAL gerados para vincular I051 e I052)
        let i050_db_ids =
            Self::bulk_insert_i050_tx(tx, reg_0000_id, &batch.reg_i050, self.batch_size).await?;
        total_inseridos += i050_db_ids.len();

        // 5. Bloco I051 (remapeando reg_i050_id em fatias de lote para evitar cópia integral em memória)
        if !batch.reg_i051.is_empty() && !i050_db_ids.is_empty() {
            let mut remapped_i051 = Vec::with_capacity(batch.reg_i051.len().min(chunk_cap));
            for raw_chunk in batch.reg_i051.chunks(chunk_cap) {
                remapped_i051.clear();
                for item in raw_chunk {
                    if let Some(real_i050_id) = resolve_parent_id(item.reg_i050_id, &i050_db_ids) {
                        let mut clone = item.clone();
                        clone.reg_0000_id = reg_0000_id;
                        clone.reg_i050_id = real_i050_id;
                        remapped_i051.push(clone);
                    }
                }
                total_inseridos +=
                    Self::bulk_insert_i051_tx(tx, reg_0000_id, &remapped_i051, self.batch_size)
                        .await?;
            }
        }

        // 6. Bloco I052 (remapeando reg_i050_id opcional em fatias de lote)
        if !batch.reg_i052.is_empty() {
            let mut remapped_i052 = Vec::with_capacity(batch.reg_i052.len().min(chunk_cap));
            for raw_chunk in batch.reg_i052.chunks(chunk_cap) {
                remapped_i052.clear();
                for item in raw_chunk {
                    let mut clone = item.clone();
                    clone.reg_0000_id = reg_0000_id;
                    clone.reg_i050_id = item
                        .reg_i050_id
                        .and_then(|raw_id| resolve_parent_id(raw_id, &i050_db_ids));
                    remapped_i052.push(clone);
                }
                total_inseridos +=
                    Self::bulk_insert_i052_tx(tx, reg_0000_id, &remapped_i052, self.batch_size)
                        .await?;
            }
        }

        // 7. Bloco I053
        total_inseridos +=
            Self::bulk_insert_i053_tx(tx, reg_0000_id, &batch.reg_i053, self.batch_size).await?;

        // 8. RegI050 Consolidado (regi050_plano_contas)
        total_inseridos +=
            Self::bulk_insert_reg_i050_tx(tx, &batch.plano_contas, self.batch_size).await?;

        // 9. Bloco I150 (retorna os IDs BIGSERIAL gerados para vincular I155)
        let i150_db_ids =
            Self::bulk_insert_i150_tx(tx, reg_0000_id, &batch.reg_i150, self.batch_size).await?;
        total_inseridos += i150_db_ids.len();

        // 10. Bloco I155 (remapeando reg_i150_id em fatias de lote sem duplicar o vetor completo)
        if !batch.reg_i155.is_empty() && !i150_db_ids.is_empty() {
            let mut remapped_i155 = Vec::with_capacity(batch.reg_i155.len().min(chunk_cap));
            for raw_chunk in batch.reg_i155.chunks(chunk_cap) {
                remapped_i155.clear();
                for item in raw_chunk {
                    if let Some(real_i150_id) = resolve_parent_id(item.reg_i150_id, &i150_db_ids) {
                        let mut clone = item.clone();
                        clone.reg_0000_id = reg_0000_id;
                        clone.reg_i150_id = real_i150_id;
                        remapped_i155.push(clone);
                    }
                }
                total_inseridos +=
                    Self::bulk_insert_i155_tx(tx, reg_0000_id, &remapped_i155, self.batch_size)
                        .await?;
            }
        }

        // 11. RegI355 (regi355_detalhes_contas_resultado)
        total_inseridos +=
            Self::bulk_insert_reg_i355_tx(tx, &batch.reg_i355, self.batch_size).await?;

        // 12. Bloco J005 (retorna os IDs BIGSERIAL gerados para vincular J100)
        let j005_db_ids =
            Self::bulk_insert_j005_tx(tx, reg_0000_id, &batch.reg_j005, self.batch_size).await?;
        total_inseridos += j005_db_ids.len();

        // 13. Bloco J100 (remapeando reg_j005_id em fatias de lote)
        if !batch.reg_j100.is_empty() && !j005_db_ids.is_empty() {
            let mut remapped_j100 = Vec::with_capacity(batch.reg_j100.len().min(chunk_cap));
            for raw_chunk in batch.reg_j100.chunks(chunk_cap) {
                remapped_j100.clear();
                for item in raw_chunk {
                    if let Some(real_j005_id) = resolve_parent_id(item.reg_j005_id, &j005_db_ids) {
                        let mut clone = item.clone();
                        clone.reg_0000_id = reg_0000_id;
                        clone.reg_j005_id = real_j005_id;
                        remapped_j100.push(clone);
                    }
                }
                total_inseridos +=
                    Self::bulk_insert_j100_tx(tx, reg_0000_id, &remapped_j100, self.batch_size)
                        .await?;
            }
        }

        // 14. RegJ150 (regj150_demonstracao_resultado)
        total_inseridos +=
            Self::bulk_insert_reg_j150_tx(tx, &batch.reg_j150, self.batch_size).await?;

        // 15. Bloco J210
        total_inseridos +=
            Self::bulk_insert_j210_tx(tx, reg_0000_id, &batch.reg_j210, self.batch_size).await?;

        // 16. Bloco J215
        total_inseridos +=
            Self::bulk_insert_j215_tx(tx, reg_0000_id, &batch.reg_j215, self.batch_size).await?;

        // 17. Exclusão lógica de escrituração substituída (cod_hash_sub)
        let escrituracao_substituida_hash = batch
            .reg_0000
            .cod_hash_sub
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        if let Some(ref hash_sub) = escrituracao_substituida_hash {
            Self::excluir_logicamente_substituida_tx(tx, hash_sub).await?;
        }

        let arquivo_atual_ja_substituido =
            Self::verificar_hash_ja_substituido_tx(tx, hash_arquivo).await?;
        if arquivo_atual_ja_substituido {
            Self::excluir_logicamente_substituida_tx(tx, hash_arquivo).await?;
        }

        Ok(BatchInsertSummary {
            reg_0000_id,
            total_registros_inseridos: total_inseridos,
            escrituracao_substituida_hash,
            arquivo_atual_ja_substituido,
        })
    }

    /// Remove registros anteriores do mesmo `hash_arquivo` antes de reinserir.
    pub async fn limpar_registros_por_hash_tx(
        tx: &mut Transaction<'_, Postgres>,
        hash_arquivo: &str,
    ) -> Result<(), StorageError> {
        let h = truncate_chars(hash_arquivo, 40);
        // ON DELETE CASCADE limpa as tabelas core_registro* filhas de core_registro0000
        sqlx::query("DELETE FROM core_registro0000 WHERE hash_arquivo_id = $1")
            .bind(h)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM regi050_plano_contas WHERE hash_arquivo = $1")
            .bind(h)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM regi355_detalhes_contas_resultado WHERE hash_arquivo = $1")
            .bind(h)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM regj150_demonstracao_resultado WHERE hash_arquivo = $1")
            .bind(h)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// Marca logicamente uma escrituração substituída (`cod_hash_sub`) como `Substituida` em
    /// `importacoes_ecd` e `situacao = 'X'` em `resultados_exercicio` (compatível com `WHERE situacao != 'X'`).
    pub async fn excluir_logicamente_substituida_tx(
        tx: &mut Transaction<'_, Postgres>,
        cod_hash_sub: &str,
    ) -> Result<(), StorageError> {
        let h = truncate_chars(cod_hash_sub, 40);
        sqlx::query(
            "UPDATE importacoes_ecd
             SET status = $1, atualizado_em = NOW()
             WHERE hash_arquivo = $2",
        )
        .bind(status_importacao_as_str(StatusImportacao::Substituida))
        .bind(h)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            "UPDATE resultados_exercicio
             SET situacao = 'X'
             WHERE hash_arquivo = $1",
        )
        .bind(h)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// Verifica se o `hash_arquivo` atual já consta como `cod_hash_sub` de outra escrituração no banco.
    pub async fn verificar_hash_ja_substituido_tx(
        tx: &mut Transaction<'_, Postgres>,
        hash_arquivo: &str,
    ) -> Result<bool, StorageError> {
        let row = sqlx::query(
            "SELECT EXISTS(
                SELECT 1 FROM core_registro0000 WHERE cod_hash_sub = $1
            ) AS substituido",
        )
        .bind(truncate_chars(hash_arquivo, 40))
        .fetch_one(&mut **tx)
        .await?;

        let substituido: bool = row.try_get("substituido")?;
        Ok(substituido)
    }

    /// Persiste ou atualiza a apuração fiscal na tabela `resultados_exercicio`.
    pub async fn salvar_resultado_exercicio_tx(
        tx: &mut Transaction<'_, Postgres>,
        res: &ResultadoExercicio,
    ) -> Result<(), StorageError> {
        let id = if res.id.is_nil() {
            Uuid::now_v7()
        } else {
            res.id
        };
        let hash = truncate_chars(&res.hash_arquivo, 40);

        // Remove eventual linha anterior do mesmo hash_arquivo para evitar colisão na constraint UNIQUE(hash_arquivo)
        sqlx::query("DELETE FROM resultados_exercicio WHERE hash_arquivo = $1")
            .bind(hash)
            .execute(&mut **tx)
            .await?;

        if res.situacao != "S" {
            // Se este arquivo já foi substituído por um mais recente (situacao = 'X'), não sobrescreve o resultado ativo do período
            sqlx::query(
                "INSERT INTO resultados_exercicio (
                    id, cnpj, razao_social, periodo_inicial, periodo_final, cnpj_scp,
                    receita_bruta, receita_servico, outras_receitas, receita_acumulada, total_nota,
                    hash_arquivo, cod_hash_sub, situacao, detalhes_j150, detalhes_i355, criado_em
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                ON CONFLICT ON CONSTRAINT unq_resultado_cnpj_periodo DO NOTHING",
            )
            .bind(id)
            .bind(truncate_chars(&res.cnpj, 14))
            .bind(truncate_chars(&res.razao_social, 200))
            .bind(res.periodo_inicial)
            .bind(res.periodo_final)
            .bind(truncate_opt_chars(&res.cnpj_scp, 14))
            .bind(res.receita_bruta)
            .bind(res.receita_servico)
            .bind(res.outras_receitas)
            .bind(res.receita_acumulada)
            .bind(res.total_nota)
            .bind(hash)
            .bind(truncate_opt_chars(&res.cod_hash_sub, 40))
            .bind(truncate_chars(&res.situacao, 5))
            .bind(&res.detalhes_j150)
            .bind(&res.detalhes_i355)
            .bind(res.criado_em)
            .execute(&mut **tx)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO resultados_exercicio (
                    id, cnpj, razao_social, periodo_inicial, periodo_final, cnpj_scp,
                    receita_bruta, receita_servico, outras_receitas, receita_acumulada, total_nota,
                    hash_arquivo, cod_hash_sub, situacao, detalhes_j150, detalhes_i355, criado_em
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                ON CONFLICT ON CONSTRAINT unq_resultado_cnpj_periodo DO UPDATE SET
                    razao_social = EXCLUDED.razao_social,
                    receita_bruta = EXCLUDED.receita_bruta,
                    receita_servico = EXCLUDED.receita_servico,
                    outras_receitas = EXCLUDED.outras_receitas,
                    receita_acumulada = EXCLUDED.receita_acumulada,
                    total_nota = EXCLUDED.total_nota,
                    hash_arquivo = EXCLUDED.hash_arquivo,
                    cod_hash_sub = EXCLUDED.cod_hash_sub,
                    situacao = EXCLUDED.situacao,
                    detalhes_j150 = EXCLUDED.detalhes_j150,
                    detalhes_i355 = EXCLUDED.detalhes_i355,
                    criado_em = EXCLUDED.criado_em",
            )
            .bind(id)
            .bind(truncate_chars(&res.cnpj, 14))
            .bind(truncate_chars(&res.razao_social, 200))
            .bind(res.periodo_inicial)
            .bind(res.periodo_final)
            .bind(truncate_opt_chars(&res.cnpj_scp, 14))
            .bind(res.receita_bruta)
            .bind(res.receita_servico)
            .bind(res.outras_receitas)
            .bind(res.receita_acumulada)
            .bind(res.total_nota)
            .bind(hash)
            .bind(truncate_opt_chars(&res.cod_hash_sub, 40))
            .bind(truncate_chars(&res.situacao, 5))
            .bind(&res.detalhes_j150)
            .bind(&res.detalhes_i355)
            .bind(res.criado_em)
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

    pub async fn insert_registro_0000_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg: &Registro0000,
    ) -> Result<i64, StorageError> {
        let mut qb = build_insert_0000_query(reg);
        let row = qb.build().fetch_one(&mut **tx).await?;
        let id: i64 = row.try_get("id")?;
        Ok(id)
    }

    pub async fn bulk_insert_i010_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroI010],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 3);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_i010_query(reg_0000_id, chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_i030_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroI030],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 12);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_i030_query(reg_0000_id, chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_i050_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroI050],
        batch_size: usize,
    ) -> Result<Vec<i64>, StorageError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let step = effective_chunk_size(batch_size, 11);
        let mut ids = Vec::with_capacity(records.len());
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_i050_query(reg_0000_id, chunk);
            let rows = qb.build().fetch_all(&mut **tx).await?;
            for row in rows {
                ids.push(row.try_get("id")?);
            }
        }
        Ok(ids)
    }

    pub async fn bulk_insert_i051_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroI051],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 4);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_i051_query(reg_0000_id, chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_i052_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroI052],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 4);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_i052_query(reg_0000_id, chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_i053_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroI053],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 4);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_i053_query(reg_0000_id, chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_reg_i050_tx(
        tx: &mut Transaction<'_, Postgres>,
        records: &[RegI050],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 19);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_reg_i050_query(chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_i150_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroI150],
        batch_size: usize,
    ) -> Result<Vec<i64>, StorageError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let step = effective_chunk_size(batch_size, 3);
        let mut ids = Vec::with_capacity(records.len());
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_i150_query(reg_0000_id, chunk);
            let rows = qb.build().fetch_all(&mut **tx).await?;
            for row in rows {
                ids.push(row.try_get("id")?);
            }
        }
        Ok(ids)
    }

    pub async fn bulk_insert_i155_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroI155],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 10);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_i155_query(reg_0000_id, chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_reg_i355_tx(
        tx: &mut Transaction<'_, Postgres>,
        records: &[RegI355],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 14);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_reg_i355_query(chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_j005_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroJ005],
        batch_size: usize,
    ) -> Result<Vec<i64>, StorageError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let step = effective_chunk_size(batch_size, 7);
        let mut ids = Vec::with_capacity(records.len());
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_j005_query(reg_0000_id, chunk);
            let rows = qb.build().fetch_all(&mut **tx).await?;
            for row in rows {
                ids.push(row.try_get("id")?);
            }
        }
        Ok(ids)
    }

    pub async fn bulk_insert_j100_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroJ100],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 15);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_j100_query(reg_0000_id, chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_reg_j150_tx(
        tx: &mut Transaction<'_, Postgres>,
        records: &[RegJ150],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 16);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_reg_j150_query(chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_j210_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroJ210],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 11);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_j210_query(reg_0000_id, chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }

    pub async fn bulk_insert_j215_tx(
        tx: &mut Transaction<'_, Postgres>,
        reg_0000_id: i64,
        records: &[RegistroJ215],
        batch_size: usize,
    ) -> Result<usize, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let step = effective_chunk_size(batch_size, 7);
        let mut inserted = 0usize;
        for chunk in records.chunks(step) {
            let mut qb = build_bulk_insert_j215_query(reg_0000_id, chunk);
            let res = qb.build().execute(&mut **tx).await?;
            inserted += res.rows_affected() as usize;
        }
        Ok(inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::types::chrono::{NaiveDate, Utc};
    use sqlx::types::Decimal;

    fn sample_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()
    }

    #[test]
    fn test_effective_chunk_size_respeita_5000_e_limite_pg() {
        assert_eq!(BATCH_SIZE, 5_000);
        // Para 10 colunas (ex: I155), 5.000 * 10 = 50.000 <= 65.535 -> mantém 5.000 linhas exatas
        assert_eq!(effective_chunk_size(BATCH_SIZE, 10), 5_000);
        // Para 11 colunas (ex: I050), 5.000 * 11 = 55.000 <= 65.535 -> mantém 5.000 linhas exatas
        assert_eq!(effective_chunk_size(BATCH_SIZE, 11), 5_000);
        // Para 19 colunas (ex: RegI050), limita a 65535 / 19 = 3449 para nunca estourar u16::MAX no Postgres
        assert_eq!(effective_chunk_size(BATCH_SIZE, 19), 3_449);
        assert!(effective_chunk_size(BATCH_SIZE, 19) * 19 <= MAX_PG_BIND_PARAMS);
        assert_eq!(effective_chunk_size(5_000, 0), 5_000);
    }

    #[test]
    fn test_truncate_chars_utf8_seguro() {
        let texto = " AÇÃO CONTÁBIL DE EXPORTAÇÃO ";
        let truncado = truncate_chars(texto, 5);
        assert_eq!(truncado.chars().count(), 5);
        assert_eq!(truncado, " AÇÃO");
        assert_eq!(truncate_chars("ABC", 10), "ABC");
    }

    #[test]
    fn test_resolve_parent_id_remapping() {
        let db_ids = vec![101, 102, 103];
        // Índices locais 1-based vindos do SpedDocument
        assert_eq!(resolve_parent_id(1, &db_ids), Some(101));
        assert_eq!(resolve_parent_id(2, &db_ids), Some(102));
        assert_eq!(resolve_parent_id(3, &db_ids), Some(103));
        // Se já veio com o ID real do banco
        assert_eq!(resolve_parent_id(102, &db_ids), Some(102));
        // Fallback para último ID quando fora de faixa
        assert_eq!(resolve_parent_id(999, &db_ids), Some(103));
        assert_eq!(resolve_parent_id(1, &[]), None);
    }

    #[test]
    fn test_query_builder_todas_as_16_tabelas_core_e_reg() {
        let dt = sample_date();

        let r0000 = Registro0000 {
            id: None,
            dt_ini: dt,
            dt_fim: dt,
            nome_razao: "EMPRESA TESTE LTDA".into(),
            cnpj: "12345678000195".into(),
            uf: "SP".into(),
            ie: None,
            cod_mun: Some(3550308),
            im: None,
            ind_sit_esp: Some(0),
            ind_sit_ini_per: 0,
            ind_nire: 1,
            ind_fin_esc: 1,
            cod_hash_sub: Some("HASH_ANTERIOR_123".into()),
            ind_grande_porte: 0,
            tip_ecd: 0,
            cod_scp: None,
            ident_mf: "N".into(),
            ind_esc_cons: "N".into(),
            ind_centralizada: 0,
            ind_mudanc_pc: 0,
            cod_plan_ref: Some("1".into()),
            i990: 100,
            j990: 50,
            hash_arquivo_id: "HASH_ATUAL_456".into(),
        };
        let q0000 = build_insert_0000_query(&r0000);
        assert!(q0000.sql().contains("INSERT INTO core_registro0000"));
        assert!(q0000.sql().contains("RETURNING id"));

        let qi010 = build_bulk_insert_i010_query(
            10,
            &[RegistroI010 {
                id: None,
                ind_esc: "G".into(),
                cod_ver_lc: "9.00".into(),
                reg_0000_id: 0,
            }],
        );
        assert!(qi010.sql().contains("INSERT INTO core_registroi010"));

        let qi030 = build_bulk_insert_i030_query(
            10,
            &[RegistroI030 {
                id: None,
                num_ord: 1,
                nat_livr: "DIARIO GERAL".into(),
                qtd_lin: 150,
                nome: "EMPRESA TESTE LTDA".into(),
                nire: "35200000001".into(),
                cnpj: "12345678000195".into(),
                dt_arq: Some(dt),
                dt_arq_conv: None,
                desc_mun: Some("SAO PAULO".into()),
                dt_ex_social: dt,
                reg_0000_id: 0,
                hash_arquivo_id: Some("HASH_ATUAL_456".into()),
            }],
        );
        assert!(qi030.sql().contains("INSERT INTO core_registroi030"));

        let qi050 = build_bulk_insert_i050_query(
            10,
            &[RegistroI050 {
                id: Some(1),
                cod_cta: "1.01".into(),
                cta: "CAIXA GERAL".into(),
                nivel: 2,
                ind_cta: "A".into(),
                cod_nat: "01".into(),
                cod_cta_sup: Some("1".into()),
                dt_alt: dt,
                descr_cta: "CAIXA GERAL".into(),
                descr_conta: Some("CAIXA GERAL".into()),
                codigo_conta: Some("1.01".into()),
                reg_0000_id: 0,
            }],
        );
        assert!(qi050.sql().contains("INSERT INTO core_registroi050"));
        assert!(qi050.sql().contains("RETURNING id"));

        let qi051 = build_bulk_insert_i051_query(
            10,
            &[RegistroI051 {
                id: None,
                cod_ccus: None,
                cod_cta_ref: "1.01.01.01.01".into(),
                reg_0000_id: 10,
                reg_i050_id: 99,
            }],
        );
        assert!(qi051.sql().contains("INSERT INTO core_registroi051"));

        let qi052 = build_bulk_insert_i052_query(
            10,
            &[RegistroI052 {
                id: None,
                cod_ccus: None,
                cod_agl: "AGL01".into(),
                reg_0000_id: 10,
                reg_i050_id: Some(99),
            }],
        );
        assert!(qi052.sql().contains("INSERT INTO core_registroi052"));

        let qi053 = build_bulk_insert_i053_query(
            10,
            &[RegistroI053 {
                id: None,
                cod_idt: "001".into(),
                cod_cnt_corr: "1.01.99".into(),
                nat_sub_cnt: "01".into(),
                reg_0000_id: 10,
            }],
        );
        assert!(qi053.sql().contains("INSERT INTO core_registroi053"));

        let qreg_i050 = build_bulk_insert_reg_i050_query(&[RegI050 {
            id: Uuid::now_v7(),
            cod_cta: "1.01".into(),
            cta: "CAIXA".into(),
            nivel: 2,
            ind_cta: "A".into(),
            cod_nat: "01".into(),
            cod_cta_sup: Some("1".into()),
            dt_alt: dt,
            cod_ccus_51: None,
            cod_cta_ref: Some("1.01.01".into()),
            cod_ccus_52: None,
            cod_agl: Some("AGL01".into()),
            cnpj: "12345678000195".into(),
            data_entrega: Utc::now(),
            cta_pai: Some("ATIVO".into()),
            nome_cta_referencia: Some("CAIXA MATRIZ".into()),
            reg051_json: None,
            reg052_json: None,
            hash_arquivo: "HASH_ATUAL_456".into(),
        }]);
        assert!(qreg_i050.sql().contains("INSERT INTO regi050_plano_contas"));

        let qi150 = build_bulk_insert_i150_query(
            10,
            &[RegistroI150 {
                id: Some(1),
                dt_ini: dt,
                dt_fim: dt,
                reg_0000_id: 10,
            }],
        );
        assert!(qi150.sql().contains("INSERT INTO core_registroi150"));
        assert!(qi150.sql().contains("RETURNING id"));

        // Lote de 5.000 linhas em core_registroi155
        let lote_5000_i155: Vec<RegistroI155> = (0..5_000)
            .map(|idx| RegistroI155 {
                id: None,
                cod_cta: format!("1.01.{idx}"),
                cod_ccus: None,
                vl_sld_ini: Decimal::ZERO,
                ind_dc_ini: "D".into(),
                vl_deb: Decimal::from(100),
                vl_cred: Decimal::from(50),
                vl_sld_fim: Decimal::from(50),
                ind_dc_fim: "D".into(),
                reg_0000_id: 10,
                reg_i150_id: 20,
            })
            .collect();
        let qi155 = build_bulk_insert_i155_query(10, &lote_5000_i155);
        assert!(qi155.sql().contains("INSERT INTO core_registroi155"));
        assert!(qi155.sql().contains("vl_sld_fim, ind_dc_fim"));
        // Último bind de 5.000 linhas x 10 colunas deve ser $50000
        assert!(qi155.sql().contains("$50000)"));

        let qreg_i355 = build_bulk_insert_reg_i355_query(&[RegI355 {
            id: Uuid::now_v7(),
            cnpj: "12345678000195".into(),
            cod_cta: "3.01.01".into(),
            nome_cta_referencia: Some("RECEITA BRUTA".into()),
            cod_ccus: None,
            vl_cta: Decimal::from(10000),
            ind_dc: "C".into(),
            vl_cta_mf: None,
            ind_dc_mf: None,
            dt_res: dt,
            cod_agl: Some("AGL_REC".into()),
            cod_cta_ref: Some("3.01.01.01".into()),
            nome_aglutinador: Some("RECEITA OPERACIONAL BRUTA".into()),
            hash_arquivo: "HASH_ATUAL_456".into(),
        }]);
        assert!(qreg_i355
            .sql()
            .contains("INSERT INTO regi355_detalhes_contas_resultado"));

        let qj005 = build_bulk_insert_j005_query(
            10,
            &[RegistroJ005 {
                id: Some(1),
                dt_ini: dt,
                dt_fim: dt,
                id_dem: 1,
                cab_dem: None,
                cnpj: "12345678000195".into(),
                hash_arquivo_id: Some("HASH_ATUAL_456".into()),
                reg_0000_id: 10,
            }],
        );
        assert!(qj005.sql().contains("INSERT INTO core_registroj005"));
        assert!(qj005.sql().contains("RETURNING id"));

        let qj100 = build_bulk_insert_j100_query(
            10,
            &[RegistroJ100 {
                id: None,
                cod_agl: "1".into(),
                ind_cod_agl: "T".into(),
                nivel_agl: 1,
                cod_agl_sup: None,
                ind_grp_bal: "A".into(),
                descr_cod_agl: "ATIVO TOTAL".into(),
                vl_cta_ini: Decimal::from(1000),
                ind_dc_cta_ini: "D".into(),
                vl_cta_fin: Decimal::from(2000),
                ind_dc_cta_fin: "D".into(),
                nota_exp_ref: None,
                cnpj: "12345678000195".into(),
                hash_arquivo_id: Some("HASH_ATUAL_456".into()),
                reg_0000_id: 10,
                reg_j005_id: 30,
            }],
        );
        assert!(qj100.sql().contains("INSERT INTO core_registroj100"));

        let qreg_j150 = build_bulk_insert_reg_j150_query(&[RegJ150 {
            id: None,
            nu_ordem: 1,
            cod_agl: Some("3.01".into()),
            ind_cod_agl: "T".into(),
            nivel_agl: 1,
            cod_agl_sup: None,
            descr_cod_agl: "RECEITA BRUTA DE VENDAS".into(),
            vl_cta_ini: Decimal::ZERO,
            ind_dc_cta_ini: Some("C".into()),
            vl_cta_fin: Decimal::from(250000),
            ind_dc_cta_fin: "C".into(),
            ind_grp_dre: "R".into(),
            nota_exp_ref: None,
            cnpj: "12345678000195".into(),
            dt_ini: Some(dt),
            dt_fim: Some(dt),
            hash_arquivo: "HASH_ATUAL_456".into(),
        }]);
        assert!(qreg_j150
            .sql()
            .contains("INSERT INTO regj150_demonstracao_resultado"));

        let qj210 = build_bulk_insert_j210_query(
            10,
            &[RegistroJ210 {
                id: None,
                ind_tip: "0".into(),
                cod_agl: "PL1".into(),
                descr_cod_agl: "LUCROS ACUMULADOS".into(),
                vl_cta_ini: Decimal::ZERO,
                ind_dc_cta_ini: "C".into(),
                vl_cta_fin: Decimal::from(50000),
                ind_dc_cta_fin: "C".into(),
                nota_exp_ref: None,
                cnpj: "12345678000195".into(),
                hash_arquivo_id: Some("HASH_ATUAL_456".into()),
                reg_0000_id: Some(10),
            }],
        );
        assert!(qj210.sql().contains("INSERT INTO core_registroj210"));

        let qj215 = build_bulk_insert_j215_query(
            10,
            &[RegistroJ215 {
                id: None,
                cod_hist_fat: "H01".into(),
                desc_fat: "DESTINACAO DE LUCRO".into(),
                vl_fat_cont: Decimal::from(50000),
                ind_dc_fat: "C".into(),
                cnpj: "12345678000195".into(),
                hash_arquivo_id: Some("HASH_ATUAL_456".into()),
                reg_0000_id: Some(10),
            }],
        );
        assert!(qj215.sql().contains("INSERT INTO core_registroj215"));
    }

    struct EphemeralPg {
        data_dir: std::path::PathBuf,
        database_url: String,
    }

    impl EphemeralPg {
        fn start() -> Option<Self> {
            use std::net::TcpListener;
            use std::process::{Command, Stdio};

            let listener = TcpListener::bind("127.0.0.1:0").ok()?;
            let port = listener.local_addr().ok()?.port();
            drop(listener);

            let data_dir =
                std::env::temp_dir().join(format!("pg_ecd_storage_test_{}", Uuid::now_v7()));
            std::fs::create_dir_all(&data_dir).ok()?;

            let init_ok = Command::new("initdb")
                .args([
                    "-D",
                    data_dir.to_str()?,
                    "-U",
                    "postgres",
                    "-A",
                    "trust",
                    "--no-sync",
                    "--encoding=UTF8",
                    "--locale=C",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if !init_ok {
                let _ = std::fs::remove_dir_all(&data_dir);
                return None;
            }

            let log_file = data_dir.join("pg.log");
            let opts = format!("-F -p {port} -k {} -h 127.0.0.1", data_dir.display());
            let start_ok = Command::new("pg_ctl")
                .args([
                    "-D",
                    data_dir.to_str()?,
                    "-l",
                    log_file.to_str()?,
                    "-o",
                    &opts,
                    "start",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if !start_ok {
                let _ = std::fs::remove_dir_all(&data_dir);
                return None;
            }

            let migration_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()?
                .parent()?
                .join("migrations/0001_initial_schema.sql");

            let mig_ok = Command::new("psql")
                .args([
                    "-h",
                    "127.0.0.1",
                    "-p",
                    &port.to_string(),
                    "-U",
                    "postgres",
                    "-d",
                    "postgres",
                    "-v",
                    "ON_ERROR_STOP=1",
                    "-f",
                    migration_path.to_str()?,
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if !mig_ok {
                let _ = Command::new("pg_ctl")
                    .args([
                        "-D",
                        data_dir.to_str().unwrap_or(""),
                        "-m",
                        "immediate",
                        "stop",
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                let _ = std::fs::remove_dir_all(&data_dir);
                return None;
            }

            Some(Self {
                data_dir,
                database_url: format!("postgres://postgres@127.0.0.1:{port}/postgres"),
            })
        }
    }

    impl Drop for EphemeralPg {
        fn drop(&mut self) {
            if let Some(dir_str) = self.data_dir.to_str() {
                let _ = std::process::Command::new("pg_ctl")
                    .args(["-D", dir_str, "-m", "immediate", "stop"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
            let _ = std::fs::remove_dir_all(&self.data_dir);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn criar_batch_completo(
        cnpj: &str,
        hash_arquivo: &str,
        cod_hash_sub: Option<&str>,
        dt_ini: NaiveDate,
        dt_fim: NaiveDate,
        qtd_reg_i050: usize,
        qtd_i155: usize,
        placeholder_reg_0000_id: i64,
    ) -> SpedBatchRecords {
        let reg_0000 = Registro0000 {
            id: None,
            dt_ini,
            dt_fim,
            nome_razao: "EMPRESA INTEGRACAO POSTGRES S.A.".into(),
            cnpj: cnpj.into(),
            uf: "SP".into(),
            ie: Some("110042490114".into()),
            cod_mun: Some(3550308),
            im: None,
            ind_sit_esp: Some(0),
            ind_sit_ini_per: 0,
            ind_nire: 1,
            ind_fin_esc: if cod_hash_sub.is_some() { 1 } else { 0 },
            cod_hash_sub: cod_hash_sub.map(str::to_string),
            ind_grande_porte: 0,
            tip_ecd: 0,
            cod_scp: None,
            ident_mf: "N".into(),
            ind_esc_cons: "N".into(),
            ind_centralizada: 0,
            ind_mudanc_pc: 0,
            cod_plan_ref: Some("1".into()),
            i990: 100,
            j990: 50,
            hash_arquivo_id: hash_arquivo.into(),
        };

        let reg_i010 = vec![RegistroI010 {
            id: None,
            ind_esc: "G".into(),
            cod_ver_lc: "9.00".into(),
            reg_0000_id: placeholder_reg_0000_id,
        }];

        let reg_i030 = vec![RegistroI030 {
            id: None,
            num_ord: 1,
            nat_livr: "DIARIO GERAL".into(),
            qtd_lin: 200,
            nome: "EMPRESA INTEGRACAO POSTGRES S.A.".into(),
            nire: "35212345678".into(),
            cnpj: cnpj.into(),
            dt_arq: Some(dt_ini),
            dt_arq_conv: None,
            desc_mun: Some("SAO PAULO".into()),
            dt_ex_social: dt_fim,
            reg_0000_id: placeholder_reg_0000_id,
            hash_arquivo_id: Some(hash_arquivo.into()),
        }];

        let reg_i050 = vec![
            RegistroI050 {
                id: Some(1),
                cod_cta: "3".into(),
                cta: "RECEITAS".into(),
                nivel: 1,
                ind_cta: "S".into(),
                cod_nat: "04".into(),
                cod_cta_sup: None,
                dt_alt: dt_ini,
                descr_cta: "RECEITAS".into(),
                descr_conta: Some("RECEITAS".into()),
                codigo_conta: Some("3".into()),
                reg_0000_id: placeholder_reg_0000_id,
            },
            RegistroI050 {
                id: Some(2),
                cod_cta: "3.01".into(),
                cta: "RECEITA BRUTA DE VENDAS".into(),
                nivel: 2,
                ind_cta: "A".into(),
                cod_nat: "04".into(),
                cod_cta_sup: Some("3".into()),
                dt_alt: dt_ini,
                descr_cta: "RECEITA BRUTA DE VENDAS".into(),
                descr_conta: Some("RECEITA BRUTA DE VENDAS".into()),
                codigo_conta: Some("3.01".into()),
                reg_0000_id: placeholder_reg_0000_id,
            },
        ];

        let reg_i051 = vec![RegistroI051 {
            id: None,
            cod_ccus: None,
            cod_cta_ref: "3.01.01.01.01.06".into(),
            reg_0000_id: placeholder_reg_0000_id,
            reg_i050_id: 2,
        }];

        let reg_i052 = vec![RegistroI052 {
            id: None,
            cod_ccus: None,
            cod_agl: "AGL_REC_01".into(),
            reg_0000_id: placeholder_reg_0000_id,
            reg_i050_id: Some(2),
        }];

        let reg_i053 = vec![RegistroI053 {
            id: None,
            cod_idt: "001".into(),
            cod_cnt_corr: "3.01.99".into(),
            nat_sub_cnt: "04".into(),
            reg_0000_id: placeholder_reg_0000_id,
        }];

        let plano_contas: Vec<RegI050> = (0..qtd_reg_i050)
            .map(|idx| RegI050 {
                id: Uuid::now_v7(),
                cod_cta: format!("3.01.{idx}"),
                cta: format!("CONTA ANALITICA {idx}"),
                nivel: 3,
                ind_cta: "A".into(),
                cod_nat: "04".into(),
                cod_cta_sup: Some("3.01".into()),
                dt_alt: dt_ini,
                cod_ccus_51: None,
                cod_cta_ref: Some("3.01.01.01.01.06".into()),
                cod_ccus_52: None,
                cod_agl: Some("AGL_REC_01".into()),
                cnpj: cnpj.into(),
                data_entrega: Utc::now(),
                cta_pai: Some("RECEITAS".into()),
                nome_cta_referencia: Some("RECEITA DE SERVICOS".into()),
                reg051_json: None,
                reg052_json: None,
                hash_arquivo: hash_arquivo.into(),
            })
            .collect();

        let reg_i150 = vec![RegistroI150 {
            id: Some(1),
            dt_ini,
            dt_fim,
            reg_0000_id: placeholder_reg_0000_id,
        }];

        let reg_i155: Vec<RegistroI155> = (0..qtd_i155)
            .map(|idx| RegistroI155 {
                id: None,
                cod_cta: format!("3.01.{idx}"),
                cod_ccus: None,
                vl_sld_ini: Decimal::ZERO,
                ind_dc_ini: "C".into(),
                vl_deb: Decimal::from(10),
                vl_cred: Decimal::from(110),
                vl_sld_fim: Decimal::from(100),
                ind_dc_fim: "C".into(),
                reg_0000_id: placeholder_reg_0000_id,
                reg_i150_id: 1,
            })
            .collect();

        let reg_i355 = vec![RegI355 {
            id: Uuid::now_v7(),
            cnpj: cnpj.into(),
            cod_cta: "3.01".into(),
            nome_cta_referencia: Some("RECEITA BRUTA DE VENDAS".into()),
            cod_ccus: None,
            vl_cta: Decimal::from(500_000),
            ind_dc: "C".into(),
            vl_cta_mf: Some(Decimal::from(500_000)),
            ind_dc_mf: Some("C".into()),
            dt_res: dt_fim,
            cod_agl: Some("AGL_REC_01".into()),
            cod_cta_ref: Some("3.01.01.01.01.06".into()),
            nome_aglutinador: Some("RECEITA BRUTA OPERACIONAL".into()),
            hash_arquivo: hash_arquivo.into(),
        }];

        let reg_j005 = vec![RegistroJ005 {
            id: Some(1),
            dt_ini,
            dt_fim,
            id_dem: 1,
            cab_dem: Some("DEMONSTRACOES CONTABEIS".into()),
            cnpj: cnpj.into(),
            hash_arquivo_id: Some(hash_arquivo.into()),
            reg_0000_id: placeholder_reg_0000_id,
        }];

        let reg_j100 = vec![RegistroJ100 {
            id: None,
            cod_agl: "1".into(),
            ind_cod_agl: "T".into(),
            nivel_agl: 1,
            cod_agl_sup: None,
            ind_grp_bal: "A".into(),
            descr_cod_agl: "ATIVO TOTAL".into(),
            vl_cta_ini: Decimal::from(100_000),
            ind_dc_cta_ini: "D".into(),
            vl_cta_fin: Decimal::from(600_000),
            ind_dc_cta_fin: "D".into(),
            nota_exp_ref: None,
            cnpj: cnpj.into(),
            hash_arquivo_id: Some(hash_arquivo.into()),
            reg_0000_id: placeholder_reg_0000_id,
            reg_j005_id: 1,
        }];

        let reg_j150 = vec![RegJ150 {
            id: None,
            nu_ordem: 1,
            cod_agl: Some("AGL_REC_01".into()),
            ind_cod_agl: "T".into(),
            nivel_agl: 1,
            cod_agl_sup: None,
            descr_cod_agl: "RECEITA BRUTA OPERACIONAL".into(),
            vl_cta_ini: Decimal::ZERO,
            ind_dc_cta_ini: Some("C".into()),
            vl_cta_fin: Decimal::from(500_000),
            ind_dc_cta_fin: "C".into(),
            ind_grp_dre: "R".into(),
            nota_exp_ref: None,
            cnpj: cnpj.into(),
            dt_ini: Some(dt_ini),
            dt_fim: Some(dt_fim),
            hash_arquivo: hash_arquivo.into(),
        }];

        let reg_j210 = vec![RegistroJ210 {
            id: None,
            ind_tip: "0".into(),
            cod_agl: "PL01".into(),
            descr_cod_agl: "LUCROS ACUMULADOS".into(),
            vl_cta_ini: Decimal::ZERO,
            ind_dc_cta_ini: "C".into(),
            vl_cta_fin: Decimal::from(200_000),
            ind_dc_cta_fin: "C".into(),
            nota_exp_ref: None,
            cnpj: cnpj.into(),
            hash_arquivo_id: Some(hash_arquivo.into()),
            reg_0000_id: Some(placeholder_reg_0000_id),
        }];

        let reg_j215 = vec![RegistroJ215 {
            id: None,
            cod_hist_fat: "H001".into(),
            desc_fat: "LUCRO DO EXERCICIO".into(),
            vl_fat_cont: Decimal::from(200_000),
            ind_dc_fat: "C".into(),
            cnpj: cnpj.into(),
            hash_arquivo_id: Some(hash_arquivo.into()),
            reg_0000_id: Some(placeholder_reg_0000_id),
        }];

        SpedBatchRecords {
            reg_0000,
            reg_i010,
            reg_i030,
            reg_i050,
            reg_i051,
            reg_i052,
            reg_i053,
            plano_contas,
            reg_i150,
            reg_i155,
            reg_i355,
            reg_j005,
            reg_j100,
            reg_j150,
            reg_j210,
            reg_j215,
        }
    }

    #[tokio::test]
    async fn test_postgres_real_16_tabelas_lotes_5000_fk_e_substituicao_nulls_not_distinct() {
        let Some(pg) = EphemeralPg::start() else {
            return;
        };

        let pool = crate::create_postgres_pool(&pg.database_url)
            .await
            .expect("Deve conectar ao PostgreSQL efêmero");
        let repo = SpedRepository::new(pool.clone());
        let fiscal_repo = crate::fiscal_repository::FiscalRepository::new(pool.clone());

        // 1. Insere conta no plano_contas_base e valida leitura
        sqlx::query(
            "INSERT INTO plano_contas_base (codigo_conta, nome_conta, nivel, natureza)
             VALUES ('3.01.01.01.01.06', 'RECEITA DE SERVICOS PRESTADOS', 5, 4)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let mapa_base = repo.carregar_plano_contas_base().await.unwrap();
        assert_eq!(
            mapa_base.get("3.01.01.01.01.06").map(String::as_str),
            Some("RECEITA DE SERVICOS PRESTADOS")
        );

        let dt_ini = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let dt_fim = NaiveDate::from_ymd_opt(2023, 12, 31).unwrap();
        let cnpj = "12345678000195";

        // 2. Importa escrituração original (HASH_ORIG_PG) com >5.000 linhas em RegI050 (19 cols) e RegistroI155 (10 cols)
        let job_orig = ImportSpedJobPayload {
            job_id: Uuid::now_v7(),
            hash_arquivo: "HASH_ORIG_PG".into(),
            cnpj: cnpj.into(),
            caminho_arquivo: "/tmp/aguardando/orig.txt".into(),
            nome_original: "orig.txt".into(),
            tamanho_bytes: 1024,
            enfileirado_em: Utc::now(),
        };
        repo.registrar_inicio_importacao(&job_orig).await.unwrap();

        let batch_orig = criar_batch_completo(
            cnpj,
            "HASH_ORIG_PG",
            None,
            dt_ini,
            dt_fim,
            5_200, // > 3.449 (força 2 chunks para 19 colunas sem estourar 65.535 bind params)
            5_500, // > 5.000 (força 2 chunks de 5.000 + 500 para 10 colunas)
            999, // placeholder_reg_0000_id != id real do banco (valida precedência de reg_0000_id)
        );
        let res_orig = ResultadoExercicio {
            id: Uuid::now_v7(),
            cnpj: cnpj.into(),
            razao_social: "EMPRESA INTEGRACAO POSTGRES S.A.".into(),
            periodo_inicial: dt_ini,
            periodo_final: dt_fim,
            cnpj_scp: None,
            receita_bruta: Decimal::from(500_000),
            receita_servico: Decimal::from(500_000),
            outras_receitas: Decimal::ZERO,
            receita_acumulada: Decimal::from(500_000),
            total_nota: Decimal::ZERO,
            hash_arquivo: "HASH_ORIG_PG".into(),
            cod_hash_sub: None,
            situacao: "S".into(),
            detalhes_j150: None,
            detalhes_i355: None,
            criado_em: Utc::now(),
        };

        let summary_orig = repo
            .persistir_ingestao_completa(&batch_orig, &res_orig)
            .await
            .expect("Persistência do lote de >5.000 linhas nas 16 tabelas deve suceder");
        assert_eq!(summary_orig.reg_0000_id, 1);
        assert_eq!(summary_orig.total_registros_inseridos, 10_715);
        assert!(!summary_orig.arquivo_atual_ja_substituido);

        repo.finalizar_importacao_sucesso(
            job_orig.job_id,
            "HASH_ORIG_PG",
            cnpj,
            "/tmp/processados/orig.txt",
            10_714,
            150,
            false,
        )
        .await
        .unwrap();

        // Verifica contagem real no PostgreSQL
        let count_reg_i050: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM regi050_plano_contas WHERE hash_arquivo = 'HASH_ORIG_PG'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count_reg_i050, 5_200);

        let count_i155: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM core_registroi155 WHERE reg_0000_id = $1")
                .bind(summary_orig.reg_0000_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count_i155, 5_500);

        // 3. Importa retificadora (HASH_RET_PG) com cod_hash_sub = HASH_ORIG_PG e cnpj_scp = NULL
        // Valida UNIQUE NULLS NOT DISTINCT (cnpj, periodo_inicial, periodo_final, cnpj_scp) e precedência de reg_0000_id = 2
        let job_ret = ImportSpedJobPayload {
            job_id: Uuid::now_v7(),
            hash_arquivo: "HASH_RET_PG".into(),
            cnpj: cnpj.into(),
            caminho_arquivo: "/tmp/aguardando/ret.txt".into(),
            nome_original: "ret.txt".into(),
            tamanho_bytes: 2048,
            enfileirado_em: Utc::now(),
        };
        repo.registrar_inicio_importacao(&job_ret).await.unwrap();

        let batch_ret = criar_batch_completo(
            cnpj,
            "HASH_RET_PG",
            Some("HASH_ORIG_PG"),
            dt_ini,
            dt_fim,
            10,
            20,
            1, // placeholder_reg_0000_id = 1 enquanto o id real no banco será 2
        );
        let res_ret = ResultadoExercicio {
            id: Uuid::now_v7(),
            cnpj: cnpj.into(),
            razao_social: "EMPRESA INTEGRACAO POSTGRES S.A.".into(),
            periodo_inicial: dt_ini,
            periodo_final: dt_fim,
            cnpj_scp: None,
            receita_bruta: Decimal::from(750_000),
            receita_servico: Decimal::from(750_000),
            outras_receitas: Decimal::ZERO,
            receita_acumulada: Decimal::from(750_000),
            total_nota: Decimal::ZERO,
            hash_arquivo: "HASH_RET_PG".into(),
            cod_hash_sub: Some("HASH_ORIG_PG".into()),
            situacao: "S".into(),
            detalhes_j150: None,
            detalhes_i355: None,
            criado_em: Utc::now(),
        };

        let summary_ret = repo
            .persistir_ingestao_completa(&batch_ret, &res_ret)
            .await
            .expect("Persistência da retificadora deve suceder sem violar FK ou UNIQUE NULLS NOT DISTINCT");
        assert_eq!(summary_ret.reg_0000_id, 2);
        assert_eq!(
            summary_ret.escrituracao_substituida_hash.as_deref(),
            Some("HASH_ORIG_PG")
        );
        assert!(!summary_ret.arquivo_atual_ja_substituido);

        // Verifica que HASH_ORIG_PG foi marcada como Substituida em importacoes_ecd
        let imp_orig = fiscal_repo
            .obter_importacao_por_hash("HASH_ORIG_PG")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(imp_orig.status, StatusImportacao::Substituida);

        // Verifica que FiscalRepository retorna apenas a receita da retificadora (750.000)
        let soma_kpi = fiscal_repo.somar_receita_bruta_apurada().await.unwrap();
        assert_eq!(soma_kpi, Decimal::from(750_000));

        let consolidado = fiscal_repo
            .consultar_cnpj_consolidado(cnpj, Some(2023))
            .await
            .unwrap();
        assert_eq!(consolidado.reg_0000.len(), 1);
        assert_eq!(consolidado.reg_0000[0].hash_arquivo_id, "HASH_RET_PG");
        assert_eq!(
            consolidado.resultado_exercicio.unwrap().receita_bruta,
            Decimal::from(750_000)
        );

        // 4. Reimporta HASH_ORIG_PG fora de ordem (com período distinto para testar situacao = 'X' em resultados_exercicio)
        let dt_ini_antigo = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let dt_fim_antigo = NaiveDate::from_ymd_opt(2023, 6, 30).unwrap();
        let batch_orig_atrasado = criar_batch_completo(
            cnpj,
            "HASH_ORIG_PG",
            None,
            dt_ini_antigo,
            dt_fim_antigo,
            2,
            2,
            1,
        );
        let res_orig_atrasado = ResultadoExercicio {
            id: Uuid::now_v7(),
            cnpj: cnpj.into(),
            razao_social: "EMPRESA INTEGRACAO POSTGRES S.A.".into(),
            periodo_inicial: dt_ini_antigo,
            periodo_final: dt_fim_antigo,
            cnpj_scp: None,
            receita_bruta: Decimal::from(500_000),
            receita_servico: Decimal::from(500_000),
            outras_receitas: Decimal::ZERO,
            receita_acumulada: Decimal::from(500_000),
            total_nota: Decimal::ZERO,
            hash_arquivo: "HASH_ORIG_PG".into(),
            cod_hash_sub: None,
            situacao: "S".into(),
            detalhes_j150: None,
            detalhes_i355: None,
            criado_em: Utc::now(),
        };

        let summary_atrasado = repo
            .persistir_ingestao_completa(&batch_orig_atrasado, &res_orig_atrasado)
            .await
            .unwrap();
        assert!(summary_atrasado.arquivo_atual_ja_substituido);

        // Como HASH_ORIG_PG recebeu situacao = 'X', a soma KPI no FiscalRepository (WHERE situacao != 'X') continua 750.000!
        let soma_kpi_apos_atrasado = fiscal_repo.somar_receita_bruta_apurada().await.unwrap();
        assert_eq!(soma_kpi_apos_atrasado, Decimal::from(750_000));
    }
}
