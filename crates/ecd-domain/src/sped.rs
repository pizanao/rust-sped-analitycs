use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Bloco 0000: Abertura do Arquivo Digital e Identificação da Entidade (`core_registro0000`)
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
    pub hash_arquivo_id: String,
}

/// Bloco I010: Identificação da Escrituração Contábil (`core_registroi010`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI010 {
    pub id: Option<i64>,
    pub ind_esc: String,
    pub cod_ver_lc: String,
    pub reg_0000_id: i64,
}

/// Bloco I030: Termo de Abertura do Livro (`core_registroi030`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI030 {
    pub id: Option<i64>,
    pub num_ord: i32,
    pub nat_livr: String,
    pub qtd_lin: i32,
    pub nome: String,
    pub nire: String,
    pub cnpj: String,
    pub dt_arq: Option<NaiveDate>,
    pub dt_arq_conv: Option<NaiveDate>,
    pub desc_mun: Option<String>,
    pub dt_ex_social: NaiveDate,
    pub reg_0000_id: i64,
    pub hash_arquivo_id: Option<String>,
}

/// Bloco I050 Relacional (`core_registroi050`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI050 {
    pub id: Option<i64>,
    pub cod_cta: String,
    pub cta: String,
    pub nivel: i32,
    pub ind_cta: String,
    pub cod_nat: String,
    pub cod_cta_sup: Option<String>,
    pub dt_alt: NaiveDate,
    pub descr_cta: String,
    pub descr_conta: Option<String>,
    pub codigo_conta: Option<String>,
    pub reg_0000_id: i64,
}

/// Bloco I051: Plano de Contas Referencial (`core_registroi051`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI051 {
    pub id: Option<i64>,
    pub cod_ccus: Option<String>,
    pub cod_cta_ref: String,
    pub reg_0000_id: i64,
    pub reg_i050_id: i64,
}

/// Bloco I052: Indicação dos Códigos de Aglutinação (`core_registroi052`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI052 {
    pub id: Option<i64>,
    pub cod_ccus: Option<String>,
    pub cod_agl: String,
    pub reg_0000_id: i64,
    pub reg_i050_id: Option<i64>,
}

/// Bloco I053: Subcontas Correlatas (`core_registroi053`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI053 {
    pub id: Option<i64>,
    pub cod_idt: String,
    pub cod_cnt_corr: String,
    pub nat_sub_cnt: String,
    pub reg_0000_id: i64,
}

/// Bloco I050 Consolidado (`regi050_plano_contas`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegI050 {
    pub id: Uuid,
    pub cod_cta: String,
    pub cta: String,
    pub nivel: i32,
    pub ind_cta: String,
    pub cod_nat: String,
    pub cod_cta_sup: Option<String>,
    pub dt_alt: NaiveDate,
    pub cod_ccus_51: Option<String>,
    pub cod_cta_ref: Option<String>,
    pub cod_ccus_52: Option<String>,
    pub cod_agl: Option<String>,
    pub cnpj: String,
    pub data_entrega: DateTime<Utc>,
    pub cta_pai: Option<String>,
    pub nome_cta_referencia: Option<String>,
    pub reg051_json: Option<serde_json::Value>,
    pub reg052_json: Option<serde_json::Value>,
    pub hash_arquivo: String,
}

/// Bloco I150: Saldos Periódicos - Identificação do Período (`core_registroi150`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI150 {
    pub id: Option<i64>,
    pub dt_ini: NaiveDate,
    pub dt_fim: NaiveDate,
    pub reg_0000_id: i64,
}

/// Bloco I155: Detalhe dos Saldos Periódicos (`core_registroi155`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroI155 {
    pub id: Option<i64>,
    pub cod_cta: String,
    pub cod_ccus: Option<String>,
    pub vl_sld_ini: Decimal,
    pub ind_dc_ini: String,
    pub vl_deb: Decimal,
    pub vl_cred: Decimal,
    pub vl_sld_fim: Decimal,
    pub ind_dc_fim: String,
    pub reg_0000_id: i64,
    pub reg_i150_id: i64,
}

/// Bloco I355 / RegI355: Saldos das Contas de Resultado Antes do Encerramento (`regi355_detalhes_contas_resultado`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegI355 {
    pub id: Uuid,
    pub cnpj: String,
    pub cod_cta: String,
    pub nome_cta_referencia: Option<String>,
    pub cod_ccus: Option<String>,
    pub vl_cta: Decimal,
    pub ind_dc: String,
    pub vl_cta_mf: Option<Decimal>,
    pub ind_dc_mf: Option<String>,
    pub dt_res: NaiveDate,
    pub cod_agl: Option<String>,
    pub cod_cta_ref: Option<String>,
    pub nome_aglutinador: Option<String>,
    pub hash_arquivo: String,
}

/// Bloco J005: Demonstrações Contábeis (`core_registroj005`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroJ005 {
    pub id: Option<i64>,
    pub dt_ini: NaiveDate,
    pub dt_fim: NaiveDate,
    pub id_dem: i32,
    pub cab_dem: Option<String>,
    pub cnpj: String,
    pub hash_arquivo_id: Option<String>,
    pub reg_0000_id: i64,
}

/// Bloco J100: Balanço Patrimonial (`core_registroj100`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroJ100 {
    pub id: Option<i64>,
    pub cod_agl: String,
    pub ind_cod_agl: String,
    pub nivel_agl: i32,
    pub cod_agl_sup: Option<String>,
    pub ind_grp_bal: String,
    pub descr_cod_agl: String,
    pub vl_cta_ini: Decimal,
    pub ind_dc_cta_ini: String,
    pub vl_cta_fin: Decimal,
    pub ind_dc_cta_fin: String,
    pub nota_exp_ref: Option<String>,
    pub cnpj: String,
    pub hash_arquivo_id: Option<String>,
    pub reg_0000_id: i64,
    pub reg_j005_id: i64,
}

/// Bloco J150 / RegJ150: Demonstração do Resultado do Exercício - DRE (`regj150_demonstracao_resultado`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegJ150 {
    pub id: Option<i64>,
    pub nu_ordem: i64,
    pub cod_agl: Option<String>,
    pub ind_cod_agl: String,
    pub nivel_agl: i32,
    pub cod_agl_sup: Option<String>,
    pub descr_cod_agl: String,
    pub vl_cta_ini: Decimal,
    pub ind_dc_cta_ini: Option<String>,
    pub vl_cta_fin: Decimal,
    pub ind_dc_cta_fin: String,
    pub ind_grp_dre: String,
    pub nota_exp_ref: Option<String>,
    pub cnpj: String,
    pub dt_ini: Option<NaiveDate>,
    pub dt_fim: Option<NaiveDate>,
    pub hash_arquivo: String,
}

/// Bloco J210: DLPA / DMPL (`core_registroj210`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroJ210 {
    pub id: Option<i64>,
    pub ind_tip: String,
    pub cod_agl: String,
    pub descr_cod_agl: String,
    pub vl_cta_ini: Decimal,
    pub ind_dc_cta_ini: String,
    pub vl_cta_fin: Decimal,
    pub ind_dc_cta_fin: String,
    pub nota_exp_ref: Option<String>,
    pub cnpj: String,
    pub hash_arquivo_id: Option<String>,
    pub reg_0000_id: Option<i64>,
}

/// Bloco J215: Fato Contábil que Altera a Conta Lucros/Prejuízos Acumulados (`core_registroj215`)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroJ215 {
    pub id: Option<i64>,
    pub cod_hist_fat: String,
    pub desc_fat: String,
    pub vl_fat_cont: Decimal,
    pub ind_dc_fat: String,
    pub cnpj: String,
    pub hash_arquivo_id: Option<String>,
    pub reg_0000_id: Option<i64>,
}
