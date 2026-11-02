//! Adversarial challenge test suite for Milestone 3:
//! - FTS5 Tokenizer & Portuguese Diacritic Removal (unicode61 remove_diacritics 2)
//! - Wildcard, prefix, and adversarial syntax resilience
//! - FTS5 segment optimization
//! - SQLite multi-database attachment lifecycle, RAII safety, and lock avoidance
//! - WAL mode persistence on rede.db and rede_search.db
//! - Memory ceiling bounding (< 1.8 GB RAM) and PRAGMA configurations

mod common;
mod fixtures;

use rusqlite::Connection;
use std::fs;
use tempfile::{NamedTempFile, TempDir};

use rfb_cnpj_etl::cli::BuildRedeArgs;
use rfb_cnpj_etl::graph::attachment::{
    apply_graph_pragmas, finalize_database_to_wal, AttachedDatabase,
};
use rfb_cnpj_etl::graph::search::{
    search_by_cnpj_basico, search_by_term, FTS5_OPTIMIZE_SQL, FTS5_SEARCH_TABLE_DDL,
};
use rfb_cnpj_etl::graph::{get_resident_memory_bytes, GraphOrchestrator};
use rfb_cnpj_etl::ingestion::schema::create_initial_tables;

// ============================================================================
// 1. FTS5 TOKENIZER & PORTUGUESE DIACRITIC INVARIANCE
// ============================================================================

#[test]
fn test_adversarial_fts5_portuguese_diacritics_exhaustive() {
    let conn = Connection::open_in_memory().expect("open memory db");
    conn.execute_batch(FTS5_SEARCH_TABLE_DDL)
        .expect("init fts5 table");

    // Insert accented Brazilian corporate names & partner entities
    conn.execute_batch(
        "INSERT INTO id_search VALUES ('PJ_01', 'AUTO POSTO SÃO PAULO LTDA');
         INSERT INTO id_search VALUES ('PJ_02', 'COMÉRCIO E INDÚSTRIA DE ALIMENTOS S.A.');
         INSERT INTO id_search VALUES ('PJ_03', 'PRESTAÇÃO DE SERVIÇO TÉCNICO ESPECIALIZADO');
         INSERT INTO id_search VALUES ('PJ_04', 'COMPANHIA DE ÁGUA E ESGOTO SANITÁRIO');
         INSERT INTO id_search VALUES ('PF_05', 'PF_***123456**-JOÃO GONÇALVES DE OLIVEIRA');
         INSERT INTO id_search VALUES ('PJ_06', 'MINERAÇÃO CONCEIÇÃO E AÇÚCAR UNIÃO');
         INSERT INTO id_search VALUES ('PJ_07', 'DISTRIBUIDORA DE ÓLEO E GÁS BRASIL');
         INSERT INTO id_search VALUES ('PJ_08', 'FARMÁCIA SAÚDE & BEM ESTAR');",
    )
    .expect("insert records");

    // Test matrix: (Search term, Expected matching ID)
    let test_cases = vec![
        // SÃO PAULO
        ("sao", "PJ_01"),
        ("SÃO", "PJ_01"),
        ("são", "PJ_01"),
        ("SAO", "PJ_01"),
        ("paulo", "PJ_01"),
        ("PAULO", "PJ_01"),
        // COMÉRCIO
        ("comercio", "PJ_02"),
        ("COMÉRCIO", "PJ_02"),
        ("comércio", "PJ_02"),
        ("COMERCIO", "PJ_02"),
        // INDÚSTRIA
        ("industria", "PJ_02"),
        ("INDÚSTRIA", "PJ_02"),
        ("indústria", "PJ_02"),
        ("INDUSTRIA", "PJ_02"),
        // SERVIÇO
        ("servico", "PJ_03"),
        ("SERVIÇO", "PJ_03"),
        ("serviço", "PJ_03"),
        ("SERVICO", "PJ_03"),
        // ÁGUA
        ("agua", "PJ_04"),
        ("ÁGUA", "PJ_04"),
        ("água", "PJ_04"),
        ("AGUA", "PJ_04"),
        // JOÃO
        ("joao", "PF_05"),
        ("JOÃO", "PF_05"),
        ("joão", "PF_05"),
        ("JOAO", "PF_05"),
        ("goncalves", "PF_05"),
        ("GONÇALVES", "PF_05"),
        // CONCEIÇÃO & AÇÚCAR
        ("conceicao", "PJ_06"),
        ("CONCEIÇÃO", "PJ_06"),
        ("acucar", "PJ_06"),
        ("AÇÚCAR", "PJ_06"),
        // ÓLEO
        ("oleo", "PJ_07"),
        ("ÓLEO", "PJ_07"),
        // SAÚDE
        ("saude", "PJ_08"),
        ("SAÚDE", "PJ_08"),
    ];

    for (query, expected_id) in test_cases {
        let hits = search_by_term(&conn, query, 10)
            .unwrap_or_else(|e| panic!("Search failed for query '{query}': {e}"));
        assert!(
            hits.iter().any(|(id, _)| id == expected_id),
            "Query '{query}' did not match expected ID '{expected_id}'. Actual hits: {hits:?}"
        );
    }
}

#[test]
fn test_adversarial_fts5_symmetric_diacritic_matching() {
    let conn = Connection::open_in_memory().expect("open memory db");
    conn.execute_batch(FTS5_SEARCH_TABLE_DDL)
        .expect("init fts5 table");

    // Insert UNACCENTED record in database
    conn.execute_batch(
        "INSERT INTO id_search VALUES ('PJ_UNACCENTED', 'COMERCIO E INDUSTRIA SAO PAULO');",
    )
    .expect("insert unaccented");

    // Verify ACCENTED search terms successfully match the unaccented record
    let res1 = search_by_term(&conn, "COMÉRCIO", 10).expect("search COMÉRCIO");
    assert_eq!(res1.len(), 1);
    assert_eq!(res1[0].0, "PJ_UNACCENTED");

    let res2 = search_by_term(&conn, "INDÚSTRIA", 10).expect("search INDÚSTRIA");
    assert_eq!(res2.len(), 1);
    assert_eq!(res2[0].0, "PJ_UNACCENTED");

    let res3 = search_by_term(&conn, "SÃO", 10).expect("search SÃO");
    assert_eq!(res3.len(), 1);
    assert_eq!(res3[0].0, "PJ_UNACCENTED");
}

// ============================================================================
// 2. WILDCARD, PREFIX & ADVERSARIAL QUERY SYNTAX
// ============================================================================

#[test]
fn test_adversarial_wildcard_and_prefix_matching() {
    let conn = Connection::open_in_memory().expect("open memory db");
    conn.execute_batch(FTS5_SEARCH_TABLE_DDL)
        .expect("init fts5 table");

    conn.execute_batch(
        "INSERT INTO id_search VALUES ('PJ_11223344000191', 'ALFA TECNOLOGIA MATRIZ');
         INSERT INTO id_search VALUES ('PJ_11223344000272', 'ALFA TECNOLOGIA FILIAL');
         INSERT INTO id_search VALUES ('PJ_99887766000100', 'BETA TELECOM');
         INSERT INTO id_search VALUES ('PF_***123456**-JOAO SILVA', '');",
    )
    .expect("insert");

    // 1. Prefix query with search_by_cnpj_basico
    let cnpj_hits = search_by_cnpj_basico(&conn, "11223344", 10).expect("cnpj basico search");
    assert_eq!(cnpj_hits.len(), 2);
    assert!(cnpj_hits.contains(&"PJ_11223344000191".to_string()));
    assert!(cnpj_hits.contains(&"PJ_11223344000272".to_string()));

    // 2. Prefix wildcard on term
    let prefix_hits = search_by_term(&conn, "TECNOL*", 10).expect("prefix search");
    assert_eq!(prefix_hits.len(), 2);

    let tele_hits = search_by_term(&conn, "TELE*", 10).expect("prefix search tele");
    assert_eq!(tele_hits.len(), 1);
    assert_eq!(tele_hits[0].0, "PJ_99887766000100");

    // 3. CPF radical matching
    let cpf_hits = search_by_term(&conn, "123456", 10).expect("cpf search");
    assert_eq!(cpf_hits.len(), 1);
    assert_eq!(cpf_hits[0].0, "PF_***123456**-JOAO SILVA");

    // 4. Exact full ID match
    let exact_hits = search_by_term(&conn, "\"PJ_11223344000191\"", 10).expect("exact ID search");
    assert_eq!(exact_hits.len(), 1);
    assert_eq!(exact_hits[0].0, "PJ_11223344000191");
}

#[test]
fn test_adversarial_fts5_syntax_error_resilience() {
    let conn = Connection::open_in_memory().expect("open memory db");
    conn.execute_batch(FTS5_SEARCH_TABLE_DDL)
        .expect("init fts5 table");

    conn.execute_batch("INSERT INTO id_search VALUES ('PJ_11111111000191', 'TEST COMPANY');")
        .expect("insert");

    // Raw user inputs with special FTS5 operators should return rusqlite::Error safely without crashing
    let malformed_inputs = [
        "AND",
        "OR",
        "NOT",
        "NEAR",
        "\"unclosed quote",
        "***123456**",
        "id:invalid_col",
        "PJ_11111111**",
    ];

    for input in malformed_inputs {
        let res = search_by_term(&conn, input, 10);
        // It must either be Err (syntax error handled by SQLite) or empty Ok, but NEVER panic
        assert!(
            res.is_err() || res.unwrap().is_empty(),
            "Malformed FTS5 input '{input}' should not produce invalid state"
        );
    }
}

#[test]
fn test_adversarial_fts5_optimize_command() {
    let conn = Connection::open_in_memory().expect("open memory db");
    conn.execute_batch(FTS5_SEARCH_TABLE_DDL).expect("init");

    for i in 0..500 {
        let sql =
            format!("INSERT INTO id_search VALUES ('PJ_{i:014}', 'EMPRESA NUMERO {i} LTDA');");
        conn.execute_batch(&sql).expect("insert batch");
    }

    // Execute optimize segment merging
    conn.execute_batch(FTS5_OPTIMIZE_SQL)
        .expect("optimize must succeed");

    let count: i64 = conn
        .query_row("SELECT count(*) FROM id_search;", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 500);
}

// ============================================================================
// 3. ATTACHMENT LIFECYCLE, CONCURRENCY & LOCK AVOIDANCE
// ============================================================================

#[test]
fn test_adversarial_attached_database_path_escaping() {
    let dir = TempDir::new().expect("tempdir");
    // Directory containing single quotes and spaces
    let tricky_dir = dir.path().join("d'souza & co's db");
    fs::create_dir_all(&tricky_dir).expect("create tricky dir");

    let db_path = tricky_dir.join("source'test.db");
    {
        let ext = Connection::open(&db_path).expect("create ext db");
        ext.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('escaped_ok');")
            .expect("init ext");
    }

    let conn = Connection::open_in_memory().expect("open memory db");
    // Attach database whose path contains single quotes
    let attached = AttachedDatabase::attach(&conn, &db_path, "ext_db")
        .expect("attach with escaped single quotes");

    let val: String = conn
        .query_row("SELECT v FROM ext_db.t;", [], |r| r.get(0))
        .expect("query attached");
    assert_eq!(val, "escaped_ok");

    attached.detach().expect("detach must succeed cleanly");
}

#[test]
fn test_adversarial_attached_database_raii_drop_and_idempotency() {
    let tmp = NamedTempFile::new().expect("tempfile");
    {
        let ext = Connection::open(tmp.path()).expect("open ext");
        ext.execute_batch("CREATE TABLE sample (num INT); INSERT INTO sample VALUES (99);")
            .expect("init sample");
    }

    let conn = Connection::open_in_memory().expect("open memory db");

    // 1. Attach in inner scope, let RAII Drop detach it automatically
    {
        let _att = AttachedDatabase::attach(&conn, tmp.path(), "scope_db").expect("attach");
        let val: i32 = conn
            .query_row("SELECT num FROM scope_db.sample;", [], |r| r.get(0))
            .expect("query");
        assert_eq!(val, 99);
    } // _att drops here

    // Query on detached database must return error (no such table)
    let res = conn.query_row("SELECT num FROM scope_db.sample;", [], |r| {
        r.get::<_, i32>(0)
    });
    assert!(
        res.is_err(),
        "Table must not be accessible after drop detachment"
    );

    // 2. Attach again and verify explicit detach
    let att2 = AttachedDatabase::attach(&conn, tmp.path(), "scope_db").expect("attach again");
    att2.detach().expect("explicit detach");

    // Verify subsequent query fails
    let res2 = conn.query_row("SELECT num FROM scope_db.sample;", [], |r| {
        r.get::<_, i32>(0)
    });
    assert!(res2.is_err());
}

#[test]
fn test_adversarial_sequential_attachment_lock_avoidance() {
    let dir = TempDir::new().expect("tempdir");
    let cnpj_path = dir.path().join("cnpj.db");
    let rede_path = dir.path().join("rede.db");

    // Create cnpj.db with sample data
    {
        let conn = Connection::open(&cnpj_path).expect("open cnpj");
        conn.execute_batch(
            "CREATE TABLE estabelecimento (cnpj_basico TEXT, cnpj TEXT, matriz_filial TEXT, nome_fantasia TEXT);
             CREATE TABLE empresas (cnpj_basico TEXT, razao_social TEXT);
             INSERT INTO estabelecimento VALUES ('12345678', '12345678000195', '1', 'FANTASIA');
             INSERT INTO empresas VALUES ('12345678', 'RAZAO EMPRESA');",
        )
        .expect("init cnpj");
    }

    // Attach cnpj into rede repeatedly across 50 iterations to ensure zero lock accumulation
    for i in 0..50 {
        let conn = Connection::open(&rede_path).expect("open rede");
        apply_graph_pragmas(&conn, 256).expect("apply pragmas");

        let att = AttachedDatabase::attach(&conn, &cnpj_path, "cnpj").expect("attach cnpj");

        let count: i64 = conn
            .query_row("SELECT count(*) FROM cnpj.estabelecimento;", [], |r| {
                r.get(0)
            })
            .expect("query attached");
        assert_eq!(count, 1, "Iteration {i} failed to read attached db");

        att.detach().expect("detach cnpj");
        finalize_database_to_wal(&conn).expect("finalize wal");
        drop(conn); // Must release all locks completely
    }
}

// ============================================================================
// 4. WAL MODE PERSISTENCE ON GENERATED DATABASES
// ============================================================================

#[test]
fn test_adversarial_wal_mode_permanence_on_both_databases() {
    let dir = TempDir::new().expect("tempdir");
    let cnpj_path = dir.path().join("cnpj.db");
    let rede_path = dir.path().join("rede.db");
    let search_path = dir.path().join("rede_search.db");

    // Seed minimal valid cnpj.db
    {
        let conn = Connection::open(&cnpj_path).expect("open cnpj");
        create_initial_tables(&conn).expect("create tables");
        conn.execute_batch(
            "INSERT INTO empresas (cnpj_basico, razao_social) VALUES ('11111111', 'ALPHA S.A.');
             INSERT INTO estabelecimento (cnpj_basico, cnpj, matriz_filial, nome_fantasia) VALUES ('11111111', '11111111000191', '1', 'ALPHA');
             INSERT INTO socios (cnpj, identificador_de_socio, nome_socio, cnpj_cpf_socio, qualificacao_socio) VALUES ('11111111000191', '2', 'MARCOS ALVES', '***123456**', '49');",
        )
        .expect("seed data");
    }

    let args = BuildRedeArgs {
        cnpj_db: cnpj_path,
        output_rede: rede_path.clone(),
        output_search: search_path.clone(),
        drop_existing: true,
        memory_limit_mb: 512,
    };

    let orchestrator = GraphOrchestrator::new(args);
    let stats = orchestrator.execute().expect("orchestrator execute");
    assert!(stats.total_edges >= 1);
    assert!(stats.total_search_entries >= 1);

    // 1. Verify rede.db WAL mode & page size in completely fresh connection
    {
        let conn_rede = Connection::open(&rede_path).expect("fresh open rede");
        let journal_mode: String = conn_rede
            .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
            .expect("read journal_mode");
        assert_eq!(
            journal_mode.to_lowercase(),
            "wal",
            "rede.db must permanently retain WAL journal_mode in DB header"
        );

        let page_size: i32 = conn_rede
            .query_row("PRAGMA page_size;", [], |r| r.get(0))
            .expect("read page_size");
        assert_eq!(page_size, 4096, "rede.db page_size must be 4096 bytes");
    }

    // 2. Verify rede_search.db WAL mode & page size in completely fresh connection
    {
        let conn_search = Connection::open(&search_path).expect("fresh open search");
        let journal_mode: String = conn_search
            .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
            .expect("read journal_mode");
        assert_eq!(
            journal_mode.to_lowercase(),
            "wal",
            "rede_search.db must permanently retain WAL journal_mode in DB header"
        );

        let page_size: i32 = conn_search
            .query_row("PRAGMA page_size;", [], |r| r.get(0))
            .expect("read page_size");
        assert_eq!(
            page_size, 4096,
            "rede_search.db page_size must be 4096 bytes"
        );
    }
}

// ============================================================================
// 5. MEMORY CEILING & RSS BOUNDS (< 1.8 GB RAM)
// ============================================================================

#[test]
fn test_adversarial_cache_size_and_temp_store_pragmas() {
    let tmp = NamedTempFile::new().expect("tempfile");
    let conn = Connection::open(tmp.path()).expect("open db");

    // Apply pragmas with 1,536 MB (1.5 GB) budget
    apply_graph_pragmas(&conn, 1536).expect("apply pragmas");

    // cache_kib = (1536 * 0.35) * 1024 = 550,502 KiB
    // In SQLite, PRAGMA cache_size returns negative page count if set in KiB
    let cache_size: i64 = conn
        .query_row("PRAGMA cache_size;", [], |r| r.get(0))
        .expect("cache_size");
    assert!(
        cache_size < 0,
        "cache_size must be negative indicating KiB memory bounds"
    );
    assert!(
        cache_size.abs() >= 550_000,
        "cache_size should allocate ~550,000 KiB for 1.5 GB budget, got {cache_size}"
    );

    // Verify temp_store = FILE (1) to prevent temporary table OOM
    let temp_store: i32 = conn
        .query_row("PRAGMA temp_store;", [], |r| r.get(0))
        .expect("temp_store");
    assert_eq!(temp_store, 1, "PRAGMA temp_store must be 1 (FILE)");
}

#[test]
fn test_adversarial_memory_bounds_under_heavy_graph_derivation() {
    let dir = TempDir::new().expect("tempdir");
    let cnpj_path = dir.path().join("cnpj_heavy.db");
    let rede_path = dir.path().join("rede_heavy.db");
    let search_path = dir.path().join("rede_search_heavy.db");

    let initial_rss = get_resident_memory_bytes().unwrap_or(0);

    // Synthesize 10,000 corporate groups (20,000 establishments, 10,000 partners, 10,000 filiais)
    let n_companies = 10_000;
    {
        let conn = Connection::open(&cnpj_path).expect("open cnpj");
        apply_graph_pragmas(&conn, 512).expect("pragmas");
        create_initial_tables(&conn).expect("initial tables");

        conn.execute_batch("BEGIN TRANSACTION;").expect("begin");

        {
            let mut stmt_emp = conn
                .prepare("INSERT INTO empresas (cnpj_basico, razao_social) VALUES (?1, ?2);")
                .expect("prep emp");
            let mut stmt_est = conn
                .prepare("INSERT INTO estabelecimento (cnpj_basico, cnpj, matriz_filial, nome_fantasia) VALUES (?1, ?2, ?3, ?4);")
                .expect("prep est");
            let mut stmt_soc = conn
                .prepare("INSERT INTO socios (cnpj, identificador_de_socio, nome_socio, cnpj_cpf_socio, qualificacao_socio) VALUES (?1, ?2, ?3, ?4, ?5);")
                .expect("prep soc");

            for i in 0..n_companies {
                let basico = format!("{i:08}");
                let cnpj_matriz = format!("{i:08}000191");
                let cnpj_filial = format!("{i:08}000272");

                stmt_emp
                    .execute([&basico, &format!("EMPRESA STRESS {i} S.A.")])
                    .expect("insert emp");

                // Matriz
                stmt_est
                    .execute([&basico, &cnpj_matriz, "1", &format!("FANTASIA {i}")])
                    .expect("insert est matriz");

                // Filial
                stmt_est
                    .execute([&basico, &cnpj_filial, "2", ""])
                    .expect("insert est filial");

                // Sócio PF
                stmt_soc
                    .execute(rusqlite::params![
                        &cnpj_matriz,
                        "2",
                        &format!("SOCIO STRESS {i}"),
                        &format!("***{:06}**", i % 1_000_000),
                        "49"
                    ])
                    .expect("insert soc");
            }
        }

        conn.execute_batch("COMMIT;").expect("commit");
        finalize_database_to_wal(&conn).expect("finalize");
    }

    let args = BuildRedeArgs {
        cnpj_db: cnpj_path,
        output_rede: rede_path.clone(),
        output_search: search_path.clone(),
        drop_existing: true,
        memory_limit_mb: 512,
    };

    let orchestrator = GraphOrchestrator::new(args);
    let stats = orchestrator
        .execute()
        .expect("execute heavy graph derivation");

    let peak_rss = stats.peak_rss_bytes;
    eprintln!(
        "Milestone 3 Graph Stress RSS: Initial: {:.2} MB, Peak: {:.2} MB, Memory Budget: 1,800 MB",
        initial_rss as f64 / (1024.0 * 1024.0),
        peak_rss as f64 / (1024.0 * 1024.0)
    );

    // Hard constraint: Peak RSS MUST remain below 1,800 MB
    let max_budget_bytes = 1800 * 1024 * 1024;
    assert!(
        peak_rss < max_budget_bytes,
        "Peak RSS ({peak_rss} bytes) exceeded maximum ceiling ({max_budget_bytes} bytes)"
    );

    // Verify derived relationships count
    assert!(stats.total_edges >= (n_companies as u64) * 2); // At least 10k filial + 10k socio
    assert!(stats.total_search_entries >= (n_companies as u64) * 2);

    // Verify indices on rede.db
    let conn_rede = Connection::open(&rede_path).expect("open rede");
    let idx_origem: bool = conn_rede
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name='idx_ligacao_origem';",
            [],
            |r| r.get(0),
        )
        .expect("check index");
    assert!(idx_origem, "idx_ligacao_origem must exist in rede.db");

    let idx_destino: bool = conn_rede
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name='idx_ligacao_destino';",
            [],
            |r| r.get(0),
        )
        .expect("check index");
    assert!(idx_destino, "idx_ligacao_destino must exist in rede.db");
}
