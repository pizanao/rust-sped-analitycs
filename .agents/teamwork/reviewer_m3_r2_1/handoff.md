# Handoff Report — Systems & Schema Review: Milestone 3 (Iteration 2)

- **Agent**: `reviewer_m3_r2_1`
- **Role**: Systems & Schema Reviewer / Adversarial Critic
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_r2_1`
- **Handoff Type**: Hard Handoff
- **Date**: 2026-10-25T01:42:00Z
- **Verdict**: **APPROVE**

---

## Review Summary

**Verdict**: **APPROVE**
The Iteration 2 remediations performed by `worker_m3_r2` thoroughly resolve all architectural, schema qualification, constraint integrity, and query sanitization defects flagged in Iteration 1. Independent execution of the entire test suite confirms 129 passing tests in `rfb-cnpj-etl` and 8 passing tests in `ecd-graph` with zero clippy warnings and zero integrity violations.

---

## 1. Observation

### 1.1 Remediation of Schema Qualification in `derive_filial_matriz`
In `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs` (lines 240-264):
```rust
pub fn derive_filial_matriz(conn: &Connection) -> Result<u64> {
    // Check if cnpj.cnpj_base2matriz table exists and has rows.
    // Querying table presence in cnpj.sqlite_master first prevents SQLite statement
    // preparation errors (`no such table: cnpj.cnpj_base2matriz`) when the table does not exist.
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

    let sql = if has_b2m {
        DERIVE_FILIAL_MATRIZ_SQL
    } else {
        DERIVE_FILIAL_MATRIZ_FALLBACK_SQL
    };
...
```
1. Directly queries `cnpj.sqlite_master` targeting attached database `cnpj`.
2. Employs Rust short-circuit `&&`: if `cnpj.sqlite_master` returns `false` (or fails with `unwrap_or(false)`), the second query (`SELECT EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);`) is NEVER prepared or executed, eliminating SQLite `no such table` preparation errors.
3. If `cnpj.cnpj_base2matriz` exists and contains rows, `has_b2m` cleanly evaluates to `true`, selecting `DERIVE_FILIAL_MATRIZ_SQL`.

### 1.2 Constraint Integrity and NULL Guards Across Rules 1-5 and Deduplication
In `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs`:
- **Rule 1 (lines 12-25, 28-41)**:
  `WHERE tf.matriz_filial = '2' AND tf.cnpj IS NOT NULL AND length(tf.cnpj) = 14 AND tm.cnpj IS NOT NULL AND length(tm.cnpj) = 14 AND tf.cnpj <> tm.cnpj;`
  Enforces 14-digit CNPJ strings for both filial and matriz, eliminating NULLs, bare prefixes `'PJ_'`, and preventing self-loops.
- **Rule 2 (lines 43-56)**:
  `WHERE length(t.cnpj_cpf_socio) = 14 AND t.cnpj IS NOT NULL AND length(t.cnpj) = 14;`
  Ensures partner records missing establishment CNPJ cannot insert NULL or bare `'PJ_'` into destination.
- **Rule 3 (lines 58-73)**:
  `WHERE length(t.cnpj_cpf_socio) = 11 AND t.nome_socio IS NOT NULL AND trim(t.nome_socio) <> '' AND t.cnpj IS NOT NULL AND length(t.cnpj) = 14;`
  Guards individual partner strings against NULL or whitespace-only names.
- **Rule 4 (lines 75-90)**:
  `WHERE (t.cnpj_cpf_socio = '' OR t.cnpj_cpf_socio IS NULL) AND t.nome_socio IS NOT NULL AND trim(t.nome_socio) <> '' AND t.cnpj IS NOT NULL AND length(t.cnpj) = 14;`
  Guards foreign partner names and ensures valid destination CNPJ.
- **Rule 5a-5c (lines 92-148)**:
  Filters dummy representative CPF `'***000000**'`, verifies non-empty `representante_legal` and `nome_representante`, and verifies `t.nome_socio` is not null or empty.
- **Deduplication `DEDUPLICATE_LIGACAO_SQL` (lines 151-175)**:
  ```sql
  WHERE origem IS NOT NULL
    AND destino IS NOT NULL
    AND trim(origem) <> ''
    AND trim(destino) <> ''
    AND origem NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
    AND destino NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
  GROUP BY origem, destino, tipo, base;
  ```
  Provides defense-in-depth protection ensuring `ligacao`'s `NOT NULL` constraint (`id1 TEXT NOT NULL, id2 TEXT NOT NULL`) cannot fail.

### 1.3 FTS5 Input Sanitization
In `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/search.rs` (lines 130-185):
- `sanitize_fts5_term` encloses individual word tokens in double quotes (`"word"`), escaping FTS5 operator keywords (`AND`, `OR`, `NOT`, `NEAR`).
- Detects formatted CNPJ inputs (`11.223.344/0001-91` -> `"11223344000191"`; `11.223.344` -> `"11223344"*`).
- Escapes internal double quotes (`replace('"', "\"\"")`).
- Safely handles prefix wildcards (`"TOKEN"*`) while stripping multiple or leading asterisks (e.g., `PJ_11111111**`, `***123456**`).

### 1.4 Verification Outputs
1. `cargo check --all-targets`: Finished dev profile in 0.27s (Exit code: 0).
2. `cargo test -p rfb-cnpj-etl --all-targets`:
   - `src/lib.rs`: 50 passed, 0 failed
   - `tests/adversarial_memory_bounds_test.rs`: 13 passed, 0 failed
   - `tests/adversarial_transforms_and_quirks_test.rs`: 13 passed, 0 failed
   - `tests/e2e_pipeline_test.rs`: 32 passed, 0 failed
3. `cargo test --test adversarial_graph_traversal_test`: 8 passed, 0 failed
4. `cargo test --test adversarial_stress_test`: 13 passed, 0 failed
5. `cargo clippy --all-targets -- -D warnings`: Finished dev profile in 0.26s — 0 warnings, 0 errors.
6. `cargo test -p ecd-graph` in `/home/pizani/Projects/rust-sped-analitycs`:
   - `tests/bfs_traversal_test.rs`: 4 passed, 0 failed
   - `tests/sqlite_pool_test.rs`: 4 passed, 0 failed

---

## 2. Logic Chain

1. **Schema Check Correctness**:
   - Observation 1.1 establishes that `cnpj.sqlite_master` is queried for `type IN ('table', 'view') AND name='cnpj_base2matriz'`.
   - In SQLite, attached databases maintain their own master catalog. Querying `cnpj.sqlite_master` accurately detects the table within the attached schema, resolving the bug from Iteration 1 where unqualified `sqlite_master` queried `main` (`rede.db`).
   - The short-circuit `&&` guarantees that if `cnpj_base2matriz` does not exist, SQLite never attempts to prepare the row-count query `SELECT EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);`. This guarantees that no `OperationalError: no such table` can ever be thrown.
   - When the table is populated, `has_b2m` evaluates to `true` and selects `DERIVE_FILIAL_MATRIZ_SQL`. When empty or absent, it falls back to `DERIVE_FILIAL_MATRIZ_FALLBACK_SQL`.

2. **Constraint Integrity**:
   - Observation 1.2 demonstrates that every query generating edges (`ligacao1`) filters on length and non-null values.
   - Observation 1.2 demonstrates that `DEDUPLICATE_LIGACAO_SQL` explicitly filters `origem IS NOT NULL AND destino IS NOT NULL AND trim(...) <> ''` and excludes bare prefix tokens (`'PJ_'`, `'PF_'`, `'PE_'`, `'PF_-'`).
   - Therefore, `ligacao.id1 NOT NULL` and `ligacao.id2 NOT NULL` constraints can never be violated even when ingesting dirty, corrupted, or incomplete RFB open data.

3. **Query Safety & Resilience**:
   - Observation 1.3 demonstrates that input strings with hyphens, slashes, unclosed quotes, and trailing operators are normalized into safely-quoted FTS5 tokens or return empty strings.
   - Therefore, runtime invocations of `search_by_term` and `search_by_cnpj_basico` will not crash with SQLite syntax errors.

4. **Integrity Validation**:
   - Observations 1.1, 1.2, 1.3, and 1.4 confirm that implementations contain genuine SQLite SQL joins, dynamic Rust checks, real FTS5 token sanitization, and full integration test passes.
   - No hardcoded test values, dummy facades, task bypasses, or fabricated outputs were found.

---

## 3. Caveats

- **External Database Attachment**: Attachment of external databases relies on `ATTACH DATABASE` SQL statements handled by RAII guard `AttachedDatabase`. SQLite limits the number of attached databases to 10 (default) or 125 (max). The pipeline only attaches 2 databases (`cnpj` and `rede`), well within SQLite limits.
- **Foreign Radicals**: Partners that are foreign companies or individuals not registered in `estabelecimento` will not link to non-existent corporate nodes. This is correct behavior per graph semantics.

---

## 4. Conclusion

The Iteration 2 remediations for Milestone 3 meet all functional, performance, schema integrity, and architectural requirements. All 129 crate tests and 8 consumer tests pass without warning. The code is clean, robust, and ready for production deployment.

**Verdict: APPROVE**

---

## 5. Verification Method

### 5.1 Independent Commands Executed
To replicate verification independently, run the following commands:

```bash
# 1. Check compilation and targets
cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
cargo check --all-targets

# 2. Run unit and integration tests
cargo test -p rfb-cnpj-etl --all-targets
cargo test --test adversarial_graph_traversal_test
cargo test --test adversarial_stress_test

# 3. Verify zero clippy warnings
cargo clippy --all-targets -- -D warnings

# 4. Verify downstream consumer compatibility
cd /home/pizani/Projects/rust-sped-analitycs
cargo test -p ecd-graph
```

### 5.2 Invalidation Conditions
- Any occurrence of `sqlite3.IntegrityError: NOT NULL constraint failed: ligacao.id2` during `rede.db` generation.
- Any occurrence of `OperationalError: no such table: cnpj.cnpj_base2matriz` during `derive_filial_matriz`.
- Any SQLite syntax error emitted from `search_by_term` when querying punctuation, hyphens, formatted CNPJs, or boolean operators.
