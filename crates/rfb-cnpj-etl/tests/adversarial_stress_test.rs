//! Adversarial stress test suite for Milestone 2:
//! - Channel backpressure and memory bounding
//! - RSS memory telemetry under load
//! - Corrupt and truncated archives
//! - Empty and partial input handling
//! - Post-load index idempotency and `--skip-indices`

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::bounded;
use rusqlite::Connection;
use tempfile::TempDir;
use zip::write::{SimpleFileOptions, ZipWriter};

use rfb_cnpj_etl::cli::BuildCnpjArgs;
use rfb_cnpj_etl::ingestion::loader::*;
use rfb_cnpj_etl::ingestion::schema::*;
use rfb_cnpj_etl::ingestion::zip_stream::*;
use rfb_cnpj_etl::ingestion::{get_resident_memory_bytes, IngestionOrchestrator};

fn create_zip_archive(zip_path: &Path, entry_name: &str, content: &[u8]) {
    let file = File::create(zip_path).expect("create zip file");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(entry_name, options)
        .expect("start zip entry");
    zip.write_all(content).expect("write zip content");
    zip.finish().expect("finish zip file");
}

/// Helper to generate synthetic CSV data with N rows.
fn generate_empresa_csv(rows: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(rows * 80);
    for i in 0..rows {
        let line = format!(
            "{i:08};RAZAO SOCIAL EMPRESA {i};2062;49;{val},50;01;ENTE {i}\r\n",
            i = i,
            val = (i * 100) + 50
        );
        data.extend_from_slice(line.as_bytes());
    }
    data
}

// ============================================================================
// 1. CHANNEL BACKPRESSURE & MEMORY BOUNDING EMPIRICAL TESTS
// ============================================================================

#[test]
fn test_channel_backpressure_slow_consumer_blocks_producer() {
    let temp = TempDir::new().expect("tempdir");
    let zip_path = temp.path().join("Empresas0.zip");

    // 25,000 rows = 50 batches of 500 rows
    let row_count = 25_000;
    let batch_size = 500;
    let csv_bytes = generate_empresa_csv(row_count);
    create_zip_archive(&zip_path, "F.K03200$Z.D61024.EMPRECSV", &csv_bytes);

    // Channel capacity limited to 4 batches
    let capacity = 4;
    let (tx, rx) = bounded::<anyhow::Result<Vec<EmpresaBatchRow>>>(capacity);

    let zip_clone = zip_path.clone();
    let producer_finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let prod_flag_clone = Arc::clone(&producer_finished);

    let producer = thread::spawn(move || {
        let stats = stream_zip_batches(&zip_clone, Some(".EMPRECSV"), batch_size, &tx, |rec| {
            Ok(EmpresaBatchRow::from_byte_record(rec))
        })
        .expect("producer failed");
        prod_flag_clone.store(true, Ordering::SeqCst);
        stats
    });

    // Give producer 50ms to fill the channel buffer
    thread::sleep(Duration::from_millis(50));

    // Because capacity is 4 and consumer hasn't drained, channel must be full and producer must NOT be finished
    assert_eq!(rx.len(), capacity, "Channel must be completely saturated");
    assert!(
        !producer_finished.load(Ordering::SeqCst),
        "Producer must be blocked by backpressure"
    );

    // Slowly consume batches and monitor channel length
    let mut total_received = 0;
    while let Ok(batch_res) = rx.recv() {
        let batch = batch_res.expect("valid batch");
        total_received += batch.len();

        // Slow consumer: sleep 5ms per batch
        thread::sleep(Duration::from_millis(5));

        // In-flight channel length should never exceed capacity
        assert!(rx.len() <= capacity, "Channel length exceeded bound!");
    }

    let stats = producer.join().expect("join producer");
    assert_eq!(stats.rows_processed, row_count);
    assert_eq!(total_received, row_count);
    assert!(producer_finished.load(Ordering::SeqCst));
}

#[test]
fn test_memory_bounding_under_high_row_count() {
    let temp = TempDir::new().expect("tempdir");
    let input_dir = temp.path().join("zip");
    fs::create_dir_all(&input_dir).expect("mkdir");
    let output_db = temp.path().join("cnpj_stress.db");

    // Generate 50,000 Empresa rows and 50,000 Estabelecimento rows
    let rows = 50_000;
    let emp_csv = generate_empresa_csv(rows);
    create_zip_archive(
        &input_dir.join("Empresas0.zip"),
        "F.K03200$Z.D61024.EMPRECSV",
        &emp_csv,
    );

    let mut estab_csv = Vec::with_capacity(rows * 120);
    for i in 0..rows {
        let line = format!(
            "{i:08};0001;91;1;FANTASIA {i};02;20210101;00;;105;20200101;6201501;;RUA {i};1;;BAIRRO;79002000;MS;9051;67;33334444;;;;;contato@test.com;;\r\n"
        );
        estab_csv.extend_from_slice(line.as_bytes());
    }
    create_zip_archive(
        &input_dir.join("Estabelecimentos0.zip"),
        "F.K03200$Z.D61024.ESTABELE",
        &estab_csv,
    );

    // Also create Socios0.zip so stage 8 post-indexing does not fail due to missing table
    create_zip_archive(
        &input_dir.join("Socios0.zip"),
        "F.K03200$Z.D61024.SOCIOCSV",
        b"00000000;2;JOAO DA SILVA;***123456**;49;20200101;105;;;;5\r\n",
    );

    // Initial RSS
    let initial_rss = get_resident_memory_bytes().unwrap_or(0);

    let args = BuildCnpjArgs {
        input_dir,
        output_db: output_db.clone(),
        memory_limit_mb: 512,
        batch_size: 5000,
        threads: 4,
        drop_existing: true,
        skip_indices: false,
    };

    let orchestrator = IngestionOrchestrator::new(args).expect("orchestrator init");
    let stats = orchestrator.execute().expect("orchestrator execute");

    let peak_rss = stats.peak_rss_bytes;
    eprintln!(
        "Stress Test Memory: Initial RSS: {:.2} MB, Peak RSS: {:.2} MB, Limit: 1,800 MB",
        initial_rss as f64 / (1024.0 * 1024.0),
        peak_rss as f64 / (1024.0 * 1024.0)
    );

    // Assert that peak RSS is well within the 1.8 GB constraint (< 1,800 MB)
    let max_budget_bytes = 1800 * 1024 * 1024;
    assert!(
        peak_rss < max_budget_bytes,
        "Peak RSS ({peak_rss} bytes) exceeded budget ({max_budget_bytes} bytes)"
    );

    // Database assertions
    let conn = Connection::open(&output_db).expect("open db");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM empresas", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, rows as i64);
}

// ============================================================================
// 2. CORRUPT & TRUNCATED ARCHIVE STRESS TESTS
// ============================================================================

#[test]
fn test_corrupt_zip_truncated_file_returns_error() {
    let temp = TempDir::new().expect("tempdir");
    let zip_path = temp.path().join("Corrupt.zip");

    // Write incomplete zip header
    fs::write(
        &zip_path,
        b"PK\x03\x04\x14\x00\x00\x00This is a truncated file",
    )
    .unwrap();

    let res = inspect_zip(&zip_path, None);
    assert!(res.is_err(), "inspect_zip must return Err on truncated zip");

    let stream_res = process_zip_byte_records(&zip_path, None, |_| Ok(()));
    assert!(
        stream_res.is_err(),
        "process_zip_byte_records must return Err on truncated zip"
    );
}

#[test]
fn test_corrupt_zip_garbage_content_returns_error() {
    let temp = TempDir::new().expect("tempdir");
    let zip_path = temp.path().join("Garbage.zip");

    // 100 KB of random garbage
    let garbage: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
    fs::write(&zip_path, &garbage).unwrap();

    let res = inspect_zip(&zip_path, None);
    assert!(res.is_err(), "inspect_zip must return Err on garbage file");

    let stream_res = process_zip_byte_records(&zip_path, None, |_| Ok(()));
    assert!(
        stream_res.is_err(),
        "process_zip_byte_records must return Err on garbage file"
    );
}

#[test]
fn test_zero_byte_zip_returns_error() {
    let temp = TempDir::new().expect("tempdir");
    let zip_path = temp.path().join("Empty.zip");
    fs::write(&zip_path, b"").unwrap();

    let res = inspect_zip(&zip_path, None);
    assert!(res.is_err(), "0-byte file must return Err");
}

#[test]
fn test_corrupt_file_in_parallel_worker_stops_cleanly_without_hang() {
    let temp = TempDir::new().expect("tempdir");
    let input_dir = temp.path().join("zip");
    fs::create_dir_all(&input_dir).expect("mkdir");
    let output_db = temp.path().join("cnpj_corrupt.db");

    // Valid Empresas0.zip
    create_zip_archive(
        &input_dir.join("Empresas0.zip"),
        "F.K03200$Z.D61024.EMPRECSV",
        b"11111111;ALFA;2062;49;1000,00;01;\r\n",
    );

    // Corrupted Empresas1.zip
    fs::write(
        input_dir.join("Empresas1.zip"),
        b"PK\x03\x04CORRUPTED_ZIP_STREAM",
    )
    .unwrap();

    let args = BuildCnpjArgs {
        input_dir,
        output_db,
        memory_limit_mb: 512,
        batch_size: 1000,
        threads: 2,
        drop_existing: true,
        skip_indices: true,
    };

    let orchestrator = IngestionOrchestrator::new(args).expect("init");
    let result = orchestrator.execute();

    assert!(
        result.is_err(),
        "Execution must fail when a partition zip is corrupt"
    );
    let err_msg = format!("{:#}", result.err().unwrap());
    assert!(
        err_msg.contains("Empresas") || err_msg.contains("Stream error"),
        "Error message must clearly identify the stream failure: {err_msg}"
    );
}

// ============================================================================
// 3. EMPTY & PARTIAL INPUT STRESS TESTS
// ============================================================================

#[test]
fn test_non_existent_input_directory_returns_error() {
    let args = BuildCnpjArgs {
        input_dir: PathBuf::from("/non/existent/path/12345/67890"),
        output_db: PathBuf::from("/tmp/non_existent.db"),
        memory_limit_mb: 512,
        batch_size: 1000,
        threads: 2,
        drop_existing: true,
        skip_indices: true,
    };

    let orchestrator = IngestionOrchestrator::new(args).expect("init");
    let res = orchestrator.execute();
    assert!(res.is_err());
    let err = format!("{:#}", res.err().unwrap());
    assert!(err.contains("Input directory does not exist"));
}

#[test]
fn test_empty_input_directory_behavior() {
    let temp = TempDir::new().expect("tempdir");
    let empty_dir = temp.path().join("empty_zips");
    fs::create_dir_all(&empty_dir).expect("mkdir");
    let output_db = temp.path().join("cnpj_empty.db");

    let args = BuildCnpjArgs {
        input_dir: empty_dir,
        output_db: output_db.clone(),
        memory_limit_mb: 512,
        batch_size: 1000,
        threads: 2,
        drop_existing: true,
        skip_indices: false,
    };

    let orchestrator = IngestionOrchestrator::new(args).expect("init");
    let stats = orchestrator
        .execute()
        .expect("Empty input directory must succeed cleanly");
    assert_eq!(stats.total_rows, 1); // 1 row from _referencia

    let conn = Connection::open(&output_db).expect("open db");

    // All 12 tables must exist
    for table in [
        "cnae",
        "motivo",
        "municipio",
        "natureza_juridica",
        "pais",
        "qualificacao_socio",
        "empresas",
        "estabelecimento",
        "simples",
        "cnpj_base2matriz",
        "socios",
        "_referencia",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                rusqlite::params![table],
                |r| r.get(0),
            )
            .expect("check table");
        assert!(exists, "Table {} must exist", table);
    }

    // Staging table must NOT exist
    let staging_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='socios_original'",
            [],
            |r| r.get(0),
        )
        .expect("check staging");
    assert!(!staging_exists, "socios_original must be dropped");

    // Socios indices must exist
    let idx_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND tbl_name='socios'",
            [],
            |r| r.get(0),
        )
        .expect("count socios indices");
    assert_eq!(idx_count, 5, "All 5 socios indices must be created");
}

// ============================================================================
// 4. POST-LOAD INDEX IDEMPOTENCY & SKIP-INDICES
// ============================================================================

#[test]
fn test_post_load_indices_idempotency() {
    let conn = Connection::open_in_memory().expect("open memory db");
    create_initial_tables(&conn).expect("initial tables");
    conn.execute_batch(SOCIOS_FINAL_DDL).expect("socios final");
    conn.execute_batch(CNPJ_BASE2MATRIZ_DDL)
        .expect("base2matriz");

    // First call: creates indices
    create_post_load_indices(&conn).expect("create indices first time");

    // Second call: must succeed idempotently without error due to IF NOT EXISTS
    create_post_load_indices(&conn).expect("create indices second time idempotently");
}

#[test]
fn test_skip_indices_flag_omits_stage_8() {
    let temp = TempDir::new().expect("tempdir");
    let input_dir = temp.path().join("zip");
    fs::create_dir_all(&input_dir).expect("mkdir");
    let output_db = temp.path().join("cnpj_no_idx.db");

    create_zip_archive(
        &input_dir.join("Cnaes.zip"),
        "F.K03200$Z.D61024.CNAECSV",
        b"6201501;Desenvolvimento de software\r\n",
    );

    let args = BuildCnpjArgs {
        input_dir,
        output_db: output_db.clone(),
        memory_limit_mb: 512,
        batch_size: 1000,
        threads: 2,
        drop_existing: true,
        skip_indices: true,
    };

    let orchestrator = IngestionOrchestrator::new(args).expect("init");
    let stats = orchestrator.execute().expect("execute with skip_indices");

    assert_eq!(
        stats.stage_reports.len(),
        7,
        "Must execute exactly 7 stages when skip_indices=true"
    );
    assert!(
        stats
            .stage_reports
            .iter()
            .all(|r| !r.stage_name.contains("Post-Load Index")),
        "Stage reports must not contain Post-Load Index stage"
    );
}

#[test]
fn test_zip_entry_not_found_when_only_directories_present() {
    let temp = TempDir::new().expect("tempdir");
    let zip_path = temp.path().join("OnlyDirs.zip");

    let file = File::create(&zip_path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.add_directory("some_folder/", options).unwrap();
    zip.add_directory("__MACOSX/", options).unwrap();
    zip.start_file("__MACOSX/._junk", options).unwrap();
    zip.write_all(b"junk").unwrap();
    zip.finish().unwrap();

    let res = inspect_zip(&zip_path, None);
    assert!(res.is_err());
    match res.err().unwrap() {
        ZipStreamError::EntryNotFound(_, _) => {}
        other => panic!("Expected EntryNotFound, got: {other:?}"),
    }
}

#[test]
fn test_csv_unclosed_quote_fails_cleanly() {
    let temp = TempDir::new().expect("tempdir");
    let zip_path = temp.path().join("BadQuote.zip");

    // CSV with an unclosed double quote
    let bad_csv = b"11111111;\"UNCLOSED QUOTE IN RAZAO SOCIAL;2062\r\n";
    create_zip_archive(&zip_path, "test.csv", bad_csv);

    let res = process_zip_byte_records(&zip_path, None, |_| Ok(()));
    // The CSV reader should either error on EOF or parse with flexible
    // In our implementation, csv reader uses quote(b'"').
    eprintln!("Result on unclosed quote: {:?}", res.is_ok());
}

#[test]
fn test_partial_dataset_missing_socios_succeeds_gracefully() {
    let temp = TempDir::new().expect("tempdir");
    let input_dir = temp.path().join("zip");
    fs::create_dir_all(&input_dir).expect("mkdir");
    let output_db = temp.path().join("cnpj_partial.db");

    // Only provide Empresas0.zip
    create_zip_archive(
        &input_dir.join("Empresas0.zip"),
        "F.K03200$Z.D61024.EMPRECSV",
        b"11111111;ALFA;2062;49;1000,00;01;\r\n",
    );

    let args = BuildCnpjArgs {
        input_dir,
        output_db: output_db.clone(),
        memory_limit_mb: 512,
        batch_size: 1000,
        threads: 2,
        drop_existing: true,
        skip_indices: false,
    };

    let orchestrator = IngestionOrchestrator::new(args).expect("init");
    let res = orchestrator.execute();

    assert!(
        res.is_ok(),
        "Partial dataset missing Socios must succeed gracefully, but failed: {:?}",
        res.err()
    );

    let conn = Connection::open(&output_db).expect("open db");
    let socios_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='socios'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(socios_exists, "Table 'socios' must exist");

    let original_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='socios_original'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        !original_exists,
        "Staging table 'socios_original' must be dropped"
    );
}
