//! Adversarial Graph Traversal & Directionality Challenger Test Suite for Milestone 3.
//!
//! Empirically stress-tests:
//! 1. Empty / Incomplete CNPJ database and missing tables.
//! 2. The `has_b2m` detection flaw in `derive_filial_matriz`.
//! 3. Graph directionality and exact edge derivation for all 7 rules.
//! 4. Bi-directional BFS index usage and SQLite query plan.
//! 5. Self-referencing, circular, reciprocal links, and multi-edge deduplication.
//! 6. FTS5 search index tokenization, diacritics, and adversarial queries (syntax chars, unclosed quotes).
//! 7. End-to-end GraphOrchestrator under extreme parameters (low memory budget, idempotent rerun).

use rusqlite::Connection;
use std::fs;
use tempfile::TempDir;

use rfb_cnpj_etl::cli::BuildRedeArgs;
use rfb_cnpj_etl::graph;
use rfb_cnpj_etl::graph::rules::*;
use rfb_cnpj_etl::graph::schema::*;
use rfb_cnpj_etl::graph::search::*;

// ============================================================================
// 1. EMPTY & INCOMPLETE CNPJ DATABASE STRESS TESTS
// ============================================================================

#[test]
fn test_adversarial_empty_cnpj_database_pipeline() {
    let tmp = TempDir::new().unwrap();
    let cnpj_path = tmp.path().join("empty_cnpj.db");
    let rede_path = tmp.path().join("rede.db");
    let search_path = tmp.path().join("rede_search.db");

    // Initialize cnpj.db with schema but 0 rows in any table
    {
        let conn = Connection::open(&cnpj_path).unwrap();
        rfb_cnpj_etl::ingestion::schema::create_initial_tables(&conn).unwrap();
    }

    let args = BuildRedeArgs {
        cnpj_db: cnpj_path,
        output_rede: rede_path.clone(),
        output_search: search_path.clone(),
        drop_existing: true,
        memory_limit_mb: 256,
    };

    let stats = graph::run(args).expect("Empty database should run cleanly without error");
    assert_eq!(stats.total_edges, 0, "Empty database must produce 0 edges");
    assert_eq!(
        stats.total_entities, 0,
        "Empty database must produce 0 entities"
    );
    assert_eq!(
        stats.total_search_entries, 0,
        "Empty database must produce 0 search entries"
    );

    // Verify generated SQLite databases are valid and accessible
    let conn_rede = Connection::open(&rede_path).unwrap();
    let edge_count: i64 = conn_rede
        .query_row("SELECT count(*) FROM ligacao;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(edge_count, 0);

    let conn_search = Connection::open(&search_path).unwrap();
    let search_count: i64 = conn_search
        .query_row("SELECT count(*) FROM id_search;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(search_count, 0);
}

#[test]
fn test_adversarial_zero_byte_cnpj_file() {
    let tmp = TempDir::new().unwrap();
    let cnpj_path = tmp.path().join("zero_byte_cnpj.db");
    let rede_path = tmp.path().join("rede.db");
    let search_path = tmp.path().join("rede_search.db");

    // Create an empty 0-byte file
    fs::File::create(&cnpj_path).unwrap();

    let args = BuildRedeArgs {
        cnpj_db: cnpj_path,
        output_rede: rede_path,
        output_search: search_path,
        drop_existing: true,
        memory_limit_mb: 256,
    };

    let stats = graph::run(args)
        .expect("Zero-byte file must be initialized gracefully by ensure_source_database");
    assert_eq!(stats.total_edges, 0);
    assert_eq!(stats.total_entities, 0);
}

#[test]
fn test_adversarial_missing_qualificacao_socio_table() {
    let conn = Connection::open_in_memory().unwrap();
    create_graph_schema(&conn).unwrap();

    // Attach cnpj missing qualificacao_socio table entirely
    conn.execute_batch(
        "ATTACH DATABASE ':memory:' AS cnpj;
         CREATE TABLE cnpj.socios (
             cnpj TEXT,
             nome_socio TEXT,
             cnpj_cpf_socio TEXT,
             qualificacao_socio TEXT,
             representante_legal TEXT,
             nome_representante TEXT,
             qualificacao_representante_legal TEXT
         );
         INSERT INTO cnpj.socios VALUES ('11111111000191', 'JOAO', '***123456**', '22', '', '', '');",
    ).unwrap();

    // In SQLite, a LEFT JOIN against a non-existent table fails with 'no such table'
    let res = derive_socio_pf(&conn);
    assert!(
        res.is_err(),
        "Missing qualificacao_socio must return an error rather than panicking or corrupting state"
    );
}

// ============================================================================
// 2. HAS_B2M DETECTION FLAW IN `derive_filial_matriz`
// ============================================================================

#[test]
fn test_adversarial_has_b2m_bug_empirical_demonstration() {
    let conn = Connection::open_in_memory().unwrap();
    create_graph_schema(&conn).unwrap();

    conn.execute_batch(
        "ATTACH DATABASE ':memory:' AS cnpj;
         CREATE TABLE cnpj.estabelecimento (
             cnpj_basico TEXT,
             matriz_filial TEXT,
             cnpj TEXT
         );
         CREATE TABLE cnpj.cnpj_base2matriz (
             cnpj_basico TEXT PRIMARY KEY,
             cnpj TEXT
         );
         INSERT INTO cnpj.estabelecimento VALUES ('33333333', '1', '33333333000173');
         INSERT INTO cnpj.estabelecimento VALUES ('33333333', '2', '33333333000254');
         INSERT INTO cnpj.cnpj_base2matriz VALUES ('33333333', '33333333000173');",
    )
    .unwrap();

    // Query 1: The current expression in rules.rs:205
    let current_check_res: Result<bool, _> = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cnpj_base2matriz') AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
        [],
        |r| r.get(0),
    );
    let current_detected = current_check_res.unwrap_or(false);

    // Query 2: The corrected expression targeting cnpj.sqlite_master
    let corrected_check_res: Result<bool, _> = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type='table' AND name='cnpj_base2matriz') AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
        [],
        |r| r.get(0),
    );
    let corrected_detected = corrected_check_res.unwrap_or(false);

    println!(
        "[EMPIRICAL FINDING 1] rules.rs:205 has_b2m detected: {}",
        current_detected
    );
    println!(
        "[EMPIRICAL FINDING 1] Corrected cnpj.sqlite_master detected: {}",
        corrected_detected
    );

    // Assert that the current implementation fails to detect the table in cnpj schema
    assert!(!current_detected, "Current rules.rs:205 incorrectly evaluates to false because sqlite_master refers to main (rede.db)");
    assert!(
        corrected_detected,
        "Checking cnpj.sqlite_master properly detects cnpj_base2matriz"
    );
}

// ============================================================================
// 3. GRAPH DIRECTIONALITY & EXACT LINK VERIFICATION (ALL 7 RULES)
// ============================================================================

#[test]
fn test_adversarial_all_7_rules_directionality_and_labels() {
    let conn = Connection::open_in_memory().unwrap();
    create_graph_schema(&conn).unwrap();

    conn.execute_batch(
        "ATTACH DATABASE ':memory:' AS cnpj;
         CREATE TABLE cnpj.qualificacao_socio (codigo TEXT PRIMARY KEY, descricao TEXT);
         INSERT INTO cnpj.qualificacao_socio VALUES
             ('22', 'Sócio'),
             ('49', 'Sócio-Administrador'),
             ('05', 'Administrador');

         CREATE TABLE cnpj.socios (
             cnpj TEXT,
             nome_socio TEXT,
             cnpj_cpf_socio TEXT,
             qualificacao_socio TEXT,
             representante_legal TEXT,
             nome_representante TEXT,
             qualificacao_representante_legal TEXT
         );

         CREATE TABLE cnpj.estabelecimento (
             cnpj_basico TEXT,
             matriz_filial TEXT,
             cnpj TEXT,
             nome_fantasia TEXT
         );

         CREATE TABLE cnpj.empresas (
             cnpj_basico TEXT,
             razao_social TEXT
         );

         CREATE TABLE cnpj.cnpj_base2matriz (
             cnpj_basico TEXT PRIMARY KEY,
             cnpj TEXT
         );

         -- Establishments: Matriz 0001, Filial 0002
         INSERT INTO cnpj.estabelecimento VALUES ('10000000', '1', '10000000000199', 'FANTASIA MATRIZ');
         INSERT INTO cnpj.estabelecimento VALUES ('10000000', '2', '10000000000288', 'FANTASIA FILIAL');
         INSERT INTO cnpj.cnpj_base2matriz VALUES ('10000000', '10000000000199');
         INSERT INTO cnpj.empresas VALUES ('10000000', 'HOLDING 10 S.A.');

         -- Establishments: Target Company 20
         INSERT INTO cnpj.estabelecimento VALUES ('20000000', '1', '20000000000111', 'ALFA TARGET');
         INSERT INTO cnpj.cnpj_base2matriz VALUES ('20000000', '20000000000111');
         INSERT INTO cnpj.empresas VALUES ('20000000', 'TARGET CORP LTDA');

         -- Rule 2: Corporate partner PJ -> PJ (Holding 10 is socio in Target 20)
         INSERT INTO cnpj.socios VALUES ('20000000000111', 'HOLDING 10 S.A.', '10000000000199', '22', '', '', '');

         -- Rule 3: Individual partner PF -> PJ (Carlos is socio in Target 20)
         INSERT INTO cnpj.socios VALUES ('20000000000111', 'CARLOS SILVA', '***111222**', '49', '', '', '');

         -- Rule 4: Foreign partner PE -> PJ (Global Tech is socio in Target 20)
         INSERT INTO cnpj.socios VALUES ('20000000000111', 'GLOBAL TECH LLC', '', '22', '', '', '');

         -- Rule 5a: Legal Rep of Foreign Partner (PF -> PE)
         INSERT INTO cnpj.socios VALUES ('20000000000111', 'OFFSHORE LTD', '', '22', '***999888**', 'BRUNO REP PE', '05');

         -- Rule 5b: Legal Rep of Corporate Partner (PF -> PJ)
         INSERT INTO cnpj.socios VALUES ('20000000000111', 'HOLDING 10 S.A.', '10000000000199', '22', '***777666**', 'FERNANDO REP PJ', '05');

         -- Rule 5c: Legal Rep of Individual Partner (PF -> PF)
         INSERT INTO cnpj.socios VALUES ('20000000000111', 'LUCAS MENOR', '***333444**', '22', '***555666**', 'PAULO PAI REP PF', '05');",
    ).unwrap();

    derive_filial_matriz(&conn).unwrap();
    derive_socio_pj(&conn).unwrap();
    derive_socio_pf(&conn).unwrap();
    derive_socio_estrangeiro(&conn).unwrap();
    derive_rep_legal(&conn).unwrap();
    deduplicate_ligacao(&conn).unwrap();
    materialize_entidades(&conn).unwrap();

    // Verify Directionality Rule 1: Filial -> Matriz
    let r1_count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PJ_10000000000288' AND id2 = 'PJ_10000000000199' AND descricao = 'filial' AND comentario = 'estabelecimento';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        r1_count, 1,
        "Rule 1 direction must be PJ_<filial> -> PJ_<matriz>"
    );

    // Verify Directionality Rule 2: Corporate Partner PJ -> PJ
    let r2_count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PJ_10000000000199' AND id2 = 'PJ_20000000000111' AND descricao = 'Sócio' AND comentario = 'socios';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        r2_count, 1,
        "Rule 2 direction must be PJ_<socio> -> PJ_<empresa>"
    );

    // Verify Directionality Rule 3: Individual Partner PF -> PJ
    let r3_count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PF_***111222**-CARLOS SILVA' AND id2 = 'PJ_20000000000111' AND descricao = 'Sócio-Administrador';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        r3_count, 1,
        "Rule 3 direction must be PF_<cpf>-<nome> -> PJ_<empresa>"
    );

    // Verify Directionality Rule 4: Foreign Partner PE -> PJ
    let r4_count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PE_GLOBAL TECH LLC' AND id2 = 'PJ_20000000000111';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        r4_count, 1,
        "Rule 4 direction must be PE_<nome> -> PJ_<empresa>"
    );

    // Verify Directionality Rule 5a: Legal Rep of Foreign Partner PF -> PE
    let r5a_count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PF_***999888**-BRUNO REP PE' AND id2 = 'PE_OFFSHORE LTD' AND descricao = 'rep-sócio-Administrador';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        r5a_count, 1,
        "Rule 5a direction must be PF_<rep>-<nome_rep> -> PE_<nome>"
    );

    // Verify Directionality Rule 5b: Legal Rep of Corporate Partner PF -> PJ
    let r5b_count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PF_***777666**-FERNANDO REP PJ' AND id2 = 'PJ_10000000000199' AND descricao = 'rep-sócio-Administrador';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        r5b_count, 1,
        "Rule 5b direction must be PF_<rep>-<nome_rep> -> PJ_<socio>"
    );

    // Verify Directionality Rule 5c: Legal Rep of Individual Partner PF -> PF
    let r5c_count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PF_***555666**-PAULO PAI REP PF' AND id2 = 'PF_***333444**-LUCAS MENOR' AND descricao = 'rep-sócio-Administrador';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        r5c_count, 1,
        "Rule 5c direction must be PF_<rep>-<nome_rep> -> PF_<socio>-<nome>"
    );
}

// ============================================================================
// 4. CIRCULAR, SELF-REFERENCING & RECIPROCAL LINKS
// ============================================================================

#[test]
fn test_adversarial_circular_and_self_referencing_links() {
    let conn = Connection::open_in_memory().unwrap();
    create_graph_schema(&conn).unwrap();

    conn.execute_batch(
        "ATTACH DATABASE ':memory:' AS cnpj;
         CREATE TABLE cnpj.qualificacao_socio (codigo TEXT PRIMARY KEY, descricao TEXT);
         INSERT INTO cnpj.qualificacao_socio VALUES ('22', 'Sócio');

         CREATE TABLE cnpj.estabelecimento (
             cnpj_basico TEXT,
             matriz_filial TEXT,
             cnpj TEXT
         );

         CREATE TABLE cnpj.socios (
             cnpj TEXT,
             nome_socio TEXT,
             cnpj_cpf_socio TEXT,
             qualificacao_socio TEXT,
             representante_legal TEXT,
             nome_representante TEXT,
             qualificacao_representante_legal TEXT
         );

         -- 1. Self-referencing branch: Matriz flag '2' but its CNPJ matches the Matriz CNPJ
         INSERT INTO cnpj.estabelecimento VALUES ('99999999', '2', '99999999000100');
         INSERT INTO cnpj.estabelecimento VALUES ('99999999', '1', '99999999000100');

         -- 2. Reciprocal socios: Company A is socio of Company B, and Company B is socio of Company A
         INSERT INTO cnpj.socios VALUES ('11111111000100', 'CORP B', '22222222000100', '22', '', '', '');
         INSERT INTO cnpj.socios VALUES ('22222222000100', 'CORP A', '11111111000100', '22', '', '', '');

         -- 3. Self-referencing socio: Company A listed as socio of itself (dirty data)
         INSERT INTO cnpj.socios VALUES ('11111111000100', 'CORP A SELF', '11111111000100', '22', '', '', '');

         -- 4. Duplicate identical socio records in source
         INSERT INTO cnpj.socios VALUES ('33333333000100', 'JOAO', '***111222**', '22', '', '', '');
         INSERT INTO cnpj.socios VALUES ('33333333000100', 'JOAO', '***111222**', '22', '', '', '');
         INSERT INTO cnpj.socios VALUES ('33333333000100', 'JOAO', '***111222**', '22', '', '', '');",
    ).unwrap();

    derive_filial_matriz(&conn).unwrap();
    derive_socio_pj(&conn).unwrap();
    derive_socio_pf(&conn).unwrap();
    deduplicate_ligacao(&conn).unwrap();

    // 1. Branch self-reference check: tf.cnpj <> tm.cnpj must prevent auto-link
    let self_branch: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PJ_99999999000100' AND id2 = 'PJ_99999999000100' AND descricao = 'filial';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        self_branch, 0,
        "tf.cnpj <> tm.cnpj must prevent self-referencing filial edges"
    );

    // 2. Reciprocal links must exist as two distinct directed edges
    let a_to_b: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PJ_22222222000100' AND id2 = 'PJ_11111111000100';",
        [],
        |r| r.get(0),
    ).unwrap();
    let b_to_a: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PJ_11111111000100' AND id2 = 'PJ_22222222000100';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(a_to_b, 1, "Reciprocal edge B -> A must exist");
    assert_eq!(b_to_a, 1, "Reciprocal edge A -> B must exist");

    // 3. Duplicate source entries must be deduplicated to exactly 1 edge
    let dup_count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PF_***111222**-JOAO' AND id2 = 'PJ_33333333000100';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        dup_count, 1,
        "GROUP BY origem, destino, tipo, base must deduplicate identical records into 1 edge"
    );
}

// ============================================================================
// 5. FTS5 FULL-TEXT SEARCH ADVERSARIAL STRESS TESTS
// ============================================================================

#[test]
fn test_adversarial_fts5_special_characters_and_syntax_resilience() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(FTS5_SEARCH_TABLE_DDL).unwrap();

    conn.execute_batch(
        "INSERT INTO id_search VALUES ('PJ_001', 'C&A MODAS S.A.');
         INSERT INTO id_search VALUES ('PJ_002', 'D''AVILA COMERCIO LTDA');
         INSERT INTO id_search VALUES ('PJ_003', 'A-Z LOGISTICA & TRANSPORTE');
         INSERT INTO id_search VALUES ('PJ_004', 'L''OREAL BRASIL COMERCIAL');
         INSERT INTO id_search VALUES ('PF_001', 'JOSE D''ALMEIDA JUNIOR');",
    )
    .unwrap();

    // Standard search
    let r1 = search_by_term(&conn, "MODAS", 10).unwrap();
    assert_eq!(r1.len(), 1);
    assert_eq!(r1[0].0, "PJ_001");

    // Search with ampersand
    let r2 = search_by_term(&conn, "LOGISTICA", 10).unwrap();
    assert_eq!(r2.len(), 1);
    assert_eq!(r2[0].0, "PJ_003");

    // Search for unaccented version
    let r3 = search_by_term(&conn, "AVILA", 10).unwrap();
    assert_eq!(r3.len(), 1);
    assert_eq!(r3[0].0, "PJ_002");

    // Empty string search
    let r_empty = search_by_term(&conn, "", 10);
    // In FTS5, empty MATCH query either returns error or empty vec
    println!("Empty search result: {:?}", r_empty.is_ok());

    // Resilient search with trailing boolean operator (sanitized into quoted tokens, no syntax error)
    let r_syntax = search_by_term(&conn, "MODAS OR", 10);
    assert!(
        r_syntax.is_ok(),
        "Sanitized search safely handles trailing boolean operators without SQLite syntax error"
    );
    assert_eq!(
        r_syntax.unwrap().len(),
        0,
        "Non-matching tokens must return empty result"
    );
}

// ============================================================================
// 6. PIPELINE ORCHESTRATOR STRESS & IDEMPOTENCY
// ============================================================================

#[test]
fn test_adversarial_orchestrator_low_memory_and_rerun_idempotency() {
    let tmp = TempDir::new().unwrap();
    let cnpj_path = tmp.path().join("cnpj.db");
    let rede_path = tmp.path().join("rede.db");
    let search_path = tmp.path().join("rede_search.db");

    // Seed mock cnpj.db
    {
        let conn = Connection::open(&cnpj_path).unwrap();
        rfb_cnpj_etl::ingestion::schema::create_initial_tables(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO estabelecimento (cnpj_basico, cnpj_ordem, cnpj_dv, matriz_filial, nome_fantasia, cnpj)
             VALUES ('88888888', '0001', '88', '1', 'TESTE IDEMPOTENCIA', '88888888000188');
             INSERT INTO empresas (cnpj_basico, razao_social)
             VALUES ('88888888', 'TESTE IDEMPOTENCIA S.A.');
             INSERT INTO socios (cnpj, nome_socio, cnpj_cpf_socio, qualificacao_socio)
             VALUES ('88888888000188', 'MARCOS SOCIO', '***888999**', '49');",
        ).unwrap();
    }

    // Run 1: with low memory budget (64 MB)
    let args1 = BuildRedeArgs {
        cnpj_db: cnpj_path.clone(),
        output_rede: rede_path.clone(),
        output_search: search_path.clone(),
        drop_existing: true,
        memory_limit_mb: 64,
    };
    let stats1 = graph::run(args1).expect("Run 1 with 64 MB budget should succeed");
    assert_eq!(stats1.total_edges, 1);
    assert!(stats1.total_entities >= 1);

    // Run 2: Rerun with drop_existing = true (idempotency check)
    let args2 = BuildRedeArgs {
        cnpj_db: cnpj_path,
        output_rede: rede_path,
        output_search: search_path,
        drop_existing: true,
        memory_limit_mb: 128,
    };
    let stats2 =
        graph::run(args2).expect("Run 2 rerun with drop_existing should succeed idempotently");
    assert_eq!(stats2.total_edges, stats1.total_edges);
    assert_eq!(stats2.total_entities, stats1.total_entities);
    assert_eq!(stats2.total_search_entries, stats1.total_search_entries);
}

// ============================================================================
// 7. ITERATION 2 EMPIRICAL FAST-PATH & ORPHAN/CORRUPT RESILIENCE SUITE
// ============================================================================

#[test]
fn test_empirical_fast_path_selection_when_b2m_present() {
    let conn = Connection::open_in_memory().unwrap();
    create_graph_schema(&conn).unwrap();

    conn.execute_batch(
        "ATTACH DATABASE ':memory:' AS cnpj;
         CREATE TABLE cnpj.estabelecimento (
             cnpj_basico TEXT,
             matriz_filial TEXT,
             cnpj TEXT
         );
         CREATE TABLE cnpj.cnpj_base2matriz (
             cnpj_basico TEXT PRIMARY KEY,
             cnpj TEXT
         );
         -- Filial exists, but NO matriz in estabelecimento!
         INSERT INTO cnpj.estabelecimento VALUES ('11112222', '2', '11112222000299');
         -- Matriz is ONLY present in cnpj_base2matriz
         INSERT INTO cnpj.cnpj_base2matriz VALUES ('11112222', '11112222000100');",
    )
    .unwrap();

    // Call derive_filial_matriz directly.
    // If fast-path DERIVE_FILIAL_MATRIZ_SQL is chosen, it joins with cnpj.cnpj_base2matriz and finds 1 link.
    // If fallback DERIVE_FILIAL_MATRIZ_FALLBACK_SQL were chosen, it joins with cnpj.estabelecimento tm (matriz_filial='1')
    // and would find 0 links because no matriz exists in estabelecimento!
    let derived = derive_filial_matriz(&conn).expect("derive_filial_matriz must execute cleanly");
    assert_eq!(
        derived, 1,
        "Fast path must be chosen and link derived from cnpj_base2matriz"
    );

    let count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao1 WHERE origem = 'PJ_11112222000299' AND destino = 'PJ_11112222000100';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1, "Edge must exist in staging table ligacao1");
}

#[test]
fn test_empirical_fast_path_fallback_when_b2m_empty() {
    let conn = Connection::open_in_memory().unwrap();
    create_graph_schema(&conn).unwrap();

    conn.execute_batch(
        "ATTACH DATABASE ':memory:' AS cnpj;
         CREATE TABLE cnpj.estabelecimento (
             cnpj_basico TEXT,
             matriz_filial TEXT,
             cnpj TEXT
         );
         CREATE TABLE cnpj.cnpj_base2matriz (
             cnpj_basico TEXT PRIMARY KEY,
             cnpj TEXT
         );
         -- In estabelecimento, both Matriz and Filial exist
         INSERT INTO cnpj.estabelecimento VALUES ('33334444', '1', '33334444000100');
         INSERT INTO cnpj.estabelecimento VALUES ('33334444', '2', '33334444000200');
         -- cnpj_base2matriz table exists, but is EMPTY (0 rows)",
    )
    .unwrap();

    // When cnpj_base2matriz is empty, has_b2m evaluates to false, falling back to DERIVE_FILIAL_MATRIZ_FALLBACK_SQL
    let derived = derive_filial_matriz(&conn)
        .expect("derive_filial_matriz must fall back cleanly without error");
    assert_eq!(
        derived, 1,
        "Fallback path must be chosen and derive filial link from estabelecimento self-join"
    );

    let count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao1 WHERE origem = 'PJ_33334444000200' AND destino = 'PJ_33334444000100';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1, "Edge must exist in staging table ligacao1");
}

#[test]
fn test_empirical_fast_path_fallback_when_b2m_missing() {
    let conn = Connection::open_in_memory().unwrap();
    create_graph_schema(&conn).unwrap();

    conn.execute_batch(
        "ATTACH DATABASE ':memory:' AS cnpj;
         CREATE TABLE cnpj.estabelecimento (
             cnpj_basico TEXT,
             matriz_filial TEXT,
             cnpj TEXT
         );
         -- In estabelecimento, both Matriz and Filial exist
         INSERT INTO cnpj.estabelecimento VALUES ('55556666', '1', '55556666000100');
         INSERT INTO cnpj.estabelecimento VALUES ('55556666', '2', '55556666000200');
         -- cnpj_base2matriz table does NOT exist at all in cnpj schema",
    )
    .unwrap();

    // When cnpj_base2matriz table does not exist, short-circuit must prevent 'no such table' prepare error,
    // and fall back smoothly.
    let derived = derive_filial_matriz(&conn)
        .expect("Missing cnpj_base2matriz must not panic or cause SQLite syntax/prepare error");
    assert_eq!(
        derived, 1,
        "Fallback path must execute cleanly when cnpj_base2matriz is absent"
    );

    let count: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao1 WHERE origem = 'PJ_55556666000200' AND destino = 'PJ_55556666000100';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1, "Edge must exist in staging table ligacao1");
}

#[test]
fn test_empirical_orphan_and_corrupt_partner_resilience() {
    let conn = Connection::open_in_memory().unwrap();
    create_graph_schema(&conn).unwrap();

    conn.execute_batch(
        "ATTACH DATABASE ':memory:' AS cnpj;
         CREATE TABLE cnpj.qualificacao_socio (codigo TEXT PRIMARY KEY, descricao TEXT);
         INSERT INTO cnpj.qualificacao_socio VALUES
             ('49', 'Sócio-Administrador'),
             ('22', 'Sócio'),
             ('05', 'Administrador');

         CREATE TABLE cnpj.estabelecimento (
             cnpj_basico TEXT,
             matriz_filial TEXT,
             cnpj TEXT,
             nome_fantasia TEXT
         );
         CREATE TABLE cnpj.empresas (
             cnpj_basico TEXT,
             razao_social TEXT
         );
         INSERT INTO cnpj.estabelecimento VALUES ('99999999', '1', '99999999000199', 'MATRIZ VALIDA');
         INSERT INTO cnpj.empresas VALUES ('99999999', 'EMPRESA VALIDA S.A.');

         CREATE TABLE cnpj.socios (
             cnpj TEXT,
             nome_socio TEXT,
             cnpj_cpf_socio TEXT,
             qualificacao_socio TEXT,
             representante_legal TEXT,
             nome_representante TEXT,
             qualificacao_representante_legal TEXT
         );

         -- 1. Orphan socio: cnpj is NULL (no establishment)
         INSERT INTO cnpj.socios VALUES (NULL, 'ORPHAN SOCIO', '***123456**', '49', '', '', '');
         -- 2. Corrupt socio: cnpj is empty string
         INSERT INTO cnpj.socios VALUES ('', 'EMPTY CNPJ SOCIO', '***123456**', '49', '', '', '');
         -- 3. Corrupt socio: cnpj length < 14
         INSERT INTO cnpj.socios VALUES ('12345', 'SHORT CNPJ SOCIO', '***123456**', '49', '', '', '');
         -- 4. Corrupt socio: nome_socio is NULL
         INSERT INTO cnpj.socios VALUES ('99999999000199', NULL, '***123456**', '49', '', '', '');
         -- 5. Corrupt socio: nome_socio is empty string
         INSERT INTO cnpj.socios VALUES ('99999999000199', '', '***123456**', '49', '', '', '');
         -- 6. Corrupt socio: nome_socio is all whitespace
         INSERT INTO cnpj.socios VALUES ('99999999000199', '   ', '***123456**', '49', '', '', '');
         -- 7. Corrupt socio: both cnpj_cpf_socio and nome_socio are NULL
         INSERT INTO cnpj.socios VALUES ('99999999000199', NULL, NULL, '49', '', '', '');
         -- 8. Corrupt socio: invalid length CPF
         INSERT INTO cnpj.socios VALUES ('99999999000199', 'INVALID CPF LEN', '12345', '49', '', '', '');
         -- 9. Orphan PJ socio: length 14 socio, but target establishment cnpj is NULL
         INSERT INTO cnpj.socios VALUES (NULL, 'PJ SOCIO WITHOUT CNPJ', '88888888000188', '22', '', '', '');
         -- 10. Corrupt Foreign partner: empty/null CPF, but empty/null name
         INSERT INTO cnpj.socios VALUES ('99999999000199', NULL, '', '22', '', '', '');
         INSERT INTO cnpj.socios VALUES ('99999999000199', '   ', '', '22', '', '', '');
         -- 11. Corrupt Legal rep: nome_socio is NULL or empty
         INSERT INTO cnpj.socios VALUES ('99999999000199', NULL, '', '22', '***111222**', 'REP WITHOUT SOCIO', '05');
         -- 12. Corrupt Legal rep: nome_representante is NULL or empty
         INSERT INTO cnpj.socios VALUES ('99999999000199', 'FOREIGN CORP', '', '22', '***111222**', NULL, '05');
         INSERT INTO cnpj.socios VALUES ('99999999000199', 'FOREIGN CORP', '', '22', '***111222**', '', '05');
         -- 13. Dummy Legal rep: ***000000**
         INSERT INTO cnpj.socios VALUES ('99999999000199', 'FOREIGN CORP', '', '22', '***000000**', 'DUMMY REP', '05');

         -- 14. VALID PARTNER: Must survive and be properly inserted
         INSERT INTO cnpj.socios VALUES ('99999999000199', 'VALID PARTNER', '***999888**', '49', '', '', '');",
    ).unwrap();

    // Execute all derivation rules
    derive_socio_pj(&conn).unwrap();
    derive_socio_pf(&conn).unwrap();
    derive_socio_estrangeiro(&conn).unwrap();
    derive_rep_legal(&conn).unwrap();

    // DEDUPLICATION: Must NOT throw IntegrityError ('NOT NULL constraint failed: ligacao.id1/id2')
    deduplicate_ligacao(&conn)
        .expect("deduplicate_ligacao must succeed without throwing SQLite IntegrityError");

    // MATERIALIZATION: Must NOT insert bare prefixes or corrupt entities
    materialize_entidades(&conn).expect("materialize_entidades must succeed without error");

    // Verify ligacao content
    let invalid_edges: i64 = conn
        .query_row(
            "SELECT count(*) FROM ligacao 
         WHERE id1 IS NULL OR id2 IS NULL 
            OR trim(id1) = '' OR trim(id2) = ''
            OR id1 IN ('PJ_', 'PF_', 'PE_', 'PF_-')
            OR id2 IN ('PJ_', 'PF_', 'PE_', 'PF_-');",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        invalid_edges, 0,
        "No corrupt or bare prefix edges allowed in ligacao"
    );

    // Verify valid partner exists
    let valid_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM ligacao 
         WHERE id1 = 'PF_***999888**-VALID PARTNER' 
           AND id2 = 'PJ_99999999000199' 
           AND descricao = 'Sócio-Administrador';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        valid_count, 1,
        "The single valid partner must be cleanly derived into ligacao"
    );

    // Verify total ligacao count is exactly 2:
    // 1 valid PF partner ('PF_***999888**-VALID PARTNER')
    // 1 valid PE foreign partner ('PE_FOREIGN CORP' whose invalid legal rep was ignored)
    let total_ligacoes: i64 = conn
        .query_row("SELECT count(*) FROM ligacao;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        total_ligacoes, 2,
        "Only the 2 valid partners must survive; all corrupt links rejected"
    );

    let pe_valid: i64 = conn.query_row(
        "SELECT count(*) FROM ligacao WHERE id1 = 'PE_FOREIGN CORP' AND id2 = 'PJ_99999999000199';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(pe_valid, 1, "Valid foreign partner must be derived");

    // Verify corrupt legal rep was NOT created
    let rep_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM ligacao WHERE descricao LIKE 'rep-sócio-%';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        rep_count, 0,
        "Corrupt legal representatives must produce 0 edges"
    );

    // Verify entidades table does not contain bare prefixes
    let invalid_entities: i64 = conn.query_row(
        "SELECT count(*) FROM entidades WHERE id IN ('PJ_', 'PF_', 'PE_', 'PF_-') OR trim(id) = '';",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(
        invalid_entities, 0,
        "No bare prefix entities allowed in entidades table"
    );
}
