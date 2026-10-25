# Milestone 3 Review & Adversarial Challenge Report: Graph Derivation, Consumer Contract & FTS5

- **Reviewer**: `reviewer_m3_2` (Roles: `reviewer`, `critic`)
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_2`
- **Handoff Type**: Hard Handoff (Review Completed)
- **Date**: 2026-10-25T01:09:00Z

---

## Review Summary

**Verdict**: **REQUEST_CHANGES**

The worker (`worker_m3`) has established an impressive, cleanly structured native Rust implementation of the societary graph derivation and FTS5 search indexing pipeline (`build-rede`). Compilation passes without warnings, `cargo clippy --all-targets -- -D warnings` is clean, and downstream tests in `crates/ecd-graph` pass. **No integrity violations** (no hardcoded fixtures in production code, no fake stubs, no bypassing shortcuts) were detected.

However, adversarial stress testing and line-by-line inspection identified **three material defects** that require remediation before Milestone 3 can be approved:
1. **Critical Logic Flaw in Rule 1 Optimization**: An unqualified `sqlite_master` query permanently evaluates to `false` on the attached database connection, completely disabling the primary `cnpj_base2matriz` fast lookup index and permanently routing execution through an expensive self-join fallback on `estabelecimento`.
2. **Database Constraint Integrity Risk**: Absence of NULL / empty guards in derivation staging (`ligacao1`) causes `deduplicate_ligacao` to crash with SQLite IntegrityError (`NOT NULL constraint failed: ligacao.id2`) whenever orphan partner records exist.
3. **FTS5 Input Vulnerability**: Raw user inputs in `search_by_term` and `search_by_cnpj_basico` are passed directly to FTS5 `MATCH` without sanitization, crashing on formatted CNPJs (`11.223.344/0001-91`), hyphenated names, or standard punctuation.

---

## 1. Observation

1. **Rule 1 Fast Lookup Bypassed via Schema-Unqualified `sqlite_master`**:
   In `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs:203-215`:
   ```rust
   // Check if cnpj_base2matriz has rows
   let has_b2m: bool = conn
       .query_row(
           "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cnpj_base2matriz') AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
           [],
           |r| r.get(0),
       )
       .unwrap_or(false);

   let sql = if has_b2m {
       DERIVE_FILIAL_MATRIZ_SQL
   } else {
       DERIVE_FILIAL_MATRIZ_FALLBACK_SQL
   };
   ```
   Direct execution test in SQLite:
   ```bash
   python3 -c "
   import sqlite3
   c = sqlite3.connect(':memory:')
   c.execute('ATTACH DATABASE \":memory:\" AS cnpj')
   c.execute('CREATE TABLE cnpj.cnpj_base2matriz (cnpj_basico text)')
   res1 = c.execute(\"SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cnpj_base2matriz')\").fetchone()[0]
   res2 = c.execute(\"SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type='table' AND name='cnpj_base2matriz')\").fetchone()[0]
   print('Unqualified:', res1, 'Qualified:', res2)
   "
   # Output: Unqualified: 0 Qualified: 1
   ```
   Because `conn` is opened on `output_rede` (`main`), querying `sqlite_master` only inspects `main.sqlite_master`, where `cnpj_base2matriz` does not exist. `has_b2m` is **always `false`**, and `DERIVE_FILIAL_MATRIZ_FALLBACK_SQL` is unconditionally used.

2. **Integrity Crash on Orphan Socio Records with `NULL` or Empty Matriz**:
   In `src/graph/rules.rs:37-75` (Rules 2, 3, 4) and lines 128-146 (`DEDUPLICATE_LIGACAO_SQL`):
   ```sql
   -- DEDUPLICATE_LIGACAO_SQL:
   CREATE TABLE IF NOT EXISTS ligacao (
       id1 TEXT NOT NULL,
       id2 TEXT NOT NULL,
       descricao TEXT NOT NULL DEFAULT 'SOCIO',
       comentario TEXT NOT NULL DEFAULT ''
   );

   INSERT INTO ligacao (id1, id2, descricao, comentario)
   SELECT origem AS id1, destino AS id2, COALESCE(tipo, 'Sócio') AS descricao, COALESCE(base, '') AS comentario
   FROM ligacao1
   GROUP BY origem, destino, tipo, base;
   ```
   If a partner in `cnpj.socios` has a `cnpj_basico` whose establishment was not present in `estabelecimento` during Milestone 2 ingestion (or has a `NULL` `cnpj`), `'PJ_' || t.cnpj` evaluates to `NULL`.
   Inserting `NULL` into `ligacao.id2` triggers:
   `<class 'sqlite3.IntegrityError'> NOT NULL constraint failed: ligacao.id2`.
   Furthermore, an empty string `t.cnpj = ''` produces `'PJ_'`, generating corrupt phantom node IDs.

3. **Unsanitized FTS5 Syntax Crash**:
   In `src/graph/search.rs:122-150`:
   ```rust
   pub fn search_by_term(conn: &Connection, term: &str, limit: usize) -> rusqlite::Result<Vec<(String, String)>> {
       let mut stmt = conn.prepare_cached(
           "SELECT id, descricao FROM id_search WHERE id_search MATCH ?1 LIMIT ?2;",
       )?;
   ...
   pub fn search_by_cnpj_basico(conn: &Connection, cnpj_basico: &str, limit: usize) -> rusqlite::Result<Vec<String>> {
       let match_pattern = format!("PJ_{}*", cnpj_basico.trim());
       let mut stmt = conn.prepare_cached(
           "SELECT DISTINCT id FROM id_search WHERE id MATCH ?1 LIMIT ?2;",
       )?;
   ```
   Executing queries with punctuation or reserved characters raises unhandled SQLite errors:
   - Formatted CNPJ `11.223.344`: `ERROR: fts5: syntax error near "."`
   - Trade name with hyphen `ALFA-TECH`: `ERROR: no such column: TECH`
   - Unbalanced quote `"unclosed`: `ERROR: unterminated string`

4. **Independent Verification Execution Results**:
   - `cargo check --all-targets`: Passed (0 errors, 0 warnings).
   - `cargo clippy --all-targets -- -D warnings`: Passed (0 warnings).
   - `cargo test --lib --all-targets` in `rede_cria_tabelas`: 105 passed; 0 failed (47 lib tests, 13 memory tests, 13 transform tests, 32 e2e pipeline tests).
   - `cargo test -p ecd-graph` in `rust-sped-analitycs`: 8 passed; 0 failed (4 BFS tests, 4 SQLite pool tests).
   - `rfb-cnpj-etl build-rede --help`: Exits cleanly with status code 0 and valid documentation.

---

## 2. Logic Chain

1. **Why Finding 1 is Critical**:
   - In Milestone 2, `cnpj_base2matriz` was architected specifically to create a dedicated, indexed `(cnpj_basico, cnpj)` mapping of headquarters.
   - Self-joining `cnpj.estabelecimento tf INNER JOIN cnpj.estabelecimento tm ON tm.cnpj_basico = tf.cnpj_basico AND tm.matriz_filial = '1'` across 40+ million records in production forces SQLite to materialize a large temporary B-tree or scan the establishment index twice for every branch.
   - Because `sqlite_master` in `src/graph/rules.rs:205` was queried without `cnpj.`, the check silently fails every single time. The code falls back to the unoptimized query without raising an error or logging a warning, defeating the primary architectural goal of Milestone 2.

2. **Why Finding 2 Causes Production Pipeline Failures**:
   - Real-world RFB open data is notoriously dirty: there are historical records in `socios` whose matching `estabelecimento` records were purged or omitted.
   - During Milestone 2 ingestion, `socios.cnpj` is populated via `LEFT JOIN cnpj_base2matriz te`. If no match exists, `socios.cnpj` is `NULL`.
   - When Rule 2, 3, or 4 stages this row into `ligacao1`, `destino` is `NULL`.
   - The subsequent `INSERT INTO ligacao ... SELECT ... FROM ligacao1` violates the `id2 TEXT NOT NULL` constraint on `ligacao`, aborting the entire pipeline after hours of processing.

3. **Why Finding 3 Invalidates FTS5 Consumer Contracts**:
   - End-users and analytical UI tools search for companies using formatted CNPJ strings (`11.223.344/0001-91` or `11.223.344`) or corporate trade names with hyphens (`COMERCIO-LTDA`).
   - SQLite FTS5 interprets dots, slashes, hyphens, and boolean operators as internal query syntax. Without escaping or stripping punctuation, search queries fail with runtime errors.

---

## 3. Detailed Findings

### [Critical] Finding 1: Unqualified `sqlite_master` Disables Rule 1 Fast Lookup Table
- **What**: `conn.query_row(...)` checks `sqlite_master` instead of `cnpj.sqlite_master` (or directly querying `cnpj.cnpj_base2matriz`), always returning false.
- **Where**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs:204-208`.
- **Why**: `cnpj_base2matriz` is in the attached database schema `cnpj`. Unqualified `sqlite_master` queries the `main` schema (`rede.db`). As a result, `has_b2m` is always `false`, forcing Rule 1 to execute `DERIVE_FILIAL_MATRIZ_FALLBACK_SQL` (self-join on `estabelecimento`) on every run.
- **Suggestion**: Change the check to inspect `cnpj.sqlite_master` or directly query `cnpj.cnpj_base2matriz`:
  ```rust
  let has_b2m: bool = conn
      .query_row(
          "SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type='table' AND name='cnpj_base2matriz') AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
          [],
          |r| r.get(0),
      )
      .unwrap_or(false);
  ```

### [Major] Finding 2: Unhandled NULL/Empty Matriz IDs Crash Deduplication on Real Data
- **What**: Staging rules allow `NULL` or empty strings to be written into `ligacao1.destino` or `origem`, which subsequently violates `id1 TEXT NOT NULL, id2 TEXT NOT NULL` during deduplication.
- **Where**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs:37-146`.
- **Why**: Real RFB data contains orphan partner entries where `t.cnpj` is `NULL` or empty. When `ligacao1` is grouped and inserted into `ligacao`, SQLite throws `NOT NULL constraint failed: ligacao.id2`.
- **Suggestion**:
  1. Add defensive filters in Rules 2, 3, 4: `AND t.cnpj IS NOT NULL AND t.cnpj <> ''`.
  2. Add defensive filters in Rule 5a, 5c: `AND t.nome_socio IS NOT NULL AND t.nome_socio <> ''`.
  3. In `DEDUPLICATE_LIGACAO_SQL`, add:
     ```sql
     WHERE origem IS NOT NULL AND destino IS NOT NULL
       AND origem <> '' AND destino <> ''
       AND origem <> 'PJ_' AND destino <> 'PJ_'
     ```

### [Major] Finding 3: Unsanitized FTS5 Term Queries Raise SQLite Syntax Errors
- **What**: `search_by_term` and `search_by_cnpj_basico` pass raw user strings directly to FTS5 `MATCH` without escaping.
- **Where**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/search.rs:122-150`.
- **Why**: Formatted CNPJs with dots (`11.223.344`), hyphenated names (`AUTO-POSTO`), and reserved words (`NOT`, `AND`, `OR`) trigger FTS5 syntax errors.
- **Suggestion**:
  1. In `search_by_cnpj_basico`, strip non-digits:
     ```rust
     let digits: String = cnpj_basico.chars().filter(|c| c.is_ascii_digit()).collect();
     let match_pattern = format!("PJ_{digits}*");
     ```
  2. In `search_by_term`, sanitize special characters or wrap token terms in escaped double quotes (e.g., `format!("\"{}\"", term.replace('\"', "\"\""))`).

---

## 4. Adversarial Challenges & Stress-Test Results

### Challenge 1: Fallback Performance Degradation on Full Ingestion
- **Assumption Challenged**: "Rule 1 uses `cnpj_base2matriz` fast lookup table."
- **Attack Scenario**: Running `build-rede` on a complete 37-file dataset with 50M rows in `estabelecimento`.
- **Blast Radius**: Execution time increases drastically due to an unindexed self-join on 50M rows, potentially exhausting temp disk storage for SQLite sort files.
- **Stress-Test Result**: Confirmed. `has_b2m` returned `false` despite `cnpj_base2matriz` existing and containing records.

### Challenge 2: Broken Ingestion on Orphan Partner Records
- **Assumption Challenged**: "Every socio in `socios` has a corresponding valid `matriz` establishment."
- **Attack Scenario**: Insert a socio row with `cnpj_basico = '99999999'` where no establishment exists for `99999999` in `estabelecimento`.
- **Blast Radius**: Aborts pipeline at Stage 3 with `IntegrityError: NOT NULL constraint failed: ligacao.id2`.
- **Stress-Test Result**: Confirmed via Python SQLite simulation.

### Challenge 3: FTS5 Crash on Formatted CNPJ Search
- **Assumption Challenged**: "FTS5 queries safely handle all end-user search tokens."
- **Attack Scenario**: Query `search_by_term(conn, "11.223.344", 10)` or `search_by_cnpj_basico(conn, "11.223.344", 10)`.
- **Blast Radius**: UI search endpoint panics or returns HTTP 500 error on formatted input.
- **Stress-Test Result**: Confirmed (`fts5: syntax error near "."`).

---

## 5. Verified Claims & Downstream Compatibility

| Claim | Method | Result |
| :--- | :--- | :--- |
| `cargo check --all-targets` passes | CLI execution | **PASS** (0.31s) |
| `cargo clippy --all-targets -- -D warnings` | CLI execution | **PASS** (0 warnings) |
| Unit tests in `src/graph/` pass | `cargo test --lib` | **PASS** (11 graph unit tests passed) |
| `e2e_pipeline_test` passes Tier 1-4 | `cargo test --test e2e_pipeline_test` | **PASS** (32 integration tests passed) |
| Downstream `ecd-graph` test suite passes | `cargo test -p ecd-graph` | **PASS** (4 BFS tests, 4 SQLite pool tests passed) |
| Contract schema alignment with `ecd-graph` | Inspected DDLs for `ligacao`, `entidades`, `links` | **PASS** (column types, names, and index structures match) |
| Integrity Check (facades, hardcoded fixtures) | AST & source inspection of `src/graph/*.rs` | **PASS** (no integrity violations found) |

---

## 6. Caveats

- Benchmark testing was executed against synthetic fixture datasets (~50,000 rows). While memory bounding logic (`apply_graph_pragmas` with `temp_store = FILE` and bounded `cache_size`) was verified, absolute I/O throughput on the official 140 GB uncompressed production database was not benchmarked.
- The `links.db` table schema was verified for structural conformance, but actual federal sanction files (PEP, CEIS, CEPIM) are maintained outside the 37 RFB files and are populated via downstream ingestion.

---

## 7. Conclusion

Milestone 3 represents substantial, high-quality engineering: the pipeline structure is clean, concurrency and connection lifecycles avoid database locks, and downstream contracts with `crates/ecd-graph` are preserved. However, the three identified findings—particularly the disabled `cnpj_base2matriz` fast lookup in Rule 1 and the unhandled `NULL` integrity crash on orphan records—must be fixed before production deployment.

**Verdict**: **REQUEST_CHANGES**

---

## 8. Verification Method for Rework Validation

To independently verify the fixes:

1. **Verify Rule 1 Fast Lookup**:
   Add a unit test in `src/graph/rules.rs` that explicitly verifies `DERIVE_FILIAL_MATRIZ_SQL` is selected when `cnpj_base2matriz` is populated in attached `cnpj`:
   ```rust
   let has_b2m: bool = conn.query_row(
       "SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type='table' AND name='cnpj_base2matriz') AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
       [],
       |r| r.get(0),
   ).unwrap_or(false);
   assert!(has_b2m, "Must detect cnpj_base2matriz in attached database");
   ```

2. **Verify NULL Handling**:
   Seed `cnpj.socios` with a partner whose `cnpj` is `NULL` or empty and verify `build-rede` succeeds without `NOT NULL constraint failed`.

3. **Verify FTS5 Sanitization**:
   Execute `search_by_term(&conn, "11.223.344/0001-91", 10)` and `search_by_term(&conn, "ALFA-TECH", 10)` and verify no syntax errors occur.

4. **Run Full Test Suite**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   cargo check --all-targets
   cargo clippy --all-targets -- -D warnings
   cargo test --lib --all-targets
   cargo test --test e2e_pipeline_test
   ```
