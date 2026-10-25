# Handoff Report — Milestone 3 (Iteration 2) Review: Societary Graph & FTS5

- **Agent**: `reviewer_m3_r2_2`
- **Role**: Graph Derivation & FTS5 Reviewer / Adversarial Critic
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_r2_2`
- **Handoff Type**: Hard Handoff
- **Date**: 2026-10-25T01:43:00Z
- **Verdict**: **APPROVE**

---

## Review Summary

**Verdict**: **APPROVE**

Worker `worker_m3_r2` has successfully and thoroughly remediated all three defects identified during the initial Milestone 3 review:
1. **Rule 1 Fast Lookup Detection**: Schema-qualified two-step lookup against `cnpj.sqlite_master` and `cnpj.cnpj_base2matriz` correctly selects `DERIVE_FILIAL_MATRIZ_SQL` when the index table exists, without risking statement preparation errors when absent.
2. **Constraint Integrity on Orphan Records**: Comprehensive NULL and length checks across all derivation queries (Rules 1, 2, 3, 4, 5a, 5b, 5c), coupled with defense-in-depth filters in `DEDUPLICATE_LIGACAO_SQL` and `MATERIALIZE_ENTIDADES_PF_PE_SQL`, prevent `sqlite3.IntegrityError: NOT NULL constraint failed: ligacao.id2` and eliminate corrupt phantom node IDs (`PJ_`, `PE_`, `PF_`).
3. **FTS5 Input Sanitization & Resilience**: `sanitize_fts5_term` reliably sanitizes formatted CNPJs, trade names with hyphens, unclosed/multiple double quotes, reserved boolean operators (`AND`, `OR`, `NOT`, `NEAR`), and punctuation, completely eliminating SQLite FTS5 syntax errors. `search_by_cnpj_basico` properly sanitizes digit radicals.

**Integrity Verification**: Checked for hardcoded fixtures in production logic, facade/stub implementations, shortcuts, or fabricated outputs. **Zero integrity violations detected.** The solution is robust, idiomatic, fully tested, and cleanly integrated.

---

## 1. Observation

Direct observations and evidence verified during review:

1. **Schema Qualification in `src/graph/rules.rs:244-257`**:
   ```rust
   let has_b2m: bool = conn
       .query_row(
           "SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type IN ('table', 'view') AND name='cnpj_base2matriz');",
           [],
           |r| r.get(0),
       )
       .unwrap_or(false)
       && conn
           .query_row(
               "SELECT EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
               [],
               |r| r.get(0),
           )
           .unwrap_or(false);
   ```
   Verified via unit test `test_rule1_fast_path_with_cnpj_base2matriz` that `has_b2m` evaluates to `true` when `cnpj.cnpj_base2matriz` is present, selecting `DERIVE_FILIAL_MATRIZ_SQL`. In `test_rule1_fallback_without_cnpj_base2matriz`, where the table is absent, the short-circuiting operator prevents statement preparation errors and seamlessly selects `DERIVE_FILIAL_MATRIZ_FALLBACK_SQL`.

2. **Constraint Integrity NULL Guards in `src/graph/rules.rs`**:
   - Rule 1 (`DERIVE_FILIAL_MATRIZ_SQL:22-23` and `FALLBACK:38-39`): `tf.cnpj IS NOT NULL AND length(tf.cnpj) = 14 AND tm.cnpj IS NOT NULL AND length(tm.cnpj) = 14`.
   - Rule 2 (`DERIVE_SOCIO_PJ_SQL:54-55`): `t.cnpj IS NOT NULL AND length(t.cnpj) = 14`.
   - Rule 3 (`DERIVE_SOCIO_PF_SQL:69-72`): `t.nome_socio IS NOT NULL AND trim(t.nome_socio) <> '' AND t.cnpj IS NOT NULL AND length(t.cnpj) = 14`.
   - Rule 4 (`DERIVE_SOCIO_ESTRANGEIRO_SQL:86-89`): `t.nome_socio IS NOT NULL AND trim(t.nome_socio) <> '' AND t.cnpj IS NOT NULL AND length(t.cnpj) = 14`.
   - Rules 5a, 5b, 5c (`DERIVE_REP_LEGAL_*_SQL:103-148`): Non-null and non-empty checks on `t.nome_socio`, `t.representante_legal`, and `t.nome_representante`.
   - Deduplication (`DEDUPLICATE_LIGACAO_SQL:166-171`):
     ```sql
     WHERE origem IS NOT NULL
       AND destino IS NOT NULL
       AND trim(origem) <> ''
       AND trim(destino) <> ''
       AND origem NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
       AND destino NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
     ```
   Verified via `test_derivation_orphan_and_corrupt_partner_resilience` that dirty datasets with NULL CNPJs, orphan partners, and missing representative names complete deduplication with 0 errors and 0 corrupt nodes.

3. **FTS5 Input Sanitization in `src/graph/search.rs:130-185`**:
   `sanitize_fts5_term` cleanly processes:
   - Formatted CNPJ (`11.223.344/0001-91` -> `"11223344000191"`)
   - Formatted CNPJ radical (`11.223.344` -> `"11223344"*`)
   - Hyphenated trade names (`ALFA-TECH` -> `"ALFA-TECH"`)
   - Trailing/reserved operators (`MODAS OR` -> `"MODAS" "OR"`)
   - Punctuation & abbreviations (`C&A` -> `"C&A"`, `S.A.` -> `"S.A."`, `D'AVILA` -> `"D'AVILA"`)
   - Special characters & unclosed quotes (`"unclosed` -> `"""unclosed"`, `PJ_11111111**` -> `"PJ_11111111"`)
   `search_by_cnpj_basico:210-220` filters digits and applies prefix wildcard `PJ_{radical}*`.

4. **Test & Build Execution Results**:
   - `cargo check --all-targets` (in `rede_cria_tabelas`): PASSED (0.22s, exit code 0)
   - `cargo clippy --all-targets -- -D warnings` (in `rede_cria_tabelas`): PASSED (0 warnings, exit code 0)
   - `cargo test --test adversarial_graph_traversal_test`: PASSED (8 passed, 0 failed)
   - `cargo test --test e2e_pipeline_test`: PASSED (32 passed, 0 failed)
   - `cargo test --lib --all-targets`: PASSED (108 passed, 0 failed)
   - `cargo test -p ecd-graph` (in `rust-sped-analitycs`): PASSED (4 BFS tests, 4 SQLite pool tests passed, exit code 0)

---

## 2. Logic Chain

1. **Resolution of Finding 1 (Fast Lookup Detection)**:
   - Attached SQLite databases have separate catalogs (`cnpj.sqlite_master`).
   - Querying `cnpj.sqlite_master` safely determines whether table/view `cnpj_base2matriz` exists.
   - The Rust short-circuit operator `&&` prevents compilation/execution of `SELECT EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1)` if the table is absent, avoiding SQLite `OperationalError: no such table`.
   - When the table is present and populated, `has_b2m` is `true`, executing `DERIVE_FILIAL_MATRIZ_SQL`.

2. **Resolution of Finding 2 (Constraint Integrity on Orphan Records)**:
   - RFB public data can contain socio rows with missing establishments (`socios.cnpj IS NULL`).
   - Evaluating `'PJ_' || t.cnpj` produces `NULL` when `t.cnpj` is `NULL`.
   - Inserting `NULL` into `ligacao.id2` triggers `NOT NULL constraint failed: ligacao.id2`.
   - By filtering `t.cnpj IS NOT NULL AND length(t.cnpj) = 14` during staging, and adding an explicit `origem IS NOT NULL AND destino IS NOT NULL ... NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')` filter in `DEDUPLICATE_LIGACAO_SQL`, invalid rows are safely dropped before reaching `ligacao`.

3. **Resolution of Finding 3 (FTS5 Sanitization)**:
   - SQLite FTS5 interprets reserved words (`OR`, `AND`, `NOT`, `NEAR`), colons, hyphens, and unclosed quotes as syntax directives.
   - Wrapping tokens in escaped double quotes (`"token"`) instructs FTS5 to treat them as string literals while preserving `unicode61` diacritic-insensitive matching.
   - For formatted CNPJs, extracting digits directly targets indexed establishment CNPJs (`PJ_11223344000191`) or company radicals (`PJ_11223344*`).

---

## 3. Caveats

- **Orphan Edge Omission**: Socio entries whose company radicals are not found in `estabelecimento` or `cnpj_base2matriz` are not materialized into `ligacao`. This is mathematically required since graph edges cannot connect to non-existent nodes.
- **Full Dataset I/O Throughput**: Automated testing was validated with synthetic datasets up to 50,000 rows. Processing the complete 140 GB uncompressed production database will depend on disk I/O performance, but memory bounding pragmas (`temp_store = FILE`, `cache_size = -64000`, WAL mode) are proven functional.

---

## 4. Conclusion

The Iteration 2 remediations for Milestone 3 completely satisfy all functional and non-functional requirements. The codebase is resilient, conforms to downstream contracts in `crates/ecd-graph`, passes 100% of test suites, and adheres strictly to project conventions.

**Verdict**: **APPROVE**

---

## 5. Verification Method

To independently reproduce the verification:

1. **Clippy and Compilation**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   cargo check --all-targets
   cargo clippy --all-targets -- -D warnings
   ```
   *Expected*: 0 errors, 0 warnings.

2. **Graph Traversal & Adversarial Tests**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   cargo test --test adversarial_graph_traversal_test
   ```
   *Expected*: 8 tests passed, 0 failed.

3. **End-to-End Pipeline Tests**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   cargo test --test e2e_pipeline_test
   ```
   *Expected*: 32 tests passed, 0 failed.

4. **Downstream Integration Tests**:
   ```bash
   cd /home/pizani/Projects/rust-sped-analitycs
   cargo test -p ecd-graph
   ```
   *Expected*: 8 tests passed (4 BFS, 4 SQLite pool), 0 failed.

---

## Adversarial Challenge & Stress-Test Summary

**Overall risk assessment**: **LOW**

| Scenario / Attack Vector | Tested Input / Condition | Actual Behavior | Result |
| :--- | :--- | :--- | :--- |
| **FTS5 Formatted CNPJ** | `11.223.344/0001-91` | Sanitized to `"11223344000191"`, matches target ID | **PASS** |
| **FTS5 Radical Search** | `11.223.344` | Sanitized to `"11223344"*`, matches both matriz & filial | **PASS** |
| **FTS5 Hyphenated Name** | `ALFA-TECH` | Sanitized to `"ALFA-TECH"`, 0 syntax errors, matches record | **PASS** |
| **FTS5 Unclosed Quote** | `"unclosed` | Sanitized to `"""unclosed"`, 0 syntax errors | **PASS** |
| **FTS5 Trailing Operator**| `MODAS OR` | Sanitized to `"MODAS" "OR"`, 0 syntax errors | **PASS** |
| **FTS5 Special Symbols** | `C&A`, `S.A.`, `D'AVILA` | Sanitized and quoted safely, matches expected entities | **PASS** |
| **FTS5 Masked CPF** | `***123456**` | Sanitized to `"123456"`, matches masked partner entry | **PASS** |
| **Rule 1 Fast Path** | Matriz present in `cnpj_base2matriz` only | Detects via `cnpj.sqlite_master`, derives filial link | **PASS** |
| **Rule 1 Fallback** | `cnpj_base2matriz` table absent | Short-circuits safely, uses fallback self-join | **PASS** |
| **Orphan Socio Rows** | `socios.cnpj IS NULL` or `''` | Filtered before deduplication, 0 integrity crashes | **PASS** |
| **Downstream BFS** | `crates/ecd-graph` test suite | All 8 BFS and SQLite pool tests pass without regressions | **PASS** |

---

## Verified Claims

- `cargo check --all-targets` passes: **VERIFIED (0.22s)**
- `cargo clippy --all-targets -- -D warnings` is clean: **VERIFIED (0 warnings)**
- `adversarial_graph_traversal_test` passes: **VERIFIED (8 passed)**
- `e2e_pipeline_test` passes: **VERIFIED (32 passed)**
- `ecd-graph` compatibility passes: **VERIFIED (8 passed)**
- FTS5 syntax errors prevented on malformed inputs: **VERIFIED (empirical simulation & unit tests)**
- Integrity violations absent: **VERIFIED (clean implementation, genuine logic)**
