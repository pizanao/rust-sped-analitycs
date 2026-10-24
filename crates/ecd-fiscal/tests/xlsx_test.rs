use std::io::Cursor;

use chrono::NaiveDate;
use ecd_domain::{RegI355, RegJ150, Registro0000, RegistroI155, RegistroJ100};
use ecd_fiscal::{ExportacaoSpedXlsx, SpedXlsxGenerator};
use rust_decimal::Decimal;

#[test]
fn test_geracao_planilha_multi_aba_em_memoria() {
    let cnpj = "12345678000195";
    let razao_social = "EMPRESA DE TESTE MULTI ABA S.A.";

    let mut dados = ExportacaoSpedXlsx::new(cnpj, razao_social);

    // Bloco 0000
    dados.bloco_0000.push(Registro0000 {
        id: Some(1),
        dt_ini: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        dt_fim: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
        nome_razao: razao_social.into(),
        cnpj: cnpj.into(),
        uf: "SP".into(),
        ie: Some("110220330".into()),
        cod_mun: Some(3550308),
        im: None,
        ind_sit_esp: None,
        ind_sit_ini_per: 0,
        ind_nire: 1,
        ind_fin_esc: 0,
        cod_hash_sub: None,
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
        hash_arquivo_id: "HASH_ECD_2023".into(),
    });

    // Bloco J100
    dados.bloco_j100.push(RegistroJ100 {
        id: Some(1),
        cod_agl: "1.01".into(),
        ind_cod_agl: "T".into(),
        nivel_agl: 2,
        cod_agl_sup: Some("1".into()),
        ind_grp_bal: "A".into(),
        descr_cod_agl: "ATIVO CIRCULANTE".into(),
        vl_cta_ini: Decimal::from(100000),
        ind_dc_cta_ini: "D".into(),
        vl_cta_fin: Decimal::from(150000),
        ind_dc_cta_fin: "D".into(),
        nota_exp_ref: Some("Nota 1".into()),
        cnpj: cnpj.into(),
        hash_arquivo_id: Some("HASH_ECD_2023".into()),
        reg_0000_id: 1,
        reg_j005_id: 1,
    });

    // Bloco J150
    dados.bloco_j150.push(RegJ150 {
        id: Some(1),
        nu_ordem: 1,
        cod_agl: Some("3.01".into()),
        ind_cod_agl: "T".into(),
        nivel_agl: 2,
        cod_agl_sup: None,
        descr_cod_agl: "RECEITA BRUTA OPERACIONAL".into(),
        vl_cta_ini: Decimal::ZERO,
        ind_dc_cta_ini: Some("C".into()),
        vl_cta_fin: Decimal::from(500000),
        ind_dc_cta_fin: "C".into(),
        ind_grp_dre: "R".into(),
        nota_exp_ref: None,
        cnpj: cnpj.into(),
        dt_ini: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        dt_fim: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        hash_arquivo: "HASH_ECD_2023".into(),
    });

    // Bloco I155
    dados.bloco_i155.push(RegistroI155 {
        id: Some(1),
        cod_cta: "1.1.01.01".into(),
        cod_ccus: Some("CC01".into()),
        vl_sld_ini: Decimal::from(10000),
        ind_dc_ini: "D".into(),
        vl_deb: Decimal::from(5000),
        vl_cred: Decimal::from(2000),
        vl_sld_fim: Decimal::from(13000),
        ind_dc_fim: "D".into(),
        reg_0000_id: 1,
        reg_i150_id: 1,
    });

    // Bloco I355
    dados.bloco_i355.push(RegI355 {
        id: uuid::Uuid::new_v4(),
        cnpj: cnpj.into(),
        cod_cta: "3.1.01".into(),
        nome_cta_referencia: Some("RECEITA DE VENDAS".into()),
        cod_ccus: None,
        vl_cta: Decimal::from(500000),
        ind_dc: "C".into(),
        vl_cta_mf: None,
        ind_dc_mf: None,
        dt_res: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
        cod_agl: Some("3.1".into()),
        cod_cta_ref: Some("3.01.01.01.01.01".into()),
        nome_aglutinador: None,
        hash_arquivo: "HASH_ECD_2023".into(),
    });

    // 1. Exportar para bytes
    let bytes = SpedXlsxGenerator::exportar_bytes(&dados).expect("Falha ao exportar bytes XLSX");
    assert!(!bytes.is_empty(), "Buffer XLSX gerado não pode ser vazio");

    // 2. Validar assinatura de arquivo ZIP (XLSX é um pacote OpenXML zipado)
    assert_eq!(
        &bytes[0..4],
        &[0x50, 0x4B, 0x03, 0x04],
        "Arquivo deve começar com o cabeçalho de magic bytes PK (ZIP/XLSX)"
    );

    // 3. Ler de volta com umya_spreadsheet para assegurar conformidade do OpenXML
    let mut cursor = Cursor::new(&bytes);
    let book = umya_spreadsheet::reader::xlsx::read_reader(&mut cursor, true)
        .expect("O arquivo gerado deve ser um XLSX válido e legível");

    // 4. Validar abas geradas
    let sheet_names: Vec<&str> = book
        .get_sheet_collection()
        .iter()
        .map(|s| s.get_name())
        .collect();

    assert!(sheet_names.contains(&"0000"), "Aba 0000 deve existir");
    assert!(sheet_names.contains(&"J100"), "Aba J100 deve existir");
    assert!(sheet_names.contains(&"J150"), "Aba J150 deve existir");
    assert!(sheet_names.contains(&"I155"), "Aba I155 deve existir");
    assert!(sheet_names.contains(&"I355"), "Aba I355 deve existir");

    // 5. Validar conteúdo e estilo na aba J150
    let sheet_j150 = book.get_sheet_by_name("J150").unwrap();
    let header_desc = sheet_j150.get_cell_value("G1").get_value();
    assert_eq!(header_desc.as_ref(), "Descrição");

    let row2_desc = sheet_j150.get_cell_value("G2").get_value();
    assert_eq!(row2_desc.as_ref(), "RECEITA BRUTA OPERACIONAL");

    let row2_val_fin = sheet_j150.get_cell_value("J2").get_value_number().unwrap();
    assert_eq!(row2_val_fin, 500000.0);
}

#[test]
fn test_exportar_para_arquivo_temporario() {
    let cnpj = "99888777000122";
    let razao_social = "EMPRESA TEMPORARIA LTDA";
    let dados = ExportacaoSpedXlsx::new(cnpj, razao_social);

    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("teste_sped_{}.xlsx", uuid::Uuid::new_v4()));

    SpedXlsxGenerator::exportar_arquivo(&dados, &temp_file).expect("Falha ao exportar em arquivo");

    assert!(temp_file.exists());
    let metadata = std::fs::metadata(&temp_file).unwrap();
    assert!(metadata.len() > 0);

    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_planilha_vazia_sem_registros() {
    let dados = ExportacaoSpedXlsx::new("00000000000000", "EMPRESA VAZIA");
    let bytes = SpedXlsxGenerator::exportar_bytes(&dados)
        .expect("Deve gerar planilha vazia com cabeçalhos");
    assert!(!bytes.is_empty());

    let mut cursor = Cursor::new(&bytes);
    let book = umya_spreadsheet::reader::xlsx::read_reader(&mut cursor, true)
        .expect("Deve ler planilha vazia gerada");

    assert!(book.get_sheet_by_name("0000").is_some());
    assert!(book.get_sheet_by_name("J100").is_some());
    assert!(book.get_sheet_by_name("J150").is_some());
    assert!(book.get_sheet_by_name("I155").is_some());
}

#[test]
fn test_planilha_caracteres_especiais_e_acentos() {
    let cnpj = "12345678000195";
    let razao_social = "COMÉRCIO & INDÚSTRIA <AÇÃO> \"S.A.\" 'BRASIL'";

    let mut dados = ExportacaoSpedXlsx::new(cnpj, razao_social);
    dados.bloco_j150.push(RegJ150 {
        id: Some(1),
        nu_ordem: 1,
        cod_agl: Some("3.01".into()),
        ind_cod_agl: "T".into(),
        nivel_agl: 2,
        cod_agl_sup: None,
        descr_cod_agl: "RECEITA BRUTA COM CARACTERES: & < > \" ' Ç Ã É".into(),
        vl_cta_ini: Decimal::ZERO,
        ind_dc_cta_ini: Some("C".into()),
        vl_cta_fin: Decimal::from(100),
        ind_dc_cta_fin: "C".into(),
        ind_grp_dre: "R".into(),
        nota_exp_ref: None,
        cnpj: cnpj.into(),
        dt_ini: None,
        dt_fim: None,
        hash_arquivo: "H_SPECIAL".into(),
    });

    let bytes =
        SpedXlsxGenerator::exportar_bytes(&dados).expect("Deve suportar caracteres XML especiais");
    let mut cursor = Cursor::new(&bytes);
    let book = umya_spreadsheet::reader::xlsx::read_reader(&mut cursor, true)
        .expect("Deve ler planilha com caracteres especiais sem corrupção");

    let sheet = book.get_sheet_by_name("J150").unwrap();
    let desc = sheet.get_cell_value("G2").get_value();
    assert_eq!(
        desc.as_ref(),
        "RECEITA BRUTA COM CARACTERES: & < > \" ' Ç Ã É"
    );
}
