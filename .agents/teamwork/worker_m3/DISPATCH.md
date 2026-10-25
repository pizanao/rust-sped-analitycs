# Task Assignment: Milestone 3 — Societary Network Graph & FTS5 Search

- **Agent**: `worker_m3`
- **Role**: Rust Societary Graph & FTS5 Engineer
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m3`
- **Date**: 2026-10-25T00:48:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Project Blueprint**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
3. **Graph Derivation SQL & Schema Specification**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_1/handoff.md`
4. **FTS5 & Attachment Specification**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_2/handoff.md`
5. **Graph Pipeline & Memory Bounding (< 1.8 GB) Specification**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_3/handoff.md`
6. **Consumer Contract**: `/home/pizani/Projects/rust-sped-analitycs/crates/ecd-graph/src/db.rs` and `bfs.rs`
7. **Automated E2E Test Contract**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/e2e_pipeline_test.rs` (especially lines 520–650)

---

## Mandatory Integrity Warning
> DO NOT CHEAT. All implementations must be genuine. DO NOT
> hardcode test results, create dummy/facade implementations, or
> circumvent the intended task. A teamwork_preview_auditor will independently
> verify your work. Integrity violations WILL be detected and your
> work WILL be rejected.

---

## Write Ownership Boundaries
You have **exclusive write ownership** of:
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/**`
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/lib.rs`
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/cli.rs` (if minor tweaks are required)

DO NOT modify `src/ingestion/**`, `src/downloader/**`, or any tests without explicit necessity.

---

## Detailed Task Requirements

Implement the complete native Rust `build-rede` pipeline in `src/graph/`:

### 1. Attachment & PRAGMA Management (`src/graph/attachment.rs`)
- Implement RAII guard `AttachedDatabase`:
  - `attach(conn: &Connection, db_path: &Path, schema_alias: &str) -> anyhow::Result<AttachedDatabase>`
  - Executes `ATTACH DATABASE ?1 AS ?2` safely.
  - Implement `detach(mut self, conn: &Connection) -> anyhow::Result<()>` and `Drop` fallback.
  - Crucial: Ensure transactions are committed and prepared statements dropped BEFORE calling `DETACH DATABASE`, avoiding SQLite `database locked` errors.
- Implement high-performance PRAGMAs:
  - Cache size budgeted to stay strictly under 1.8 GB RAM (e.g., `-100000` or ~400MB per connection).
  - `temp_store = MEMORY` or `FILE` depending on budget.
  - Post-build switch to `PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;`.

### 2. Schema & DDL (`src/graph/schema.rs`)
- `dados-publicos/rede.db`:
  - Staging table `ligacao1 (origem TEXT, destino TEXT, tipo TEXT, base TEXT)`
  - Final table `ligacao (id1 TEXT NOT NULL, id2 TEXT NOT NULL, descricao TEXT NOT NULL DEFAULT 'SOCIO', comentario TEXT NOT NULL DEFAULT '')`
  - Entity table `entidades (id TEXT PRIMARY KEY, nome TEXT NOT NULL, tipo TEXT NOT NULL DEFAULT 'PJ')`
  - Indices created AFTER loading:
    - `CREATE INDEX idx_ligacao_origem ON ligacao (id1);`
    - `CREATE INDEX idx_ligacao_destino ON ligacao (id2);`
    - `CREATE INDEX idx_entidades_tipo ON entidades (tipo);`
  - Note: ecd-graph consumer queries `SELECT id1, id2, descricao, comentario FROM ligacao WHERE id1 = ?1 OR id2 = ?1`.

### 3. Graph Derivation Rules (`src/graph/rules.rs`)
Implement the exact 7 derivation stages defined in `explorer_m3_1/handoff.md`:
- **Stage 1 (Filial -> Matriz)**: Join `estabelecimento` (where `matriz_filial = '2'`) with `cnpj_base2matriz` on `cnpj_basico`.
  - Edge: `id1 = 'PJ_' || tf.cnpj`, `id2 = 'PJ_' || tm.cnpj`, `tipo = 'filial'`, `base = 'estabelecimento'`.
- **Stage 2 (Empresa -> Sócio PJ)**: Join `socios` (where `length(cnpj_cpf_socio) = 14`) with `qualificacao_socio`.
  - Edge: `id1 = 'PJ_' || t.cnpj_cpf_socio`, `id2 = 'PJ_' || t.cnpj`, `tipo = coalesce(tq.descricao, 'Sócio')`, `base = 'socios'`.
- **Stage 3 (Empresa -> Sócio PF)**: Sócio PF (`length(cnpj_cpf_socio) = 11`).
  - Edge: `id1 = 'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio`, `id2 = 'PJ_' || t.cnpj`, `tipo = coalesce(tq.descricao, 'Sócio')`, `base = 'socios'`.
- **Stage 4 (Empresa -> Sócio Estrangeiro)**: Foreign partner (`cnpj_cpf_socio IS NULL OR cnpj_cpf_socio = ''`).
  - Edge: `id1 = 'PE_' || t.nome_socio`, `id2 = 'PJ_' || t.cnpj`, `tipo = coalesce(tq.descricao, 'Sócio')`, `base = 'socios'`.
- **Stage 5 (Sócio -> Representante Legal)**:
  - 5a: `PF_<rep>` -> `PE_<estrangeiro>` (`tipo = 'rep-sócio-' || qualificacao`)
  - 5b: `PF_<rep>` -> `PJ_<socio_pj>`
  - 5c: `PF_<rep>` -> `PF_<socio_pf>`
- **Deduplication into `ligacao`**:
  ```sql
  INSERT INTO ligacao (id1, id2, descricao, comentario)
  SELECT origem, destino, tipo, base
  FROM ligacao1
  GROUP BY origem, destino, tipo, base;
  DROP TABLE ligacao1;
  ```
- **Populate `entidades`**:
  - PJ: Matrizes and filiais with `razao_social` or `nome_fantasia`.
  - PF & PE: Distinct non-PJ nodes present in `ligacao`.

### 4. FTS5 Search Database (`src/graph/search.rs`)
- `dados-publicos/rede_search.db`:
  - DDL: `CREATE VIRTUAL TABLE IF NOT EXISTS id_search USING fts5 (id, descricao, tokenize = 'unicode61 remove_diacritics 2');`
  - Populate from `cnpj.db` and `rede.db`:
    - Matrizes: `id = 'PJ_' || te.cnpj`, `descricao = t.razao_social`
    - Filiais / Establishments: `id = 'PJ_' || te.cnpj`, `descricao = coalesce(te.nome_fantasia, '')` (do NOT filter out empty trade names, to support `busca_cnpj(cnpj_basico)`)
    - Non-PJ IDs: Distinct `PF_` and `PE_` from `rede.ligacao` (using `replace(replace(id, 'PF_', ''), 'PE_', '')`)
  - Run `INSERT INTO id_search(id_search) VALUES('optimize');` for FTS index compression and b-tree flattening.

### 5. Orchestrator Pipeline (`src/graph/mod.rs`)
- Expose `pub fn run(args: BuildRedeArgs) -> anyhow::Result<GraphBuildSummary>`
- Two-phase sequential execution:
  1. Derive `rede.db`: Open `rede.db`, attach `cnpj.db`, execute derivation & deduplication, create indices, detach `cnpj.db`, checkpoint WAL, close.
  2. Derive `rede_search.db`: Open `rede_search.db`, attach `cnpj.db` and `rede.db` in read-only mode, create virtual table, populate, detach, optimize, checkpoint WAL, close.
- Support `--drop-existing` flag to clean old DB files if requested.
- Print clear progress indications (stages, durations, edge counts).
- Ensure RSS memory stays strictly < 1.8 GB RAM.

---

## Verification & Acceptance Commands
Execute the following verification commands and record output in your handoff report:
1. `cargo check --all-targets`
2. `cargo test -p rfb-cnpj-etl --lib --all-targets`
3. `cargo test --test e2e_pipeline_test -- test_tier1_rede`
4. `cargo test --test e2e_pipeline_test` (full suite!)
5. `cargo clippy --all-targets -- -D warnings`

Write a comprehensive `handoff.md` to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m3/handoff.md`.
Notify orchestrator when done via `send_message`.
