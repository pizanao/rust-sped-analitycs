use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use ecd_domain::{
    RegI050, RegI355, RegJ150, RegistroI010, RegistroI030, RegistroI050, RegistroI051,
    RegistroI052, RegistroI053, RegistroI150, RegistroI155,
};
use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

use crate::reader::{
    decode_field, decode_opt_field, parse_opt_sped_date, parse_opt_sped_decimal, parse_sped_date,
    parse_sped_decimal, parse_sped_decimal_or_zero, parse_sped_i32, parse_sped_i64, process_cta,
    SpedRecord,
};
use crate::ParserError;

/// Decodifica o Registro `I010` (Identificação da Escrituração Contábil).
pub fn decode_i010(record: &SpedRecord<'_>, reg_0000_id: i64) -> Result<RegistroI010, ParserError> {
    decode_i010_at_line(record, reg_0000_id, 1)
}

pub fn decode_i010_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    line_num: usize,
) -> Result<RegistroI010, ParserError> {
    if record.bloco != b"I010" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I010', encontrado '{}'", record.bloco_str()),
        ));
    }
    let c = &record.campos;
    if c.len() < 2 {
        return Err(ParserError::InvalidLine(
            line_num,
            "Registro I010 incompleto: campo IND_ESC ausente".to_string(),
        ));
    }

    let ind_esc = decode_field(c[1]);
    if ind_esc.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'IND_ESC' vazio no registro I010".to_string(),
        ));
    }
    let cod_ver_lc = c
        .get(2)
        .and_then(|b| decode_opt_field(b))
        .unwrap_or_else(|| "9.00".to_string());

    Ok(RegistroI010 {
        id: None,
        ind_esc,
        cod_ver_lc,
        reg_0000_id,
    })
}

/// Decodifica o Registro `I030` (Termo de Abertura do Livro).
pub fn decode_i030(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    hash_arquivo_id: Option<String>,
) -> Result<RegistroI030, ParserError> {
    decode_i030_at_line(record, reg_0000_id, hash_arquivo_id, 1)
}

pub fn decode_i030_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    hash_arquivo_id: Option<String>,
    line_num: usize,
) -> Result<RegistroI030, ParserError> {
    if record.bloco != b"I030" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I030', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    // No layout oficial SPED ECD, c[1] é DNRC_ABERT ("TERMO DE ABERTURA") e c[2] é NUM_ORD (total 12 campos).
    // Suporta também linhas compactas de 11 campos caso DNRC_ABERT tenha sido omitido e c[1] seja numérico.
    let base = if c.len() >= 12 {
        2
    } else if c.len() == 11 && decode_field(c[1]).parse::<i32>().is_ok() {
        1
    } else {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro I030 incompleto: esperado 12 campos (com DNRC_ABERT), encontrado {}",
                c.len()
            ),
        ));
    };

    let num_ord = parse_sped_i32(c[base], line_num, "NUM_ORD")?;
    let nat_livr = decode_field(c[base + 1]);
    let qtd_lin = parse_sped_i32(c[base + 2], line_num, "QTD_LIN")?;
    let nome = decode_field(c[base + 3]);
    let nire = decode_field(c[base + 4]);
    let raw_cnpj = decode_field(c[base + 5]);
    let cnpj: String = raw_cnpj.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if nat_livr.is_empty() || nome.is_empty() || cnpj.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campos obrigatórios ('NAT_LIVR', 'NOME' ou 'CNPJ') vazios no registro I030"
                .to_string(),
        ));
    }
    let dt_arq = parse_opt_sped_date(c[base + 6], line_num, "DT_ARQ")?;
    let dt_arq_conv = parse_opt_sped_date(c[base + 7], line_num, "DT_ARQ_CONV")?;
    let desc_mun = decode_opt_field(c[base + 8]);
    let dt_ex_social = parse_sped_date(c[base + 9], line_num, "DT_EX_SOCIAL")?;

    Ok(RegistroI030 {
        id: None,
        num_ord,
        nat_livr,
        qtd_lin,
        nome,
        nire,
        cnpj,
        dt_arq,
        dt_arq_conv,
        desc_mun,
        dt_ex_social,
        reg_0000_id,
        hash_arquivo_id,
    })
}

/// Decodifica o Registro `I050` relacional (`core_registroi050`).
pub fn decode_i050(record: &SpedRecord<'_>, reg_0000_id: i64) -> Result<RegistroI050, ParserError> {
    decode_i050_at_line(record, reg_0000_id, 1)
}

pub fn decode_i050_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    line_num: usize,
) -> Result<RegistroI050, ParserError> {
    if record.bloco != b"I050" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I050', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 8 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro I050 incompleto: esperado 8 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let dt_alt = parse_sped_date(c[1], line_num, "DT_ALT")?;
    let cod_nat = decode_field(c[2]);
    let ind_cta = decode_field(c[3]);
    if cod_nat.is_empty() || ind_cta.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campos obrigatórios 'COD_NAT' ou 'IND_CTA' vazios no registro I050".to_string(),
        ));
    }
    let nivel = parse_sped_i32(c[4], line_num, "NIVEL")?;
    let cod_cta = decode_field(c[5]);
    if cod_cta.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'COD_CTA' vazio no registro I050".to_string(),
        ));
    }
    let cod_cta_sup = decode_opt_field(c[6]);
    let cta = decode_field(c[7]);
    if cta.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'CTA' vazio no registro I050".to_string(),
        ));
    }

    let limpa = process_cta(&cta);
    let descr_cta = if limpa.is_empty() {
        cta.clone()
    } else {
        limpa.clone()
    };

    Ok(RegistroI050 {
        id: None,
        cod_cta: cod_cta.clone(),
        cta,
        nivel,
        ind_cta,
        cod_nat,
        cod_cta_sup,
        dt_alt,
        descr_cta,
        descr_conta: Some(limpa),
        codigo_conta: Some(cod_cta),
        reg_0000_id,
    })
}

/// Decodifica o Registro `I051` (Plano de Contas Referencial).
pub fn decode_i051(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    reg_i050_id: i64,
) -> Result<RegistroI051, ParserError> {
    decode_i051_at_line(record, reg_0000_id, reg_i050_id, 1)
}

pub fn decode_i051_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    reg_i050_id: i64,
    line_num: usize,
) -> Result<RegistroI051, ParserError> {
    if record.bloco != b"I051" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I051', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 3 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro I051 incompleto: esperado ao menos 3 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    // No Leiaute 8 e 9 (padrão atual): |I051|COD_CCUS|COD_CTA_REF| (3 campos).
    // Em leiautes antigos (<= 7): |I051|COD_PLAN_REF|COD_CCUS|COD_CTA_REF| (4 campos).
    let (cod_ccus, cod_cta_ref) = if c.len() >= 4 && !decode_field(c[3]).is_empty() {
        (decode_opt_field(c[2]), decode_field(c[3]))
    } else {
        (decode_opt_field(c[1]), decode_field(c[2]))
    };

    if cod_cta_ref.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'COD_CTA_REF' vazio no registro I051".to_string(),
        ));
    }

    Ok(RegistroI051 {
        id: None,
        cod_ccus,
        cod_cta_ref,
        reg_0000_id,
        reg_i050_id,
    })
}

/// Decodifica o Registro `I052` (Indicação dos Códigos de Aglutinação).
pub fn decode_i052(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    reg_i050_id: Option<i64>,
) -> Result<RegistroI052, ParserError> {
    decode_i052_at_line(record, reg_0000_id, reg_i050_id, 1)
}

pub fn decode_i052_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    reg_i050_id: Option<i64>,
    line_num: usize,
) -> Result<RegistroI052, ParserError> {
    if record.bloco != b"I052" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I052', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 3 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro I052 incompleto: esperado 3 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let cod_ccus = decode_opt_field(c[1]);
    let cod_agl = decode_field(c[2]);
    if cod_agl.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'COD_AGL' vazio no registro I052".to_string(),
        ));
    }

    Ok(RegistroI052 {
        id: None,
        cod_ccus,
        cod_agl,
        reg_0000_id,
        reg_i050_id,
    })
}

/// Decodifica o Registro `I053` (Subcontas Correlatas).
pub fn decode_i053(record: &SpedRecord<'_>, reg_0000_id: i64) -> Result<RegistroI053, ParserError> {
    decode_i053_at_line(record, reg_0000_id, 1)
}

pub fn decode_i053_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    line_num: usize,
) -> Result<RegistroI053, ParserError> {
    if record.bloco != b"I053" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I053', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 4 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro I053 incompleto: esperado 4 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let cod_idt = decode_field(c[1]);
    let cod_cnt_corr = decode_field(c[2]);
    let nat_sub_cnt = decode_field(c[3]);

    if cod_idt.is_empty() || cod_cnt_corr.is_empty() || nat_sub_cnt.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campos obrigatórios vazios no registro I053".to_string(),
        ));
    }

    Ok(RegistroI053 {
        id: None,
        cod_idt,
        cod_cnt_corr,
        nat_sub_cnt,
        reg_0000_id,
    })
}

/// Consolida um `RegistroI050` e seus registros filhos `I051` e `I052` na estrutura `RegI050` (`regi050_plano_contas`).
#[allow(clippy::too_many_arguments)]
pub fn consolidar_reg_i050(
    i050: &RegistroI050,
    i051_list: &[RegistroI051],
    i052_list: &[RegistroI052],
    cnpj: &str,
    hash_arquivo: &str,
    data_entrega: DateTime<Utc>,
    cta_pai: Option<String>,
    nome_cta_referencia: Option<String>,
) -> RegI050 {
    let cod_ccus_51 = i051_list.last().and_then(|r| r.cod_ccus.clone());
    let cod_cta_ref = i051_list.last().map(|r| r.cod_cta_ref.clone());
    let cod_ccus_52 = i052_list.last().and_then(|r| r.cod_ccus.clone());
    let cod_agl = i052_list.last().map(|r| r.cod_agl.clone());

    let nome_ref = nome_cta_referencia.or_else(|| {
        if !i051_list.is_empty() {
            Some(process_cta(&i050.cta))
        } else {
            None
        }
    });

    let reg051_json = if i051_list.is_empty() {
        None
    } else {
        let items: Vec<_> = i051_list
            .iter()
            .map(|r| {
                json!({
                    "cod_ccus": r.cod_ccus.clone().unwrap_or_default(),
                    "cod_cta_ref": r.cod_cta_ref,
                    "nome_cta_referencia": nome_ref.clone().unwrap_or_default(),
                })
            })
            .collect();
        Some(serde_json::Value::Array(items))
    };

    let reg052_json = if i052_list.is_empty() {
        None
    } else {
        let items: Vec<_> = i052_list
            .iter()
            .map(|r| {
                json!({
                    "cod_ccus": r.cod_ccus.clone().unwrap_or_default(),
                    "cod_agl": r.cod_agl,
                })
            })
            .collect();
        Some(serde_json::Value::Array(items))
    };

    RegI050 {
        id: Uuid::now_v7(),
        cod_cta: i050.cod_cta.clone(),
        cta: i050.cta.clone(),
        nivel: i050.nivel,
        ind_cta: i050.ind_cta.clone(),
        cod_nat: i050.cod_nat.clone(),
        cod_cta_sup: i050.cod_cta_sup.clone(),
        dt_alt: i050.dt_alt,
        cod_ccus_51,
        cod_cta_ref,
        cod_ccus_52,
        cod_agl,
        cnpj: cnpj.to_string(),
        data_entrega,
        cta_pai,
        nome_cta_referencia: nome_ref,
        reg051_json,
        reg052_json,
        hash_arquivo: hash_arquivo.to_string(),
    }
}

/// Acumulador em streaming para consolidar registros `I050`, `I051` e `I052` em `RegI050`
/// mantendo o rastreamento da última conta sintética pai (`cta_pai`) como no legado (`filtrar_reg_i050`).
pub struct PlanoContasConsolidator {
    cnpj: String,
    hash_arquivo: String,
    data_entrega: DateTime<Utc>,
    plano_referencial: HashMap<String, String>,
    ultimo_cta_pai: Option<String>,
    atual_i050: Option<RegistroI050>,
    atual_cta_pai: Option<String>,
    atuais_i051: Vec<RegistroI051>,
    atuais_i052: Vec<RegistroI052>,
    consolidados: Vec<RegI050>,
}

impl PlanoContasConsolidator {
    pub fn new(
        cnpj: impl Into<String>,
        hash_arquivo: impl Into<String>,
        data_entrega: DateTime<Utc>,
    ) -> Self {
        Self {
            cnpj: cnpj.into(),
            hash_arquivo: hash_arquivo.into(),
            data_entrega,
            plano_referencial: HashMap::new(),
            ultimo_cta_pai: None,
            atual_i050: None,
            atual_cta_pai: None,
            atuais_i051: Vec::new(),
            atuais_i052: Vec::new(),
            consolidados: Vec::new(),
        }
    }

    /// Configura um catálogo de contas referenciais (`plano_contas_base`: `codigo_conta -> nome_conta`)
    /// para enriquecer `nome_cta_referencia` com `process_cta` conforme `filtrar_reg_i050`.
    pub fn with_plano_referencial(mut self, plano_referencial: HashMap<String, String>) -> Self {
        self.plano_referencial = plano_referencial;
        self
    }

    fn flush_current(&mut self) {
        if let Some(i050) = self.atual_i050.take() {
            let cta_pai = if i050.ind_cta == "S" {
                None
            } else {
                self.atual_cta_pai.clone()
            };
            let nome_ref = self
                .atuais_i051
                .last()
                .and_then(|r51| self.plano_referencial.get(&r51.cod_cta_ref))
                .map(|nome| process_cta(nome));
            let reg = consolidar_reg_i050(
                &i050,
                &self.atuais_i051,
                &self.atuais_i052,
                &self.cnpj,
                &self.hash_arquivo,
                self.data_entrega,
                cta_pai,
                nome_ref,
            );
            self.consolidados.push(reg);
            self.atuais_i051.clear();
            self.atuais_i052.clear();
        }
    }

    pub fn push_i050(&mut self, i050: RegistroI050) {
        self.flush_current();
        if i050.ind_cta == "S" {
            self.ultimo_cta_pai = Some(process_cta(&i050.cta));
            self.atual_cta_pai = None;
        } else {
            self.atual_cta_pai = self.ultimo_cta_pai.clone();
        }
        self.atual_i050 = Some(i050);
    }

    pub fn push_i051(&mut self, i051: RegistroI051) {
        if self.atual_i050.as_ref().is_some_and(|r| r.ind_cta != "S")
            && !self
                .atuais_i051
                .iter()
                .any(|existing| existing.cod_cta_ref == i051.cod_cta_ref)
        {
            self.atuais_i051.push(i051);
        }
    }

    pub fn push_i052(&mut self, i052: RegistroI052) {
        if self.atual_i050.as_ref().is_some_and(|r| r.ind_cta != "S") {
            self.atuais_i052.push(i052);
        }
    }

    pub fn finish(mut self) -> Vec<RegI050> {
        self.flush_current();
        self.consolidados
    }
}

/// Enriquece uma fatia de `RegI050` já consolidada com o mapa `plano_contas_base` (`codigo_conta -> nome_conta`).
pub fn enrich_plano_contas(plano: &mut [RegI050], plano_referencial: &HashMap<String, String>) {
    for item in plano {
        if let Some(cod_ref) = item.cod_cta_ref.as_deref() {
            if let Some(nome_base) = plano_referencial.get(cod_ref) {
                let normalizado = process_cta(nome_base);
                item.nome_cta_referencia = Some(normalizado.clone());
                if let Some(serde_json::Value::Array(arr)) = item.reg051_json.as_mut() {
                    for entry in arr {
                        if let Some(obj) = entry.as_object_mut() {
                            if let Some(c_ref) = obj.get("cod_cta_ref").and_then(|v| v.as_str()) {
                                if let Some(n_ref) = plano_referencial.get(c_ref) {
                                    obj.insert(
                                        "nome_cta_referencia".to_string(),
                                        serde_json::Value::String(process_cta(n_ref)),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Decodifica o Registro `I150` (Saldos Periódicos - Identificação do Período).
pub fn decode_i150(record: &SpedRecord<'_>, reg_0000_id: i64) -> Result<RegistroI150, ParserError> {
    decode_i150_at_line(record, reg_0000_id, 1)
}

pub fn decode_i150_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    line_num: usize,
) -> Result<RegistroI150, ParserError> {
    if record.bloco != b"I150" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I150', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 3 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro I150 incompleto: esperado 3 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let dt_ini = parse_sped_date(c[1], line_num, "DT_INI")?;
    let dt_fim = parse_sped_date(c[2], line_num, "DT_FIM")?;

    Ok(RegistroI150 {
        id: None,
        dt_ini,
        dt_fim,
        reg_0000_id,
    })
}

/// Decodifica o Registro `I155` (Detalhe dos Saldos Periódicos).
pub fn decode_i155(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    reg_i150_id: i64,
) -> Result<RegistroI155, ParserError> {
    decode_i155_at_line(record, reg_0000_id, reg_i150_id, 1)
}

pub fn decode_i155_at_line(
    record: &SpedRecord<'_>,
    reg_0000_id: i64,
    reg_i150_id: i64,
    line_num: usize,
) -> Result<RegistroI155, ParserError> {
    if record.bloco != b"I155" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I155', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 9 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro I155 incompleto: esperado 9 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let cod_cta = decode_field(c[1]);
    if cod_cta.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'COD_CTA' vazio no registro I155".to_string(),
        ));
    }
    let cod_ccus = decode_opt_field(c[2]);
    let vl_sld_ini = parse_sped_decimal(c[3], line_num, "VL_SLD_INI")?;
    let ind_dc_ini = decode_field(c[4]);
    let vl_deb = parse_sped_decimal_or_zero(c[5], line_num, "VL_DEB")?;
    let vl_cred = parse_sped_decimal_or_zero(c[6], line_num, "VL_CRED")?;
    let vl_sld_fim = parse_sped_decimal(c[7], line_num, "VL_SLD_FIN")?;
    let ind_dc_fim = decode_field(c[8]);

    if ind_dc_ini.is_empty() || ind_dc_fim.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Indicador D/C vazio no registro I155".to_string(),
        ));
    }

    Ok(RegistroI155 {
        id: None,
        cod_cta,
        cod_ccus,
        vl_sld_ini,
        ind_dc_ini,
        vl_deb,
        vl_cred,
        vl_sld_fim,
        ind_dc_fim,
        reg_0000_id,
        reg_i150_id,
    })
}

/// Decodifica o Registro `I350` (Saldo das Contas de Resultado Antes do Encerramento – Identificação da Data).
pub fn decode_i350(record: &SpedRecord<'_>) -> Result<NaiveDate, ParserError> {
    decode_i350_at_line(record, 1)
}

pub fn decode_i350_at_line(
    record: &SpedRecord<'_>,
    line_num: usize,
) -> Result<NaiveDate, ParserError> {
    if record.bloco != b"I350" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I350', encontrado '{}'", record.bloco_str()),
        ));
    }
    let c = &record.campos;
    if c.len() < 2 {
        return Err(ParserError::InvalidLine(
            line_num,
            "Registro I350 incompleto: campo DT_RES ausente".to_string(),
        ));
    }
    parse_sped_date(c[1], line_num, "DT_RES")
}

/// Decodifica o Registro `I355` (`RegI355` - Detalhes dos Saldos das Contas de Resultado Antes do Encerramento).
pub fn decode_i355(
    record: &SpedRecord<'_>,
    cnpj: &str,
    hash_arquivo: &str,
    dt_res: NaiveDate,
) -> Result<RegI355, ParserError> {
    decode_i355_at_line(record, cnpj, hash_arquivo, dt_res, 1)
}

pub fn decode_i355_at_line(
    record: &SpedRecord<'_>,
    cnpj: &str,
    hash_arquivo: &str,
    dt_res: NaiveDate,
    line_num: usize,
) -> Result<RegI355, ParserError> {
    if record.bloco != b"I355" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I355', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 5 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro I355 incompleto: esperado ao menos 5 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let cod_cta = decode_field(c[1]);
    if cod_cta.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'COD_CTA' vazio no registro I355".to_string(),
        ));
    }
    let cod_ccus = decode_opt_field(c[2]);
    let vl_cta = parse_sped_decimal(c[3], line_num, "VL_CTA")?;
    let ind_dc = decode_field(c[4]);
    if ind_dc.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'IND_DC' vazio no registro I355".to_string(),
        ));
    }

    let vl_cta_mf = match c.get(5) {
        Some(b) => parse_opt_sped_decimal(b, line_num, "VL_CTA_MF")?,
        None => None,
    };
    let ind_dc_mf = c.get(6).and_then(|b| decode_opt_field(b));

    Ok(RegI355 {
        id: Uuid::now_v7(),
        cnpj: cnpj.to_string(),
        cod_cta,
        nome_cta_referencia: None,
        cod_ccus,
        vl_cta,
        ind_dc,
        vl_cta_mf,
        ind_dc_mf,
        dt_res,
        cod_agl: None,
        cod_cta_ref: None,
        nome_aglutinador: None,
        hash_arquivo: hash_arquivo.to_string(),
    })
}

/// Enriquecimento de `RegI355` cruzando com o plano de contas consolidado (`RegI050`) e a DRE (`RegJ150`),
/// reproduzindo a lógica de `carrega_regi355` do sistema legado (`apps/parametros/utils.py`).
pub fn enrich_i355(reg: &mut RegI355, plano_contas: &[RegI050], dre_j150: &[RegJ150]) {
    let conta = plano_contas.iter().find(|p| p.cod_cta == reg.cod_cta);
    let (nome_conta, cod_agl, cod_cta_ref) = if let Some(c) = conta {
        let nome = c.cta_pai.clone().unwrap_or_else(|| process_cta(&c.cta));
        (nome, c.cod_agl.clone(), c.cod_cta_ref.clone())
    } else {
        ("OUTROS".to_string(), None, None)
    };

    let j150_match = cod_agl.as_ref().and_then(|agl| {
        dre_j150
            .iter()
            .filter(|j| j.cod_agl.as_ref() == Some(agl))
            .min_by_key(|j| j.nu_ordem)
    });

    let nome_aglut = if let Some(j) = j150_match {
        j.descr_cod_agl.clone()
    } else {
        nome_conta.clone()
    };

    if reg.vl_cta_mf.is_none() {
        let vlrecb = match j150_match {
            Some(j) if j.descr_cod_agl == "RECEITA BRUTA" => j.vl_cta_fin,
            _ => Decimal::ZERO,
        };
        reg.vl_cta_mf = Some(vlrecb);
    }

    reg.nome_cta_referencia = Some(nome_conta);
    reg.cod_agl = cod_agl;
    reg.cod_cta_ref = cod_cta_ref;
    reg.nome_aglutinador = Some(nome_aglut);
}

/// Decodifica o Registro `I990` (Encerramento do Bloco I) retornando a quantidade total de linhas do Bloco I.
pub fn decode_i990(record: &SpedRecord<'_>) -> Result<i64, ParserError> {
    decode_i990_at_line(record, 1)
}

pub fn decode_i990_at_line(record: &SpedRecord<'_>, line_num: usize) -> Result<i64, ParserError> {
    if record.bloco != b"I990" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado 'I990', encontrado '{}'", record.bloco_str()),
        ));
    }
    let c = &record.campos;
    if c.len() < 2 {
        return Err(ParserError::InvalidLine(
            line_num,
            "Registro I990 incompleto: campo QTD_LIN_I ausente".to_string(),
        ));
    }
    parse_sped_i64(c[1], line_num, "QTD_LIN_I")
}
