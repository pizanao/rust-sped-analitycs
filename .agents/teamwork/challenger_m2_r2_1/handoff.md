# Adversarial Challenge Report — Milestone 2 Remediation Round 2 (Empty Directory & Schema Invariants)

- **Agent**: `challenger_m2_r2_1`
- **Role**: Empty Directory & Schema Adversarial Challenger (critic, specialist)
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m2_r2_1`
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Recipient**: `orchestrator_1` (Conversation ID: `0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)
- **Date**: 2026-10-25T00:33:00Z
- **Verdict**: **APPROVE**

---

## 1. Observation

1. **Target Project Test Suite Execution**:
   - Command: `cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --all-targets`
   - Results: All 94 tests passed (36 unit tests, 13 adversarial stress tests, 13 adversarial transforms tests, 32 E2E pipeline tests). 0 failed, 0 ignored.
   - Clippy check: `cargo clippy --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --all-targets -- -D warnings` exited with code 0 (0 warnings).

2. **Empirical Test Case 1: Completely Empty Input Directory**:
   - Execution: Ran CLI binary `rfb-cnpj-etl build-cnpj --input-dir /tmp/adv_m2_r2_verification/case1_empty/input --output-db /tmp/adv_m2_r2_verification/case1_empty/cnpj.db --memory-limit-mb 512`.
   - Process Result: Exited with code 0 in 7.42ms.
   - SQLite DB Invariants (`/tmp/adv_m2_r2_verification/case1_empty/cnpj.db`):
     * `PRAGMA journal_mode`: returned `wal`.
     * `SELECT name FROM sqlite_master WHERE type='table'`: exactly 12 canonical tables found (`_referencia`, `cnae`, `cnpj_base2matriz`, `empresas`, `estabelecimento`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio`, `simples`, `socios`).
     * `SELECT count(*) FROM sqlite_master WHERE name='socios_original'`: returned `0` (`socios_original` is completely absent).
     * `SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%'`: exactly 18 secondary indices found (`idx_cnae`, `idx_cnpj_base2matriz`, `idx_empresas_cnpj_basico`, `idx_empresas_razao_social`, `idx_estabelecimento_cnpj`, `idx_estabelecimento_cnpj_basico`, `idx_estabelecimento_nomefantasia`, `idx_motivo`, `idx_municipio`, `idx_natureza_juridica`, `idx_pais`, `idx_qualificacao_socio`, `idx_simples_cnpj_basico`, `idx_socios_cnpj`, `idx_socios_cnpj_cpf_socio`, `idx_socios_nome_socio`, `idx_socios_representante`, `idx_socios_representante_nome`).
     * Row counts: All data tables have 0 rows; `_referencia` contains 2 audit records (reference date + establishment count 0).

3. **Empirical Test Case 2: Only `Empresas0.zip` (Missing Socios, Estabelecimentos, Simples)**:
   - Input: Directory containing only `Empresas0.zip` (with 3 valid enterprises encoded in Windows-1252: Alfa, Beta, Gama).
   - Execution: Ran CLI binary `rfb-cnpj-etl build-cnpj --input-dir /tmp/adv_m2_r2_verification/case2_empresas/input --output-db /tmp/adv_m2_r2_verification/case2_empresas/cnpj.db --memory-limit-mb 512`.
   - Process Result: Exited with code 0 in 10.09ms.
   - SQLite DB Invariants:
     * `PRAGMA journal_mode`: `wal`.
     * Tables: Exactly 12 tables present.
     * `socios_original`: Absent (`0`).
     * Indices: All 18 secondary indices created and operational.
     * Row counts: `empresas`: 3, `_referencia`: 2, all other data tables: 0.

4. **Empirical Test Case 3: Only `Socios0.zip` (Missing Empresas, Estabelecimentos)**:
   - Input: Directory containing only `Socios0.zip` (with 3 partner records: João da Silva [PF], Alfa Tecnologia [PJ], Beta Comércio [PJ]).
   - Execution: Ran CLI binary `rfb-cnpj-etl build-cnpj --input-dir /tmp/adv_m2_r2_verification/case3_socios/input --output-db /tmp/adv_m2_r2_verification/case3_socios/cnpj.db --memory-limit-mb 512`.
   - Process Result: Exited with code 0 in 11.23ms.
   - SQLite DB Invariants:
     * `PRAGMA journal_mode`: `wal`.
     * Tables: Exactly 12 tables present.
     * `socios_original`: Cleanly dropped after transformation join (`0`).
     * Indices: All 18 secondary indices created and operational (including all 5 `idx_socios_*` indices).
     * Row counts: `socios`: 3, `_referencia`: 2, all other data tables: 0.

5. **Empirical Test Case 4: Extraneous Files & Directories + `Simples.zip`**:
   - Input: Directory containing `.hidden_subfolder/`, `README.txt`, and `Simples.zip`.
   - Process Result: Exited with code 0 in 9.38ms.
   - SQLite DB Invariants: All 12 tables, all 18 indices, `socios_original` absent, `wal` mode, `simples` row count = 1.

---

## 2. Logic Chain

1. **Root Cause Analysis Verification (VULN-M2-01)**:
   - Observation 1 and code review of `src/ingestion/schema.rs:134-146` confirm that `create_initial_tables(&conn)` now executes `SOCIOS_FINAL_DDL` and `CNPJ_BASE2MATRIZ_DDL` upfront during Stage 0 (Initialization).
   - This prevents the fatal schema lifecycle failure where `socios` was never created when `Socios*.zip` partitions were absent.

2. **Staging Cleanup Invariant**:
   - When no `Socios*.zip` files are discovered (`src/ingestion/mod.rs:509-514`), `conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?;` explicitly drops the staging table before returning `Ok(0)`.
   - When `Socios*.zip` files are discovered (`src/ingestion/loader.rs:543`), `transform_socios_and_expand_radicals(&mut conn)` executes `conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?;` immediately after the radical expansion insert.
   - Consequently, in Observations 2, 3, 4, and 5, under all tested permutations (empty directory, missing socios, only socios, extraneous files), `socios_original` was absent in 100% of cases.

3. **Post-Load Indexing Invariant**:
   - Because all 12 contractual tables exist in the schema prior to Stage 8 (`create_post_load_indices`), all 18 `CREATE INDEX IF NOT EXISTS` statements succeed unconditionally without `no such table` SQLite errors.
   - All 18 secondary indices were empirically confirmed present and queryable in Observations 2, 3, 4, and 5.

4. **Exit Code & Memory Bounding**:
   - Under all permutations, `rfb-cnpj-etl build-cnpj` terminated cleanly with exit code 0.
   - Memory consumption remained under 17 MB RSS, well below the 1,800 MB constraint.

---

## 3. Caveats

- **Network Graph Downstream (Milestone 3)**: When `build-cnpj` is executed on an empty directory or partial dataset missing `estabelecimento`, `cnpj_base2matriz` contains 0 rows and `socios.cnpj` contains `NULL`. Milestone 3 (`build-rede`) must ensure its societary graph derivation queries handle `NULL` CNPJ values gracefully without aborting.
- **Review Scope Boundary**: This challenge specifically verified the fix for VULN-M2-01 (schema lifecycle, empty directory handling, and partial dataset ingestion). Full pipeline download and graph derivation belong to Milestones 1 and 3.

---

## 4. Conclusion

The remediation implemented by `worker_m2_r2` completely resolves VULN-M2-01 and satisfies all contractual schema and operational invariants:
- Clean exit code 0 on empty input directory, partial datasets, and mixed directories.
- Exactly 12 tables exist (`_referencia`, `cnae`, `cnpj_base2matriz`, `empresas`, `estabelecimento`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio`, `simples`, `socios`).
- All 18 secondary indices exist.
- Temporary staging table `socios_original` is cleanly dropped under all conditions.
- `PRAGMA journal_mode = wal` is applied.
- All 94 automated tests in the workspace pass.

**Verdict**: **APPROVE**

---

## 5. Verification Method

To independently reproduce the empirical findings:

```bash
cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

# 1. Build binary
cargo build --bin rfb-cnpj-etl

# 2. Run test on empty directory
rm -rf /tmp/emp_test /tmp/emp_test.db*
mkdir -p /tmp/emp_test
./target/debug/rfb-cnpj-etl build-cnpj --input-dir /tmp/emp_test --output-db /tmp/emp_test.db
echo "Exit code: $?" # Must be 0

# 3. Query SQLite invariants
sqlite3 /tmp/emp_test.db "PRAGMA journal_mode;" # Must be wal
sqlite3 /tmp/emp_test.db "SELECT count(*) FROM sqlite_master WHERE type='table';" # Must be 12
sqlite3 /tmp/emp_test.db "SELECT count(*) FROM sqlite_master WHERE name='socios_original';" # Must be 0
sqlite3 /tmp/emp_test.db "SELECT count(*) FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%';" # Must be 18

# 4. Clean up
rm -rf /tmp/emp_test /tmp/emp_test.db*
```

### Invalidation Conditions
- If `rfb-cnpj-etl build-cnpj` exits non-zero on an empty directory or partial dataset.
- If `socios_original` is present in the output SQLite database.
- If any of the 12 contractual tables or 18 secondary indices are missing.
- If `PRAGMA journal_mode` is not `wal`.
