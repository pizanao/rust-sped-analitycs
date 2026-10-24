use std::fs::{self, File};
use std::io::Write;
use std::str::FromStr;

use chrono::{NaiveDate, TimeZone, Utc};
use ecd_parser::{
    decode_i350, decode_i355, decode_j005, decode_j100, decode_j150, decode_j210, decode_j215,
    decode_j990, ParserError, SpedDocument, SpedRecord, SpedStreamReader,
};
use rust_decimal::Decimal;
use uuid::Uuid;

#[test]
fn test_decode_i350_i355_and_bloco_j_records() {
    let rec_i350 = SpedRecord::from_line(b"|I350|31122023|").unwrap();
    let dt_res = decode_i350(&rec_i350).expect("decode I350");
    assert_eq!(dt_res, NaiveDate::from_ymd_opt(2023, 12, 31).unwrap());

    let rec_i355 = SpedRecord::from_line(b"|I355|3.1.01.001|CC01|450000,75|C|").unwrap();
    let i355 = decode_i355(&rec_i355, "06057223000171", "HASH_TESTE", dt_res).expect("decode I355");
    assert_eq!(i355.cod_cta, "3.1.01.001");
    assert_eq!(i355.cod_ccus.as_deref(), Some("CC01"));
    assert_eq!(i355.vl_cta, Decimal::from_str("450000.75").unwrap());
    assert_eq!(i355.ind_dc, "C");
    assert_eq!(i355.dt_res, dt_res);

    let rec_j005 =
        SpedRecord::from_line(b"|J005|01012023|31122023|1|DEMONSTRACOES INDIVIDUALIZADAS|")
            .unwrap();
    let j005 = decode_j005(
        &rec_j005,
        1,
        "06057223000171",
        Some("HASH_TESTE".to_string()),
    )
    .expect("decode J005");
    assert_eq!(j005.id_dem, 1);
    assert_eq!(
        j005.cab_dem.as_deref(),
        Some("DEMONSTRACOES INDIVIDUALIZADAS")
    );

    let rec_j100 = SpedRecord::from_line(
        b"|J100|AGL_AT_CIRC|T|2|AGL_ATIVO|A|ATIVO CIRCULANTE|250000,00|D|310000,40|D|NE_01|",
    )
    .unwrap();
    let j100 = decode_j100(
        &rec_j100,
        1,
        10,
        "06057223000171",
        Some("HASH_TESTE".to_string()),
    )
    .expect("decode J100");
    assert_eq!(j100.cod_agl, "AGL_AT_CIRC");
    assert_eq!(j100.ind_cod_agl, "T");
    assert_eq!(j100.nivel_agl, 2);
    assert_eq!(j100.cod_agl_sup.as_deref(), Some("AGL_ATIVO"));
    assert_eq!(j100.ind_grp_bal, "A");
    assert_eq!(j100.descr_cod_agl, "ATIVO CIRCULANTE");
    assert_eq!(j100.vl_cta_ini, Decimal::from_str("250000.00").unwrap());
    assert_eq!(j100.ind_dc_cta_ini, "D");
    assert_eq!(j100.vl_cta_fin, Decimal::from_str("310000.40").unwrap());
    assert_eq!(j100.ind_dc_cta_fin, "D");
    assert_eq!(j100.nota_exp_ref.as_deref(), Some("NE_01"));

    let rec_j150 = SpedRecord::from_line(
        b"|J150|1|AGL_REC_BRUTA|T|1||(=) (+) Receita Bruta de Vendas / Servi\xE7os - Mercado Interno: #Matriz|300000,00|C|450000,75|C|R|NE_02|",
    )
    .unwrap();
    let j150 = decode_j150(
        &rec_j150,
        "06057223000171",
        "HASH_TESTE",
        Some(j005.dt_ini),
        Some(j005.dt_fim),
    )
    .expect("decode J150");
    assert_eq!(j150.nu_ordem, 1);
    assert_eq!(j150.cod_agl.as_deref(), Some("AGL_REC_BRUTA"));
    assert_eq!(j150.ind_cod_agl, "T");
    assert_eq!(j150.nivel_agl, 1);
    assert_eq!(
        j150.descr_cod_agl,
        "RECEITA BRUTA DE VENDAS   SERVIÇOS   MERCADO INTERNO  MATRIZ"
    );
    assert_eq!(j150.vl_cta_ini, Decimal::from_str("300000.00").unwrap());
    assert_eq!(j150.ind_dc_cta_ini.as_deref(), Some("C"));
    assert_eq!(j150.vl_cta_fin, Decimal::from_str("450000.75").unwrap());
    assert_eq!(j150.ind_dc_cta_fin, "C");
    assert_eq!(j150.ind_grp_dre, "R");
    assert_eq!(j150.nota_exp_ref.as_deref(), Some("NE_02"));

    let rec_j210 = SpedRecord::from_line(
        b"|J210|0|AGL_LPA|LUCROS OU PREJU\xCDZOS ACUMULADOS|50000,00|C|95000,00|C|NE_03|",
    )
    .unwrap();
    let j210 = decode_j210(
        &rec_j210,
        Some(1),
        "06057223000171",
        Some("HASH_TESTE".to_string()),
    )
    .expect("decode J210");
    assert_eq!(j210.ind_tip, "0");
    assert_eq!(j210.descr_cod_agl, "LUCROS OU PREJUÍZOS ACUMULADOS");
    assert_eq!(j210.vl_cta_fin, Decimal::from_str("95000.00").unwrap());

    let rec_j215 =
        SpedRecord::from_line(b"|J215|HIST_001|CONSTITUI\xC7\xC3O DE RESERVA LEGAL|45000,00|C|")
            .unwrap();
    let j215 = decode_j215(
        &rec_j215,
        Some(1),
        "06057223000171",
        Some("HASH_TESTE".to_string()),
    )
    .expect("decode J215");
    assert_eq!(j215.cod_hist_fat, "HIST_001");
    assert_eq!(j215.desc_fat, "CONSTITUIÇÃO DE RESERVA LEGAL");
    assert_eq!(j215.vl_fat_cont, Decimal::from_str("45000.00").unwrap());
    assert_eq!(j215.ind_dc_fat, "C");

    let rec_j990 = SpedRecord::from_line(b"|J990|85|").unwrap();
    assert_eq!(decode_j990(&rec_j990).unwrap(), 85);
}

#[test]
fn test_full_sped_fixture_windows_1252_and_enrichment() {
    let fixture_text = "\
|0000|LECD|01012023|31122023|INDÚSTRIA E COMÉRCIO SÃO JORGE S/A|06057223000171|BA|123456789|2927408|654321||0|1|0||0|0||N|N|0|0|1|\r\n\
|I001|0|\r\n\
|I010|G|9.00|\r\n\
|I030|TERMO DE ABERTURA|1|DIÁRIO GERAL|120|INDÚSTRIA E COMÉRCIO SÃO JORGE S/A|29300012345|06057223000171|10012015||SALVADOR|31122023|\r\n\
|I050|01012023|04|S|3|3.1.01|3.1|RECEITA OPERACIONAL BRUTA|\r\n\
|I050|01012023|04|A|4|3.1.01.001|3.1.01|RECEITA DE PRESTAÇÃO DE SERVIÇOS|\r\n\
|I051|CC01|3.01.01.01.01.02|\r\n\
|I052|CC01|AGL_REC_SERV|\r\n\
|I053|IDT01|3.1.01.001|01|\r\n\
|I150|01012023|31122023|\r\n\
|I155|3.1.01.001|CC01|0,00|C|1000,00|850000,50|849000,50|C|\r\n\
|I350|31122023|\r\n\
|I355|3.1.01.001|CC01|849000,50|C|\r\n\
|I990|12|\r\n\
|J001|0|\r\n\
|J005|01012023|31122023|1|DEMONSTRAÇÕES CONTÁBEIS|\r\n\
|J100|AGL_ATIVO|T|1||A|ATIVO TOTAL|500000,00|D|950000,00|D|NE01|\r\n\
|J150|1|AGL_REC_SERV|T|1||RECEITA BRUTA||C|849000,50|C|R|NE02|\r\n\
|J210|0|AGL_LPA|LUCROS ACUMULADOS|100000,00|C|250000,00|C||\r\n\
|J215|H01|LUCRO DO EXERCÍCIO|150000,00|C|\r\n\
|J990|7|\r\n";

    let (encoded_bytes, _, _) = encoding_rs::WINDOWS_1252.encode(fixture_text);
    let temp_path = std::env::temp_dir().join(format!("sped_full_fixture_{}.txt", Uuid::now_v7()));
    {
        let mut f = File::create(&temp_path).expect("criar arquivo fixture");
        f.write_all(&encoded_bytes)
            .expect("gravar fixture WINDOWS_1252");
    }

    let data_entrega = Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
    let doc = SpedDocument::parse_file(
        &temp_path,
        "4AC602DABC8D1F3105A51FCD2C7ACA30C016A78F",
        data_entrega,
    )
    .expect("parse_file completo");

    fs::remove_file(&temp_path).ok();

    assert_eq!(
        doc.reg_0000.nome_razao,
        "INDÚSTRIA E COMÉRCIO SÃO JORGE S/A"
    );
    assert_eq!(doc.reg_0000.cnpj, "06057223000171");
    assert_eq!(doc.reg_0000.i990, 12);
    assert_eq!(doc.reg_0000.j990, 7);

    assert_eq!(doc.reg_i010.len(), 1);
    assert_eq!(doc.reg_i030.len(), 1);
    assert_eq!(doc.reg_i050.len(), 2);
    assert_eq!(doc.reg_i051.len(), 1);
    assert_eq!(doc.reg_i052.len(), 1);
    assert_eq!(doc.reg_i053.len(), 1);
    assert_eq!(doc.plano_contas.len(), 2);
    assert_eq!(doc.reg_i150.len(), 1);
    assert_eq!(doc.reg_i155.len(), 1);
    assert_eq!(doc.reg_i355.len(), 1);
    assert_eq!(doc.reg_j005.len(), 1);
    assert_eq!(doc.reg_j100.len(), 1);
    assert_eq!(doc.reg_j150.len(), 1);
    assert_eq!(doc.reg_j210.len(), 1);
    assert_eq!(doc.reg_j215.len(), 1);

    // Verifica enriquecimento automático de RegI355 com RegI050 e RegJ150
    let r355 = &doc.reg_i355[0];
    assert_eq!(
        r355.nome_cta_referencia.as_deref(),
        Some("RECEITA OPERACIONAL BRUTA")
    );
    assert_eq!(r355.cod_agl.as_deref(), Some("AGL_REC_SERV"));
    assert_eq!(r355.cod_cta_ref.as_deref(), Some("3.01.01.01.01.02"));
    assert_eq!(r355.nome_aglutinador.as_deref(), Some("RECEITA BRUTA"));
    assert_eq!(
        r355.vl_cta_mf,
        Some(Decimal::from_str("849000.50").unwrap())
    );

    // Verifica MissingHeader quando o arquivo não começa com 0000
    let sem_header = SpedStreamReader::from_bytes(b"|I010|G|9.00|\n");
    let err = SpedDocument::parse_from_reader(&sem_header, "HASH", data_entrega).unwrap_err();
    assert!(matches!(err, ParserError::MissingHeader));
}
