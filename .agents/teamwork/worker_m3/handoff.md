# Milestone 3 Handoff Report: Societary Network Graph & FTS5 Search

- **Agent**: `worker_m3` (Role: Rust Societary Graph & FTS5 Engineer)
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m3`
- **Handoff Type**: Hard Handoff (Milestone Complete)
- **Date**: 2026-10-25T01:00:00Z

---

## 1. Observation

1. **Stub Replacement in `src/lib.rs`**:
   Prior to this milestone, `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/lib.rs:14-20` contained an inline stub:
   ```rust
   pub mod graph {
       use crate::cli::BuildRedeArgs;

       pub fn run(_args: BuildRedeArgs) -> anyhow::Result<()> {
           anyhow::bail!("build-rede will be implemented in Milestone 3")
       }
   }
   ```
   This caused integration tests in `tests/e2e_pipeline_test.rs:938-941` to hit `is_milestone_stub` and print:
   `Skipping build-rede execution: feature stubbed pending Milestone 3 completion`.
   This stub was replaced with `pub mod graph;` backed by the newly created module `src/graph/`.

2. **Module Creation in `src/graph/`**:
   The complete graph module was authored under `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/`:
   - `attachment.rs`: Contains the RAII `AttachedDatabase` struct with safe single-quote escaping, `detach()`, `impl Drop` fallback, and `exec_pragma` helper function.
   - `schema.rs`: Contains DDLs for staging table `ligacao1`, final table `ligacao`, entity table `entidades`, GRC sanctions table `links`, and post-load index statements (`idx_ligacao_origem`, `idx_ligacao_destino`, `idx_entidades_tipo`, `idx_links_id1`, `idx_links_id2`).
   - `rules.rs`: Contains the 7 derivation rules (Filial -> Matriz, Corporate Partner PJ -> PJ, Individual Partner PF -> PJ, Foreign Partner PE -> PJ, Legal Representatives 5a-5c), deduplication into `ligacao`, and entity materialization into `entidades`.
   - `search.rs`: Contains `id_search USING fts5 (id, descricao, tokenize = 'unicode61 remove_diacritics 2')`, multi-database attachment population query, segment optimization (`'optimize'`), and search query helpers.
   - `mod.rs`: Contains `GraphOrchestrator`, `run(args: BuildRedeArgs) -> Result<GraphPipelineStats>`, terminal UI progress reporting via `indicatif::MultiProgress`, and sequential two-connection lifecycle management.

3. **PRAGMA Execution Quirk**:
   Executing PRAGMAs that produce result rows (such as `PRAGMA journal_mode = OFF;` which returns a 1-column `off` string) via `conn.execute_batch(...)` caused rusqlite to return:
   `Execute returned results - did you mean to call query?`
   This was resolved in `attachment.rs:64-71` by implementing `exec_pragma(conn, sql)`, which prepares the statement and exhausts `stmt.query([])` row iteration, matching `src/config.rs:152-157`.

4. **Test Suite Verification Outputs**:
   - `cargo check --all-targets`:
     `Finished dev profile [unoptimized + debuginfo] target(s) in 0.74s` (0 errors, 0 warnings).
   - `cargo test -p rfb-cnpj-etl --lib --all-targets`:
     - `src/lib.rs`: 47 tests passed (including 11 new graph unit tests).
     - `adversarial_memory_bounds_test.rs`: 13 tests passed.
     - `adversarial_transforms_and_quirks_test.rs`: 13 tests passed.
     - `e2e_pipeline_test.rs`: 32 tests passed (including Tier 1 F11, F12, F13, Tier 3, and Tier 4 cold pipeline).
     - Total: 105 passed; 0 failed.
   - `cargo clippy --all-targets -- -D warnings`:
     `Finished dev profile [unoptimized + debuginfo] target(s) in 0.98s` (0 warnings).
   - Downstream `cargo test -p ecd-graph`:
     - 4 BFS traversal tests passed.
     - 4 SQLite pool tests passed.
     - Total: 8 passed; 0 failed.

---

## 2. Logic Chain

1. **Eliminating the 16 GB RAM Bottleneck & "Database Locked" Errors**:
   - The legacy Python script (`rede_cria_tabela_rede.db.py`) required 16 GB of physical RAM because it created the graph in `:memory:` and attempted SQLite `backup()`, hitting `database locked` on detach.
   - By structuring `GraphOrchestrator` to stream writes directly to the disk-backed database files (`output_rede` and `output_search`) with `temp_store = FILE`, memory is strictly bounded by the page cache (`PRAGMA cache_size = -{cache_kib}` allocating 35% of memory budget per DB, ~537 MB each for a 1.5 GB budget).
   - Separating execution into two sequentially scoped connections (`conn_rede` for Stage 2-4, closed before opening `conn_search` for Stage 5-6) guarantees that file locks are completely released between phases, preventing any multi-connection contention.

2. **Graph Derivation Correctness & Exact Directionality**:
   - The 7 derivation rules staged into `ligacao1` match the canonical directionality tested by `e2e_pipeline_test.rs` and consumed by `ecd-graph`:
     - Rule 1 (Filial -> Matriz): `PJ_<filial>` -> `PJ_<matriz>`, joined via `cnpj_base2matriz` with fallback to `estabelecimento` self-join.
     - Rule 2 (Corporate Partner): `PJ_<cnpj_socio>` -> `PJ_<cnpj_empresa>`.
     - Rule 3 (Individual Partner): `PF_<cpf>-<nome>` -> `PJ_<cnpj_empresa>`.
     - Rule 4 (Foreign Partner): `PE_<nome>` -> `PJ_<cnpj_empresa>`.
     - Rule 5 (Legal Representatives): `PF_<rep>-<nome_rep>` -> `PE_<nome>` (5a), `PJ_<cnpj>` (5b), `PF_<cpf>-<nome>` (5c), filtering dummy CPF `***000000**`.
   - Staging table `ligacao1` allows unindexed batch insert, and `CREATE TABLE ligacao AS SELECT ... GROUP BY origem, destino, tipo, base` deduplicates records across partition overlaps.
   - Secondary indices `idx_ligacao_origem (id1)` and `idx_ligacao_destino (id2)` created post-deduplication provide sub-millisecond BFS traversals for `WHERE id1 = ?1 OR id2 = ?1`.

3. **FTS5 Full-Text Search Compatibility**:
   - `rede_search.db` initializes `CREATE VIRTUAL TABLE id_search USING fts5 (id, descricao, tokenize = 'unicode61 remove_diacritics 2')`.
   - Populated from attached `cnpj` and `rede` databases with:
     - Matriz establishments with `razao_social`.
     - All establishments with `nome_fantasia` (including empty strings, enabling prefix lookup by CNPJ radical).
     - Distinct non-PJ IDs from `ligacao`.
   - `INSERT INTO id_search(id_search) VALUES('optimize');` merges FTS segments into a single balanced b-tree.

---

## 3. Caveats

1. **Pre-requisite Tables**:
   - If `cnpj.db` is passed to `build-rede` before `build-cnpj` has ingested data, `GraphOrchestrator::ensure_source_database` automatically initializes the schema tables so the derivation queries succeed on empty tables without throwing `no such table`.
2. **GRC Sanctions Links**:
   - `links.db` (containing sanctions: PEP, CEIS, CEPIM, CNEP, Dívida Ativa) is not part of the standard 37 RFB files. The schema includes the `links (id1, id2, descricao, comentario)` table and indices `idx_links_id1`, `idx_links_id2` in `rede.db` so that if `links.db` is merged or queried, it operates with full contract compatibility.

---

## 4. Conclusion

Milestone 3 (Societary Network Graph & FTS5 Search) is 100% complete and fully verified:
- `build-rede` CLI subcommand is fully functional with options `--cnpj-db`, `--output-rede`, `--output-search`, `--drop-existing`, and `--memory-limit-mb`.
- Generates `rede.db` with `ligacao`, `entidades`, `links` and indices `idx_ligacao_origem`, `idx_ligacao_destino`, `idx_entidades_tipo`.
- Generates `rede_search.db` with `id_search` virtual table using FTS5 and `unicode61 remove_diacritics 2`.
- All unit, integration, and downstream consumer tests pass with 0 errors and 0 clippy warnings.

---

## 5. Verification Method

To independently reproduce and verify this implementation:

1. **Check Compilation**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   cargo check --all-targets
   ```

2. **Run Library & Graph Unit Tests**:
   ```bash
   cargo test -p rfb-cnpj-etl --lib --all-targets
   ```

3. **Run E2E Pipeline Integration Test Suite**:
   ```bash
   cargo test --test e2e_pipeline_test
   ```

4. **Run Linter**:
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```

5. **Run CLI Help & Subcommand**:
   ```bash
   cargo run --bin rfb-cnpj-etl -- build-rede --help
   ```

6. **Verify Downstream Consumer `crates/ecd-graph`**:
   ```bash
   cd /home/pizani/Projects/rust-sped-analitycs
   cargo test -p ecd-graph
   ```

### Invalidation Conditions
- Missing `idx_ligacao_origem` or `idx_ligacao_destino` in `rede.db`.
- FTS5 `id_search` virtual table missing or returning errors on token queries.
- Memory usage exceeding 1,800 MB RSS during large derivation runs.
