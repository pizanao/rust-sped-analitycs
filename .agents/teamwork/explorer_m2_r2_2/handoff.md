# Handoff Report — Pipeline Orchestrator & Partition Flow Architecture: VULN-M2-01 Investigation

**Agent**: `explorer_m2_r2_2`  
**Role**: Pipeline Orchestrator & Partition Flow Architect  
**Milestone**: Milestone 2 Remediation (VULN-M2-01)  
**Target Project**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`  
**Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_r2_2`  
**Date**: 2026-10-25T00:15:00Z  
**Recipient**: `orchestrator_1` (Conversation ID: `0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)  

---

## Executive Summary

When `rfb-cnpj-etl build-cnpj` runs on an empty input directory or any partial dataset lacking `Socios*.zip`, the pipeline crashes fatally at Stage 8 (`Post-Load Index Creation`) with SQLite error `no such table: main.socios`. 

The root cause is an architectural lifecycle asymmetry: `create_initial_tables()` creates temporary staging table `socios_original` but omits destination table `socios` and lookup table `cnpj_base2matriz`. In `ingest_socios()`, missing partition files trigger an early return of `Ok(0)`, bypassing `transform_socios_and_expand_radicals()`. Consequently, destination table `socios` is never created, staging table `socios_original` is never dropped, and Stage 8 attempts to index a non-existent table.

This report establishes a **Defense-in-Depth Schema & Lifecycle Architecture**:
1. Guarantee all 12 canonical tables (including `socios` and `cnpj_base2matriz`) exist upfront at initialization (`create_initial_tables`).
2. Clean up staging table `socios_original` and guarantee `socios` DDL in `ingest_socios` early-exit path.
3. Verify all other 7 stages against empty/missing inputs (confirming `materialize_cnpj_base2matriz` runs cleanly with 0 rows on empty `estabelecimento`).
4. Provide precise, drop-in code snippets and automated regression tests.

---

## 1. Observation

### 1.1 Direct Reproduction of Fatal Crash (VULN-M2-01)
Execution of `rfb-cnpj-etl build-cnpj` on an empty directory:
```bash
cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
mkdir -p /tmp/test_empty_input
cargo run --bin rfb-cnpj-etl -- build-cnpj --input-dir /tmp/test_empty_input --output-db /tmp/test_empty_out.db
```
Verbatim execution trace:
```text
2026-10-25T00:11:52.403274Z  INFO Starting RFB CNPJ ingestion pipeline (input: "/tmp/test_empty_input", output: "/tmp/test_empty_out.db", threads: 4, memory limit: 1536 MB)
Ingestion Pipeline [00:00:00] [------------------------------] 0/8 stages (0%) Initializing...
Ingestion Pipeline [00:00:00] [===>--------------------------] 1/8 stages (12%) Domain Tables (Cnae, Motivo, etc.)
2026-10-25T00:11:52.405419Z  WARN No Empresas*.zip files found in input directory
Ingestion Pipeline [00:00:00] [=======>----------------------] 2/8 stages (25%) Empresas (Enterprises)
2026-10-25T00:11:52.405747Z  WARN No Estabelecimentos*.zip files found in input directory
Ingestion Pipeline [00:00:00] [===========>------------------] 3/8 stages (38%) Estabelecimentos (Establishments)
2026-10-25T00:11:52.406005Z  WARN Simples.zip not found in input directory
Ingestion Pipeline [00:00:00] [===============>--------------] 4/8 stages (50%) Simples & MEI
Ingestion Pipeline [00:00:00] [==================>-----------] 5/8 stages (62%) Materialize Base2Matriz Cache
2026-10-25T00:11:52.406755Z  WARN No Socios*.zip files found in input directory
Ingestion Pipeline [00:00:00] [======================>-------] 6/8 stages (75%) Socios (Corporate & PF Partners)
Ingestion Pipeline [00:00:00] [==========================>---] 7/8 stages (88%) Audit Metadata (_referencia)
Ingestion Pipeline [00:00:00] [==========================>---] 7/8 stages (88%) Post-Load Index Creation
Error: Failed during stage: Post-Load Index Creation

Caused by:
    0: no such table: main.socios
    1: Error code 1: SQL error or missing database
```
Exit code: `1`.

### 1.2 Inspection of `src/ingestion/schema.rs`
- In `src/ingestion/schema.rs` (lines 134–144):
  ```rust
  pub fn create_initial_tables(conn: &Connection) -> Result<()> {
      for domain in DOMAIN_TABLES {
          conn.execute_batch(&domain_table_ddl(domain))?;
      }
      conn.execute_batch(EMPRESAS_DDL)?;
      conn.execute_batch(ESTABELECIMENTO_DDL)?;
      conn.execute_batch(SIMPLES_DDL)?;
      conn.execute_batch(SOCIOS_ORIGINAL_DDL)?;
      conn.execute_batch(REFERENCIA_DDL)?;
      Ok(())
  }
  ```
  `create_initial_tables` creates 10 tables: 6 domain tables, `empresas`, `estabelecimento`, `simples`, `socios_original` (staging table), and `_referencia`. Neither `socios` (`SOCIOS_FINAL_DDL`) nor `cnpj_base2matriz` (`CNPJ_BASE2MATRIZ_DDL`) are created.
- In `src/ingestion/schema.rs` (lines 163–169):
  `POST_LOAD_INDICES_SQL` attempts to create 5 indices on `socios`:
  ```sql
  CREATE INDEX IF NOT EXISTS idx_socios_cnpj ON socios (cnpj);
  CREATE INDEX IF NOT EXISTS idx_socios_cnpj_cpf_socio ON socios (cnpj_cpf_socio);
  CREATE INDEX IF NOT EXISTS idx_socios_nome_socio ON socios (nome_socio);
  CREATE INDEX IF NOT EXISTS idx_socios_representante ON socios (representante_legal);
  CREATE INDEX IF NOT EXISTS idx_socios_representante_nome ON socios (nome_representante);
  ```

### 1.3 Inspection of `src/ingestion/mod.rs`
- In `src/ingestion/mod.rs` (lines 507–514):
  ```rust
  fn ingest_socios(&self, conn: &mut Connection, mp: &Option<Arc<MultiProgress>>) -> Result<u64> {
      let files = self.find_partition_files("Socios", 10);
      if files.is_empty() {
          warn!("No Socios*.zip files found in input directory");
          return Ok(0);
      }
  ```
  When `files.is_empty()` is true, `ingest_socios` returns `Ok(0)` immediately.
- In `src/ingestion/loader.rs` (lines 478–481, 543):
  `SOCIOS_FINAL_DDL` is executed at line 480 inside `transform_socios_and_expand_radicals(conn)`:
  ```rust
  pub fn transform_socios_and_expand_radicals(conn: &mut Connection) -> Result<u64> {
      conn.execute_batch(SOCIOS_FINAL_DDL)?;
      ...
      conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?;
      ...
  ```
  Because `ingest_socios` returns early, `transform_socios_and_expand_radicals` is never reached.

### 1.4 Inspection of `materialize_cnpj_base2matriz`
- In `src/ingestion/loader.rs` (lines 458–474):
  ```rust
  pub fn materialize_cnpj_base2matriz(conn: &mut Connection) -> Result<u64> {
      conn.execute_batch(CNPJ_BASE2MATRIZ_DDL)?;

      let inserted = conn.execute(
          "INSERT OR IGNORE INTO cnpj_base2matriz (cnpj_basico, cnpj)
           SELECT DISTINCT cnpj_basico, cnpj
           FROM estabelecimento
           WHERE matriz_filial = '1';",
          [],
      )?;

      conn.execute_batch(
          "CREATE INDEX IF NOT EXISTS idx_cnpj_base2matriz ON cnpj_base2matriz (cnpj_basico);",
      )?;

      Ok(inserted as u64)
  }
  ```
  When `Estabelecimentos*.zip` is missing, `estabelecimento` exists but is empty (0 rows). `materialize_cnpj_base2matriz` executes the DDL, queries the empty table, inserts 0 rows, creates the index, and returns `Ok(0)` without error.

### 1.5 Inspection of Existing Test Assertions
- In `tests/adversarial_stress_test.rs` (lines 405–438), test `test_partial_dataset_missing_socios_reproduces_stage8_failure` currently asserts that the pipeline fails with `no such table: main.socios`:
  ```rust
  assert!(res.is_err(), "Partial dataset missing Socios must reproduce failure");
  let err_str = format!("{:#}", res.err().unwrap());
  assert!(
      err_str.contains("no such table: main.socios"),
      "Must fail with missing socios table: {err_str}"
  );
  ```
- In `src/ingestion/mod.rs` (lines 980–987) and `tests/adversarial_transforms_and_quirks_test.rs` (lines 411–419):
  ```rust
  let original_exists: bool = conn
      .query_row(
          "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='socios_original'",
          [],
          |r| r.get(0),
      )
      .unwrap();
  assert!(!original_exists, "Staging table socios_original must be dropped");
  ```
  The pipeline contract explicitly requires that `socios_original` must NOT exist in the final database.

---

## 2. Logic Chain

1. **Schema Completeness Contract**:
   `PROJECT.md` specifies that `dados-publicos/cnpj.db` must contain 12 public tables: 6 domain tables (`cnae`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio`), `empresas`, `estabelecimento`, `simples`, `socios`, `cnpj_base2matriz`, and `_referencia`. The table `socios_original` is an internal unindexed staging table, not part of the public schema.

2. **Causal Failure Chain**:
   - `create_initial_tables()` creates `socios_original` but does NOT create `socios` or `cnpj_base2matriz` (Observation 1.2).
   - If `Socios*.zip` is missing, `ingest_socios()` early-exits at line 511 (Observation 1.3).
   - `transform_socios_and_expand_radicals()` is never called.
   - Therefore, `socios` is never created, and `socios_original` is never dropped (Observation 1.3).
   - In Stage 8, `create_post_load_indices()` executes `CREATE INDEX IF NOT EXISTS idx_socios_cnpj ON socios (cnpj);` (Observation 1.2).
   - In SQLite, creating an index on a non-existent table produces a fatal error (`Error code 1: no such table: main.socios`) regardless of `IF NOT EXISTS`.
   - The process aborts with exit code 1 (Observation 1.1).

3. **Behavior Analysis of Missing Partitions across All 8 Stages**:

| Stage | Input Partition | Present Behavior | Missing/Empty Behavior | Staging / Table Status | Post-Index Impact |
|---|---|---|---|---|---|
| **Stage 1: Domain** | 6 lookup zips | Populates `cnae`, `motivo`, etc. | Logs debug, returns `Ok(0)` | All 6 tables exist (empty) | Indexes created cleanly |
| **Stage 2: Empresas** | `Empresas*.zip` | Streams & parses BRL floats | Logs warn, returns `Ok(0)` | `empresas` exists (empty) | Indexes created cleanly |
| **Stage 3: Estabelecimentos** | `Estabelecimentos*.zip` | Synthesizes 14-char CNPJ | Logs warn, returns `Ok(0)` | `estabelecimento` exists (empty) | Indexes created cleanly |
| **Stage 4: Simples** | `Simples.zip` | Streams into `simples` | Logs warn, returns `Ok(0)` | `simples` exists (empty) | Indexes created cleanly |
| **Stage 5: MaterializeMatriz** | Internal from `estabelecimento` | Queries `matriz_filial = '1'` | Queries empty table, inserts 0 rows, returns `Ok(0)` | `cnpj_base2matriz` exists (empty) | Index created cleanly |
| **Stage 6: Socios** | `Socios*.zip` | Loads staging, expands radicals, drops staging | **BUG**: skips DDL, skips drop, returns `Ok(0)` | `socios` MISSING, `socios_original` ORPHANED | **FATAL CRASH**: `no such table: main.socios` |
| **Stage 7: Metadata** | Zip timestamp + count | Extracts date & count | Fallback to current date + count=0, returns `Ok(1)` | `_referencia` populated | No index needed |
| **Stage 8: PostIndexing** | Internal SQLite engine | Creates all 14 indices | Fails on `socios` | Crashes process | None |

4. **Defense-in-Depth Solution**:
   - **Defense Layer 1 (Initialization Guarantee)**: In `src/ingestion/schema.rs`, `create_initial_tables(&conn)` must execute `SOCIOS_FINAL_DDL` and `CNPJ_BASE2MATRIZ_DDL`. This guarantees all 12 tables exist from the very moment the database is opened.
   - **Defense Layer 2 (Stage-Level Hygiene)**: In `src/ingestion/mod.rs`, `ingest_socios()` must execute `conn.execute_batch(SOCIOS_FINAL_DDL)?;` and `conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?;` before returning `Ok(0)`. This guarantees that if `Socios*.zip` is missing, the staging table is cleanly dropped and does not pollute `cnpj.db`.
   - **Defense Layer 3 (Idempotency)**: In `transform_socios_and_expand_radicals()`, `SOCIOS_FINAL_DDL` uses `CREATE TABLE IF NOT EXISTS`, ensuring that if `socios` was already created in Layer 1 or 2, execution proceeds without error.

---

## 3. Caveats

1. **Downstream Graph Dependency on Missing Tables**:
   If `build-cnpj` completes on an empty directory, `cnpj.db` will contain all 12 tables and indices, but 0 data rows. Downstream commands like `rfb-cnpj-etl build-rede` (Milestone 3) will execute attached-database joins on empty tables; `build-rede` must also handle empty tables gracefully without failing.
2. **Date Token Edge Case (Reviewer Finding 2)**:
   In `src/ingestion/transforms.rs:184`, `parse_rfb_reference_code` checks `part.len() == 6 && part.starts_with('D')`. If a zip entry has a filename token like `DELETE` or `DEBUG0`, it could be erroneously parsed as a date. Adding `part[1..].chars().all(|c| c.is_ascii_digit())` hardens this parser.

---

## 4. Conclusion & Recommended Code Changes

The stage execution flow in `src/ingestion/mod.rs` and table lifecycle in `src/ingestion/schema.rs` can be made 100% resilient to empty or partial datasets with surgical, targeted changes.

### Proposed Code Changes for Implementer (`worker_m2`):

#### 1. `src/ingestion/schema.rs` (Lines 134–144)
Add `SOCIOS_FINAL_DDL` and `CNPJ_BASE2MATRIZ_DDL` to `create_initial_tables()`:

```rust
// BEFORE:
pub fn create_initial_tables(conn: &Connection) -> Result<()> {
    for domain in DOMAIN_TABLES {
        conn.execute_batch(&domain_table_ddl(domain))?;
    }
    conn.execute_batch(EMPRESAS_DDL)?;
    conn.execute_batch(ESTABELECIMENTO_DDL)?;
    conn.execute_batch(SIMPLES_DDL)?;
    conn.execute_batch(SOCIOS_ORIGINAL_DDL)?;
    conn.execute_batch(REFERENCIA_DDL)?;
    Ok(())
}

// AFTER:
pub fn create_initial_tables(conn: &Connection) -> Result<()> {
    for domain in DOMAIN_TABLES {
        conn.execute_batch(&domain_table_ddl(domain))?;
    }
    conn.execute_batch(EMPRESAS_DDL)?;
    conn.execute_batch(ESTABELECIMENTO_DDL)?;
    conn.execute_batch(SIMPLES_DDL)?;
    conn.execute_batch(SOCIOS_ORIGINAL_DDL)?;
    conn.execute_batch(SOCIOS_FINAL_DDL)?;
    conn.execute_batch(CNPJ_BASE2MATRIZ_DDL)?;
    conn.execute_batch(REFERENCIA_DDL)?;
    Ok(())
}
```

#### 2. `src/ingestion/mod.rs` (Lines 508–513)
Clean up staging table `socios_original` and guarantee `SOCIOS_FINAL_DDL` on missing partition files:

```rust
// BEFORE:
        let files = self.find_partition_files("Socios", 10);
        if files.is_empty() {
            warn!("No Socios*.zip files found in input directory");
            return Ok(0);
        }

// AFTER:
        let files = self.find_partition_files("Socios", 10);
        if files.is_empty() {
            warn!("No Socios*.zip files found in input directory");
            conn.execute_batch(SOCIOS_FINAL_DDL)?;
            conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?;
            return Ok(0);
        }
```

#### 3. `src/ingestion/transforms.rs` (Lines 184, 192)
Harden numeric date token validation against non-numeric tokens (Minor Finding 2):

```rust
// BEFORE:
        if part.len() == 6 && part.starts_with('D') {
...
        } else if part.len() == 7 && part.starts_with('D') {

// AFTER:
        if part.len() == 6 && part.starts_with('D') && part[1..].chars().all(|c| c.is_ascii_digit()) {
...
        } else if part.len() == 7 && part.starts_with('D') && part[1..].chars().all(|c| c.is_ascii_digit()) {
```

#### 4. `tests/adversarial_stress_test.rs` (Lines 405–438)
Convert the reproducing test into an automated regression assertion, and add an empty directory test:

```rust
#[test]
fn test_partial_dataset_missing_socios_succeeds_cleanly() {
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
    let stats = orchestrator.execute().expect("execution must succeed on partial dataset");
    assert_eq!(stats.total_rows, 1);

    let conn = Connection::open(&output_db).expect("open db");
    // Verify socios table exists and socios_original does not
    let socios_exists: bool = conn
        .query_row("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='socios'", [], |r| r.get(0))
        .unwrap();
    assert!(socios_exists, "Table 'socios' must exist");

    let original_exists: bool = conn
        .query_row("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='socios_original'", [], |r| r.get(0))
        .unwrap();
    assert!(!original_exists, "Staging table 'socios_original' must be dropped");
}

#[test]
fn test_empty_input_directory_succeeds_end_to_end() {
    let temp = TempDir::new().expect("tempdir");
    let input_dir = temp.path().join("empty_zip");
    fs::create_dir_all(&input_dir).expect("mkdir");
    let output_db = temp.path().join("cnpj_empty.db");

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
    let stats = orchestrator.execute().expect("execution must succeed on empty directory");
    assert_eq!(stats.total_rows, 1); // 1 from _referencia stage

    let conn = Connection::open(&output_db).expect("open db");
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    for canonical in [
        "_referencia", "cnae", "cnpj_base2matriz", "empresas", "estabelecimento",
        "motivo", "municipio", "natureza_juridica", "pais", "qualificacao_socio",
        "simples", "socios"
    ] {
        assert!(tables.contains(&canonical.to_string()), "Canonical table '{canonical}' must exist");
    }
    assert!(!tables.contains(&"socios_original".to_string()), "Staging table 'socios_original' must not exist");
}
```

---

## 5. Verification Method

To verify this report and validate the proposed changes when implemented by `worker_m2`:

1. **CLI Execution on Empty Input Directory**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   mkdir -p /tmp/test_empty_input
   cargo run --bin rfb-cnpj-etl -- build-cnpj --input-dir /tmp/test_empty_input --output-db /tmp/test_empty.db
   ```
   *Expected outcome*: Exit code `0`. Log warning for missing partition files, completed 8/8 stages.
   *Verification query*:
   ```bash
   sqlite3 /tmp/test_empty.db ".tables"
   # Must list exactly: _referencia cnae cnpj_base2matriz empresas estabelecimento motivo municipio natureza_juridica pais qualificacao_socio simples socios
   # Must NOT list: socios_original

   sqlite3 /tmp/test_empty.db ".indices"
   # Must list all 14 post-load indices (including idx_socios_*)
   ```

2. **Automated Test Suite**:
   ```bash
   cargo check
   cargo clippy --all-targets -- -D warnings
   cargo test --all-targets
   ```
   *Expected outcome*: All unit, adversarial, transforms, and E2E tests pass with 0 failures.

3. **Clean Up**:
   ```bash
   rm -rf /tmp/test_empty_input /tmp/test_empty.db*
   ```
