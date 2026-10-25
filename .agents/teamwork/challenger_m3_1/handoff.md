# Milestone 3 Challenger Handoff Report: Graph Traversal & Directionality

- **Agent**: `challenger_m3_1` (Role: Graph Traversal & Directionality Challenger)
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_1`
- **Handoff Type**: Hard Handoff
- **Date**: 2026-10-25T01:30:00Z
- **Verdict**: **REQUEST_CHANGES**

---

## 1. Observation

1. **Defect in `src/graph/rules.rs:201-216` (`has_b2m` detection check)**:
   In `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs:201-216`:
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
   Direct empirical test execution via `cargo test --test adversarial_graph_traversal_test -- --nocapture`:
   ```text
   EXISTS in sqlite_master (main): false
   EXISTS in cnpj.sqlite_master: true
   Current rules.rs:205 query result: Ok(false)
   Fixed query with cnpj.sqlite_master result: Ok(true)
   ```
   Because `sqlite_master` unqualified queries the `main` schema (which is `rede.db`), and `cnpj_base2matriz` is located in the attached database `cnpj`, `has_b2m` evaluates to `false` even when `cnpj.cnpj_base2matriz` exists and is populated. As a result, `DERIVE_FILIAL_MATRIZ_SQL` is permanently dead code in production, and `derive_filial_matriz` always drops into the fallback unindexed self-join `DERIVE_FILIAL_MATRIZ_FALLBACK_SQL`.

2. **Index Usage Verification & SQLite Query Plan**:
   Executing `EXPLAIN QUERY PLAN SELECT id1, id2, descricao, comentario FROM ligacao WHERE id1 = ?1 OR id2 = ?1;` with parameters in `test_adversarial_query_plan_for_bidirectional_bfs` produced:
   ```text
   PLAN: (4, 0, 0, "MULTI-INDEX OR")
   PLAN: (5, 4, 0, "INDEX 1")
   PLAN: (13, 5, 0, "SEARCH ligacao USING INDEX idx_ligacao_origem (id1=?)")
   PLAN: (18, 4, 0, "INDEX 2")
   PLAN: (26, 18, 0, "SEARCH ligacao USING INDEX idx_ligacao_destino (id2=?)")
   ```
   Both `idx_ligacao_origem (id1)` and `idx_ligacao_destino (id2)` are actively utilized via SQLite's `MULTI-INDEX OR` optimization.

3. **Directionality & Canonical Edge Verification**:
   Empirical testing of all 7 derivation rules in `test_adversarial_all_7_rules_directionality_and_labels` verified:
   - Rule 1 (Filial -> Matriz): `PJ_<filial>` -> `PJ_<matriz>`, descricao: `'filial'`, comentario: `'estabelecimento'`.
   - Rule 2 (Corporate Partner): `PJ_<cnpj_socio>` -> `PJ_<cnpj_empresa>`, descricao: `'Sócio'`, comentario: `'socios'`.
   - Rule 3 (Individual Partner): `PF_<cpf>-<nome>` -> `PJ_<cnpj_empresa>`, descricao: `'Sócio-Administrador'`, comentario: `'socios'`.
   - Rule 4 (Foreign Partner): `PE_<nome>` -> `PJ_<cnpj_empresa>`, descricao: `'Sócio'`, comentario: `'socios'`.
   - Rule 5a (Legal Rep of Foreign Partner): `PF_<rep>-<nome_rep>` -> `PE_<nome>`, descricao: `'rep-sócio-Administrador'`.
   - Rule 5b (Legal Rep of Corporate Partner): `PF_<rep>-<nome_rep>` -> `PJ_<socio>`, descricao: `'rep-sócio-Administrador'`.
   - Rule 5c (Legal Rep of Individual Partner): `PF_<rep>-<nome_rep>` -> `PF_<socio>-<nome>`, descricao: `'rep-sócio-Administrador'`.

4. **Circular & Reciprocal Link Handling**:
   In `test_adversarial_circular_and_self_referencing_links`:
   - An establishment with `matriz_filial = '2'` matching its own CNPJ produces 0 self-referencing filial edges (`tf.cnpj <> tm.cnpj` works).
   - Reciprocal corporate partnerships (Company A in Company B, and Company B in Company A) produce exactly 2 distinct directed edges (`PJ_A -> PJ_B` and `PJ_B -> PJ_A`).
   - Duplicate source rows in `socios` are deduplicated into exactly 1 edge by `GROUP BY origem, destino, tipo, base`.

5. **Empty Database & Zero-Byte Input Resilience**:
   In `test_adversarial_empty_cnpj_database_pipeline` and `test_adversarial_zero_byte_cnpj_file`:
   - Running `build-rede` on an empty `cnpj.db` (0 rows) or uninitialized 0-byte file completes without errors, producing `rede.db` with 0 edges and 0 entities, and `rede_search.db` with 0 search entries.
   - Missing required table (e.g., `qualificacao_socio`) returns a clean `Result::Err` rather than panicking.

6. **Downstream Consumer `crates/ecd-graph`**:
   `cargo test -p ecd-graph` executes 8 tests (4 BFS tests, 4 SQLite pool tests) with 0 failures:
   ```text
   test result: ok. 4 passed; 0 failed; finished in 0.43s
   test result: ok. 4 passed; 0 failed; finished in 0.02s
   ```

---

## 2. Logic Chain

1. **Materialized Index Bypassing (Observation 1)**:
   - Milestone 2 specifically introduced the materialized lookup table `cnpj_base2matriz` to accelerate headquarter lookups for establishments without scanning the 50M-row `estabelecimento` table.
   - `rules.rs:205` was designed to check if `cnpj_base2matriz` is available and has rows before selecting `DERIVE_FILIAL_MATRIZ_SQL`.
   - However, because the SQL query executes `SELECT ... FROM sqlite_master WHERE name='cnpj_base2matriz'`, it checks the `main` schema (`rede.db`).
   - In `rede.db`, `cnpj_base2matriz` is not present; it resides exclusively in `cnpj.sqlite_master`.
   - Thus, the first conjunct in `SELECT EXISTS(...) AND ...` always returns `0` (false), causing `has_b2m` to always be `false`.
   - This causes `derive_filial_matriz` to always select `DERIVE_FILIAL_MATRIZ_FALLBACK_SQL`, triggering an unindexed self-join on `estabelecimento` that incurs high CPU and I/O costs on production datasets.

2. **Required Mitigation**:
   In `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs:205`:
   Change:
   ```rust
   let has_b2m: bool = conn
       .query_row(
           "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cnpj_base2matriz') AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
           [],
           |r| r.get(0),
       )
       .unwrap_or(false);
   ```
   To:
   ```rust
   let has_b2m: bool = conn
       .query_row(
           "SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type='table' AND name='cnpj_base2matriz') AND EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
           [],
           |r| r.get(0),
       )
       .unwrap_or(false);
   ```
   Empirically confirmed: with `cnpj.sqlite_master`, when `cnpj_base2matriz` exists and is populated, it returns `Ok(true)`; when it is missing or empty, it returns `false` and smoothly falls back.

3. **Overall Architectural Health**:
   Aside from this specific query defect in `rules.rs:205`, the graph derivation logic, deduplication, index creation, FTS5 tokenization, and downstream consumer contracts are robust and fully functional.

---

## 3. Caveats

1. **Large Production Scale**: Tests were verified empirically using synthetic datasets and isolated schema tests up to several thousand rows; a full 50M-row RFB ingestion was not run during this challenge turn.
2. **Implementation Code Boundary**: Per challenger review-only constraints, `src/graph/rules.rs` was not modified directly; the fix must be applied by a worker or editor agent.
3. **Peer Test Suite**: `tests/adversarial_memory_bounds_test.rs` authored concurrently by another agent contains minor column name typos (`identificador_socio` instead of `identificador_de_socio`) which was not modified here per isolation boundaries.

---

## 4. Conclusion

**Verdict: REQUEST_CHANGES**

Milestone 3 is mechanically sound and functionally compliant across schema, indices, directionality, deduplication, and search. However, a defect in `src/graph/rules.rs:205` prevents the fast `cnpj_base2matriz` lookup path from ever executing, defeating a key optimization designed in Milestone 2. 

Once `src/graph/rules.rs:205` is updated to check `cnpj.sqlite_master` instead of `sqlite_master`, Milestone 3 should be approved.

---

## 5. Verification Method

1. **Run Challenger Adversarial Suite**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   cargo test --test adversarial_graph_traversal_test -- --nocapture
   ```
   Inspect stdout for:
   `[EMPIRICAL FINDING 1] rules.rs:205 has_b2m detected: false`
   `[EMPIRICAL FINDING 1] Corrected cnpj.sqlite_master detected: true`

2. **Verify Downstream Consumer BFS Tests**:
   ```bash
   cd /home/pizani/Projects/rust-sped-analitycs
   cargo test -p ecd-graph
   ```

3. **Invalidation Condition**:
   - If `rules.rs:205` is not changed to `cnpj.sqlite_master`, any run of `build-rede` against a populated `cnpj.db` will log:
     `Rule 1 (Filial -> Matriz) completed`
     having run the fallback self-join rather than the indexed `cnpj_base2matriz` join.
