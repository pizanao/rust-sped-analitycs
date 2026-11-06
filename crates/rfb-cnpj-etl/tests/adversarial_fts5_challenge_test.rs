//! Empirical Challenger Test Suite for Milestone 3 (Iteration 2)
//!
//! Stress-tests:
//! 1. FTS5 Punctuation & Keyword Stress Test
//! 2. Formatted CNPJs & Radicals in search_by_term and search_by_cnpj_basico
//! 3. Hyphenated trade names, dangling boolean operators, unclosed quotes, asterisks
//! 4. 0 SQLite syntax errors verification under hostile fuzzing matrix

use rfb_cnpj_etl::graph::search::{
    sanitize_fts5_term, search_by_cnpj_basico, search_by_term, FTS5_SEARCH_TABLE_DDL,
};
use rusqlite::Connection;

#[test]
fn test_empirical_fts5_challenge_comprehensive() {
    let conn = Connection::open_in_memory().expect("open memory sqlite");
    conn.execute_batch(FTS5_SEARCH_TABLE_DDL)
        .expect("init fts5 table");

    // Populate with representative test entities covering all target shapes
    conn.execute_batch(
        "INSERT INTO id_search VALUES ('PJ_11223344000191', 'ALFA MATRIZ PARTICIPACOES LTDA');
         INSERT INTO id_search VALUES ('PJ_11223344000272', 'ALFA FILIAL SUL');
         INSERT INTO id_search VALUES ('PJ_99887766000100', 'ALFA-TECH INFORMATICA E SERVICOS');
         INSERT INTO id_search VALUES ('PJ_88776655000199', 'C&A MODAS S.A.');
         INSERT INTO id_search VALUES ('PJ_77665544000188', 'DROGARIA & FARMACIA \"UNCLOSED LTDA');
         INSERT INTO id_search VALUES ('PJ_66554433000177', 'OPERADORA AND BANDA LARGA LTDA');
         INSERT INTO id_search VALUES ('PJ_55443322000166', 'REDE NOT TV COMUNICACAO');
         INSERT INTO id_search VALUES ('PJ_44332211000155', 'DISTRIBUIDORA NEAR S.A.');
         INSERT INTO id_search VALUES ('PJ_11111111000191', 'EMPRESA ASTERISCO');
         INSERT INTO id_search VALUES ('PF_***123456**-CARLOS ALBERTO', '');",
    )
    .expect("populate test data");

    // ========================================================================
    // 1. Formatted CNPJs & 8-Digit Radicals
    // ========================================================================

    // 1.1 Formatted full CNPJ with punctuation: 11.223.344/0001-91
    let r_cnpj_full = search_by_term(&conn, "11.223.344/0001-91", 10);
    assert!(
        r_cnpj_full.is_ok(),
        "search_by_term must not throw SQLite syntax error on formatted CNPJ: {:?}",
        r_cnpj_full.err()
    );
    let hits_cnpj_full = r_cnpj_full.unwrap();
    assert_eq!(
        hits_cnpj_full.len(),
        1,
        "Must find exactly the matriz for 11.223.344/0001-91"
    );
    assert_eq!(hits_cnpj_full[0].0, "PJ_11223344000191");

    // 1.2 Formatted 8-digit radical: 11.223.344
    let r_radical = search_by_term(&conn, "11.223.344", 10);
    assert!(
        r_radical.is_ok(),
        "search_by_term must not throw SQLite syntax error on formatted radical: {:?}",
        r_radical.err()
    );
    let hits_radical = r_radical.unwrap();
    assert_eq!(
        hits_radical.len(),
        2,
        "Must find both matriz and filial for radical 11.223.344"
    );
    assert!(hits_radical.iter().any(|(id, _)| id == "PJ_11223344000191"));
    assert!(hits_radical.iter().any(|(id, _)| id == "PJ_11223344000272"));

    // 1.3 search_by_cnpj_basico with formatted CNPJ
    let r_basico_full = search_by_cnpj_basico(&conn, "11.223.344/0001-91", 10);
    assert!(
        r_basico_full.is_ok(),
        "search_by_cnpj_basico must succeed with formatted full CNPJ"
    );
    let hits_basico_full = r_basico_full.unwrap();
    assert_eq!(
        hits_basico_full.len(),
        2,
        "search_by_cnpj_basico matches all establishments by 8-digit prefix"
    );
    assert!(hits_basico_full.contains(&"PJ_11223344000191".to_string()));
    assert!(hits_basico_full.contains(&"PJ_11223344000272".to_string()));

    // 1.4 search_by_cnpj_basico with formatted radical
    let r_basico_rad = search_by_cnpj_basico(&conn, "11.223.344", 10);
    assert!(
        r_basico_rad.is_ok(),
        "search_by_cnpj_basico must succeed with formatted radical"
    );
    let hits_basico_rad = r_basico_rad.unwrap();
    assert_eq!(hits_basico_rad.len(), 2);
    assert!(hits_basico_rad.contains(&"PJ_11223344000191".to_string()));
    assert!(hits_basico_rad.contains(&"PJ_11223344000272".to_string()));

    // ========================================================================
    // 2. Hyphenated Trade Name
    // ========================================================================
    let r_hyphen = search_by_term(&conn, "ALFA-TECH", 10);
    assert!(
        r_hyphen.is_ok(),
        "ALFA-TECH must not trigger 'no such column: TECH' error: {:?}",
        r_hyphen.err()
    );
    let hits_hyphen = r_hyphen.unwrap();
    assert_eq!(hits_hyphen.len(), 1);
    assert_eq!(hits_hyphen[0].0, "PJ_99887766000100");

    // ========================================================================
    // 3. Dangling Boolean Operators & Keywords
    // ========================================================================

    // 3.1 Dangling "MODAS OR"
    let r_dangling_or = search_by_term(&conn, "MODAS OR", 10);
    assert!(
        r_dangling_or.is_ok(),
        "Dangling OR must not cause FTS5 syntax error"
    );
    assert_eq!(r_dangling_or.unwrap().len(), 0);

    // 3.2 Bare "AND" operator (matches PJ_66554433000177 where description contains word AND)
    let r_and = search_by_term(&conn, "AND", 10);
    assert!(
        r_and.is_ok(),
        "Bare AND must not cause FTS5 syntax error: {:?}",
        r_and.err()
    );
    let hits_and = r_and.unwrap();
    assert_eq!(hits_and.len(), 1);
    assert_eq!(hits_and[0].0, "PJ_66554433000177");

    // 3.3 Bare "NOT" operator (matches PJ_55443322000166 where description contains word NOT)
    let r_not = search_by_term(&conn, "NOT", 10);
    assert!(
        r_not.is_ok(),
        "Bare NOT must not cause FTS5 syntax error: {:?}",
        r_not.err()
    );
    let hits_not = r_not.unwrap();
    assert_eq!(hits_not.len(), 1);
    assert_eq!(hits_not[0].0, "PJ_55443322000166");

    // 3.4 Bare "NEAR" operator (matches PJ_44332211000155 where description contains word NEAR)
    let r_near = search_by_term(&conn, "NEAR", 10);
    assert!(
        r_near.is_ok(),
        "Bare NEAR must not cause FTS5 syntax error: {:?}",
        r_near.err()
    );
    let hits_near = r_near.unwrap();
    assert_eq!(hits_near.len(), 1);
    assert_eq!(hits_near[0].0, "PJ_44332211000155");

    // 3.5 Combinations of operators: "AND NOT OR NEAR"
    let r_comb = search_by_term(&conn, "AND NOT OR NEAR", 10);
    assert!(
        r_comb.is_ok(),
        "Compound operators must not cause syntax error"
    );

    // ========================================================================
    // 4. Special Punctuation: C&A, S.A., "unclosed
    // ========================================================================

    // 4.1 C&A
    let r_ca = search_by_term(&conn, "C&A", 10);
    assert!(
        r_ca.is_ok(),
        "C&A must not trigger syntax error near &: {:?}",
        r_ca.err()
    );
    let hits_ca = r_ca.unwrap();
    assert_eq!(hits_ca.len(), 1);
    assert_eq!(hits_ca[0].0, "PJ_88776655000199");

    // 4.2 S.A.
    let r_sa = search_by_term(&conn, "S.A.", 10);
    assert!(
        r_sa.is_ok(),
        "S.A. must not trigger syntax error near .: {:?}",
        r_sa.err()
    );
    let hits_sa = r_sa.unwrap();
    assert!(hits_sa.iter().any(|(id, _)| id == "PJ_88776655000199"));
    assert!(hits_sa.iter().any(|(id, _)| id == "PJ_44332211000155"));

    // 4.3 Unclosed double quote
    let r_unclosed = search_by_term(&conn, "\"unclosed", 10);
    assert!(
        r_unclosed.is_ok(),
        "Unclosed quote must not trigger unterminated string error: {:?}",
        r_unclosed.err()
    );
    let hits_unclosed = r_unclosed.unwrap();
    assert_eq!(hits_unclosed.len(), 1);
    assert_eq!(hits_unclosed[0].0, "PJ_77665544000188");

    // 4.4 Lone quote, double quotes, triple quotes
    for q in ["\"", "\"\"", "\"\"\"", "  \"  "] {
        let r_q = search_by_term(&conn, q, 10);
        assert!(r_q.is_ok(), "Lone quotes '{q}' must safely return Ok");
        assert_eq!(r_q.unwrap().len(), 0);
    }

    // ========================================================================
    // 5. Asterisks & Wildcards
    // ========================================================================

    // 5.1 Double trailing asterisk: PJ_11111111**
    let r_ast_double = search_by_term(&conn, "PJ_11111111**", 10);
    assert!(
        r_ast_double.is_ok(),
        "PJ_11111111** must not trigger syntax error near *: {:?}",
        r_ast_double.err()
    );

    // 5.2 Masked CPF: ***123456**
    let r_masked_cpf = search_by_term(&conn, "***123456**", 10);
    assert!(
        r_masked_cpf.is_ok(),
        "***123456** must not trigger syntax error: {:?}",
        r_masked_cpf.err()
    );
    let hits_cpf = r_masked_cpf.unwrap();
    assert_eq!(hits_cpf.len(), 1, "Masked CPF lookup must find the record");
    assert_eq!(hits_cpf[0].0, "PF_***123456**-CARLOS ALBERTO");

    // 5.3 Lone and multiple asterisks: *, **, ***
    for ast in ["*", "**", "***", "****"] {
        let r_ast = search_by_term(&conn, ast, 10);
        assert!(
            r_ast.is_ok(),
            "Pure asterisks '{ast}' must safely return Ok"
        );
        assert_eq!(r_ast.unwrap().len(), 0);
    }

    // 5.4 Valid prefix wildcard: ALFA*
    let r_prefix = search_by_term(&conn, "ALFA*", 10);
    assert!(
        r_prefix.is_ok(),
        "Valid prefix wildcard ALFA* must succeed: {:?}",
        r_prefix.err()
    );
    let hits_prefix = r_prefix.unwrap();
    assert_eq!(
        hits_prefix.len(),
        3,
        "ALFA* must match ALFA MATRIZ, ALFA FILIAL, and ALFA-TECH"
    );
}

#[test]
fn test_empirical_fts5_sanitizer_unit_matrix() {
    let test_matrix = [
        // (Input, Expected sanitized string)
        ("11.223.344/0001-91", "\"11223344000191\""),
        ("11.223.344", "\"11223344\"*"),
        ("11.223.344/0002-72", "\"11223344000272\""),
        ("ALFA-TECH", "\"ALFA-TECH\""),
        ("MODAS OR", "\"MODAS\" \"OR\""),
        ("AND", "\"AND\""),
        ("NOT", "\"NOT\""),
        ("NEAR", "\"NEAR\""),
        ("C&A", "\"C&A\""),
        ("S.A.", "\"S.A.\""),
        ("\"unclosed", "\"\"\"unclosed\""),
        ("PJ_11111111**", "\"PJ_11111111\""),
        ("***123456**", "\"123456\""),
        ("TECNOL*", "\"TECNOL\"*"),
        ("", ""),
        ("   ", ""),
        ("*", ""),
        ("**", ""),
        ("***", ""),
        ("\"", ""),
        ("\"\"", ""),
        ("\"\"\"", "\"\"\"\""),
        ("\"EXACT PHRASE\"", "\"EXACT PHRASE\""),
    ];

    for (input, expected) in test_matrix {
        let actual = sanitize_fts5_term(input);
        assert_eq!(
            actual, expected,
            "Input '{input}': expected sanitized '{expected}', got '{actual}'"
        );
    }
}

#[test]
fn test_empirical_fts5_fuzzing_zero_syntax_errors() {
    let conn = Connection::open_in_memory().expect("open memory sqlite");
    conn.execute_batch(FTS5_SEARCH_TABLE_DDL)
        .expect("init fts5 table");
    conn.execute_batch(
        "INSERT INTO id_search VALUES ('PJ_11111111000191', 'FUZZ TEST COMPANY LTDA');",
    )
    .expect("insert fuzz base");

    let adversarial_fuzz_inputs = [
        // Boolean keywords and combinations
        "AND",
        "OR",
        "NOT",
        "NEAR",
        "NEAR/2",
        "AND OR NOT",
        "OR AND",
        "NOT AND",
        "AND AND AND",
        "OR OR OR",
        "NOT NOT NOT",
        "MODAS OR",
        "OR MODAS",
        "AND MODAS",
        "MODAS AND",
        "NOT MODAS",
        "MODAS NOT",
        // Punctuation and quotes
        "\"",
        "\"\"",
        "\"\"\"",
        "\"\"\"\"",
        "\"\"\"\"\"",
        "\"unclosed quote",
        "unclosed quote\"",
        "partially\"quoted\"term",
        "C&A",
        "S.A.",
        "D'AVILA",
        "A&B&C",
        "A.B.C.",
        // FTS5 column selectors
        "id:PJ_11111111000191",
        "descricao:FUZZ",
        "nonexistent_col:test",
        "id: descricao:",
        ":id",
        "::",
        // Wildcards and asterisks
        "*",
        "**",
        "***",
        "****",
        "*a",
        "**a",
        "a*",
        "a**",
        "a***",
        "*a*",
        "***123456**",
        "PJ_11111111**",
        "PF_***123456**",
        // Formatted CNPJ edge variations
        "11.223.344/0001-91",
        "11.223.344",
        "00.000.000/0001-91",
        "00.000.000",
        "11.223.344/0001-91.",
        "11/223/344",
        "11-223-344",
        "11. 223. 344 / 0001 - 91",
        // Pure punctuation
        "...",
        "---",
        "///",
        "@@@",
        "###",
        "$$$",
        "%%%",
        "^^^",
        "&&&",
        "(((",
        ")))",
        "(ALFA OR BETA)",
        "+++",
        "===",
        "{{{",
        "}}}",
        "[[[",
        "]]]",
        "|||",
        "\\\\\\",
        ";;;",
        "<<<",
        ">>>",
        "???",
        ",,,",
        "~~~",
        // Injection attempts
        "' OR 1=1 --",
        "\" OR \"1\"=\"1",
        "'; DROP TABLE id_search; --",
        "\"; DROP TABLE id_search; --",
    ];

    for input in adversarial_fuzz_inputs {
        let res_term = search_by_term(&conn, input, 10);
        assert!(
            res_term.is_ok(),
            "search_by_term failed with SQLite error for input '{input}': {:?}",
            res_term.err()
        );

        let res_cnpj = search_by_cnpj_basico(&conn, input, 10);
        assert!(
            res_cnpj.is_ok(),
            "search_by_cnpj_basico failed with SQLite error for input '{input}': {:?}",
            res_cnpj.err()
        );
    }
}
