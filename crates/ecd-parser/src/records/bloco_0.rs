use ecd_domain::Registro0000;

use crate::reader::{
    decode_field, decode_opt_field, parse_opt_sped_i64, parse_sped_date, parse_sped_i32_or,
    SpedRecord,
};
use crate::ParserError;

/// Decodifica o Registro `0000` (Abertura do Arquivo Digital e Identificação da Entidade).
pub fn decode_0000(
    record: &SpedRecord<'_>,
    hash_arquivo_id: &str,
) -> Result<Registro0000, ParserError> {
    decode_0000_at_line(record, hash_arquivo_id, 1)
}

/// Decodifica o Registro `0000` informando o número da linha para diagnóstico de erro.
pub fn decode_0000_at_line(
    record: &SpedRecord<'_>,
    hash_arquivo_id: &str,
    line_num: usize,
) -> Result<Registro0000, ParserError> {
    if record.bloco != b"0000" {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Bloco esperado '0000', encontrado '{}'", record.bloco_str()),
        ));
    }

    let c = &record.campos;
    if c.len() < 7 {
        return Err(ParserError::InvalidLine(
            line_num,
            format!(
                "Registro 0000 incompleto: esperado ao menos 7 campos, encontrado {}",
                c.len()
            ),
        ));
    }

    let dt_ini = parse_sped_date(c[2], line_num, "DT_INI")?;
    let dt_fim = parse_sped_date(c[3], line_num, "DT_FIM")?;
    let nome_razao = decode_field(c[4]);
    if nome_razao.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'NOME' (nome_razao) vazio no registro 0000".to_string(),
        ));
    }

    let raw_cnpj = decode_field(c[5]);
    let cnpj: String = raw_cnpj.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if cnpj.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'CNPJ' vazio ou inválido no registro 0000".to_string(),
        ));
    }

    let uf = decode_field(c[6]);
    if uf.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            "Campo obrigatório 'UF' vazio no registro 0000".to_string(),
        ));
    }

    let ie = c.get(7).and_then(|b| decode_opt_field(b));
    let cod_mun = match c.get(8) {
        Some(b) => parse_opt_sped_i64(b, line_num, "COD_MUN")?,
        None => None,
    };
    let im = c.get(9).and_then(|b| decode_opt_field(b));
    let ind_sit_esp = match c.get(10) {
        Some(b) => Some(parse_sped_i32_or(b, 0, line_num, "IND_SIT_ESP")?),
        None => Some(0),
    };
    let ind_sit_ini_per = match c.get(11) {
        Some(b) => parse_sped_i32_or(b, 0, line_num, "IND_SIT_INI_PER")?,
        None => 0,
    };
    let ind_nire = match c.get(12) {
        Some(b) => parse_sped_i32_or(b, 0, line_num, "IND_NIRE")?,
        None => 0,
    };
    let ind_fin_esc = match c.get(13) {
        Some(b) => parse_sped_i32_or(b, 0, line_num, "IND_FIN_ESC")?,
        None => 0,
    };
    let cod_hash_sub = c.get(14).and_then(|b| decode_opt_field(b));
    let ind_grande_porte = match c.get(15) {
        Some(b) => parse_sped_i32_or(b, 0, line_num, "IND_GRANDE_PORTE")?,
        None => 0,
    };
    let tip_ecd = match c.get(16) {
        Some(b) => parse_sped_i32_or(b, 0, line_num, "TIP_ECD")?,
        None => 0,
    };
    let cod_scp = c
        .get(17)
        .and_then(|b| decode_opt_field(b))
        .map(|s| {
            s.chars()
                .filter(|ch| ch.is_ascii_digit())
                .collect::<String>()
        })
        .filter(|s| !s.is_empty());
    let ident_mf = c
        .get(18)
        .and_then(|b| decode_opt_field(b))
        .unwrap_or_else(|| "N".to_string());
    let ind_esc_cons = c
        .get(19)
        .and_then(|b| decode_opt_field(b))
        .unwrap_or_else(|| "N".to_string());
    let ind_centralizada = match c.get(20) {
        Some(b) => parse_sped_i32_or(b, 0, line_num, "IND_CENTRALIZADA")?,
        None => 0,
    };
    let ind_mudanc_pc = match c.get(21) {
        Some(b) => parse_sped_i32_or(b, 0, line_num, "IND_MUDANC_PC")?,
        None => 0,
    };
    let cod_plan_ref = c.get(22).and_then(|b| decode_opt_field(b));

    Ok(Registro0000 {
        id: None,
        dt_ini,
        dt_fim,
        nome_razao,
        cnpj,
        uf,
        ie,
        cod_mun,
        im,
        ind_sit_esp,
        ind_sit_ini_per,
        ind_nire,
        ind_fin_esc,
        cod_hash_sub,
        ind_grande_porte,
        tip_ecd,
        cod_scp,
        ident_mf,
        ind_esc_cons,
        ind_centralizada,
        ind_mudanc_pc,
        cod_plan_ref,
        i990: 0,
        j990: 0,
        hash_arquivo_id: hash_arquivo_id.to_string(),
    })
}
