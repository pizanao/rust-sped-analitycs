use std::fs::{self, File};
use std::io::Write;
use std::str::FromStr;

use chrono::NaiveDate;
use ecd_parser::{
    decode_field, decode_latin1, open_mmap, parse_sped_date, parse_sped_decimal,
    parse_sped_decimal_or_zero, process_cta, SpedMmap, SpedStreamReader,
};
use rust_decimal::Decimal;
use uuid::Uuid;

#[test]
fn test_stream_reader_zero_copy_from_mmap() {
    let temp_path = std::env::temp_dir().join(format!("sped_reader_test_{}.txt", Uuid::now_v7()));
    let raw_content = b"|0000|LECD|01012023|31122023|EMPRESA TESTE LTDA|06057223000171|BA|\r\n\
|I010|G|9.00|\n\
|I155|1.01.01|CC01|1500,50|D|200,00|100,25|1600,25|D|\r\n\
LINHA_DE_ASSINATURA_DIGITAL_SEM_PIPE_IGNORADA\n\
|||\n";

    {
        let mut f = File::create(&temp_path).expect("criar arquivo temporário");
        f.write_all(raw_content).expect("escrever fixture SPED");
    }

    let mmap = open_mmap(&temp_path).expect("mapear arquivo SPED via memmap2");
    let reader = SpedStreamReader::from_mmap(&mmap);
    let records: Vec<_> = reader.iter_records().collect();

    assert_eq!(records.len(), 3);
    assert_eq!(records[0].bloco, b"0000");
    assert_eq!(records[0].campos[1], b"LECD");
    assert_eq!(records[0].campos[4], b"EMPRESA TESTE LTDA");

    assert_eq!(records[1].bloco, b"I010");
    assert_eq!(
        records[1].campos,
        vec![&b"I010"[..], &b"G"[..], &b"9.00"[..]]
    );

    assert_eq!(records[2].bloco, b"I155");
    assert_eq!(records[2].campo_str(3).as_deref(), Some("1500,50"));

    fs::remove_file(&temp_path).ok();
}

#[test]
fn test_sped_mmap_empty_file_and_bom() {
    let temp_path = std::env::temp_dir().join(format!("sped_empty_test_{}.txt", Uuid::now_v7()));
    File::create(&temp_path).expect("criar arquivo vazio");

    let empty_mmap = SpedMmap::open(&temp_path).expect("abrir arquivo vazio sem EINVAL");
    assert_eq!(empty_mmap.reader().iter_records().count(), 0);
    assert!(matches!(
        open_mmap(&temp_path),
        Err(ecd_parser::ParserError::MissingHeader)
    ));
    fs::remove_file(&temp_path).ok();

    let with_bom = b"\xEF\xBB\xBF|I010|G|9.00|\r\n|   |CAMPO_IGNORADO|\r\n";
    let reader = SpedStreamReader::from_bytes(with_bom);
    let records: Vec<_> = reader.iter_records_enumerated().collect();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, 1);
    assert_eq!(records[0].1.bloco, b"I010");
}

#[test]
fn test_windows_1252_decoding_and_process_cta() {
    // "COMÉRCIO E PARTICIPAÇÕES SÃO JOÃO S/A" em WINDOWS_1252 / ISO-8859-1
    let (encoded, _, _) =
        encoding_rs::WINDOWS_1252.encode("  COMÉRCIO E PARTICIPAÇÕES SÃO JOÃO S/A  ");
    assert_eq!(
        decode_field(&encoded),
        "COMÉRCIO E PARTICIPAÇÕES SÃO JOÃO S/A"
    );
    assert_eq!(
        decode_latin1(&encoded),
        "  COMÉRCIO E PARTICIPAÇÕES SÃO JOÃO S/A  "
    );

    assert_eq!(
        process_cta("C/ Receita de Prestação de Serviços - Líquida"),
        "RECEITA DE PRESTACAO DE SERVICOS  LIQUIDA"
    );
    assert_eq!(
        process_cta("1ª Parcela - 2º Trimestre (m²) c/c"),
        "1A PARCELA  2O TRIMESTRE M2 CC"
    );
}

#[test]
fn test_parse_sped_dates_and_decimals() {
    let d1 = parse_sped_date(b"01012023", 1, "DT_INI").unwrap();
    assert_eq!(d1, NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());

    let d2 = parse_sped_date(b"20231231", 1, "DT_FIM").unwrap();
    assert_eq!(d2, NaiveDate::from_ymd_opt(2023, 12, 31).unwrap());

    let d3 = parse_sped_date(b"2023-06-15", 1, "DT_ARQ").unwrap();
    assert_eq!(d3, NaiveDate::from_ymd_opt(2023, 6, 15).unwrap());

    assert!(parse_sped_date(b"99999999", 5, "DT_INVALIDA").is_err());
    assert!(parse_sped_date(b"", 5, "DT_VAZIA").is_err());

    let v1 = parse_sped_decimal(b"123456,78", 1, "VL").unwrap();
    assert_eq!(v1, Decimal::from_str("123456.78").unwrap());

    let v2 = parse_sped_decimal(b"1.234.567,89", 1, "VL").unwrap();
    assert_eq!(v2, Decimal::from_str("1234567.89").unwrap());

    let v3 = parse_sped_decimal_or_zero(b"   ", 1, "VL_OPC").unwrap();
    assert_eq!(v3, Decimal::ZERO);

    assert!(parse_sped_decimal(b"ABC,DE", 2, "VL_ERR").is_err());
}
