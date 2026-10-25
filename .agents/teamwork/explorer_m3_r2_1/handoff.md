# Handoff Report — Schema Qualification & Fast-Path Architecture (Milestone 3 Iteration 2)

- **Agent**: `explorer_m3_r2_1`
- **Role**: Schema Qualification & Fast-Path Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_1`
- **Handoff Type**: Hard Handoff
- **Date**: 2026-10-25T01:25:00Z
- **Verdict**: Analysis Complete — Exact Fix Strategy Formulated

---

## Executive Summary
In `src/graph/rules.rs:201-216`, `derive_filial_matriz` queries `sqlite_master` in `main` (`rede.db`) instead of the attached database schema `cnpj` (`cnpj.sqlite_master`). Because `cnpj_base2matriz` resides in `cnpj.db`, `has_b2m` evaluates to `false` in 100% of executions, making the optimized fast-path query `DERIVE_FILIAL_MATRIZ_SQL` unreachable dead code and forcing an unindexed self-join on 55+ million establishment records. Furthermore, in SQLite's statement preparation model, referencing `cnpj.cnpj_base2matriz` in a compound query when the table does not exist causes `sqlite3_prepare_v2` to fail with `no such table: cnpj.cnpj_base2matriz`.

We have formulated a decoupled, two-step query strategy using Rust's short-circuiting `&&` operator. This guarantees that:
1. Fast-path lookup against `cnpj.cnpj_base2matriz` is accurately detected whenever the table exists and contains records ($O(1)$ check).
2. If `cnpj_base2matriz` does not exist, the second query is never prepared, completely preventing SQLite `no such table` preparation errors.
3. If `cnpj_base2matriz` exists but is empty, it safely and cleanly selects the fallback query.

---

## 1. Observation

1. **Defect in `src/graph/rules.rs` (lines 201–216)**:
   ```rust
   pub fn derive_filial_matriz(conn: &Connection) -> Result<u64> {
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

2. **Database Attachment Topology in `src/graph/mod.rs` (lines 100–106)**:
   ```rust
   let conn_rede = Connection::open(&self.args.output_rede).with_context(|| {
       format!("Failed to open destination database at {:?}", self.args.output_rede)
   })?;
   apply_graph_pragmas(&conn_rede, self.args.memory_limit_mb)?;
   let att_cnpj = AttachedDatabase::attach(&conn_rede, &self.args.cnpj_db, "cnpj")?;
   ```
   - Database connection `conn_rede` has `rede.db` opened as the `main` database schema.
   - Database `cnpj.db` is attached under the schema alias `"cnpj"`.
   - `sqlite_master` unqualified always inspects `main.sqlite_master`.
   - In `main.sqlite_master`, no table named `cnpj_base2matriz` exists; it exists only in `cnpj.sqlite_master`.

3. **Empirical Verification of Unqualified Query**:
   Executing the check against an attached database containing `cnpj.cnpj_base2matriz`:
   ```bash
   python3 -c "
   import sqlite3
   c = sqlite3.connect(':memory:')
   c.execute('ATTACH DATABASE \":memory:\" AS cnpj')
   c.execute('CREATE TABLE cnpj.cnpj_base2matriz (cnpj_basico text)')
   res1 = c.execute(\"SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cnpj_base2matriz')\").fetchone()[0]
   res2 = c.execute(\"SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type='table' AND name='cnpj_base2matriz')\").fetchone()[0]
   print('Unqualified in main:', res1, '| Qualified in cnpj:', res2)
   "
   ```
   Output:
   ```text
   Unqualified in main: 0 | Qualified in cnpj: 1
   ```
   Because `EXISTS(...)` in `sqlite_master` returned `0`, `has_b2m` returned `0` (`false`) in every execution, disabling the fast-path join on `cnpj_base2matriz`.

4. **Empirical Verification of SQLite Query Preparation Failure**:
   When `cnpj_base2matriz` does not exist in `cnpj`, preparing the compound query fails:
   ```bash
   python3 -c "
   import sqlite3
   c = sqlite3.connect(':memory:')
   c.execute('ATTACH DATABASE \":memory:\" AS cnpj')
   try:
       c.execute(\"SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type='table' AND name='cnpj_base2matriz') AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);\")
   except Exception as e:
       print('Error:', type(e), e)
   "
   ```
   Output:
   ```text
   Error: <class 'sqlite3.OperationalError'> no such table: cnpj.cnpj_base2matriz
   ```
   SQLite statements must resolve all table identifiers during AST validation and bytecode preparation (`sqlite3_prepare_v2`). SQLite cannot lazily defer table existence checking inside an `AND` expression.

---

## 2. Logic Chain

1. **Why `has_b2m` Permanently Evaluated to `false`**:
   - In SQLite, every attached database maintains its own system catalog: `main.sqlite_master`, `cnpj.sqlite_master`, `temp.sqlite_master`.
   - Any query written as `FROM sqlite_master` without schema qualification defaults strictly to `main`.
   - `conn_rede` has `output_rede` (`rede.db`) as `main`.
   - `cnpj_base2matriz` was materialized during Milestone 2 in `cnpj.db`.
   - Therefore, `SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cnpj_base2matriz')` always evaluated to `0`.
   - In boolean arithmetic, `0 AND <expr>` always evaluates to `0`.
   - As a direct consequence, `has_b2m` was unconditionally `false`, making `DERIVE_FILIAL_MATRIZ_SQL` dead code.

2. **Performance Blast Radius of Fallback**:
   - `DERIVE_FILIAL_MATRIZ_SQL` joins `estabelecimento tf` against `cnpj_base2matriz tm` on `tm.cnpj_basico = tf.cnpj_basico`. `cnpj_base2matriz` is indexed on its `PRIMARY KEY (cnpj_basico)`. Lookups are $O(1)$ B-Tree searches.
   - `DERIVE_FILIAL_MATRIZ_FALLBACK_SQL` performs an unindexed self-join on `estabelecimento`:
     `FROM cnpj.estabelecimento tf INNER JOIN cnpj.estabelecimento tm ON tm.cnpj_basico = tf.cnpj_basico AND tm.matriz_filial = '1'`.
   - Across 55+ million records in RFB open data, this self-join forces SQLite to scan or index-lookup both sides of the 55M-row table, filter on `matriz_filial = '1'`, and spill large sorting runs to disk, degrading stage runtime and increasing I/O pressure.

3. **Why a Single Compound Query is Inherently Unsafe in SQLite**:
   - If one simply changes `sqlite_master` to `cnpj.sqlite_master` in the same SQL string:
     `SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master ...) AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);`
   - Whenever `cnpj_base2matriz` is absent (such as when running against an older database or unpopulated test fixtures), SQLite's statement compiler rejects the entire query during `prepare` with `no such table: cnpj.cnpj_base2matriz`.
   - While `.unwrap_or(false)` in Rust catches the resulting `rusqlite::Error`, relying on engine-level compilation aborts generates error logs, pollutes tracing, and masks genuine errors (such as corrupted DB files or I/O faults).

4. **Why the Decoupled Two-Step Check via Rust `&&` is Optimal**:
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
   - **Step 1**: Inspects `cnpj.sqlite_master`.
     - `cnpj.sqlite_master` is guaranteed to exist when `cnpj` is attached.
     - `type IN ('table', 'view')` ensures compatibility whether implemented as a table or view.
     - If the table does not exist, it cleanly returns `0` (`false`).
     - Because `&&` in Rust short-circuits on `false`, Step 2 is **never prepared or executed**.
     - Zero SQLite compilation errors are produced.
   - **Step 2**: Only runs if Step 1 confirmed the table exists.
     - Since the table exists, `SELECT EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);` compiles cleanly without error.
     - `LIMIT 1` ensures $O(1)$ execution time.
     - Returns `1` (`true`) if $\ge 1$ row exists; `0` (`false`) if the table is empty.

---

## 3. Caveats

1. **Attachment Alias Dependency**:
   - The query explicitly relies on the schema alias `"cnpj"`. This matches `AttachedDatabase::attach(&conn_rede, &self.args.cnpj_db, "cnpj")` in `src/graph/mod.rs:105`. If the schema alias is ever refactored, the SQL prefix must be updated synchronously.
2. **Read-Only Explorer Scope**:
   - As an explorer agent, no project source code was directly modified. The exact patch file is provided in this directory (`rules_patch.diff`).
3. **Subsystem Isolation**:
   - Related defects (NULL/empty guards on orphan partner records in `rules.rs`, and FTS5 input sanitization in `search.rs`) are scoped to peer architects `explorer_m3_r2_2` and `explorer_m3_r2_3` respectively. The fix proposed here does not conflict with their work.

---

## 4. Conclusion

The defect in `src/graph/rules.rs:205` is completely understood and reproducible. The exact fix strategy requires replacing the compound query with the two-step short-circuiting query in `src/graph/rules.rs:201-216`.

### Proposed Code Change:
In `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs`:

```rust
// Replace lines 201-216 with:
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

    let before = count_staging(conn);
    conn.execute_batch(sql)
        .context("Failed executing Rule 1: Filial -> Matriz links")?;
    let after = count_staging(conn);
    let count = after.saturating_sub(before);
    info!(count = count, "Rule 1 (Filial -> Matriz) completed");
    Ok(count)
}
```

### Proposed Accompanying Unit Test:
Add to `src/graph/rules.rs` under `mod tests`:

```rust
    #[test]
    fn test_rule1_fast_path_with_cnpj_base2matriz() {
        let conn = Connection::open_in_memory().unwrap();
        create_graph_schema(&conn).unwrap();

        conn.execute_batch(
            "ATTACH DATABASE ':memory:' AS cnpj;

             CREATE TABLE cnpj.estabelecimento (
                 cnpj_basico TEXT,
                 matriz_filial TEXT,
                 cnpj TEXT
             );

             CREATE TABLE cnpj.cnpj_base2matriz (
                 cnpj_basico TEXT PRIMARY KEY,
                 cnpj TEXT
             );

             -- Filial in estabelecimento, but Matriz ONLY in cnpj_base2matriz
             INSERT INTO cnpj.estabelecimento VALUES ('44444444', '2', '44444444000288');
             INSERT INTO cnpj.cnpj_base2matriz VALUES ('44444444', '44444444000199');
            ",
        )
        .unwrap();

        // If fast path (DERIVE_FILIAL_MATRIZ_SQL) is used, 1 edge is derived.
        // If fallback (DERIVE_FILIAL_MATRIZ_FALLBACK_SQL) was used, 0 edges would be derived
        // because the matriz is not in estabelecimento.
        let count = derive_filial_matriz(&conn).unwrap();
        assert_eq!(count, 1, "Must execute fast-path join against cnpj_base2matriz");

        deduplicate_ligacao(&conn).unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM ligacao WHERE id1 = 'PJ_44444444000288' AND id2 = 'PJ_44444444000199' AND descricao = 'filial');",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "Derived edge must connect filial to matriz via cnpj_base2matriz");
    }
```

---

## 5. Verification Method

1. **Verify Implementation Compiles and Passes Clippy**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   cargo check --all-targets
   cargo clippy --all-targets -- -D warnings
   ```

2. **Verify Fast-Path and Fallback Execution via Unit Tests**:
   ```bash
   cargo test -p rfb-cnpj-etl --lib graph::rules::tests
   ```
   Must pass all three branch scenarios:
   - `test_rule1_fast_path_with_cnpj_base2matriz`: passes and confirms `count == 1`.
   - `test_rule1_fallback_without_cnpj_base2matriz`: passes without SQLite `no such table` errors.
   - `test_derivation_rules_in_memory`: passes with 5 edges derived.

3. **Verify Adversarial Graph Traversal Suite**:
   ```bash
   cargo test --test adversarial_graph_traversal_test -- --nocapture
   ```

4. **Verify Downstream Consumer `crates/ecd-graph`**:
   ```bash
   cd /home/pizani/Projects/rust-sped-analitycs
   cargo test -p ecd-graph
   ```

### Invalidation Conditions:
- `has_b2m` returning `false` when `cnpj.cnpj_base2matriz` exists and contains rows.
- SQLite preparation error `no such table: cnpj.cnpj_base2matriz` emitted when running against an input database without `cnpj_base2matriz`.
- Fallback self-join being executed when `cnpj_base2matriz` is populated.
