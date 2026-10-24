use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Bloco 0000: Abertura do Arquivo Digital e Identificação da Entidade
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Registro0000 {
    pub id: Option<i64>,
    pub dt_ini: NaiveDate,
    pub dt_fim: NaiveDate,
    pub nome_razao: String,
    pub cnpj: String,
    pub uf: String,
    pub ie: Option<String>,
    pub cod_mun: Option<i64>,
    pub im: Option<String>,
    pub ind_sit_esp: Option<i32>,
    pub ind_sit_ini_per: i32,
    pub ind_nire: i32,
    pub ind_fin_esc: i32,
    pub cod_hash_sub: Option<String>,
    pub ind_grande_porte: i32,
    pub tip_ecd: i32,
    pub cod_scp: Option<String>,
    pub ident_mf: String,
    pub ind_esc_cons: String,
    pub ind_centralizada: i32,
    pub ind_mudanc_pc: i32,
    pub cod_plan_ref: Option<String>,
    pub i990: i64,
    pub j990: i64,
    pub hash_arquivo: String,
}

/// Bloco I010: Identificação da Escrituração Contábil
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI010 {
    pub id: Option<i64>,
    pub reg_0000_id: i64,
    pub ind_esc: String,
    pub cod_ver_lc: String,
    pub hash_arquivo: String,
}

/// Bloco I030: Termo de Abertura do Livro
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI030 {
    pub id: Option<i64>,
    pub reg_0000_id: i64,
    pub num_ord: i64,
    pub nat_livr: String,
    pub qtd_lin: i64,
    pub nome: String,
    pub nire: Option<String>,
    pub cnpj: String,
    pub dt_arq: Option<NaiveDate>,
    pub dt_arq_conv: Option<NaiveDate>,
    pub desc_mun: Option<String>,
    pub dt_ex_social: Option<NaiveDate>,
    pub hash_arquivo: String,
}

/// Bloco I050 / RegI050: Plano de Contas
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegI050 {
    pub id: String, // hash MD5
    pub dt_alt: NaiveDate,
    pub cod_nat: String,
    pub ind_cta: String,
    pub nivel: i32,
    pub cod_cta: String,
    pub cod_cta_sup: Option<String>,
    pub cta: String,
    pub cta_pai: Option<String>,
    pub cnpj: String,
    pub cod_ccus_51: Option<String>,
    pub cod_cta_ref: Option<String>,
    pub nome_cta_referencia: Option<String>,
    pub cod_ccus_52: Option<String>,
    pub cod_agl: Option<String>,
    pub hash_arquivo_id: String,
    pub data_entrega: Option<NaiveDateTime>,
}

/// Bloco I150: Saldos Periódicos - Identificação do Período
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI150 {
    pub id: Option<i64>,
    pub reg_0000_id: i64,
    pub dt_ini: NaiveDate,
    pub dt_fim: NaiveDate,
    pub hash_arquivo: String,
}

/// Bloco I155: Detalhe dos Saldos Periódicos
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI155 {
    pub id: Option<i64>,
    pub reg_150_id: i64,
    pub cod_cta: String,
    pub cod_ccus: Option<String>,
    pub vl_sld_ini: Decimal,
    pub ind_dc_ini: String,
    pub vl_deb: Decimal,
    pub vl_cred: Decimal,
    pub vl_sld_fin: Decimal,
    pub ind_dc_fin: String,
    pub hash_arquivo: String,
}

/// Bloco I355 / RegI355: Saldos das Contas de Resultado Antes do Encerramento
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegI355 {
    pub id: String,
    pub dt_res: NaiveDate,
    pub cod_cta: String,
    pub cod_ccus: Option<String>,
    pub vl_cta: Decimal,
    pub ind_dc: String,
    pub cnpj: String,
    pub hash_arquivo: String,
    pub data_entrega: Option<NaiveDateTime>,
}

/// Bloco J005: Demonstrações Contábeis
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroJ005 {
    pub id: Option<i64>,
    pub reg_0000_id: i64,
    pub dt_ini: NaiveDate,
    pub dt_fim: NaiveDate,
    pub id_dem: i32,
    pub cab_dem: Option<String>,
    pub hash_arquivo: String,
    pub cnpj: String,
}

/// Bloco J100: Balanço Patrimonial
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroJ100 {
    pub id: Option<i64>,
    pub reg_005_id: i64,
    pub cod_agl: String,
    pub nivel_agl: i32,
    pub descr_cod_agl: String,
    pub vl_cta: Decimal,
    pub ind_dc_cta: String,
    pub vl_cta_ini: Decimal,
    pub ind_dc_cta_ini: String,
    pub hash_arquivo: String,
    pub cnpj: String,
}

/// Bloco J150 / RegJ150: Demonstração do Resultado do Exercício (DRE)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegJ150 {
    pub id: String,
    pub reg_005_id: Option<i64>,
    pub nu_ordem: Option<i64>,
    pub cod_agl: String,
    pub nivel_agl: i32,
    pub descr_cod_agl: String,
    pub vl_cta_fin: Decimal,
    pub ind_dc_cta_fin: String,
    pub vl_cta_ini: Decimal,
    pub ind_dc_cta_ini: String,
    pub ind_cod_agl: String,
    pub hash_arquivo: String,
    pub cnpj: String,
    pub dt_ini: NaiveDate,
    pub dt_fim: NaiveDate,
    pub data_entrega: Option<NaiveDateTime>,
}
