use chrono::NaiveDate;
use ecd_domain::{RegJ150, RegistroJ005, RegistroJ100, RegistroJ210, RegistroJ215};

use crate::reader::{
    decode_field, decode_opt_field, parse_opt_sped_i64, parse_sped_date, parse_sped_decimal,
    parse_sped_decimal_or_zero, parse_sped_i32, parse_sped_i64, SpedRecord,
};
pub use crate::records::bloco_i::{
    decode_i350, decode_i350_at_line, decode_i355, decode_i355_at_line, enrich_i355,
};
use crate::ParserError;

/// Decodifica o Registro `J005` (Demonstrações Contábeis).
pub fn decode_j005(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    cnpj: &str,
    hash_arquivo_id: Option<String>,
) -> Result<RegistroJ005, ParserError> {
    decode_j005_at_line(record, reg_0000_id, cnpj, hash_arquivo_id, 1)
}

pub fn decode_j005_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    cnpj: &str,
    hash_arquivo_id: Option<String>,
    line_num: usize,
) -> Result<RegistroJ005, ParserError> {
    if record.bloco != b"J005" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'J005', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 4 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro J005 incompleto: esperado ao menos 4 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let dt_ini = parse_sped_date(c[1], line_num, "DT_INI")?;
    let dt_fim = parse_sped_date(c[2], line_num, "DT_FIM")?;
    let id_dem = parse_sped_i32(c[3], line_num, "ID_DEM")?;
    let cab_dem = c.get(4).and_then(|b| decode_opt_field(b));

    Ok(RegistroJ005 {
        id: None,
        dt_ini,
        dt_fim,
        id_dem,
        cab_dem,
        cnpj: cnpj.to_string(),
        hash_arquivo_id,
        reg_0000_id,
    })
}

/// Decodifica o Registro `J100` (Balanço Patrimonial).
pub fn decode_j100(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    reg_j005_id: i64,
    cnpj: &str,
    hash_arquivo_id: Option<String>,
) -> Result<RegistroJ100, ParserError> {
    decode_j100_at_line(record, reg_0000_id, reg_j005_id, cnpj, hash_arquivo_id, 1)
}

pub fn decode_j100_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    reg_j005_id: i64,
    cnpj: &str,
    hash_arquivo_id: Option<String>,
    line_num: usize,
) -> Result<RegistroJ100, ParserError> {
    if record.bloco != b"J100" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'J100', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 11 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro J100 incompleto: esperado ao menos 11 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let cod_agl = decode_field(c[1]);
    if cod_agl.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'COD_AGL' vazio no registro J100".to_string(),
        ));
    }
    let ind_cod_agl = decode_field(c[2]);
    let nivel_agl = parse_sped_i32(c[3], line_num, "NIVEL_AGL")?;
    let cod_agl_sup = decode_opt_field(c[4]);
    let ind_grp_bal = decode_field(c[5]);
    let descr_cod_agl = decode_field(c[6]);
    if descr_cod_agl.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'DESCR_COD_AGL' vazio no registro J100".to_string(),
        ));
    }
    let vl_cta_ini = parse_sped_decimal(c[7], line_num, "VL_CTA_INI")?;
    let ind_dc_cta_ini = decode_field(c[8]);
    let vl_cta_fin = parse_sped_decimal(c[9], line_num, "VL_CTA_FIN")?;
    let ind_dc_cta_fin = decode_field(c[10]);
    let nota_exp_ref = c.get(11).and_then(|b| decode_opt_field(b));

    if ind_cod_agl.is_empty()
        || ind_grp_bal.is_empty()
        || ind_dc_cta_ini.is_empty()
        || ind_dc_cta_fin.is_empty()
    {
        return Err(ParserError::InvalidLine(
            line_num,
            "Indicadores obrigatórios de aglutinação/grupo/D-C vazios no registro J100".to_string(),
        ));
    }

    Ok(RegistroJ100 {
        id: None,
        cod_agl,
        ind_cod_agl,
        nivel_agl,
        cod_agl_sup,
        ind_grp_bal,
        descr_cod_agl,
        vl_cta_ini,
        ind_dc_cta_ini,
        vl_cta_fin,
        ind_dc_cta_fin,
        nota_exp_ref,
        cnpj: cnpj.to_string(),
        hash_arquivo_id,
        reg_0000_id,
        reg_j005_id,
    })
}

/// Normaliza a descrição da rubrica da DRE (`J150`) com paridade a `carrega_regj150`
/// (`apps/parametros/utils.py` e `apps/core/tratamento.py`).
pub fn normalize_j150_descr(raw: &str) -> String {
    raw.replace(['#', '(', ')', '+', '='], "")
        .replace(['/', '-', ':'], " ")
        .to_uppercase()
        .trim()
        .to_string()
}

/// Decodifica o Registro `J150` (`RegJ150` - Demonstração do Resultado do Exercício - DRE).
pub fn decode_j150(
    record: &SpedRecord<'_>,
    cnpj: &str,
    hash_arquivo: &str,
    dt_ini: Option<NaiveDate>,
    dt_fim: Option<NaiveDate>,
) -> Result<RegJ150, ParserError> {
    decode_j150_at_line(record, cnpj, hash_arquivo, dt_ini, dt_fim, 1)
}

pub fn decode_j150_at_line(
    record: &SpedRecord<'_>,
    cnpj: &str,
    hash_arquivo: &str,
    dt_ini: Option<NaiveDate>,
    dt_fim: Option<NaiveDate>,
    line_num: usize,
) -> Result<RegJ150, ParserError> {
    if record.bloco != b"J150" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'J150', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 12 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro J150 incompleto: esperado ao menos 12 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let nu_ordem = parse_opt_sped_i64(c[1], line_num, "NU_ORDEM")?.unwrap_or(0);
    let cod_agl = decode_opt_field(c[2]);
    let ind_cod_agl = decode_field(c[3]);
    let nivel_agl = parse_sped_i32(c[4], line_num, "NIVEL_AGL")?;
    let cod_agl_sup = decode_opt_field(c[5]);
    let descr_cod_agl = normalize_j150_descr(&decode_field(c[6]));
    if descr_cod_agl.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'DESCR_COD_AGL' vazio no registro J150".to_string(),
        ));
    }
    let vl_cta_ini = parse_sped_decimal_or_zero(c[7], line_num, "VL_CTA_INI")?;
    let ind_dc_cta_ini = decode_opt_field(c[8]);
    let vl_cta_fin = parse_sped_decimal_or_zero(c[9], line_num, "VL_CTA_FIN")?;
    let ind_dc_cta_fin = decode_field(c[10]);
    let ind_grp_dre = decode_field(c[11]);
    let nota_exp_ref = c.get(12).and_then(|b| decode_opt_field(b));

    if ind_cod_agl.is_empty() || ind_dc_cta_fin.is_empty() || ind_grp_dre.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Indicadores obrigatórios (IND_COD_AGL / IND_DC_CTA_FIN / IND_GRP_DRE) vazios no registro J150"
                .to_string(),
        ));
    }

    Ok(RegJ150 {
        id: None,
        nu_ordem,
        cod_agl,
        ind_cod_agl,
        nivel_agl,
        cod_agl_sup,
        descr_cod_agl,
        vl_cta_ini,
        ind_dc_cta_ini,
        vl_cta_fin,
        ind_dc_cta_fin,
        ind_grp_dre,
        nota_exp_ref,
        cnpj: cnpj.to_string(),
        dt_ini,
        dt_fim,
        hash_arquivo: hash_arquivo.to_string(),
    })
}

/// Decodifica o Registro `J210` (DLPA / DMPL).
pub fn decode_j210(
    record: &SpedRecord<'_>,
    reg_0000_id: Option<i64>,
    cnpj: &str,
    hash_arquivo_id: Option<String>,
) -> Result<RegistroJ210, ParserError> {
    decode_j210_at_line(record, reg_0000_id, cnpj, hash_arquivo_id, 1)
}

pub fn decode_j210_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: Option<i64>,
    cnpj: &str,
    hash_arquivo_id: Option<String>,
    line_num: usize,
) -> Result<RegistroJ210, ParserError> {
    if record.bloco != b"J210" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'J210', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 8 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro J210 incompleto: esperado ao menos 8 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let ind_tip = decode_field(c[1]);
    let cod_agl = decode_field(c[2]);
    let descr_cod_agl = decode_field(c[3]);
    let vl_cta_ini = parse_sped_decimal(c[4], line_num, "VL_CTA_INI")?;
    let ind_dc_cta_ini = decode_field(c[5]);
    let vl_cta_fin = parse_sped_decimal(c[6], line_num, "VL_CTA_FIN")?;
    let ind_dc_cta_fin = decode_field(c[7]);
    let nota_exp_ref = c.get(8).and_then(|b| decode_opt_field(b));

    if ind_tip.is_empty()
        || cod_agl.is_empty()
        || descr_cod_agl.is_empty()
        || ind_dc_cta_ini.is_empty()
        || ind_dc_cta_fin.is_empty()
    {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campos obrigatórios vazios no registro J210".to_string(),
        ));
    }

    Ok(RegistroJ210 {
        id: None,
        ind_tip,
        cod_agl,
        descr_cod_agl,
        vl_cta_ini,
        ind_dc_cta_ini,
        vl_cta_fin,
        ind_dc_cta_fin,
        nota_exp_ref,
        cnpj: cnpj.to_string(),
        hash_arquivo_id,
        reg_0000_id,
    })
}

/// Decodifica o Registro `J215` (Fato Contábil que Altera a Conta Lucros ou Prejuízos Acumulados).
pub fn decode_j215(
    record: &SpedRecord<'_>,
    reg_0000_id: Option<i64>,
    cnpj: &str,
    hash_arquivo_id: Option<String>,
) -> Result<RegistroJ215, ParserError> {
    decode_j215_at_line(record, reg_0000_id, cnpj, hash_arquivo_id, 1)
}

pub fn decode_j215_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: Option<i64>,
    cnpj: &str,
    hash_arquivo_id: Option<String>,
    line_num: usize,
) -> Result<RegistroJ215, ParserError> {
    if record.bloco != b"J215" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'J215', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 5 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro J215 incompleto: esperado 5 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let cod_hist_fat = decode_field(c[1]);
    let desc_fat = decode_field(c[2]);
    let vl_fat_cont = parse_sped_decimal(c[3], line_num, "VL_FAT_CONT")?;
    let ind_dc_fat = decode_field(c[4]);

    if cod_hist_fat.is_empty() || desc_fat.is_empty() || ind_dc_fat.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campos obrigatórios vazios no registro J215".to_string(),
        ));
    }

    Ok(RegistroJ215 {
        id: None,
        cod_hist_fat,
        desc_fat,
        vl_fat_cont,
        ind_dc_fat,
        cnpj: cnpj.to_string(),
        hash_arquivo_id,
        reg_0000_id,
    })
}

/// Decodifica o Registro `J990` (Encerramento do Bloco J) retornando a quantidade total de linhas do Bloco J.
pub fn decode_j990(record: &SpedRecord<'_>) -> Result<i64, ParserError> {
    decode_j990_at_line(record, 1)
}

pub fn decode_j990_at_line(record: &SpedRecord<'_>, line_num: usize) -> Result<i64, ParserError> {
    if record.bloco != b"J990" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'J990', encontrado '{}'", record.bloco_str()),
        ));
    }
    let c = &record.campos;
    if c.len() < 2 {
        return Err(ParserError::InvalidLine(
            line_num,
            "Registro J990 incompleto: campo QTD_LIN_J ausente".to_string(),
        ));
    }
    parse_sped_i64(c[1], line_num, "QTD_LIN_J")
}
