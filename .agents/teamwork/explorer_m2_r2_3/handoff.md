# Handoff Report — Test Suite & Date Parsing Architecture: Milestone 2 Round 2

**Agent**: `explorer_m2_r2_3`  
**Role**: Test Suite & Date Parsing Architect  
**Milestone**: Milestone 2 Round 2  
**Target Repository**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`  
**Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_r2_3`  
**Recipient**: `orchestrator_1` (Conversation ID: `0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)  
**Date**: 2026-10-25T00:15:00Z  

---

## Executive Summary

1. **RFB Date Token Validation (`parse_rfb_reference_code`)**:
   - In `src/ingestion/transforms.rs` (lines 182–203), `parse_rfb_reference_code` matches any 6-character part starting with `'D'` without verifying whether trailing characters are numeric digits, nor does it check the 7-character branch. Consequently, alphabetic tokens like `DELETE`, `DEBUG0`, `DEFAULT`, or `DATASET` match as false dates, producing corrupted date strings (e.g. `TE/LE/202E`) and masking legitimate date tokens in multi-token dot-separated filenames (e.g. `K3241.DELETE.D61024.EMPRECSV`).
   - Slicing strings with non-ASCII multibyte UTF-8 characters at fixed byte offsets can cause panics if not preceded by ASCII byte validation.
   - We formulate a panic-safe, calendar-bounded implementation (validating ASCII digits, month $\in [1, 12]$, day $\in [1, 31]$) for both 6- and 7-character formats, with complete adversarial unit tests.

2. **Empty / Partial Dataset Execution & Pipeline Crash (Finding 1 / VULN-M2-01)**:
   - When executing `rfb-cnpj-etl build-cnpj` on an empty directory or any dataset missing `Socios*.zip`, Stage 6 (`ingest_socios`) returns early without executing `SOCIOS_FINAL_DDL`, leaving the staging table `socios_original` orphaned and destination table `socios` non-existent.
   - Stage 8 (`create_post_load_indices`) then fails fatally with SQLite error code 1 (`no such table: main.socios`).
   - We design an automated regression test in `tests/e2e_pipeline_test.rs` executing `rfb-cnpj-etl build-cnpj` on an empty temporary directory and strictly asserting exit code 0, database existence, presence of all 12 contractual tables, presence of all 18 secondary indices, absence of `socios_original`, and `PRAGMA journal_mode = wal`.
   - We also note that `test_partial_dataset_missing_socios_reproduces_stage8_failure` in `tests/adversarial_stress_test.rs` must be converted from asserting error to asserting success once the fix is applied.

---

## 1. Observation

### 1.1 Verbatim Code Inspection: Date Parsing in `src/ingestion/transforms.rs`

Lines 182–203 of `src/ingestion/transforms.rs`:
```rust
pub fn parse_rfb_reference_code(entry_name: &str) -> Option<(String, String)> {
    for part in entry_name.split('.') {
        if part.len() == 6 && part.starts_with('D') {
            let year_digit = part.chars().nth(1)?;
            let month = &part[2..4];
            let day = &part[4..6];
            let full_year = format!("202{}", year_digit);
            let data_referencia = format!("{}/{}/{}", day, month, full_year);
            let ano_mes = format!("{}{}", full_year, month);
            return Some((data_referencia, ano_mes));
        } else if part.len() == 7 && part.starts_with('D') {
            let year_digits = &part[1..3];
            let month = &part[3..5];
            let day = &part[5..7];
            let full_year = format!("20{}", year_digits);
            let data_referencia = format!("{}/{}/{}", day, month, full_year);
            let ano_mes = format!("{}{}", full_year, month);
            return Some((data_referencia, ano_mes));
        }
    }
    None
}
```

Direct observations:
- `part.len()` returns byte length, not character count.
- Line 184: If `part` is `"DELETE"`, `part.len() == 6 && part.starts_with('D')` evaluates to `true`.
  - `year_digit` is `'E'`.
  - `month` is `"LE"`.
  - `day` is `"TE"`.
  - Result returned: `Some(("TE/LE/202E".to_string(), "202ELE".to_string()))`.
- Line 192: If `part` is `"DEFAULT"` or `"DATASET"`, `part.len() == 7 && part.starts_with('D')` evaluates to `true`.
  - `year_digits` is `"EF"`.
  - `month` is `"AU"`.
  - `day` is `"LT"`.
  - Result returned: `Some(("LT/AU/20EF".to_string(), "20EFAU".to_string()))`.
- In a filename like `K3241.DATASET.D61024.EMPRECSV`, the loop stops at `DATASET` and returns `("LT/AU/20EF", "20EFAU")`, completely masking the valid date token `D61024`.
- There is no calendar validation: a token like `D69999` or `D60000` evaluates to valid format, creating non-existent dates.

### 1.2 Verbatim Reproduction of Pipeline Crash on Empty Directory

Command executed:
```bash
mkdir -p /tmp/test_empty_input
cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
cargo run --bin rfb-cnpj-etl -- build-cnpj --input-dir /tmp/test_empty_input --output-db /tmp/test_empty_out.db
```

Verbatim terminal output observed:
```text
2026-10-25T00:13:22.647815Z  INFO Starting RFB CNPJ ingestion pipeline (input: "/tmp/test_empty_input", output: "/tmp/test_empty_out.db", threads: 4, memory limit: 1536 MB)
Ingestion Pipeline [00:00:00] [------------------------------] 0/8 stages (0%) Initializing...
Ingestion Pipeline [00:00:00] [------------------------------] 0/8 stages (0%) Domain Tables (Cnae, Motivo, etc.)
Ingestion Pipeline [00:00:00] [===>--------------------------] 1/8 stages (12%) Domain Tables (Cnae, Motivo, etc.)
Ingestion Pipeline [00:00:00] [===>--------------------------] 1/8 stages (12%) Empresas (Enterprises)
2026-10-25T00:13:22.651280Z  WARN No Empresas*.zip files found in input directory
Ingestion Pipeline [00:00:00] [=======>----------------------] 2/8 stages (25%) Empresas (Enterprises)
Ingestion Pipeline [00:00:00] [=======>----------------------] 2/8 stages (25%) Estabelecimentos (Establishments)
2026-10-25T00:13:22.651587Z  WARN No Estabelecimentos*.zip files found in input directory
Ingestion Pipeline [00:00:00] [===========>------------------] 3/8 stages (38%) Estabelecimentos (Establishments)
Ingestion Pipeline [00:00:00] [===========>------------------] 3/8 stages (38%) Simples & MEI
2026-10-25T00:13:22.651809Z  WARN Simples.zip not found in input directory
Ingestion Pipeline [00:00:00] [===============>--------------] 4/8 stages (50%) Simples & MEI
Ingestion Pipeline [00:00:00] [===============>--------------] 4/8 stages (50%) Materialize Base2Matriz Cache
Ingestion Pipeline [00:00:00] [==================>-----------] 5/8 stages (62%) Materialize Base2Matriz Cache
Ingestion Pipeline [00:00:00] [==================>-----------] 5/8 stages (62%) Socios (Corporate & PF Partners)
2026-10-25T00:13:22.652700Z  WARN No Socios*.zip files found in input directory
Ingestion Pipeline [00:00:00] [======================>-------] 6/8 stages (75%) Socios (Corporate & PF Partners)
Ingestion Pipeline [00:00:00] [======================>-------] 6/8 stages (75%) Audit Metadata (_referencia)
Ingestion Pipeline [00:00:00] [==========================>---] 7/8 stages (88%) Audit Metadata (_referencia)
Ingestion Pipeline [00:00:00] [==========================>---] 7/8 stages (88%) Post-Load Index Creation
Error: Failed during stage: Post-Load Index Creation

Caused by:
    0: no such table: main.socios
    1: Error code 1: SQL error or missing database
```
Exit code: 1.

### 1.3 Code Inspection of the Schema Lifecycle Defect

1. In `src/ingestion/schema.rs` (lines 134–144):
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
   `create_initial_tables` creates 6 domain tables, `empresas`, `estabelecimento`, `simples`, `socios_original` (staging table), and `_referencia`. Table `socios` is **omitted**.

2. In `src/ingestion/mod.rs` (lines 508–512):
   ```rust
   let files = self.find_partition_files("Socios", 10);
   if files.is_empty() {
       warn!("No Socios*.zip files found in input directory");
       return Ok(0);
   }
   ```
   When `Socios*.zip` is missing, `ingest_socios` returns early.

3. In `src/ingestion/loader.rs` (lines 478–481):
   `SOCIOS_FINAL_DDL` and `DROP TABLE IF EXISTS socios_original;` are only executed inside `transform_socios_and_expand_radicals(&mut conn)`. Because `ingest_socios` returned early, `transform_socios_and_expand_radicals` is never invoked.

4. In `src/ingestion/schema.rs` (lines 163–169):
   `create_post_load_indices` runs `CREATE INDEX IF NOT EXISTS idx_socios_cnpj ON socios (cnpj);`. In SQLite, creating an index on a non-existent table always errors fatally with `no such table: main.socios`.

5. In `tests/adversarial_stress_test.rs` (lines 406–438):
   A temporary adversarial test named `test_partial_dataset_missing_socios_reproduces_stage8_failure` currently expects `res.is_err()` and asserts `err_str.contains("no such table: main.socios")`. Once the bug is fixed, this test will fail unless updated to assert graceful success.

---

## 2. Logic Chain

1. **Date Validation Logic**:
   - RFB internal entry naming conventions follow either `D` + 1-digit year (e.g. `D6` for 2026) + 2-digit month + 2-digit day (6 chars total) or `D` + 2-digit year (e.g. `D26`) + 2-digit month + 2-digit day (7 chars total) (Observation 1.1).
   - In both cases, character 0 must be `'D'` and all remaining characters must be numeric ASCII digits (`0`–`9`).
   - If characters $1..$ are validated as ASCII digits, slicing at byte offsets $[2..4]$, $[4..6]$, $[3..5]$, $[5..7]$ is guaranteed to be panic-free because every byte is an independent 1-byte ASCII character.
   - Enforcing month $\in [1, 12]$ and day $\in [1, 31]$ prevents invalid calendar values (`D60000`, `D69999`) and guarantees that false metadata tokens starting with `'D'` do not mask subsequent valid tokens.

2. **Schema Lifecycle & Crash Logic**:
   - `PROJECT.md` specifies that `cnpj.db` must contain 12 tables (`cnae`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio`, `empresas`, `estabelecimento`, `simples`, `socios`, `cnpj_base2matriz`, `_referencia`) plus 18 post-load indices (Observation 1.3).
   - `socios_original` is an internal unindexed staging table used exclusively to ingest raw `Socios*.zip` rows before August 2026 radical expansion. It must never persist in the completed database.
   - Initializing table `socios` in `create_initial_tables()` (`conn.execute_batch(SOCIOS_FINAL_DDL)?;`) ensures that the table exists regardless of file presence.
   - In `ingest_socios()`, if `files.is_empty()`, explicitly dropping `socios_original` (`DROP TABLE IF EXISTS socios_original;`) guarantees that the staging table is not left orphaned.
   - With `socios` present in the catalog, `create_post_load_indices()` creates `idx_socios_cnpj`, `idx_socios_cnpj_cpf_socio`, `idx_socios_nome_socio`, `idx_socios_representante`, and `idx_socios_representante_nome` successfully (with 0 index entries), allowing the pipeline to complete with exit code 0.

3. **Regression Test Suite Architecture**:
   - An end-to-end regression test invoking the binary via `assert_cmd` (`TestEnvironment::run_cli`) directly tests the external interface contract.
   - Asserting exit code 0, file existence, all 12 tables, all 18 secondary indices, absence of `socios_original`, and `PRAGMA journal_mode = wal` covers all acceptance criteria from `PROJECT.md` and `DISPATCH.md`.

---

## 3. Caveats

1. **Read-Only Scope**: In strict accordance with the explorer role constraints, no production source code in `src/**` or existing test files in `tests/**` has been modified.
2. **Adversarial Test Suite Synchronization**: When `worker_m2` implements the fix, `test_partial_dataset_missing_socios_reproduces_stage8_failure` in `tests/adversarial_stress_test.rs` will fail if not inverted to assert success, because it was authored to reproduce the crash.

---

## 4. Conclusion & Concrete Recommendations

### 4.1 Recommendation 1: Hardened Date Token Validation (`src/ingestion/transforms.rs`)

Replace lines 182–203 in `src/ingestion/transforms.rs` with the following implementation:

```rust
/// Extracts reference date and year-month from an RFB inner zip entry filename.
///
/// Typical RFB entry filenames:
/// - `K3241.K03200Y0.D30610.EMPRECSV` -> `("10/06/2023", "202306")`
/// - `F.K03200$Z.D60810.ESTABELE` -> `("10/08/2026", "202608")`
/// - `F.K03200$Z.D61024.ESTABELE` -> `("24/10/2026", "202610")`
///
/// Format: `D` + `Y` (last digit of year 202Y) + `MM` (month) + `DD` (day)
///     or: `D` + `YY` (two-digit year 20YY) + `MM` (month) + `DD` (day).
pub fn parse_rfb_reference_code(entry_name: &str) -> Option<(String, String)> {
    for part in entry_name.split('.') {
        if !part.starts_with('D') {
            continue;
        }

        let bytes = part.as_bytes();

        // 6-character format: D + Y + MM + DD (e.g. D61024 -> 24/10/2026, 202610)
        if bytes.len() == 6 && bytes[1..].iter().all(|b| b.is_ascii_digit()) {
            let year_digit = bytes[1] as char;
            let month = &part[2..4];
            let day = &part[4..6];

            if let (Ok(m), Ok(d)) = (month.parse::<u32>(), day.parse::<u32>()) {
                if (1..=12).contains(&m) && (1..=31).contains(&d) {
                    let full_year = format!("202{}", year_digit);
                    let data_referencia = format!("{}/{}/{}", day, month, full_year);
                    let ano_mes = format!("{}{}", full_year, month);
                    return Some((data_referencia, ano_mes));
                }
            }
        // 7-character format: D + YY + MM + DD (e.g. D261024 -> 24/10/2026, 202610)
        } else if bytes.len() == 7 && bytes[1..].iter().all(|b| b.is_ascii_digit()) {
            let year_digits = &part[1..3];
            let month = &part[3..5];
            let day = &part[5..7];

            if let (Ok(m), Ok(d)) = (month.parse::<u32>(), day.parse::<u32>()) {
                if (1..=12).contains(&m) && (1..=31).contains(&d) {
                    let full_year = format!("20{}", year_digits);
                    let data_referencia = format!("{}/{}/{}", day, month, full_year);
                    let ano_mes = format!("{}{}", full_year, month);
                    return Some((data_referencia, ano_mes));
                }
            }
        }
    }
    None
}
```

Add the following unit test to `src/ingestion/transforms.rs` (under `mod tests`) and `tests/adversarial_transforms_and_quirks_test.rs`:

```rust
#[test]
fn test_parse_rfb_reference_code_adversarial_validation() {
    // Valid 6-character formats
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

    // Valid 7-character format
    assert_eq!(
        parse_rfb_reference_code("F.K03200$Z.D261024.SOCIOCSV"),
        Some(("24/10/2026".to_string(), "202610".to_string()))
    );

    // Alphabetic non-date tokens starting with 'D' must NOT match
    assert_eq!(parse_rfb_reference_code("K3241.DELETE.EMPRECSV"), None);
    assert_eq!(parse_rfb_reference_code("K3241.DEFAULT.EMPRECSV"), None);
    assert_eq!(parse_rfb_reference_code("K3241.DEBUG0.EMPRECSV"), None);
    assert_eq!(parse_rfb_reference_code("K3241.DOMAIN.EMPRECSV"), None);

    // Out-of-bounds calendar values must NOT match
    assert_eq!(parse_rfb_reference_code("K3241.D60000.EMPRECSV"), None);
    assert_eq!(parse_rfb_reference_code("K3241.D61301.EMPRECSV"), None);
    assert_eq!(parse_rfb_reference_code("K3241.D60532.EMPRECSV"), None);

    // Masking resilience: dotted words starting with D preceding a valid date
    assert_eq!(
        parse_rfb_reference_code("K3241.DELETE.D61024.EMPRECSV"),
        Some(("24/10/2026".to_string(), "202610".to_string()))
    );
    assert_eq!(
        parse_rfb_reference_code("K3241.DEFAULT.D261024.EMPRECSV"),
        Some(("24/10/2026".to_string(), "202610".to_string()))
    );
}
```

---

### 4.2 Recommendation 2: Schema Lifecycle Fix (`schema.rs` and `mod.rs`)

1. In `src/ingestion/schema.rs` (`create_initial_tables`):
   ```rust
   pub fn create_initial_tables(conn: &Connection) -> Result<()> {
       for domain in DOMAIN_TABLES {
           conn.execute_batch(&domain_table_ddl(domain))?;
       }
       conn.execute_batch(EMPRESAS_DDL)?;
       conn.execute_batch(ESTABELECIMENTO_DDL)?;
       conn.execute_batch(SIMPLES_DDL)?;
       conn.execute_batch(SOCIOS_ORIGINAL_DDL)?;
       conn.execute_batch(SOCIOS_FINAL_DDL)?;           // <-- GUARANTEES socios TABLE ALWAYS EXISTS
       conn.execute_batch(CNPJ_BASE2MATRIZ_DDL)?;       // <-- GUARANTEES cnpj_base2matriz ALWAYS EXISTS
       conn.execute_batch(REFERENCIA_DDL)?;
       Ok(())
   }
   ```

2. In `src/ingestion/mod.rs` (`ingest_socios`):
   ```rust
       fn ingest_socios(&self, conn: &mut Connection, mp: &Option<Arc<MultiProgress>>) -> Result<u64> {
           let files = self.find_partition_files("Socios", 10);
           if files.is_empty() {
               warn!("No Socios*.zip files found in input directory");
               conn.execute_batch(SOCIOS_FINAL_DDL)?;
               conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?; // <-- CLEANS UP STAGING
               return Ok(0);
           }
   ```

---

### 4.3 Recommendation 3: Automated Regression Test for `tests/e2e_pipeline_test.rs`

Add the following complete test function to `tests/e2e_pipeline_test.rs`:

```rust
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
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND name NOT LIKE 'sqlite_%';")
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
    assert!(ref_count >= 1, "_referencia table must contain audit metadata entries");
}
```

### 4.4 Recommendation 4: Update Adversarial Test in `tests/adversarial_stress_test.rs`

In `tests/adversarial_stress_test.rs`, convert lines 406–438 from asserting failure to asserting graceful success:

```rust
#[test]
fn test_partial_dataset_missing_socios_succeeds_gracefully() {
    let temp = TempDir::new().expect("tempdir");
    let input_dir = temp.path().join("zip");
    fs::create_dir_all(&input_dir).expect("mkdir");
    let output_db = temp.path().join("cnpj_partial.db");

    // Only provide Empresas0.zip (missing Socios*.zip)
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
    assert!(table_exists(&conn, "socios"), "socios table must exist");
    assert!(table_exists(&conn, "cnpj_base2matriz"), "cnpj_base2matriz table must exist");
    assert!(!table_exists(&conn, "socios_original"), "socios_original must be dropped");
}
```

---

## 5. Verification Method

To independently verify the bug, the proposed fix, and the regression tests:

```bash
cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

# 1. Reproduce Finding 1 (CLI fails with exit code 1 and missing socios table)
mkdir -p /tmp/test_empty_input
cargo run --bin rfb-cnpj-etl -- build-cnpj --input-dir /tmp/test_empty_input --output-db /tmp/test_empty_out.db
# Observe error: "no such table: main.socios" and exit code 1
rm -rf /tmp/test_empty_input /tmp/test_empty_out.db*

# 2. Run the test suite prior to fix (all currently pass, with adversarial test asserting failure)
cargo test --all-targets

# 3. After worker_m2 applies Recommendations 1, 2, 3, and 4:
cargo check
cargo clippy --all-targets -- -D warnings
cargo test --test e2e_pipeline_test -- test_tier2_boundary_empty_input_directory_build_cnpj_regression
cargo test --all-targets
```

### Invalidation Conditions
- If running `build-cnpj` against an empty directory exits with non-zero code or fails during post-load indexing.
- If `parse_rfb_reference_code("K3241.DELETE.EMPRECSV")` returns `Some(...)` instead of `None`.
- If the output SQLite database lacks any of the 12 contractual tables or 18 secondary indices.
- If `socios_original` remains in the database after execution.
