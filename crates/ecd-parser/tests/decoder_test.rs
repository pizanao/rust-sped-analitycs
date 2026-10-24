use std::str::FromStr;

use chrono::{NaiveDate, TimeZone, Utc};
use ecd_parser::{
    consolidar_reg_i050, decode_0000, decode_i010, decode_i030, decode_i050, decode_i051,
    decode_i052, decode_i053, decode_i150, decode_i155, decode_i990, PlanoContasConsolidator,
    SpedRecord, SpedStreamReader,
};
use rust_decimal::Decimal;

#[test]
fn test_decode_bloco_0000_and_error_cases() {
    let line = b"|0000|LECD|01012023|31122023|METAL\xDARGICA S\xC3O PAULO S/A|06057223000171|BA|12345678|2927408|998877|0|0|1|1|HASH_ANTERIOR_12345678901234567890123456|1|0||N|N|0|1|1|";
    let rec = SpedRecord::from_line(line).expect("parse 0000 line");
    let reg = decode_0000(&rec, "HASH_ATUAL_40_CHARS_00000000000000000000").expect("decode 0000");

    assert_eq!(reg.dt_ini, NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
    assert_eq!(reg.dt_fim, NaiveDate::from_ymd_opt(2023, 12, 31).unwrap());
    assert_eq!(reg.nome_razao, "METALÚRGICA SÃO PAULO S/A");
    assert_eq!(reg.cnpj, "06057223000171");
    assert_eq!(reg.uf, "BA");
    assert_eq!(reg.ie.as_deref(), Some("12345678"));
    assert_eq!(reg.cod_mun, Some(2927408));
    assert_eq!(reg.im.as_deref(), Some("998877"));
    assert_eq!(reg.ind_sit_esp, Some(0));
    assert_eq!(reg.ind_nire, 1);
    assert_eq!(reg.ind_fin_esc, 1);
    assert_eq!(
        reg.cod_hash_sub.as_deref(),
        Some("HASH_ANTERIOR_12345678901234567890123456")
    );
    assert_eq!(reg.ind_grande_porte, 1);
    assert_eq!(reg.ind_mudanc_pc, 1);
    assert_eq!(reg.cod_plan_ref.as_deref(), Some("1"));
    assert_eq!(
        reg.hash_arquivo_id,
        "HASH_ATUAL_40_CHARS_00000000000000000000"
    );

    // IND_SIT_ESP em branco deve retornar Some(0), conforme legado (apps/core/utils.py)
    let line_blank_sit = b"|0000|LECD|01012023|31122023|METAL\xDARGICA S\xC3O PAULO S/A|06057223000171|BA|12345678|2927408|998877||0|1|1|HASH_ANTERIOR_12345678901234567890123456|1|0||N|N|0|1|1|";
    let rec_blank_sit = SpedRecord::from_line(line_blank_sit).expect("parse 0000 blank sit");
    let reg_blank_sit = decode_0000(&rec_blank_sit, "HASH_ATUAL").expect("decode 0000 blank sit");
    assert_eq!(reg_blank_sit.ind_sit_esp, Some(0));

    // Linha truncada deve retornar erro
    let invalid_rec = SpedRecord::from_line(b"|0000|LECD|01012023|").unwrap();
    assert!(decode_0000(&invalid_rec, "HASH").is_err());
}

#[test]
fn test_decode_i010_and_i030() {
    let rec_i010 = SpedRecord::from_line(b"|I010|G|9.00|").unwrap();
    let i010 = decode_i010(&rec_i010, 42).expect("decode I010");
    assert_eq!(i010.ind_esc, "G");
    assert_eq!(i010.cod_ver_lc, "9.00");
    assert_eq!(i010.reg_0000_id, 42);

    let rec_i030 = SpedRecord::from_line(
        b"|I030|TERMO DE ABERTURA|12|DI\xC1RIO GERAL|5432|EMPRESA EXEMPLO S/A|29300012345|06057223000171|15032010||SALVADOR|31122023|",
    )
    .unwrap();
    let i030 = decode_i030(&rec_i030, 42, Some("HASH123".to_string())).expect("decode I030");
    assert_eq!(i030.num_ord, 12);
    assert_eq!(i030.nat_livr, "DIÁRIO GERAL");
    assert_eq!(i030.qtd_lin, 5432);
    assert_eq!(i030.nome, "EMPRESA EXEMPLO S/A");
    assert_eq!(i030.nire, "29300012345");
    assert_eq!(i030.cnpj, "06057223000171");
    assert_eq!(i030.dt_arq, NaiveDate::from_ymd_opt(2010, 3, 15));
    assert_eq!(i030.dt_arq_conv, None);
    assert_eq!(i030.desc_mun.as_deref(), Some("SALVADOR"));
    assert_eq!(
        i030.dt_ex_social,
        NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()
    );
}

#[test]
fn test_decode_i050_i051_i052_i053_and_consolidate_reg_i050() {
    let data_entrega = Utc.with_ymd_and_hms(2024, 5, 30, 14, 0, 0).unwrap();
    let mut plano_ref = std::collections::HashMap::new();
    plano_ref.insert(
        "3.01.01.01.01.01".to_string(),
        "RECEITA DA VENDA DE BENS E SERVICOS".to_string(),
    );
    let mut consolidator =
        PlanoContasConsolidator::new("06057223000171", "HASH_ARQ_01", data_entrega)
            .with_plano_referencial(plano_ref.clone());

    // Conta sintética (pai)
    let rec_sint =
        SpedRecord::from_line(b"|I050|01012023|04|S|3|3.1.01|3.1|RECEITA BRUTA DE VENDAS|")
            .unwrap();
    let i050_sint = decode_i050(&rec_sint, 1).expect("decode I050 sintetica");
    assert_eq!(i050_sint.ind_cta, "S");
    assert_eq!(i050_sint.nivel, 3);
    consolidator.push_i050(i050_sint);

    // Conta analítica (filha) com I051 (duplicado para testar deduplicação) e I052
    let rec_analit = SpedRecord::from_line(
        b"|I050|01012023|04|A|4|3.1.01.001|3.1.01|VENDA DE MERCADORIAS NO MERCADO INTERNO|",
    )
    .unwrap();
    let i050_analit = decode_i050(&rec_analit, 1).expect("decode I050 analitica");
    let rec_i051 = SpedRecord::from_line(b"|I051|CC_VENDAS|3.01.01.01.01.01|").unwrap();
    let i051 = decode_i051(&rec_i051, 1, 2).expect("decode I051");
    let rec_i051_dup = SpedRecord::from_line(b"|I051|CC_FILIAL|3.01.01.01.01.01|").unwrap();
    let i051_dup = decode_i051(&rec_i051_dup, 1, 2).expect("decode I051 dup");
    let rec_i052 = SpedRecord::from_line(b"|I052|CC_VENDAS|AGL_REC_BRUTA|").unwrap();
    let i052 = decode_i052(&rec_i052, 1, Some(2)).expect("decode I052");

    consolidator.push_i050(i050_analit.clone());
    consolidator.push_i051(i051.clone());
    consolidator.push_i051(i051_dup);
    consolidator.push_i052(i052.clone());

    let consolidados = consolidator.finish();
    assert_eq!(consolidados.len(), 2);

    // Conta sintética não deve ter cta_pai preenchido com o próprio nome
    assert_eq!(consolidados[0].cta_pai, None);

    let analit_cons = &consolidados[1];
    assert_eq!(analit_cons.cod_cta, "3.1.01.001");
    assert_eq!(
        analit_cons.cta_pai.as_deref(),
        Some("RECEITA BRUTA DE VENDAS")
    );
    assert_eq!(analit_cons.cod_ccus_51.as_deref(), Some("CC_VENDAS"));
    assert_eq!(analit_cons.cod_cta_ref.as_deref(), Some("3.01.01.01.01.01"));
    assert_eq!(
        analit_cons.nome_cta_referencia.as_deref(),
        Some("RECEITA DA VENDA DE BENS E SERVICOS")
    );
    assert_eq!(analit_cons.cod_ccus_52.as_deref(), Some("CC_VENDAS"));
    assert_eq!(analit_cons.cod_agl.as_deref(), Some("AGL_REC_BRUTA"));
    assert_eq!(
        analit_cons
            .reg051_json
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(1)
    );
    assert!(analit_cons.reg052_json.is_some());

    // Teste direto da função consolidar_reg_i050
    let direto = consolidar_reg_i050(
        &i050_analit,
        &[i051],
        &[i052],
        "06057223000171",
        "HASH_ARQ_01",
        data_entrega,
        Some("RECEITA BRUTA DE VENDAS".to_string()),
        Some("RECEITA DA VENDA DE BENS E SERVICOS".to_string()),
    );
    assert_eq!(direto.cod_agl.as_deref(), Some("AGL_REC_BRUTA"));
    assert_eq!(
        direto.nome_cta_referencia.as_deref(),
        Some("RECEITA DA VENDA DE BENS E SERVICOS")
    );

    // Teste I053
    let rec_i053 = SpedRecord::from_line(b"|I053|SUB001|1.1.01.002|01|").unwrap();
    let i053 = decode_i053(&rec_i053, 1).expect("decode I053");
    assert_eq!(i053.cod_idt, "SUB001");
    assert_eq!(i053.cod_cnt_corr, "1.1.01.002");
    assert_eq!(i053.nat_sub_cnt, "01");
}

#[test]
fn test_decode_i150_i155_and_i990() {
    let raw = b"|I150|01012023|31012023|\n\
|I155|1.1.01.001|CC01|10000,00|D|5000,50|2000,25|13000,25|D|\n\
|I990|250|\n";
    let reader = SpedStreamReader::from_bytes(raw);
    let recs: Vec<_> = reader.iter_records().collect();
    assert_eq!(recs.len(), 3);

    let i150 = decode_i150(&recs[0], 10).expect("decode I150");
    assert_eq!(i150.dt_ini, NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
    assert_eq!(i150.dt_fim, NaiveDate::from_ymd_opt(2023, 1, 31).unwrap());

    let i155 = decode_i155(&recs[1], 10, 20).expect("decode I155");
    assert_eq!(i155.cod_cta, "1.1.01.001");
    assert_eq!(i155.cod_ccus.as_deref(), Some("CC01"));
    assert_eq!(i155.vl_sld_ini, Decimal::from_str("10000.00").unwrap());
    assert_eq!(i155.ind_dc_ini, "D");
    assert_eq!(i155.vl_deb, Decimal::from_str("5000.50").unwrap());
    assert_eq!(i155.vl_cred, Decimal::from_str("2000.25").unwrap());
    assert_eq!(i155.vl_sld_fim, Decimal::from_str("13000.25").unwrap());
    assert_eq!(i155.ind_dc_fim, "D");

    let qtd_i = decode_i990(&recs[2]).expect("decode I990");
    assert_eq!(qtd_i, 250);
}
