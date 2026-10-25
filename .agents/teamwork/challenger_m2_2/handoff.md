# Empirical Challenge Report — Milestone 2: Concurrency, Memory Bound & Pipeline Stress

**Agent**: `challenger_m2_2`  
**Role**: Concurrency, Memory Bound & Pipeline Stress Challenger  
**Milestone**: Milestone 2 (High-Performance CNPJ Ingestion Engine)  
**Target Project**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`  
**Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m2_2`  
**Recipient**: `orchestrator_1` (Conversation ID: `0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)  
**Date**: 2026-10-25T00:08:00Z  
**Verdict**: **REQUEST_CHANGES** (Blocking on VULN-M2-01: fatal crash on empty input directory or missing `Socios*.zip`)

---

## 1. Observation

### 1.1 Fatal Crash on Empty Input Directory and Partial Datasets (VULN-M2-01)
When `rfb-cnpj-etl build-cnpj` is executed on an empty input directory or any dataset missing `Socios*.zip`, the pipeline crashes at Stage 8 with exit code 1:

- **Command executed**:
  ```bash
  mkdir -p /tmp/test_empty_input
  ./target/debug/rfb-cnpj-etl build-cnpj --input-dir /tmp/test_empty_input --output-db /tmp/test_empty_out.db
  ```
- **Verbatim Output**:
  ```
  2026-10-25T00:03:34.488733Z  INFO Starting RFB CNPJ ingestion pipeline (input: "/tmp/test_empty_input", output: "/tmp/test_empty_out.db", threads: 4, memory limit: 1536 MB)
  Ingestion Pipeline [00:00:00] [===>--------------------------] 1/8 stages (12%) Domain Tables (Cnae, Motivo, etc.)
  2026-10-25T00:03:34.492816Z  WARN No Empresas*.zip files found in input directory
  Ingestion Pipeline [00:00:00] [=======>----------------------] 2/8 stages (25%) Estabelecimentos (Establishments)
  2026-10-25T00:03:34.493171Z  WARN No Estabelecimentos*.zip files found in input directory
  Ingestion Pipeline [00:00:00] [===========>------------------] 3/8 stages (38%) Simples & MEI
  2026-10-25T00:03:34.493480Z  WARN Simples.zip not found in input directory
  Ingestion Pipeline [00:00:00] [===============>--------------] 4/8 stages (50%) Materialize Base2Matriz Cache
  Ingestion Pipeline [00:00:00] [==================>-----------] 5/8 stages (62%) Socios (Corporate & PF Partners)
  2026-10-25T00:03:34.494544Z  WARN No Socios*.zip files found in input directory
  Ingestion Pipeline [00:00:00] [======================>-------] 6/8 stages (75%) Audit Metadata (_referencia)
  Ingestion Pipeline [00:00:00] [==========================>---] 7/8 stages (88%) Post-Load Index Creation
  Error: Failed during stage: Post-Load Index Creation

  Caused by:
      0: no such table: main.socios
      1: Error code 1: SQL error or missing database
  ```
- **Code Inspection**:
  - `src/ingestion/schema.rs` (lines 134–144): `create_initial_tables(&conn)` creates domain tables, `empresas`, `estabelecimento`, `simples`, `socios_original` (staging table), and `_referencia`. Table `socios` is **not** created.
  - `src/ingestion/mod.rs` (lines 507–513): `ingest_socios` checks:
    ```rust
    let files = self.find_partition_files("Socios", 10);
    if files.is_empty() {
        warn!("No Socios*.zip files found in input directory");
        return Ok(0);
    }
    ```
    If no `Socios*.zip` files exist, `ingest_socios` returns `Ok(0)` immediately, skipping `transform_socios_and_expand_radicals`.
  - `src/ingestion/loader.rs` (lines 478–480): Table `socios` DDL (`SOCIOS_FINAL_DDL`) is only executed inside `transform_socios_and_expand_radicals`. Because this function is skipped, table `socios` is **never created**.
  - `src/ingestion/schema.rs` (lines 163–169): `POST_LOAD_INDICES_SQL` attempts to create 5 indices on `socios`:
    ```sql
    CREATE INDEX IF NOT EXISTS idx_socios_cnpj ON socios (cnpj);
    CREATE INDEX IF NOT EXISTS idx_socios_cnpj_cpf_socio ON socios (cnpj_cpf_socio);
    CREATE INDEX IF NOT EXISTS idx_socios_nome_socio ON socios (nome_socio);
    CREATE INDEX IF NOT EXISTS idx_socios_representante ON socios (representante_legal);
    CREATE INDEX IF NOT EXISTS idx_socios_representante_nome ON socios (nome_representante);
    ```
    In SQLite, `CREATE INDEX IF NOT EXISTS` fails with `no such table: main.<table_name>` if the target table does not exist.
  - Furthermore, `socios_original` (the staging table) remains orphaned in the database, while the public contract table `socios` is missing.

### 1.2 Channel Backpressure & Producer Blocking
Empirical verification using `test_channel_backpressure_slow_consumer_blocks_producer` in `tests/adversarial_stress_test.rs`:
- Test configuration: 25,000 rows (50 batches of 500 rows), bounded channel capacity = 4.
- Producer reading from zip stream into channel; consumer thread artificially slowed by 5ms delay per batch.
- **Results**:
  - `assert_eq!(rx.len(), capacity)`: Confirmed channel immediately saturated at exactly 4 batches.
  - `assert!(!producer_finished.load(Ordering::SeqCst))`: Confirmed producer thread blocked on `sender.send()` and did not finish ahead of the consumer.
  - In-flight buffer size never exceeded 4 chunks ($4 \times 500 = 2,000$ rows). Total rows processed: 25,000. Total rows received: 25,000.
  - Pass: channel backpressure strictly bounds in-flight rows.

### 1.3 Memory Bounding & Peak RSS Telemetry (< 1.8 GB Limit)
Empirical verification using `test_memory_bounding_under_high_row_count` in `tests/adversarial_stress_test.rs`:
- Ingestion workload: 100,000 synthetic rows across `Empresas0.zip`, `Estabelecimentos0.zip`, and `Socios0.zip`.
- Memory telemetry from `/proc/self/statm` (`get_resident_memory_bytes()`):
  - **Initial RSS**: 24.77 MB
  - **Peak RSS during execution**: 54.71 MB
  - **Memory Budget**: 1,800.00 MB
  - Peak RSS consumed **3.04%** of the 1.8 GB budget. Process memory cannot run away due to bounded MPSC channels and 40% SQLite cache ceiling.

### 1.4 Corrupted and Truncated Archives
Empirical verification across archive corruption modes:
- **Truncated Zip Header**: `inspect_zip` and `process_zip_byte_records` returned `Err(ZipStreamError::Zip("...": invalid Zip archive: Could not find central directory end))` without panic (`test_corrupt_zip_truncated_file_returns_error`).
- **Garbage / Non-Zip Content**: Returned `Err(ZipStreamError::Zip(...))` without panic (`test_corrupt_zip_garbage_content_returns_error`).
- **0-Byte File**: Returned `Err(ZipStreamError::Zip(...))` without panic (`test_zero_byte_zip_returns_error`).
- **Parallel Worker Rayon Failure**: In `test_corrupt_file_in_parallel_worker_stops_cleanly_without_hang`, 1 of 2 partition zip files was corrupted. `abort_flag` set to `true`, `tx_clone.send(Err(...))` notified consumer, and Rayon threads exited cleanly without channel deadlock or infinite blocking.
- **Directory-Only Archive**: `test_zip_entry_not_found_when_only_directories_present` correctly returned `Err(ZipStreamError::EntryNotFound)`.

### 1.5 Index Idempotency and `--skip-indices`
- **Idempotency**: `test_post_load_indices_idempotency` executed `create_post_load_indices(&conn)` twice consecutively. Succeeded with 0 errors due to `CREATE INDEX IF NOT EXISTS`.
- **Flag `--skip-indices`**: `test_skip_indices_flag_omits_stage_8` verified that passing `--skip-indices` executes exactly 7 stages, skipping stage 8 completely and leaving tables unindexed as requested.

### 1.6 Compiler and Test Verification
- `cargo check`: Exit code 0.
- `cargo clippy --all-targets -- -D warnings`: Exit code 0 (0 warnings).
- `cargo test --all-targets`:
  - 36 library unit tests passed.
  - 13 adversarial stress tests (`tests/adversarial_stress_test.rs`) passed.
  - 13 transforms and quirks tests (`tests/adversarial_transforms_and_quirks_test.rs`) passed.
  - 31 E2E tests (`tests/e2e_pipeline_test.rs`) passed.
  - Total: **93 tests passed, 0 failed, 0 ignored**.

---

## 2. Logic Chain

1. **Refutation of Worker Claim & Violation of Contract**:
   - In `worker_m2/handoff.md` Section 3 (Caveats), `worker_m2` claimed:
     *"If a table has no matching files in the directory, the orchestrator logs a warning and proceeds without error."*
   - DISPATCH.md assigned explicit verification:
     *"Empty input directory (should gracefully warn or handle without panic)."*
     *"Missing tables / partial dataset handling."*
   - Direct execution of `./target/debug/rfb-cnpj-etl build-cnpj --input-dir /tmp/test_empty_input --output-db /tmp/test_empty_out.db` produces exit code 1 with `Error: Failed during stage: Post-Load Index Creation / Caused by: no such table: main.socios` (Observation 1.1).
   - This empirically refutes worker_m2's claim and violates both DISPATCH.md requirements and the PROJECT.md schema contract (Observation 1.1).

2. **Root Cause Identification**:
   - In `schema.rs`, `create_initial_tables(&conn)` creates `socios_original` (staging table), but does NOT create `socios` (final table).
   - In `mod.rs`, `ingest_socios` skips calling `transform_socios_and_expand_radicals` when `files.is_empty()`.
   - In `loader.rs`, `SOCIOS_FINAL_DDL` is only executed inside `transform_socios_and_expand_radicals`.
   - When `Socios*.zip` is absent, `socios` table is never created, leaving `socios_original` orphaned in the database.
   - Stage 8 executes `POST_LOAD_INDICES_SQL` containing `CREATE INDEX IF NOT EXISTS idx_socios_cnpj ON socios (cnpj);`. SQLite rejects `CREATE INDEX` on non-existent tables regardless of `IF NOT EXISTS`.

3. **Validation of Concurrency and Memory Limits**:
   - The bounded channel capacity of 16 blocks producers when consumer is slow (Observation 1.2).
   - Real-world RSS memory for 100,000 records was 54.71 MB, remaining well within the 1.8 GB constraint (Observation 1.3).
   - Rayon parallel error propagation terminates worker tasks cleanly upon encountering corrupted zip files without thread hangs (Observation 1.4).

---

## 3. Caveats

1. **Test Dataset Size**: Memory bounding was tested up to 100,000 synthetic rows (54.71 MB RSS). Full 80 GB RFB open data was not tested locally due to disk and bandwidth constraints, but the mathematical in-flight memory ceiling ($16 \times 5,000 \times \text{row\_size} \le 60\text{ MB}$) is invariant to dataset size.
2. **Implementation Code Untouched**: In compliance with the role constraints ("Review-only — do NOT modify implementation code"), no changes were made to `src/**`. The fix must be implemented by `worker_m2`.

---

## 4. Conclusion

**Verdict: REQUEST_CHANGES**

Milestone 2 is **almost complete and architecturally high-performing**, but must be returned to `worker_m2` to fix **VULN-M2-01**:

### Required Remediation for `worker_m2`:
1. In `src/ingestion/schema.rs` (`create_initial_tables`):
   - Add `conn.execute_batch(SOCIOS_FINAL_DDL)?;` and `conn.execute_batch(CNPJ_BASE2MATRIZ_DDL)?;` so that all 12 destination tables + lookup caches always exist from the start.
2. In `src/ingestion/mod.rs` (`ingest_socios`):
   - If `files.is_empty()`, ensure `socios_original` is cleaned up/dropped, or allow `transform_socios_and_expand_radicals` to run on an empty `socios_original` table (which executes instantly with 0 rows and drops `socios_original`).
3. Add an automated test verifying `rfb-cnpj-etl build-cnpj` on an empty directory succeeds with code 0 and creates all 12 tables + indices + `_referencia`.

---

## 5. Verification Method

To independently verify this report and reproduce the findings:

```bash
cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

# 1. Reproduce VULN-M2-01 directly via CLI (exits with code 1):
mkdir -p /tmp/test_empty_input
cargo run --bin rfb-cnpj-etl -- build-cnpj --input-dir /tmp/test_empty_input --output-db /tmp/test_empty_out.db

# 2. Run the adversarial stress test suite:
cargo test --test adversarial_stress_test -- --nocapture

# 3. Verify memory telemetry and backpressure tests:
cargo test test_channel_backpressure_slow_consumer_blocks_producer -- --nocapture
cargo test test_memory_bounding_under_high_row_count -- --nocapture

# 4. Strict clippy verification:
cargo clippy --all-targets -- -D warnings
```
