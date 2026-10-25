# Handoff Report — Code Review & Adversarial Analysis: Milestone 2

**Agent**: `reviewer_m2_1`  
**Role**: Systems & Database Code Reviewer (Reviewer & Adversarial Critic)  
**Target Repository**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`  
**Date**: 2026-10-25T00:10:00Z  
**Recipient**: `orchestrator_1` (Conversation ID: `0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)  
**Verdict**: **`REQUEST_CHANGES`**

---

## Review Summary

**Verdict**: **`REQUEST_CHANGES`**

The implementation of Milestone 2 (High-Performance CNPJ Ingestion Engine) demonstrates high technical quality: clean streaming decompression, zero-copy byte recording, in-flight Windows-1252 decoding, zero-allocation BRL float parsing, synthesized 14-digit CNPJs, and robust August 2026 partner radical expansion. Crucially, **no integrity violations were detected** (no dummy facades, no shortcuts, no hardcoded production outputs).

However, an adversarial stress test revealed a **Critical schema lifecycle bug**: when `Socios*.zip` is absent (such as when running on an empty input directory or an incremental/partial dataset), `rfb-cnpj-etl build-cnpj` crashes fatally during the `Post-Load Index Creation` stage with `no such table: main.socios`. This breaks the system contract and invalidates Worker M2's claim of graceful handling for missing partition files. A surgical 2-line fix is required before Milestone 2 can be approved.

---

## Findings

### [Critical] Finding 1: Pipeline Crashes with `no such table: main.socios` when `Socios*.zip` is Missing

- **What**: Executing `rfb-cnpj-etl build-cnpj` when no `Socios*.zip` archive is present in the input directory causes the pipeline to abort with error:
  ```text
  Error: Failed during stage: Post-Load Index Creation
  Caused by:
      0: no such table: main.socios
      1: Error code 1: SQL error or missing database
  ```
- **Where**: 
  - `src/ingestion/schema.rs` (lines 133–144): `create_initial_tables()`
  - `src/ingestion/mod.rs` (lines 508–512): `ingest_socios()`
  - `src/ingestion/loader.rs` (lines 478–481): `transform_socios_and_expand_radicals()`
- **Why**: 
  1. `create_initial_tables(&conn)` creates the temporary staging table `socios_original`, but does **not** create the final `socios` table.
  2. The DDL for `socios` (`SOCIOS_FINAL_DDL`) is executed exclusively inside `transform_socios_and_expand_radicals(&mut conn)`.
  3. When `Socios*.zip` is missing, `ingest_socios()` encounters an empty file list and immediately returns early:
     ```rust
     let files = self.find_partition_files("Socios", 10);
     if files.is_empty() {
         warn!("No Socios*.zip files found in input directory");
         return Ok(0);
     }
     ```
  4. Because it returns early, `transform_socios_and_expand_radicals(&mut conn)` is never called, meaning `socios` is **never created**, and `socios_original` is **never dropped**.
  5. In Stage 8 (`PostIndexing`), `create_post_load_indices(&conn)` executes:
     ```sql
     CREATE INDEX IF NOT EXISTS idx_socios_cnpj ON socios (cnpj);
     ```
     Because table `socios` does not exist, SQLite throws a fatal error, crashing the process.
  6. This refutes Worker M2's claim in Caveat 1 of `worker_m2/handoff.md` stating: *"If a table has no matching files in the directory, the orchestrator logs a warning and proceeds without error."*
- **Suggestion**:
  Ensure table `socios` is guaranteed to exist regardless of whether `Socios*.zip` files are present. Specifically:
  1. In `src/ingestion/schema.rs` (`create_initial_tables`), add:
     ```rust
     conn.execute_batch(SOCIOS_FINAL_DDL)?;
     ```
  2. In `src/ingestion/mod.rs` (`ingest_socios`), ensure that if `files.is_empty()`, the staging table `socios_original` is cleaned up:
     ```rust
     if files.is_empty() {
         warn!("No Socios*.zip files found in input directory");
         conn.execute_batch(SOCIOS_FINAL_DDL)?;
         conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?;
         return Ok(0);
     }
     ```

---

### [Minor] Finding 2: Loose Validation in RFB Date Token Parser

- **What**: `parse_rfb_reference_code` matches any 6-character part starting with `'D'` without verifying that trailing characters are numeric digits.
- **Where**: `src/ingestion/transforms.rs` (lines 184–201).
- **Why**: A non-date token such as `DELETE` or `DEBUG0` would match `part.len() == 6 && part.starts_with('D')`, generating an invalid date string (e.g., `TE/LE/202E`).
- **Suggestion**: Add a check verifying that all subsequent characters are ASCII digits before slicing:
  ```rust
  if part.len() == 6 && part.starts_with('D') && part[1..].chars().all(|c| c.is_ascii_digit())
  ```

---

## 1. Observation

1. **Integrity Check**:
   - Inspected `src/ingestion/transforms.rs`, `src/ingestion/zip_stream.rs`, `src/ingestion/schema.rs`, `src/ingestion/loader.rs`, and `src/ingestion/mod.rs`.
   - Grep search for hardcoded test patterns (e.g. `"11111111"`, `"ALFA"`) confirmed they exist exclusively within `#[cfg(test)] mod tests` blocks. Zero hardcoded test outputs or shortcuts exist in production code.
   - Decompression, parsing, and SQLite batch loading execute genuine logic via `zip::ZipArchive`, `csv::ReaderBuilder`, `encoding_rs::WINDOWS_1252`, and `rusqlite::Transaction`.

2. **Automated Verification Commands**:
   - `cargo check`: Exited with code 0 in 0.23s.
   - `cargo clippy --all-targets -- -D warnings`: Exited with code 0 in 0.29s (0 warnings).
   - `cargo test --all-targets`: All 67 tests passed (36 unit tests + 31 E2E integration tests), 0 failed, 0 ignored.

3. **Adversarial Reproduction of Finding 1**:
   Executed `build-cnpj` in an empty directory:
   ```bash
   rfb-cnpj-etl build-cnpj --input-dir /tmp/empty_zips --output-db /tmp/cnpj.db
   ```
   Direct output observed:
   ```text
   INFO Starting RFB CNPJ ingestion pipeline ...
   WARN No Empresas*.zip files found in input directory
   WARN No Estabelecimentos*.zip files found in input directory
   WARN Simples.zip not found in input directory
   WARN No Socios*.zip files found in input directory
   Error: Failed during stage: Post-Load Index Creation
   Caused by:
       0: no such table: main.socios
       1: Error code 1: SQL error or missing database
   ```
   Exit code: 1.

4. **Independent Functional Verification with Synthetic Dataset**:
   Generated 5 synthetic RFB archives (`Cnaes.zip`, `Empresas0.zip`, `Estabelecimentos0.zip`, `Simples.zip`, `Socios0.zip`) and executed `build-cnpj`:
   - `_referencia`: `[('CNPJ', '24/10/2026'), ('cnpj_qtde', '3')]` (matches official RFB key format).
   - `empresas`: `capital_social` correctly stored as REAL float `150000.5` from `"150000,50"`.
   - `estabelecimento`: Synthesized 14-character CNPJs (`11111111000191`, `11111111000272`, `22222222000182`) stored in `cnpj` column.
   - `cnpj_base2matriz`: Correctly filtered out branch `11111111000272` and retained 2 matriz records.
   - `socios`: 8-digit radical `'11111111'` for PJ partner expanded to 14-digit Matriz CNPJ `'11111111000191'`. PF partner kept masked CPF `'***123456**'`.
   - `PRAGMA journal_mode`: Verified as `wal`.

---

## 2. Logic Chain

1. **Schema Integrity**:
   - `PROJECT.md` defines 12 required tables: 6 domain tables (`cnae`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio`), `empresas`, `estabelecimento`, `simples`, `cnpj_base2matriz`, `socios`, and `_referencia` (Observation 1, 4).
   - `create_initial_tables()` creates 10 tables upfront. `cnpj_base2matriz` is created in Stage 5. `socios` is only created in Stage 6 if `Socios*.zip` files are present.
   - If `Socios*.zip` is missing, `socios` is never created.

2. **Causal Chain Leading to Pipeline Failure**:
   - Observation 3 proves that running `build-cnpj` without `Socios*.zip` causes Stage 8 (`PostIndexing`) to execute `CREATE INDEX IF NOT EXISTS idx_socios_cnpj ON socios (cnpj);`.
   - In SQLite, attempting to create an index on a non-existent table is an immediate fatal error (`SQL error: no such table: main.socios`), terminating the entire pipeline with exit code 1.
   - Therefore, the pipeline fails when handling missing or partial file sets.

3. **Core Transformation Robustness**:
   - Where files are present, the data pipeline correctly transcodes Windows-1252, parses BRL floats without allocations, right-aligns and pads synthesized 14-char CNPJs, and applies the August 2026 join logic (`COALESCE(tes.cnpj, ts.cnpj_cpf_socio)`) (Observation 4).
   - The RSS memory telemetry correctly reads `/proc/self/statm` on Linux and confirms execution well within the 1.8 GB memory ceiling.

---

## 3. Caveats

1. **E2E Test Suite Scope**:
   - Existing E2E tests (`tests/e2e_pipeline_test.rs`) only test `status` against an empty directory, not `build-cnpj` against an empty or partial directory. This allowed Finding 1 to go undetected until this adversarial review.
2. **Platform-Specific Memory Telemetry**:
   - Memory RSS telemetry uses `/proc/self/statm`, which is Linux-specific; on other platforms, it returns `None` and skips the memory line.

---

## 4. Conclusion

The Milestone 2 implementation is well-architected and adheres to performance and memory requirements. However, due to **Finding 1** (fatal crash during post-load indexing when `Socios*.zip` is absent), the verdict is **`REQUEST_CHANGES`**.

Once `worker_m2` ensures that `SOCIOS_FINAL_DDL` is applied in `create_initial_tables()` (or in the early-exit path of `ingest_socios()`) and adds a regression test for partial/empty file inputs, Milestone 2 will be ready for final approval.

---

## 5. Verification Method

To reproduce Finding 1 and verify the fix:

```bash
cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

# 1. Reproduce Finding 1 (Fatal crash on missing Socios*.zip)
mkdir -p /tmp/test_empty_zips
./target/debug/rfb-cnpj-etl build-cnpj --input-dir /tmp/test_empty_zips --output-db /tmp/test_empty.db
# Expected before fix: Crash with "no such table: main.socios" (Exit code 1)
# Expected after fix: Complete with 0 rows inserted (Exit code 0)

# 2. Standard compilation and test suite
cargo check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

# 3. Clean up
rm -rf /tmp/test_empty_zips /tmp/test_empty.db*
```
