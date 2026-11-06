//! Empirical Challenger Adversarial Test Suite for Milestone 2
//!
//! Stress-tests:
//! 1. `parse_brl_float` against Brazilian currency formatting edge cases
//! 2. `synthesize_cnpj` against edge case padding
//! 3. Windows-1252 accent decoding against Brazilian Portuguese characters & stream boundaries
//! 4. August 2026 radical expansion vs fallback logic

use rusqlite::Connection;
use std::borrow::Cow;
use std::io::{Cursor, Read};

use rfb_cnpj_etl::config::IngestionPragmas;
use rfb_cnpj_etl::ingestion::loader::{
    insert_audit_metadata, materialize_cnpj_base2matriz, transform_socios_and_expand_radicals,
};
use rfb_cnpj_etl::ingestion::schema::{create_initial_tables, create_post_load_indices};
use rfb_cnpj_etl::ingestion::transforms::{
    decode_opt_windows_1252, decode_trimmed_str, decode_windows_1252, parse_brl_float,
    parse_brl_float_opt, parse_rfb_reference_code, synthesize_cnpj, synthesize_cnpj_bytes,
    Windows1252Reader,
};

// =========================================================================
// 1. EMPIRICAL STRESS TESTS: parse_brl_float
// =========================================================================

#[test]
fn test_parse_brl_float_standard_and_extreme_cases() {
    // Standard RFB cases
    assert_eq!(parse_brl_float(b"1.234.567,89"), 1234567.89);
    assert_eq!(parse_brl_float(b"0,00"), 0.0);
    assert_eq!(parse_brl_float(b""), 0.0);
    assert_eq!(parse_brl_float(b"  500,00 "), 500.0);
    assert_eq!(parse_brl_float(b"-250,75"), -250.75);
    assert_eq!(parse_brl_float(b"-1.234.567,89"), -1234567.89);

    // Zero variations
    assert_eq!(parse_brl_float(b"0"), 0.0);
    assert_eq!(parse_brl_float(b"0.00"), 0.0);
    assert_eq!(parse_brl_float(b"0,0"), 0.0);
    assert_eq!(parse_brl_float(b"000000,00"), 0.0);
    assert_eq!(parse_brl_float(b"-0,00"), 0.0);

    // Whitespace variations
    assert_eq!(parse_brl_float(b"   "), 0.0);
    assert_eq!(parse_brl_float(b"\t\r\n"), 0.0);
    assert_eq!(parse_brl_float(b"   100,50   "), 100.50);

    // Small decimals
    assert_eq!(parse_brl_float(b"0,01"), 0.01);
    assert_eq!(parse_brl_float(b",50"), 0.50);
    assert_eq!(parse_brl_float(b".50"), 0.50);

    // Millions / Billions with multiple dots
    assert_eq!(parse_brl_float(b"10.000.000.000,00"), 10000000000.0);
    assert_eq!(parse_brl_float(b"1.000.000"), 1000000.0);

    // Positive sign
    assert_eq!(parse_brl_float(b"+1.500,00"), 1500.0);
    assert_eq!(parse_brl_float(b"+0,50"), 0.50);

    // Standard dot float fallback
    assert_eq!(parse_brl_float(b"1500.50"), 1500.50);
    assert_eq!(parse_brl_float(b"0.99"), 0.99);

    // Invalid string handling: must not panic, should return 0.0
    assert_eq!(parse_brl_float(b"invalid"), 0.0);
    assert_eq!(parse_brl_float(b"N/A"), 0.0);
    assert_eq!(parse_brl_float(b"null"), 0.0);
    assert_eq!(parse_brl_float(b"--100"), 0.0);
    assert_eq!(parse_brl_float(b",,"), 0.0);

    // Currency prefix tolerance (embedded letters stripped by digit/symbol filter)
    assert_eq!(parse_brl_float(b"R$ 1.500,00"), 1500.0);
}

#[test]
fn test_parse_brl_float_buffer_overflow_fallback() {
    // Tests strings longer than 64 bytes hitting parse_brl_float_str
    let long_padded = b"                                                   1.234.567,89        ";
    assert!(long_padded.len() > 64);
    // Even if slice is > 64, after trim it fits in 64
    assert_eq!(parse_brl_float(long_padded), 1234567.89);

    // A genuinely long numeric string >= 64 chars
    let huge_num = b"1000000000000000000000000000000000000000000000000000000000000,00";
    assert!(huge_num.len() >= 64);
    let val = parse_brl_float(huge_num);
    assert_eq!(val, 1e60);
}

#[test]
fn test_parse_brl_float_opt() {
    assert_eq!(parse_brl_float_opt(b""), None);
    assert_eq!(parse_brl_float_opt(b"   "), None);
    assert_eq!(parse_brl_float_opt(b"150,00"), Some(150.0));
    assert_eq!(parse_brl_float_opt(b"0,00"), Some(0.0));
}

// =========================================================================
// 2. EMPIRICAL STRESS TESTS: synthesize_cnpj
// =========================================================================

#[test]
fn test_synthesize_cnpj_padding_and_edge_cases() {
    // Mandatory assignment case: "1", "1", "1" -> "00000001000101"
    assert_eq!(synthesize_cnpj(b"1", b"1", b"1"), "00000001000101");

    // Standard cases
    assert_eq!(
        synthesize_cnpj(b"00000000", b"0001", b"91"),
        "00000000000191"
    );
    assert_eq!(
        synthesize_cnpj(b"33000167", b"0001", b"01"),
        "33000167000101"
    );

    // Empty fields: all padded with zeros
    assert_eq!(synthesize_cnpj(b"", b"", b""), "00000000000000");

    // Partial empty
    assert_eq!(synthesize_cnpj(b"12345", b"", b"9"), "00012345000009");

    // Spaces around inputs
    assert_eq!(
        synthesize_cnpj(b"  12345678  ", b" 0002 ", b" 99 "),
        "12345678000299"
    );

    // Non-numeric bytes in input are padded and copied
    let raw_res = synthesize_cnpj(b"ABC", b"X", b"9");
    assert_eq!(raw_res, "00000ABC000X09");
    assert_eq!(raw_res.len(), 14);

    // Direct byte function check
    let b_res = synthesize_cnpj_bytes(b"1", b"1", b"1");
    assert_eq!(&b_res, b"00000001000101");
}

#[test]
fn test_synthesize_cnpj_overflow_clamping() {
    // Fields longer than standard slot:
    // basico > 8, ordem > 4, dv > 2
    // pad_copy_right_aligned takes the trailing slice of length dest_len
    let res = synthesize_cnpj(b"9912345678", b"880001", b"7799");
    assert_eq!(res, "12345678000199");
    assert_eq!(res.len(), 14);
}

// =========================================================================
// 3. EMPIRICAL STRESS TESTS: Windows-1252 accent decoding & streaming
// =========================================================================

#[test]
fn test_windows_1252_all_brazilian_accents() {
    // Exhaustive test of Brazilian Portuguese characters in Windows-1252:
    // ç, Ç, ã, Ã, õ, Õ, á, Á, é, É, í, Í, ó, Ó, ú, Ú, â, Â, ê, Ê, ô, Ô, à, À, ü, Ü, º, ª
    let win1252_bytes: [u8; 28] = [
        0xE7, // ç
        0xC7, // Ç
        0xE3, // ã
        0xC3, // Ã
        0xF5, // õ
        0xD5, // Õ
        0xE1, // á
        0xC1, // Á
        0xE9, // é
        0xC9, // É
        0xED, // í
        0xCD, // Í
        0xF3, // ó
        0xD3, // Ó
        0xFA, // ú
        0xDA, // Ú
        0xE2, // â
        0xC2, // Â
        0xEA, // ê
        0xCA, // Ê
        0xF4, // ô
        0xD4, // Ô
        0xE0, // à
        0xC0, // À
        0xFC, // ü
        0xDC, // Ü
        0xBA, // º
        0xAA, // ª
    ];

    let expected = "çÇãÃõÕáÁéÉíÍóÓúÚâÂêÊôÔàÀüÜºª";
    let decoded = decode_windows_1252(&win1252_bytes);
    assert_eq!(decoded, expected);
    assert!(matches!(decoded, Cow::Owned(_)));
}

#[test]
fn test_windows_1252_real_world_corporate_names() {
    // "MINERAÇÃO E SIDERÚRGICA ESPÍRITO SANTO S/A"
    let mut raw = Vec::new();
    raw.extend_from_slice(b"MINERA");
    raw.push(0xC7); // Ç
    raw.push(0xC3); // Ã
    raw.extend_from_slice(b"O E SIDER");
    raw.push(0xDA); // Ú
    raw.extend_from_slice(b"RGICA ESP");
    raw.push(0xCD); // Í
    raw.extend_from_slice(b"RITO SANTO S/A");

    let decoded = decode_trimmed_str(&raw);
    assert_eq!(decoded, "MINERAÇÃO E SIDERÚRGICA ESPÍRITO SANTO S/A");

    // Pure ASCII check: must return Cow::Borrowed (zero allocation)
    let ascii_input = b"PETROLEO BRASILEIRO S A PETROBRAS";
    let cow = decode_windows_1252(ascii_input);
    assert_eq!(cow, "PETROLEO BRASILEIRO S A PETROBRAS");
    assert!(matches!(cow, Cow::Borrowed(_)));

    // Optional decode check
    assert_eq!(decode_opt_windows_1252(b""), None);
    assert_eq!(decode_opt_windows_1252(b"   "), None);
    assert_eq!(
        decode_opt_windows_1252(b"S\xC3O PAULO"),
        Some("SÃO PAULO".to_string())
    );
}

#[test]
fn test_windows_1252_reader_streaming_chunk_boundaries() {
    // Test streaming reader across very small buffer chunks (1 to 5 bytes)
    // to verify multibyte UTF-8 transitions across internal buffer boundaries.
    let phrase = "COMÉRCIO, INDÚSTRIA & MINERAÇÃO DE SÃO PAULO - Nº 10, 2º ANDAR";
    let (encoded_bytes, _, _) = encoding_rs::WINDOWS_1252.encode(phrase);

    for chunk_size in [1, 2, 3, 5, 7, 16, 64] {
        let cursor = Cursor::new(encoded_bytes.as_ref());
        let mut reader = Windows1252Reader::new(cursor);
        let mut result = Vec::new();
        let mut buf = vec![0u8; chunk_size];

        loop {
            let n = reader.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            result.extend_from_slice(&buf[..n]);
        }

        let decoded_str = String::from_utf8(result).expect("valid UTF-8 stream");
        assert_eq!(decoded_str, phrase, "Failed with chunk size {}", chunk_size);
    }
}

// =========================================================================
// 4. EMPIRICAL STRESS TESTS: August 2026 radical expansion vs fallback
// =========================================================================

#[test]
fn test_august_2026_radical_expansion_matrix() {
    let mut conn = Connection::open_in_memory().unwrap();
    IngestionPragmas::default().apply_to(&conn).unwrap();
    create_initial_tables(&conn).unwrap();

    // 1. Seed estabelecimento with:
    //    - Company A: Matriz 11111111000199 (matriz_filial = '1')
    //    - Company B (Partner): Matriz 22222222000188 (matriz_filial = '1')
    //    - Company C: Filial only! 33333333000277 (matriz_filial = '2', NO matriz in dataset!)
    //    - Company D: Two establishments: Filial 44444444000266 (matriz_filial = '2')
    //                 and Matriz 44444444000155 (matriz_filial = '1')
    conn.execute_batch(
        "INSERT INTO estabelecimento (cnpj_basico, cnpj_ordem, cnpj_dv, matriz_filial, cnpj) VALUES
            ('11111111', '0001', '99', '1', '11111111000199'),
            ('22222222', '0001', '88', '1', '22222222000188'),
            ('33333333', '0002', '77', '2', '33333333000277'),
            ('44444444', '0002', '66', '2', '44444444000266'),
            ('44444444', '0001', '55', '1', '44444444000155');",
    )
    .unwrap();

    // 2. Materialize lookup cache
    let matrizes_inserted = materialize_cnpj_base2matriz(&mut conn).unwrap();
    // Expected: 11111111, 22222222, 44444444 (3 distinct matrizes)
    assert_eq!(matrizes_inserted, 3);

    // Verify Company C (filial only) is NOT in cnpj_base2matriz
    let c_matriz: Option<String> = conn
        .query_row(
            "SELECT cnpj FROM cnpj_base2matriz WHERE cnpj_basico = '33333333'",
            [],
            |r| r.get(0),
        )
        .ok();
    assert_eq!(c_matriz, None);

    // 3. Seed socios_original with 6 diverse cases:
    conn.execute_batch(
        "INSERT INTO socios_original (
            cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio,
            qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
            nome_representante, qualificacao_representante_legal, faixa_etaria
        ) VALUES
        -- Case A: Pre-2026 PJ partner (14-digit CNPJ already present: '55555555000144')
        ('11111111', '1', 'PRE-2026 HOLDING S/A', '55555555000144', '10', '20150101', '', '', '', '', '0'),

        -- Case B: Post-2026 PJ partner with matching Matriz in cnpj_base2matriz (radical: '22222222')
        ('11111111', '1', 'POST-2026 EXPANDABLE LTDA', '22222222', '10', '20260901', '', '', '', '', '0'),

        -- Case C: Post-2026 PJ partner with NO matching Matriz (radical: '99999999' not in dataset)
        ('11111111', '1', 'POST-2026 UNMATCHED RADICAL', '99999999', '10', '20260901', '', '', '', '', '0'),

        -- Case D: PF partner (identificador 2, 11-digit masked CPF: '***456789**')
        ('11111111', '2', 'CARLOS ALBERTO SILVA', '***456789**', '05', '20200510', '', '', '', '', '5'),

        -- Case E: Foreign partner (identificador 3, empty doc or foreign ID)
        ('11111111', '3', 'GLOBAL CAPITAL GROUP LTD', '', '05', '20220315', '249', '***000000**', 'REP BRASIL', '05', '0'),

        -- Case F: Company without Matriz in dataset ('33333333') having a partner
        ('33333333', '1', 'PARTNER OF NO-MATRIZ CO', '44444444', '10', '20260901', '', '', '', '', '0');",
    )
    .unwrap();

    // 4. Execute transform and radical expansion
    let final_socios_count = transform_socios_and_expand_radicals(&mut conn).unwrap();
    assert_eq!(final_socios_count, 6);

    // Verify Case A: Pre-2026 PJ partner (14 digits) is PRESERVED exactly
    let doc_a: String = conn
        .query_row(
            "SELECT cnpj_cpf_socio FROM socios WHERE nome_socio = 'PRE-2026 HOLDING S/A'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        doc_a, "55555555000144",
        "Pre-2026 14-digit CNPJ must be preserved"
    );

    // Verify Case B: Post-2026 PJ partner (8-digit radical) EXPANDED to 14-digit Matriz CNPJ
    let doc_b: String = conn
        .query_row(
            "SELECT cnpj_cpf_socio FROM socios WHERE nome_socio = 'POST-2026 EXPANDABLE LTDA'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        doc_b, "22222222000188",
        "Post-2026 8-digit radical must be expanded to 14-digit Matriz CNPJ"
    );

    // Verify Case C: Post-2026 PJ partner without Matriz FALLS BACK to 8-digit radical via COALESCE
    let doc_c: String = conn
        .query_row(
            "SELECT cnpj_cpf_socio FROM socios WHERE nome_socio = 'POST-2026 UNMATCHED RADICAL'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        doc_c, "99999999",
        "Post-2026 PJ partner with no matching Matriz must retain 8-digit radical"
    );

    // Verify Case D: PF partner CPF remains UNTOUCHED
    let doc_d: String = conn
        .query_row(
            "SELECT cnpj_cpf_socio FROM socios WHERE nome_socio = 'CARLOS ALBERTO SILVA'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(doc_d, "***456789**", "PF partner CPF must remain untouched");

    // Verify Case E: Foreign partner remains UNTOUCHED
    let doc_e: String = conn
        .query_row(
            "SELECT cnpj_cpf_socio FROM socios WHERE nome_socio = 'GLOBAL CAPITAL GROUP LTD'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(doc_e, "", "Foreign partner doc must remain untouched");

    // Verify Case F: Company '33333333' has NULL socios.cnpj because it has no Matriz,
    // but its partner '44444444' IS expanded to '44444444000155'
    let (company_cnpj, partner_cnpj): (Option<String>, String) = conn
        .query_row(
            "SELECT cnpj, cnpj_cpf_socio FROM socios WHERE nome_socio = 'PARTNER OF NO-MATRIZ CO'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        company_cnpj, None,
        "Company without Matriz must have NULL socios.cnpj"
    );
    assert_eq!(
        partner_cnpj, "44444444000155",
        "Partner radical must expand to Matriz even if parent company has no Matriz"
    );

    // Verify temporary staging table socios_original was dropped
    let original_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='socios_original'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        !original_exists,
        "Staging table socios_original must be dropped"
    );

    // Create post-load indices & verify indices exist
    create_post_load_indices(&conn).unwrap();

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_socios%'")
        .unwrap();
    let index_names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert!(index_names.contains(&"idx_socios_cnpj".to_string()));
    assert!(index_names.contains(&"idx_socios_cnpj_cpf_socio".to_string()));
    assert!(index_names.contains(&"idx_socios_nome_socio".to_string()));
    assert!(index_names.contains(&"idx_socios_representante".to_string()));
    assert!(index_names.contains(&"idx_socios_representante_nome".to_string()));
}

#[test]
fn test_audit_metadata_insertion() {
    let mut conn = Connection::open_in_memory().unwrap();
    create_initial_tables(&conn).unwrap();

    insert_audit_metadata(&mut conn, "24/10/2026", 54321).unwrap();

    let ref_date: String = conn
        .query_row(
            "SELECT valor FROM _referencia WHERE referencia = 'CNPJ'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ref_date, "24/10/2026");

    let count: String = conn
        .query_row(
            "SELECT valor FROM _referencia WHERE referencia = 'cnpj_qtde'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, "54321");
}

#[test]
fn test_parse_rfb_reference_code_variations() {
    assert_eq!(
        parse_rfb_reference_code("K3241.K03200Y0.D30610.EMPRECSV"),
        Some(("10/06/2023".to_string(), "202306".to_string()))
    );
    assert_eq!(
        parse_rfb_reference_code("F.K03200$Z.D60810.ESTABELE"),
        Some(("10/08/2026".to_string(), "202608".to_string()))
    );
    assert_eq!(
        parse_rfb_reference_code("F.K03200$Z.D61024.SOCIOCSV"),
        Some(("24/10/2026".to_string(), "202610".to_string()))
    );
    assert_eq!(
        parse_rfb_reference_code("F.K03200$Z.D261024.SOCIOCSV"),
        Some(("24/10/2026".to_string(), "202610".to_string()))
    );
    // Invalid patterns
    assert_eq!(parse_rfb_reference_code("random_file.zip"), None);
    assert_eq!(parse_rfb_reference_code("K3241.DELETE.EMPRECSV"), None);
    assert_eq!(parse_rfb_reference_code("K3241.D60000.EMPRECSV"), None);
}

// =========================================================================
// 5. EMPIRICAL SCHEMA & INTEGRITY TESTS
// =========================================================================

#[test]
fn test_all_12_tables_columns_and_indices_strict_conformance() {
    let conn = Connection::open_in_memory().unwrap();
    create_initial_tables(&conn).unwrap();
    conn.execute_batch(rfb_cnpj_etl::ingestion::schema::CNPJ_BASE2MATRIZ_DDL)
        .unwrap();
    conn.execute_batch(rfb_cnpj_etl::ingestion::schema::SOCIOS_FINAL_DDL)
        .unwrap();
    create_post_load_indices(&conn).unwrap();

    let get_columns = |table: &str| -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };

    // 1. empresas (8 columns: 7 text + capital_social REAL)
    let empresas_cols = get_columns("empresas");
    assert_eq!(empresas_cols.len(), 8);
    assert_eq!(
        empresas_cols[0],
        ("cnpj_basico".to_string(), "TEXT".to_string())
    );
    assert_eq!(
        empresas_cols[1],
        ("razao_social".to_string(), "TEXT".to_string())
    );
    assert_eq!(
        empresas_cols[7],
        ("capital_social".to_string(), "REAL".to_string())
    );

    // 2. estabelecimento (31 columns: 30 original + cnpj TEXT)
    let estab_cols = get_columns("estabelecimento");
    assert_eq!(estab_cols.len(), 31);
    assert_eq!(
        estab_cols[0],
        ("cnpj_basico".to_string(), "TEXT".to_string())
    );
    assert_eq!(
        estab_cols[3],
        ("matriz_filial".to_string(), "TEXT".to_string())
    );
    assert_eq!(estab_cols[30], ("cnpj".to_string(), "TEXT".to_string()));

    // 3. socios (12 columns: cnpj TEXT + 11 socio columns)
    let socios_cols = get_columns("socios");
    assert_eq!(socios_cols.len(), 12);
    assert_eq!(socios_cols[0], ("cnpj".to_string(), "TEXT".to_string()));
    assert_eq!(
        socios_cols[1],
        ("cnpj_basico".to_string(), "TEXT".to_string())
    );
    assert_eq!(
        socios_cols[2],
        ("identificador_de_socio".to_string(), "TEXT".to_string())
    );
    assert_eq!(
        socios_cols[3],
        ("nome_socio".to_string(), "TEXT".to_string())
    );
    assert_eq!(
        socios_cols[4],
        ("cnpj_cpf_socio".to_string(), "TEXT".to_string())
    );

    // 4. simples (7 columns)
    let simples_cols = get_columns("simples");
    assert_eq!(simples_cols.len(), 7);

    // 5. cnpj_base2matriz (2 columns: cnpj_basico PRIMARY KEY, cnpj)
    let b2m_cols = get_columns("cnpj_base2matriz");
    assert_eq!(b2m_cols.len(), 2);
    assert_eq!(b2m_cols[0], ("cnpj_basico".to_string(), "TEXT".to_string()));
    assert_eq!(b2m_cols[1], ("cnpj".to_string(), "TEXT".to_string()));

    // 6. _referencia (2 columns: referencia, valor)
    let ref_cols = get_columns("_referencia");
    assert_eq!(ref_cols.len(), 2);

    // 7-12. 6 domain lookup tables (2 columns each: codigo PRIMARY KEY, descricao)
    for domain in [
        "cnae",
        "motivo",
        "municipio",
        "natureza_juridica",
        "pais",
        "qualificacao_socio",
    ] {
        let cols = get_columns(domain);
        assert_eq!(cols.len(), 2, "Table {} must have 2 columns", domain);
        assert_eq!(cols[0], ("codigo".to_string(), "TEXT".to_string()));
        assert_eq!(cols[1], ("descricao".to_string(), "TEXT".to_string()));
    }
}

#[test]
fn test_august_2026_empty_identificador_and_duplicate_matriz_resilience() {
    let mut conn = Connection::open_in_memory().unwrap();
    create_initial_tables(&conn).unwrap();

    // Test duplicate Matriz entries for same cnpj_basico
    conn.execute_batch(
        "INSERT INTO estabelecimento (cnpj_basico, cnpj_ordem, cnpj_dv, matriz_filial, cnpj) VALUES
            ('10000000', '0001', '01', '1', '10000000000101'),
            ('10000000', '0001', '01', '1', '10000000000101');",
    )
    .unwrap();

    let matrizes_inserted = materialize_cnpj_base2matriz(&mut conn).unwrap();
    // PRIMARY KEY + INSERT OR IGNORE must de-duplicate
    assert_eq!(matrizes_inserted, 1);

    // Test empty identificador_de_socio
    conn.execute_batch(
        "INSERT INTO socios_original (
            cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio
        ) VALUES
            ('10000000', '', 'SOCIO COM IDENTIFICADOR VAZIO', '12345678901');",
    )
    .unwrap();

    let count = transform_socios_and_expand_radicals(&mut conn).unwrap();
    assert_eq!(count, 1, "Row with empty identificador must not be dropped");

    let preserved_doc: String = conn
        .query_row(
            "SELECT cnpj_cpf_socio FROM socios WHERE nome_socio = 'SOCIO COM IDENTIFICADOR VAZIO'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(preserved_doc, "12345678901");
}
