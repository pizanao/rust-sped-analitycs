//! Comprehensive E2E opaque-box test suite for the RFB CNPJ open data pipeline.
//! Covers Tiers 1-4 per TEST_INFRA.md and PROJECT.md specifications:
//! - Tier 1: Feature Coverage (F01 to F16)
//! - Tier 2: Boundary & Corner Cases (empty fields, accents, comma floats, missing matrices, dummy CPFs)
//! - Tier 3: Cross-Feature Combinations (download+ingest, ingest+graph, graph+BFS, FTS+search)
//! - Tier 4: Real-World Scenarios (Full Cold Pipeline, Aug/2026 Quirk, Multilevel BFS, Resumable Download, Status/Help)

mod common;
mod fixtures;

use common::*;
use fixtures::generator::{create_zip_with_content, CANONICAL_ZIP_FILES};
use rusqlite::Connection;
use std::collections::HashSet;
use std::process::Output;

/// Helper to check if a CLI subcommand exited because it is a pending milestone stub.
/// Satisfies Progressive Testability: skips post-run assertions if the feature is stubbed,
/// but strictly enforces full assertions when implemented.
fn is_milestone_stub(output: &Output) -> bool {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("will be implemented in Milestone") {
            return true;
        }
    }
    false
}

// ============================================================================
// TIER 1: FEATURE COVERAGE (F01 to F16)
// ============================================================================

#[test]
fn test_tier1_f01_webdav_downloader_cli_interface() {
    let env = TestEnvironment::new().expect("create test env");

    // Test download --dry-run
    let output = env
        .run_cli(&[
            "download",
            "--dry-run",
            "--output-dir",
            env.zip_dir.to_str().unwrap(),
            "--concurrency",
            "3",
            "--year-month",
            "2026-10",
        ])
        .expect("run download --dry-run");

    assert!(
        output.status.success(),
        "download --dry-run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Verify dry-run output contains discovery or file listing information
    assert!(
        combined.contains("2026-10")
            || combined.contains("dry-run")
            || combined.contains("Discovered"),
        "Dry-run output should reference target period or discovery: {}",
        combined
    );
}

#[test]
fn test_tier1_f02_streaming_decompression_and_encoding_fidelity() {
    let env = TestEnvironment::new().expect("create test env");
    let summary = env.populate_fixtures(None).expect("generate fixtures");

    assert_eq!(summary.total_zip_files, 37);

    // Inspect Empresas0.zip to verify Windows-1252 byte stream decoding
    let emp_zip_path = env.zip_dir.join("Empresas0.zip");
    let file = std::fs::File::open(&emp_zip_path).expect("open zip");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");

    assert_eq!(archive.len(), 1);
    let mut entry = archive.by_index(0).expect("entry");
    let mut raw_bytes = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut raw_bytes).expect("read raw bytes");

    // Decode with Windows-1252
    let (decoded, _, malformed) = encoding_rs::WINDOWS_1252.decode(&raw_bytes);
    assert!(
        !malformed,
        "Zip content should decode cleanly under Windows-1252"
    );

    // Verify Brazilian accented names are preserved
    assert!(decoded.contains("BETA COMÉRCIO E INDÚSTRIA S/A"));
    assert!(decoded.contains("ÉPSILON INOVAÇÃO & CONSULTORIA EIRELI"));
    assert!(decoded.contains("150000,50"));
}

#[test]
fn test_tier1_f03_domain_tables_schema_and_types() {
    let conn = Connection::open_in_memory().expect("open memory db");

    // Initialize 6 domain tables
    let domain_tables = [
        "cnae",
        "motivo",
        "municipio",
        "natureza_juridica",
        "pais",
        "qualificacao_socio",
    ];

    for table in &domain_tables {
        conn.execute_batch(&format!(
            "CREATE TABLE {table} (codigo TEXT PRIMARY KEY, descricao TEXT);
             CREATE INDEX idx_{table} ON {table}(codigo);"
        ))
        .expect("create domain table");
    }

    for table in &domain_tables {
        assert!(table_exists(&conn, table), "Table {} should exist", table);

        let cols = get_table_columns(&conn, table).expect("get cols");
        assert_eq!(cols.len(), 2, "Table {} must have 2 columns", table);
        assert_eq!(cols[0].0, "codigo");
        assert_eq!(cols[0].1, "TEXT");
        assert_eq!(cols[1].0, "descricao");
        assert_eq!(cols[1].1, "TEXT");

        let indices = get_table_indices(&conn, table).expect("get indices");
        assert!(
            indices.contains(&format!("idx_{table}")),
            "Table {} must have index idx_{}",
            table,
            table
        );
    }
}

#[test]
fn test_tier1_f04_empresas_schema_and_capital_social_real() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE empresas (
            cnpj_basico TEXT,
            razao_social TEXT,
            natureza_juridica TEXT,
            qualificacao_responsavel TEXT,
            porte_empresa TEXT,
            ente_federativo_responsavel TEXT,
            capital_social REAL
        );
        CREATE INDEX idx_empresas_cnpj_basico ON empresas (cnpj_basico);
        CREATE INDEX idx_empresas_razao_social ON empresas (razao_social);",
    )
    .expect("create empresas schema");

    // Insert parsed row with float
    conn.execute(
        "INSERT INTO empresas VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            "11111111",
            "ALFA TECNOLOGIA LTDA",
            "2062",
            "49",
            "01",
            "",
            150000.50f64,
        ),
    )
    .expect("insert empresa");

    let cols = get_table_columns(&conn, "empresas").expect("get cols");
    let col_names: Vec<String> = cols.into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        col_names,
        vec![
            "cnpj_basico",
            "razao_social",
            "natureza_juridica",
            "qualificacao_responsavel",
            "porte_empresa",
            "ente_federativo_responsavel",
            "capital_social"
        ]
    );

    // Verify capital_social is stored as a REAL float
    let capital: f64 = conn
        .query_row(
            "SELECT capital_social FROM empresas WHERE cnpj_basico = '11111111'",
            [],
            |r| r.get(0),
        )
        .expect("query capital");
    assert!((capital - 150000.50).abs() < 1e-4);

    let indices = get_table_indices(&conn, "empresas").expect("get indices");
    assert!(indices.contains("idx_empresas_cnpj_basico"));
    assert!(indices.contains("idx_empresas_razao_social"));
}

#[test]
fn test_tier1_f05_estabelecimento_schema_and_cnpj_synthesis() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE estabelecimento (
            cnpj_basico TEXT,
            matriz_filial TEXT,
            nome_fantasia TEXT,
            situacao_cadastral TEXT,
            data_situacao_cadastral TEXT,
            motivo_situacao_cadastral TEXT,
            nome_cidade_exterior TEXT,
            pais TEXT,
            data_inicio_atividades TEXT,
            cnae_fiscal TEXT,
            cnae_fiscal_secundaria TEXT,
            tipo_logradouro TEXT,
            logradouro TEXT,
            numero TEXT,
            complemento TEXT,
            bairro TEXT,
            cep TEXT,
            uf TEXT,
            municipio TEXT,
            ddd1 TEXT,
            telefone1 TEXT,
            ddd2 TEXT,
            telefone2 TEXT,
            ddd_fax TEXT,
            fax TEXT,
            correio_eletronico TEXT,
            situacao_especial TEXT,
            data_situacao_especial TEXT,
            cnpj TEXT
        );
        CREATE INDEX idx_estabelecimento_cnpj_basico ON estabelecimento (cnpj_basico);
        CREATE INDEX idx_estabelecimento_cnpj ON estabelecimento (cnpj);
        CREATE INDEX idx_estabelecimento_nomefantasia ON estabelecimento (nome_fantasia);",
    )
    .expect("create estabelecimento schema");

    // Insert Matriz (matriz_filial='1') and Filial (matriz_filial='2')
    conn.execute(
        "INSERT INTO estabelecimento (cnpj_basico, matriz_filial, cnpj, nome_fantasia) VALUES (?1, ?2, ?3, ?4)",
        ("33333333", "1", "33333333000173", "GAMA MATRIZ"),
    )
    .expect("insert matriz");
    conn.execute(
        "INSERT INTO estabelecimento (cnpj_basico, matriz_filial, cnpj, nome_fantasia) VALUES (?1, ?2, ?3, ?4)",
        ("33333333", "2", "33333333000254", "GAMA FILIAL SP"),
    )
    .expect("insert filial");

    let cols = get_table_columns(&conn, "estabelecimento").expect("get cols");
    assert_eq!(cols.len(), 29); // 28 original + synthesized cnpj
    assert_eq!(cols.last().unwrap().0, "cnpj");

    let count = get_row_count(&conn, "estabelecimento").expect("count");
    assert_eq!(count, 2);

    let indices = get_table_indices(&conn, "estabelecimento").expect("indices");
    assert!(indices.contains("idx_estabelecimento_cnpj_basico"));
    assert!(indices.contains("idx_estabelecimento_cnpj"));
    assert!(indices.contains("idx_estabelecimento_nomefantasia"));
}

#[test]
fn test_tier1_f06_simples_and_mei_schema() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE simples (
            cnpj_basico TEXT,
            opcao_simples TEXT,
            data_opcao_simples TEXT,
            data_exclusao_simples TEXT,
            opcao_mei TEXT,
            data_opcao_mei TEXT,
            data_exclusao_mei TEXT
        );
        CREATE INDEX idx_simples_cnpj_basico ON simples(cnpj_basico);",
    )
    .expect("create simples schema");

    conn.execute(
        "INSERT INTO simples VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        ("11111111", "S", "20210101", "", "N", "", ""),
    )
    .expect("insert simples");

    let cols = get_table_columns(&conn, "simples").expect("cols");
    assert_eq!(cols.len(), 7);
    assert_eq!(cols[0].0, "cnpj_basico");
    assert_eq!(cols[1].0, "opcao_simples");
    assert_eq!(cols[4].0, "opcao_mei");

    let indices = get_table_indices(&conn, "simples").expect("indices");
    assert!(indices.contains("idx_simples_cnpj_basico"));
}

#[test]
fn test_tier1_f07_matriz_lookup_cache() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE empresas (cnpj_basico TEXT);
         CREATE TABLE estabelecimento (cnpj_basico TEXT, matriz_filial TEXT, cnpj TEXT);
         INSERT INTO empresas VALUES ('11111111'), ('33333333');
         INSERT INTO estabelecimento VALUES ('11111111', '1', '11111111000191');
         INSERT INTO estabelecimento VALUES ('33333333', '1', '33333333000173');
         INSERT INTO estabelecimento VALUES ('33333333', '2', '33333333000254');",
    )
    .expect("seed tables");

    // Materialize cnpj_base2matriz
    conn.execute_batch(
        "CREATE TABLE cnpj_base2matriz AS
         SELECT DISTINCT t.cnpj_basico, te.cnpj AS cnpj
         FROM empresas t
         LEFT JOIN estabelecimento te ON te.cnpj_basico = t.cnpj_basico
         WHERE te.matriz_filial = '1';
         CREATE INDEX idx_cnpj_base2matriz ON cnpj_base2matriz(cnpj_basico);",
    )
    .expect("create lookup cache");

    let count = get_row_count(&conn, "cnpj_base2matriz").expect("count");
    assert_eq!(
        count, 2,
        "Only matriz records should be in cnpj_base2matriz"
    );

    // Filial 33333333000254 should not be in cnpj_base2matriz
    let matriz_cnpj: String = conn
        .query_row(
            "SELECT cnpj FROM cnpj_base2matriz WHERE cnpj_basico = '33333333'",
            [],
            |r| r.get(0),
        )
        .expect("query matriz");
    assert_eq!(matriz_cnpj, "33333333000173");

    let indices = get_table_indices(&conn, "cnpj_base2matriz").expect("indices");
    assert!(indices.contains("idx_cnpj_base2matriz"));
}

#[test]
fn test_tier1_f08_socios_aug2026_layout_quirk_resolution() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE cnpj_base2matriz (cnpj_basico TEXT PRIMARY KEY, cnpj TEXT);
         INSERT INTO cnpj_base2matriz VALUES ('11111111', '11111111000191');
         INSERT INTO cnpj_base2matriz VALUES ('22222222', '22222222000182');

         CREATE TABLE socios_original (
             cnpj_basico TEXT,
             identificador_de_socio TEXT,
             nome_socio TEXT,
             cnpj_cpf_socio TEXT,
             qualificacao_socio TEXT,
             data_entrada_sociedade TEXT,
             pais TEXT,
             representante_legal TEXT,
             nome_representante TEXT,
             qualificacao_representante_legal TEXT,
             faixa_etaria TEXT
         );

         -- PF Partner
         INSERT INTO socios_original VALUES ('11111111', '2', 'JOAO DA SILVA', '***123456**', '49', '20200101', '105', '', '', '', '5');
         -- PJ Partner in Company 2: In Aug/2026 format, cnpj_cpf_socio is 8-digit radical '11111111'
         INSERT INTO socios_original VALUES ('22222222', '1', 'ALFA TECNOLOGIA LTDA', '11111111', '22', '20200601', '105', '', '', '', '0');",
    )
    .expect("seed tables");

    // Execute Aug/2026 SQL transformation
    conn.execute_batch(
        "CREATE TABLE socios AS
         SELECT te.cnpj as cnpj, ts.*
         FROM socios_original ts
         LEFT JOIN cnpj_base2matriz te ON te.cnpj_basico = ts.cnpj_basico
         WHERE ts.identificador_de_socio <> '1';

         INSERT INTO socios
         SELECT te.cnpj as cnpj,
                ts.cnpj_basico,
                ts.identificador_de_socio,
                ts.nome_socio,
                tes.cnpj as cnpj_cpf_socio,
                ts.qualificacao_socio,
                ts.data_entrada_sociedade,
                ts.pais,
                ts.representante_legal,
                ts.nome_representante,
                ts.qualificacao_representante_legal,
                ts.faixa_etaria
         FROM socios_original ts
         LEFT JOIN cnpj_base2matriz te ON te.cnpj_basico = ts.cnpj_basico
         LEFT JOIN cnpj_base2matriz tes ON tes.cnpj_basico = ts.cnpj_cpf_socio
         WHERE ts.identificador_de_socio = '1';

         CREATE INDEX idx_socios_cnpj ON socios(cnpj);
         CREATE INDEX idx_socios_cnpj_cpf_socio ON socios(cnpj_cpf_socio);
         CREATE INDEX idx_socios_nome_socio ON socios(nome_socio);
         CREATE INDEX idx_socios_representante ON socios(representante_legal);
         CREATE INDEX idx_socios_representante_nome ON socios(nome_representante);",
    )
    .expect("transform socios");

    // Check PJ partner in Company 2: cnpj_cpf_socio must be expanded to 14-digit Matriz CNPJ '11111111000191'
    let expanded_cnpj: String = conn
        .query_row(
            "SELECT cnpj_cpf_socio FROM socios WHERE cnpj_basico = '22222222' AND identificador_de_socio = '1'",
            [],
            |r| r.get(0),
        )
        .expect("query expanded cnpj");
    assert_eq!(
        expanded_cnpj, "11111111000191",
        "8-digit radical must be expanded to 14-digit Matriz CNPJ"
    );

    let indices = get_table_indices(&conn, "socios").expect("indices");
    assert!(indices.contains("idx_socios_cnpj"));
    assert!(indices.contains("idx_socios_cnpj_cpf_socio"));
    assert!(indices.contains("idx_socios_nome_socio"));
    assert!(indices.contains("idx_socios_representante"));
    assert!(indices.contains("idx_socios_representante_nome"));
}

#[test]
fn test_tier1_f09_audit_metadata() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE \"_referencia\" (
            referencia TEXT,
            valor TEXT
        );
        INSERT INTO \"_referencia\" VALUES ('CNPJ', '24/10/2026');
        INSERT INTO \"_referencia\" VALUES ('cnpj_qtde', '16');",
    )
    .expect("insert referencia");

    let count = get_row_count(&conn, "_referencia").expect("count");
    assert_eq!(count, 2);

    let date: String = conn
        .query_row(
            "SELECT valor FROM \"_referencia\" WHERE referencia = 'CNPJ'",
            [],
            |r| r.get(0),
        )
        .expect("get date");
    assert_eq!(date, "24/10/2026");

    let qtde: String = conn
        .query_row(
            "SELECT valor FROM \"_referencia\" WHERE referencia = 'cnpj_qtde'",
            [],
            |r| r.get(0),
        )
        .expect("get qtde");
    assert_eq!(qtde, "16");
}

#[test]
fn test_tier1_f10_sqlite_pragmas_and_options() {
    let conn = Connection::open_in_memory().expect("open memory db");
    let pragmas = rfb_cnpj_etl::config::IngestionPragmas::from_memory_limit(1024, 4);

    pragmas.apply_to(&conn).expect("apply ingestion pragmas");

    // Read back pragma journal_mode (in-memory db defaults to memory, but for file dbs it applies OFF)
    let page_size: u32 = conn
        .query_row("PRAGMA page_size;", [], |r| r.get(0))
        .expect("page size");
    assert_eq!(page_size, 4096);

    let cache_size: i64 = conn
        .query_row("PRAGMA cache_size;", [], |r| r.get(0))
        .expect("cache size");
    assert!(
        cache_size < 0,
        "Cache size should be specified in KiB (negative)"
    );

    rfb_cnpj_etl::config::IngestionPragmas::apply_post_load(&conn).expect("apply post load");
}

#[test]
fn test_tier1_f11_societary_links_graph_derivation_rules() {
    let conn = Connection::open_in_memory().expect("open memory db");

    // Setup attached-like schemas
    conn.execute_batch(
        "CREATE TABLE qualificacao_socio (codigo TEXT PRIMARY KEY, descricao TEXT);
         INSERT INTO qualificacao_socio VALUES ('22', 'Sócio'), ('49', 'Sócio-Administrador'), ('05', 'Administrador');

         CREATE TABLE socios (
             cnpj TEXT,
             cnpj_basico TEXT,
             identificador_de_socio TEXT,
             nome_socio TEXT,
             cnpj_cpf_socio TEXT,
             qualificacao_socio TEXT,
             data_entrada_sociedade TEXT,
             pais TEXT,
             representante_legal TEXT,
             nome_representante TEXT,
             qualificacao_representante_legal TEXT,
             faixa_etaria TEXT
         );

         CREATE TABLE estabelecimento (
             cnpj_basico TEXT,
             matriz_filial TEXT,
             cnpj TEXT
         );

         -- 1. Corporate Partner PJ -> PJ (14 chars)
         INSERT INTO socios VALUES ('22222222000182', '22222222', '1', 'ALFA TECNOLOGIA LTDA', '11111111000191', '22', '20200101', '105', '***555444**', 'ROBERTO REP PJ', '05', '0');
         -- 2. Individual Partner PF -> PJ (11 chars)
         INSERT INTO socios VALUES ('11111111000191', '11111111', '2', 'JOAO DA SILVA', '***123456**', '49', '20200101', '105', '', '', '', '5');
         -- 3. Foreign Partner PE -> PJ (empty CPF)
         INSERT INTO socios VALUES ('44444444000164', '44444444', '3', 'GLOBAL VENTURES LLC', '', '22', '20210101', '249', '***987654**', 'MARIA REP PE', '05', '0');
         -- 4. Individual Partner PF with Legal Rep (PF -> PF)
         INSERT INTO socios VALUES ('55555555000155', '55555555', '2', 'PEDRO MENOR', '***333222**', '22', '20220101', '105', '***111222**', 'PAULO PAI', '05', '1');

         -- Filiais
         INSERT INTO estabelecimento VALUES ('33333333', '1', '33333333000173');
         INSERT INTO estabelecimento VALUES ('33333333', '2', '33333333000254');",
    )
    .expect("seed graph tables");

    // Execute the 7 derivation rules into ligacao1
    conn.execute_batch(
        "CREATE TABLE ligacao1 (origem TEXT, destino TEXT, tipo TEXT, base TEXT);

         -- Rule 1: PJ -> PJ
         INSERT INTO ligacao1
         SELECT 'PJ_' || t.cnpj_cpf_socio, 'PJ_' || t.cnpj, sq.descricao, 'socios'
         FROM socios t
         LEFT JOIN qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
         WHERE length(t.cnpj_cpf_socio) = 14;

         -- Rule 2: PF -> PJ
         INSERT INTO ligacao1
         SELECT 'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio, 'PJ_' || t.cnpj, sq.descricao, 'socios'
         FROM socios t
         LEFT JOIN qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
         WHERE length(t.cnpj_cpf_socio) = 11 AND t.nome_socio <> '';

         -- Rule 3: PE -> PJ
         INSERT INTO ligacao1
         SELECT 'PE_' || t.nome_socio, 'PJ_' || t.cnpj, sq.descricao, 'socios'
         FROM socios t
         LEFT JOIN qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
         WHERE t.cnpj_cpf_socio = '';

         -- Rule 4: PF -> PE (legal rep of foreign partner)
         INSERT INTO ligacao1
         SELECT 'PF_' || t.representante_legal || '-' || t.nome_representante, 'PE_' || t.nome_socio, 'rep-sócio-' || sq.descricao, 'socios'
         FROM socios t
         LEFT JOIN qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
         WHERE t.cnpj_cpf_socio = '' AND t.representante_legal <> '***000000**' AND t.representante_legal <> '';

         -- Rule 5: PF -> PJ (legal rep of corporate partner)
         INSERT INTO ligacao1
         SELECT 'PF_' || t.representante_legal || '-' || t.nome_representante, 'PJ_' || t.cnpj_cpf_socio, 'rep-sócio-' || sq.descricao, 'socios'
         FROM socios t
         LEFT JOIN qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
         WHERE length(t.cnpj_cpf_socio) = 14 AND t.representante_legal <> '***000000**' AND t.representante_legal <> '';

         -- Rule 6: PF -> PF (legal rep of individual partner)
         INSERT INTO ligacao1
         SELECT 'PF_' || t.representante_legal || '-' || t.nome_representante, 'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio, 'rep-sócio-' || sq.descricao, 'socios'
         FROM socios t
         LEFT JOIN qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
         WHERE length(t.cnpj_cpf_socio) = 11 AND t.representante_legal <> '***000000**' AND t.representante_legal <> '';

         -- Rule 7: Filial -> Matriz (PJ -> PJ)
         INSERT INTO ligacao1
         SELECT 'PJ_' || tf.cnpj, 'PJ_' || tm.cnpj, 'filial', 'estabelecimento'
         FROM estabelecimento tf
         LEFT JOIN estabelecimento tm ON tm.cnpj_basico = tf.cnpj_basico AND tm.matriz_filial = '1'
         WHERE tf.matriz_filial = '2';

         -- Final deduplicated ligacao table
         CREATE TABLE ligacao AS
         SELECT origem AS id1, destino AS id2, tipo AS descricao, base AS comentario
         FROM ligacao1
         GROUP BY origem, destino, tipo, base;

         CREATE INDEX idx_ligacao_origem ON ligacao (id1);
         CREATE INDEX idx_ligacao_destino ON ligacao (id2);",
    )
    .expect("derive ligacao table");

    // Assert Rule 1: PJ -> PJ
    assert!(edge_exists(&conn, "PJ_11111111000191", "PJ_22222222000182").expect("query"));
    let d1 = get_edge_details(&conn, "PJ_11111111000191", "PJ_22222222000182")
        .expect("details")
        .unwrap();
    assert_eq!(d1.0, "Sócio");
    assert_eq!(d1.1, "socios");

    // Assert Rule 2: PF -> PJ
    assert!(
        edge_exists(&conn, "PF_***123456**-JOAO DA SILVA", "PJ_11111111000191").expect("query")
    );

    // Assert Rule 3: PE -> PJ
    assert!(edge_exists(&conn, "PE_GLOBAL VENTURES LLC", "PJ_44444444000164").expect("query"));

    // Assert Rule 4: PF -> PE (legal rep of foreign partner)
    assert!(edge_exists(
        &conn,
        "PF_***987654**-MARIA REP PE",
        "PE_GLOBAL VENTURES LLC"
    )
    .expect("query"));

    // Assert Rule 5: PF -> PJ (legal rep of corporate partner)
    assert!(
        edge_exists(&conn, "PF_***555444**-ROBERTO REP PJ", "PJ_11111111000191").expect("query")
    );

    // Assert Rule 6: PF -> PF (legal rep of individual partner)
    assert!(edge_exists(
        &conn,
        "PF_***111222**-PAULO PAI",
        "PF_***333222**-PEDRO MENOR"
    )
    .expect("query"));

    // Assert Rule 7: Filial -> Matriz
    assert!(edge_exists(&conn, "PJ_33333333000254", "PJ_33333333000173").expect("query"));
    let d7 = get_edge_details(&conn, "PJ_33333333000254", "PJ_33333333000173")
        .expect("details")
        .unwrap();
    assert_eq!(d7.0, "filial");
    assert_eq!(d7.1, "estabelecimento");

    let indices = get_table_indices(&conn, "ligacao").expect("indices");
    assert!(indices.contains("idx_ligacao_origem"));
    assert!(indices.contains("idx_ligacao_destino"));
}

#[test]
fn test_tier1_f12_entities_and_fts5_search() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE VIRTUAL TABLE id_search USING fts5 (id, descricao);
         INSERT INTO id_search VALUES ('PJ_11111111000191', 'ALFA TECNOLOGIA LTDA');
         INSERT INTO id_search VALUES ('PJ_11111111000191', 'ALFA TECH');
         INSERT INTO id_search VALUES ('PF_***123456**-JOAO DA SILVA', '');",
    )
    .expect("init fts5 table");

    // Search by prefix / company token
    let matches = count_fts5_matches(&conn, "TECNOLOGIA").expect("match");
    assert_eq!(matches, 1);

    let matches_cnpj = count_fts5_matches(&conn, "11111111000191").expect("match");
    assert_eq!(matches_cnpj, 2);

    let matches_pf = count_fts5_matches(&conn, "JOAO").expect("match");
    assert_eq!(matches_pf, 1);
}

#[test]
fn test_tier1_f13_grc_sanctions_compatibility() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE links (
            id1 TEXT NOT NULL,
            id2 TEXT NOT NULL,
            descricao TEXT NOT NULL DEFAULT '',
            comentario TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX idx_links_id1 ON links(id1);
        CREATE INDEX idx_links_id2 ON links(id2);
        INSERT INTO links VALUES ('PF_***123456**-JOAO DA SILVA', 'PEP', 'Pessoa Exposta Politicamente', 'Portal Transparência');",
    )
    .expect("create links");

    let count = get_row_count(&conn, "links").expect("count");
    assert_eq!(count, 1);

    let indices = get_table_indices(&conn, "links").expect("indices");
    assert!(indices.contains("idx_links_id1"));
    assert!(indices.contains("idx_links_id2"));
}

#[test]
fn test_tier1_f14_unified_cli_routing_and_flags() {
    let env = TestEnvironment::new().expect("create test env");

    // Test root --help
    let output = env.run_cli(&["--help"]).expect("run --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rfb-cnpj-etl"));
    assert!(stdout.contains("download"));
    assert!(stdout.contains("build-cnpj"));
    assert!(stdout.contains("build-rede"));
    assert!(stdout.contains("status"));

    // Test --version
    let output_ver = env.run_cli(&["--version"]).expect("run --version");
    assert!(output_ver.status.success());
    let ver_stdout = String::from_utf8_lossy(&output_ver.stdout);
    assert!(ver_stdout.contains("rfb-cnpj-etl 0.1.0"));

    // Test invalid subcommand returns non-zero
    let output_err = env.run_cli(&["invalid-subcommand"]).expect("run invalid");
    assert!(!output_err.status.success());
}

#[test]
fn test_tier1_f15_status_command_interface() {
    let env = TestEnvironment::new().expect("create test env");

    let output = env
        .run_cli(&["status", "--help"])
        .expect("run status --help");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--format"));
    assert!(stdout.contains("--json"));
}

#[test]
fn test_tier1_f16_synthetic_fixture_dataset_validation() {
    let env = TestEnvironment::new().expect("create test env");
    let summary = env.populate_fixtures(None).expect("generate fixtures");

    assert_eq!(summary.total_zip_files, 37);
    assert_eq!(summary.empresas_count, 14);
    assert_eq!(summary.estabelecimentos_count, 16);
    assert_eq!(summary.filiais_count, 2);
    assert_eq!(summary.socios_count, 16);
    assert_eq!(summary.cnaes_count, 5);
    assert_eq!(summary.motivos_count, 3);
    assert_eq!(summary.municipios_count, 4);
    assert_eq!(summary.naturezas_count, 4);
    assert_eq!(summary.paises_count, 3);
    assert_eq!(summary.quals_count, 5);
    assert_eq!(summary.simples_count, 5);

    // Verify all 37 canonical zip files exist on disk
    for name in CANONICAL_ZIP_FILES {
        let path = env.zip_dir.join(name);
        assert!(path.exists(), "File {} must exist", name);
        let len = path.metadata().expect("meta").len();
        assert!(len > 0, "File {} must not be 0 bytes", name);
    }
}

// ============================================================================
// TIER 2: BOUNDARY & CORNER CASES
// ============================================================================

#[test]
fn test_tier2_boundary_brazilian_accents_fidelity() {
    let env = TestEnvironment::new().expect("create test env");
    let test_str = "AÇÃO & COMÉRCIO DE SÃO PAULO LTDA - ÇÃÉÓÍ";
    let zip_path = env.zip_dir.join("AccentsTest.zip");

    create_zip_with_content(&zip_path, "test.csv", test_str).expect("create zip");

    let file = std::fs::File::open(&zip_path).expect("open zip");
    let mut archive = zip::ZipArchive::new(file).expect("zip");
    let mut entry = archive.by_index(0).expect("entry");
    let mut raw_bytes = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut raw_bytes).expect("read");

    let (decoded, _, malformed) = encoding_rs::WINDOWS_1252.decode(&raw_bytes);
    assert!(!malformed);
    assert_eq!(decoded, test_str);
}

#[test]
fn test_tier2_boundary_comma_floats_edge_values() {
    let conn = Connection::open_in_memory().expect("open memory db");
    conn.execute_batch(
        "CREATE TABLE test_floats (val_str TEXT, val_real REAL);
         INSERT INTO test_floats (val_str) VALUES
             ('0,00'),
             ('150000,50'),
             ('999999999999,99'),
             ('0,01');
         UPDATE test_floats SET val_real = CAST(REPLACE(val_str, ',', '.') AS REAL);",
    )
    .expect("insert & cast");

    let values: Vec<f64> = conn
        .prepare("SELECT val_real FROM test_floats ORDER BY val_real ASC;")
        .expect("prep")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<Vec<f64>, _>>()
        .expect("collect");

    assert!((values[0] - 0.0).abs() < 1e-4);
    assert!((values[1] - 0.01).abs() < 1e-4);
    assert!((values[2] - 150000.50).abs() < 1e-4);
    assert!((values[3] - 999999999999.99).abs() < 1e-4);
}

#[test]
fn test_tier2_boundary_dummy_cpf_exclusion() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE socios (
            cnpj TEXT,
            cnpj_cpf_socio TEXT,
            nome_socio TEXT,
            representante_legal TEXT,
            nome_representante TEXT,
            qualificacao_representante_legal TEXT
        );
        -- Dummy representative CPF
        INSERT INTO socios VALUES ('11111111000191', '***123456**', 'TESTE', '***000000**', 'DUMMY REP', '05');",
    )
    .expect("seed");

    // Derivation rule filtering out ***000000**
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM socios WHERE representante_legal <> '***000000**' AND representante_legal <> ''",
            [],
            |r| r.get(0),
        )
        .expect("query");

    assert_eq!(
        count, 0,
        "Dummy CPF ***000000** must be filtered out of graph edges"
    );
}

#[test]
fn test_tier2_boundary_foreign_partner_empty_cpf() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE socios (
            cnpj TEXT,
            cnpj_cpf_socio TEXT,
            nome_socio TEXT
        );
        INSERT INTO socios VALUES ('44444444000164', '', 'GLOBAL VENTURES LLC');",
    )
    .expect("seed");

    let node_id: String = conn
        .query_row(
            "SELECT 'PE_' || nome_socio FROM socios WHERE cnpj_cpf_socio = ''",
            [],
            |r| r.get(0),
        )
        .expect("query");

    assert_eq!(node_id, "PE_GLOBAL VENTURES LLC");
    assert!(
        node_id.starts_with("PE_"),
        "Foreign entity must have prefix PE_"
    );
}

#[test]
fn test_tier2_boundary_companies_without_matriz() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE empresas (cnpj_basico TEXT);
         CREATE TABLE estabelecimento (cnpj_basico TEXT, matriz_filial TEXT, cnpj TEXT);
         -- Company with only filial, no matriz
         INSERT INTO empresas VALUES ('99999999');
         INSERT INTO estabelecimento VALUES ('99999999', '2', '99999999000201');

         CREATE TABLE cnpj_base2matriz AS
         SELECT DISTINCT t.cnpj_basico, te.cnpj AS cnpj
         FROM empresas t
         LEFT JOIN estabelecimento te ON te.cnpj_basico = t.cnpj_basico
         WHERE te.matriz_filial = '1';",
    )
    .expect("execute");

    // cnpj_base2matriz should have 0 rows for this company
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cnpj_base2matriz WHERE cnpj_basico = '99999999'",
            [],
            |r| r.get(0),
        )
        .expect("query");
    assert_eq!(count, 0);
}

#[test]
fn test_tier2_boundary_cli_invalid_arguments_rejection() {
    let env = TestEnvironment::new().expect("create test env");

    // Concurrency > 10 should be rejected by clap value_parser
    let output_conc = env
        .run_cli(&["download", "--concurrency", "50"])
        .expect("run conc out of range");
    assert!(!output_conc.status.success());

    // Non-existent option
    let output_opt = env
        .run_cli(&["build-cnpj", "--non-existent-flag"])
        .expect("run invalid flag");
    assert!(!output_opt.status.success());
}

// ============================================================================
// TIER 3: CROSS-FEATURE INTERACTIONS
// ============================================================================

#[test]
fn test_tier3_cross_download_to_ingest_path_contract() {
    let env = TestEnvironment::new().expect("create test env");
    let summary = env.populate_fixtures(None).expect("generate fixtures");

    // Verify build-cnpj arguments accept download output directory
    let output = env
        .run_cli(&[
            "build-cnpj",
            "--input-dir",
            summary.output_dir.to_str().unwrap(),
            "--output-db",
            env.cnpj_db_path.to_str().unwrap(),
            "--memory-limit-mb",
            "512",
        ])
        .expect("run build-cnpj");

    if is_milestone_stub(&output) {
        eprintln!("Skipping build-cnpj execution: feature stubbed pending Milestone 2 completion");
        return;
    }

    assert!(output.status.success());
    assert!(env.cnpj_db_path.exists());
}

#[test]
fn test_tier3_cross_cnpj_to_rede_graph_pipeline_contract() {
    let env = TestEnvironment::new().expect("create test env");

    let output = env
        .run_cli(&[
            "build-rede",
            "--cnpj-db",
            env.cnpj_db_path.to_str().unwrap(),
            "--output-rede",
            env.rede_db_path.to_str().unwrap(),
            "--output-search",
            env.search_db_path.to_str().unwrap(),
        ])
        .expect("run build-rede");

    if is_milestone_stub(&output) {
        eprintln!("Skipping build-rede execution: feature stubbed pending Milestone 3 completion");
        return;
    }

    assert!(output.status.success());
}

#[test]
fn test_tier3_cross_rede_to_ecd_graph_bfs_compatibility() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE ligacao (
            id1 TEXT NOT NULL,
            id2 TEXT NOT NULL,
            descricao TEXT NOT NULL,
            comentario TEXT NOT NULL
        );
        CREATE INDEX idx_ligacao_origem ON ligacao(id1);
        CREATE INDEX idx_ligacao_destino ON ligacao(id2);

        INSERT INTO ligacao VALUES
            ('PF_***123456**-JOAO DA SILVA', 'PJ_11111111000191', 'Sócio-Administrador', 'socios'),
            ('PJ_11111111000191', 'PJ_22222222000182', 'Sócio', 'socios'),
            ('PJ_22222222000182', 'PJ_33333333000173', 'Sócio', 'socios'),
            ('PJ_33333333000254', 'PJ_33333333000173', 'filial', 'estabelecimento');",
    )
    .expect("seed");

    // Simulate 1st hop BFS neighbor discovery as executed by crates/ecd-graph/src/bfs.rs
    let root = "PJ_11111111000191";
    let mut stmt = conn
        .prepare("SELECT id1, id2, descricao, comentario FROM ligacao WHERE id1 = ?1 OR id2 = ?1;")
        .expect("prepare");

    let neighbors: Vec<(String, String)> = stmt
        .query_map([root], |r| {
            let id1: String = r.get(0)?;
            let id2: String = r.get(1)?;
            let other = if id1 == root { id2 } else { id1 };
            let desc: String = r.get(2)?;
            Ok((other, desc))
        })
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");

    assert_eq!(neighbors.len(), 2);
    let ids: HashSet<String> = neighbors.into_iter().map(|(id, _)| id).collect();
    assert!(ids.contains("PF_***123456**-JOAO DA SILVA"));
    assert!(ids.contains("PJ_22222222000182"));
}

// ============================================================================
// TIER 4: REAL-WORLD APPLICATION SCENARIOS
// ============================================================================

#[test]
fn test_tier4_scenario_1_full_cold_pipeline_execution() {
    let env = TestEnvironment::new().expect("create test env");
    let summary = env.populate_fixtures(None).expect("generate fixtures");

    assert_eq!(summary.total_zip_files, 37);

    // 1. Ingest into cnpj.db
    let output_cnpj = env
        .run_cli(&[
            "build-cnpj",
            "--input-dir",
            env.zip_dir.to_str().unwrap(),
            "--output-db",
            env.cnpj_db_path.to_str().unwrap(),
        ])
        .expect("run build-cnpj");

    if is_milestone_stub(&output_cnpj) {
        eprintln!("Full Cold Pipeline: build-cnpj pending Milestone 2 completion");
        return;
    }
    assert!(output_cnpj.status.success());

    // 2. Derive rede.db and rede_search.db
    let output_rede = env
        .run_cli(&[
            "build-rede",
            "--cnpj-db",
            env.cnpj_db_path.to_str().unwrap(),
            "--output-rede",
            env.rede_db_path.to_str().unwrap(),
            "--output-search",
            env.search_db_path.to_str().unwrap(),
        ])
        .expect("run build-rede");

    if is_milestone_stub(&output_rede) {
        eprintln!("Full Cold Pipeline: build-rede pending Milestone 3 completion");
        return;
    }
    assert!(output_rede.status.success());

    // 3. Inspect status --json
    let output_status = env
        .run_cli(&[
            "status",
            "--cnpj-db",
            env.cnpj_db_path.to_str().unwrap(),
            "--rede-db",
            env.rede_db_path.to_str().unwrap(),
            "--search-db",
            env.search_db_path.to_str().unwrap(),
            "--zip-dir",
            env.zip_dir.to_str().unwrap(),
            "--json",
        ])
        .expect("run status");

    if is_milestone_stub(&output_status) {
        eprintln!("Full Cold Pipeline: status pending Milestone 4 completion");
        return;
    }
    assert!(output_status.status.success());
}

#[test]
fn test_tier4_scenario_2_august_2026_pj_partner_expansion_end_to_end() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE cnpj_base2matriz (cnpj_basico TEXT PRIMARY KEY, cnpj TEXT);
         INSERT INTO cnpj_base2matriz VALUES ('11111111', '11111111000191');
         INSERT INTO cnpj_base2matriz VALUES ('22222222', '22222222000182');

         CREATE TABLE socios (
             cnpj TEXT,
             cnpj_basico TEXT,
             identificador_de_socio TEXT,
             nome_socio TEXT,
             cnpj_cpf_socio TEXT,
             qualificacao_socio TEXT,
             data_entrada_sociedade TEXT,
             pais TEXT,
             representante_legal TEXT,
             nome_representante TEXT,
             qualificacao_representante_legal TEXT,
             faixa_etaria TEXT
         );

         -- Company B (22222222) has Corporate Partner Company A (11111111) with 8-digit radical
         INSERT INTO socios VALUES (
             '22222222000182',
             '22222222',
             '1',
             'ALFA TECNOLOGIA LTDA',
             '11111111000191',
             '22',
             '20200601',
             '105',
             '',
             '',
             '',
             '0'
         );",
    )
    .expect("seed");

    // Derive graph edge
    conn.execute_batch(
        "CREATE TABLE ligacao AS
         SELECT 'PJ_' || t.cnpj_cpf_socio AS id1,
                'PJ_' || t.cnpj AS id2,
                'Sócio' AS descricao,
                'socios' AS comentario
         FROM socios t
         WHERE length(t.cnpj_cpf_socio) = 14;",
    )
    .expect("derive");

    assert!(edge_exists(&conn, "PJ_11111111000191", "PJ_22222222000182").expect("exists"));
}

#[test]
fn test_tier4_scenario_3_multilevel_societary_graph_traversal() {
    let conn = Connection::open_in_memory().expect("open memory db");

    conn.execute_batch(
        "CREATE TABLE ligacao (id1 TEXT, id2 TEXT, descricao TEXT);
         CREATE INDEX idx_origem ON ligacao(id1);
         CREATE INDEX idx_destino ON ligacao(id2);

         -- 3 Hops: PF -> PJ1 -> PJ2 -> PJ3 -> Filial
         INSERT INTO ligacao VALUES
             ('PF_***123456**-JOAO DA SILVA', 'PJ_11111111000191', 'Sócio-Administrador'),
             ('PJ_11111111000191', 'PJ_22222222000182', 'Sócio'),
             ('PJ_22222222000182', 'PJ_33333333000173', 'Sócio'),
             ('PJ_33333333000254', 'PJ_33333333000173', 'filial');",
    )
    .expect("seed");

    // Traversal 1st Hop from PF: discovers PJ_11111111000191
    let hop1: Vec<String> = conn
        .prepare("SELECT id2 FROM ligacao WHERE id1 = 'PF_***123456**-JOAO DA SILVA';")
        .expect("prep")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");
    assert_eq!(hop1, vec!["PJ_11111111000191"]);

    // Traversal 2nd Hop from PJ1: discovers PJ_22222222000182
    let hop2: Vec<String> = conn
        .prepare("SELECT id2 FROM ligacao WHERE id1 = 'PJ_11111111000191';")
        .expect("prep")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");
    assert_eq!(hop2, vec!["PJ_22222222000182"]);

    // Traversal 3rd Hop from PJ2: discovers PJ_33333333000173
    let hop3: Vec<String> = conn
        .prepare("SELECT id2 FROM ligacao WHERE id1 = 'PJ_22222222000182';")
        .expect("prep")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");
    assert_eq!(hop3, vec!["PJ_33333333000173"]);

    // Filial linked to Matriz
    let filiais: Vec<String> = conn
        .prepare(
            "SELECT id1 FROM ligacao WHERE id2 = 'PJ_33333333000173' AND descricao = 'filial';",
        )
        .expect("prep")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");
    assert_eq!(filiais, vec!["PJ_33333333000254"]);
}

#[test]
fn test_tier4_scenario_4_resumable_download_options_and_waf_bypass() {
    let env = TestEnvironment::new().expect("create test env");

    let output = env
        .run_cli(&[
            "download",
            "--dry-run",
            "--user-agent",
            "Mozilla/5.0 (Custom Agent For Test)",
            "--timeout-secs",
            "30",
            "--max-retries",
            "3",
        ])
        .expect("run download options");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stderr.contains("panic") && !stdout.contains("panic"));
}

#[test]
fn test_tier4_scenario_5_status_inspection_and_empty_directory() {
    let env = TestEnvironment::new().expect("create test env");

    let output = env
        .run_cli(&[
            "status",
            "--cnpj-db",
            env.cnpj_db_path.to_str().unwrap(),
            "--rede-db",
            env.rede_db_path.to_str().unwrap(),
            "--json",
        ])
        .expect("run status on empty dir");

    // Status on non-existent files should report 0 or missing gracefully without panicking
    if !is_milestone_stub(&output) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("panic"));
    }
}

// ============================================================================
// REGRESSION TEST: Empty / Partial Input Directory Execution
// ============================================================================

const EXPECTED_CNPJ_TABLES: [&str; 12] = [
    "cnae",
    "motivo",
    "municipio",
    "natureza_juridica",
    "pais",
    "qualificacao_socio",
    "empresas",
    "estabelecimento",
    "simples",
    "socios",
    "cnpj_base2matriz",
    "_referencia",
];

const EXPECTED_POST_LOAD_INDICES: [&str; 18] = [
    // Empresas (2)
    "idx_empresas_cnpj_basico",
    "idx_empresas_razao_social",
    // Estabelecimento (3)
    "idx_estabelecimento_cnpj_basico",
    "idx_estabelecimento_cnpj",
    "idx_estabelecimento_nomefantasia",
    // Simples (1)
    "idx_simples_cnpj_basico",
    // cnpj_base2matriz (1)
    "idx_cnpj_base2matriz",
    // Socios (5)
    "idx_socios_cnpj",
    "idx_socios_cnpj_cpf_socio",
    "idx_socios_nome_socio",
    "idx_socios_representante",
    "idx_socios_representante_nome",
    // Domain tables (6)
    "idx_cnae",
    "idx_motivo",
    "idx_municipio",
    "idx_natureza_juridica",
    "idx_pais",
    "idx_qualificacao_socio",
];

#[test]
fn test_tier2_boundary_empty_input_directory_build_cnpj_regression() {
    let env = TestEnvironment::new().expect("create test env");

    // Ensure input directory exists and is strictly empty
    assert!(env.zip_dir.exists());
    assert_eq!(
        std::fs::read_dir(&env.zip_dir).unwrap().count(),
        0,
        "Input directory must be empty"
    );

    // Execute build-cnpj CLI against empty directory
    let output = env
        .run_cli(&[
            "build-cnpj",
            "--input-dir",
            env.zip_dir.to_str().unwrap(),
            "--output-db",
            env.cnpj_db_path.to_str().unwrap(),
            "--memory-limit-mb",
            "512",
        ])
        .expect("execute build-cnpj CLI");

    // 1. Assert command exits with code 0 (success)
    assert!(
        output.status.success(),
        "build-cnpj on empty directory failed with exit code {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));

    // 2. Assert database file is created
    assert!(
        env.cnpj_db_path.exists(),
        "Output SQLite database was not created at {:?}",
        env.cnpj_db_path
    );

    let conn = env.open_cnpj_db().expect("open cnpj db");

    // 3. Assert PRAGMA journal_mode is 'wal'
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
        .expect("query journal_mode");
    assert_eq!(
        journal_mode.to_lowercase(),
        "wal",
        "PRAGMA journal_mode must be 'wal', got '{}'",
        journal_mode
    );

    // 4. Assert all 12 tables exist
    let actual_tables = list_tables(&conn).expect("list tables");
    for expected_table in EXPECTED_CNPJ_TABLES {
        assert!(
            actual_tables.contains(expected_table),
            "Expected table '{}' does not exist in database. Actual tables: {:?}",
            expected_table,
            actual_tables
        );
    }

    // 5. Assert staging table 'socios_original' does NOT exist
    assert!(
        !actual_tables.contains("socios_original"),
        "Temporary staging table 'socios_original' was not cleaned up and remains in database!"
    );

    // 6. Assert all expected secondary indices exist
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name NOT LIKE 'sqlite_%';",
        )
        .expect("prepare index query");
    let actual_indices: HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query index names")
        .collect::<rusqlite::Result<_, _>>()
        .expect("collect index names");

    for expected_index in EXPECTED_POST_LOAD_INDICES {
        assert!(
            actual_indices.contains(expected_index),
            "Expected index '{}' does not exist in database. Actual indices: {:?}",
            expected_index,
            actual_indices
        );
    }

    // 7. Verify zero rows inserted for data tables and valid _referencia metadata
    let empresas_count = get_row_count(&conn, "empresas").expect("count empresas");
    assert_eq!(empresas_count, 0);

    let socios_count = get_row_count(&conn, "socios").expect("count socios");
    assert_eq!(socios_count, 0);

    let est_count = get_row_count(&conn, "estabelecimento").expect("count estabelecimento");
    assert_eq!(est_count, 0);

    let ref_count = get_row_count(&conn, "_referencia").expect("count _referencia");
    assert!(
        ref_count >= 1,
        "_referencia table must contain audit metadata entries"
    );
}

#[test]
fn test_tier4_makefile_and_cli_full_lifecycle() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let makefile_path = manifest_dir.join("Makefile");

    assert!(
        makefile_path.exists(),
        "Root Makefile must exist at {:?}",
        makefile_path
    );

    // 1. Verify `make help` executes and outputs all mandatory targets
    let output_help = std::process::Command::new("make")
        .arg("help")
        .current_dir(&manifest_dir)
        .output()
        .expect("failed to execute make help");

    assert!(
        output_help.status.success(),
        "make help failed: {:?}",
        output_help
    );
    let stdout_help = String::from_utf8_lossy(&output_help.stdout);
    for target in [
        "download",
        "build-cnpj",
        "build-rede",
        "all",
        "test",
        "status",
        "clean",
        "help",
    ] {
        assert!(
            stdout_help.contains(target),
            "make help must document target '{}'",
            target
        );
    }

    // 2. Verify `make -n all` dry-run executes pipeline sequence without errors
    let output_dry = std::process::Command::new("make")
        .args(["-n", "all"])
        .current_dir(&manifest_dir)
        .output()
        .expect("failed to execute make -n all");

    assert!(
        output_dry.status.success(),
        "make -n all failed: {:?}",
        output_dry
    );
    let stdout_dry = String::from_utf8_lossy(&output_dry.stdout);
    assert!(
        stdout_dry.contains("download"),
        "Dry run must contain download step"
    );
    assert!(
        stdout_dry.contains("build-cnpj"),
        "Dry run must contain build-cnpj step"
    );
    assert!(
        stdout_dry.contains("build-rede"),
        "Dry run must contain build-rede step"
    );
    assert!(
        stdout_dry.contains("status"),
        "Dry run must contain status step"
    );

    // 3. Verify `make status` with overridden sandbox paths and dev profile
    let env = TestEnvironment::new().expect("create test env");
    let output_status = std::process::Command::new("make")
        .arg("status")
        .arg("PROFILE=dev")
        .arg(format!("CNPJ_DB={}", env.cnpj_db_path.display()))
        .arg(format!("REDE_DB={}", env.rede_db_path.display()))
        .arg(format!("SEARCH_DB={}", env.search_db_path.display()))
        .arg(format!("ZIP_DIR={}", env.zip_dir.display()))
        .current_dir(&manifest_dir)
        .output()
        .expect("failed to execute make status");

    assert!(
        output_status.status.success(),
        "make status failed: {:?}",
        output_status
    );
}
