# Dispatch Assignment — explorer_m3_3

- **Role**: Graph Pipeline & Memory Bound Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Scope**: Milestone 3 — Graph Pipeline Orchestration & Memory Bounding (< 1.8 GB)
- **Mandatory References**:
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
  - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/cli.rs`
  - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/config.rs`

## Your Task
1. Inspect `src/cli.rs` (`BuildRedeArgs`) and `src/main.rs` router to define the interface for `rfb-cnpj-etl build-rede`.
2. Formulate the stage-by-stage pipeline for `src/graph/mod.rs`:
   - Stage 1: Validate input `cnpj.db` exists and has required tables.
   - Stage 2: Initialize `rede.db` schema (`ligacao`, `entidades`).
   - Stage 3: Execute the 7 derivation SQL rules sequentially with progress bars (`indicatif`).
   - Stage 4: Create secondary indices (`idx_ligacao_origem`, `idx_ligacao_destino`) post-load.
   - Stage 5: Initialize and populate `rede_search.db` (`id_search` FTS5).
   - Stage 6: Apply production PRAGMAs (`journal_mode = WAL`, `synchronous = NORMAL`, `optimize`).
3. Formulate memory bounding strategy (< 1.8 GB RAM):
   - SQLite cache settings during attached queries.
   - Temp store settings (`PRAGMA temp_store = MEMORY` or `FILE` for large external sorts).
4. Propose code design and CLI integration.
5. Deliver your handoff report to `handoff.md` and message parent (orchestrator_1).

## 2026-10-25T00:35:36Z
You are explorer_m3_3 (Role: Graph Pipeline & Memory Bound Architect).
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_3.
Read your task description in DISPATCH.md in your working directory.
Also read:
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md

Inspect the target project /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Formulate the stage-by-stage pipeline for src/graph/mod.rs, PRAGMAs, memory bounding (< 1.8 GB RAM), indicatif progress bars, and CLI command build-rede.
Do NOT modify source code.
Deliver your handoff report to handoff.md in your working directory and message parent (orchestrator_1).
