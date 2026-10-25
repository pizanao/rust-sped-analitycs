# Challenger Assignment: Milestone 3 — Graph Traversal & Directionality Challenger

- **Agent**: `challenger_m3_1`
- **Role**: Graph Traversal & Directionality Challenger
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_1`
- **Date**: 2026-10-25T01:05:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Worker Handoff Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m3/handoff.md`
3. **Graph Source Code**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/**`
4. **Consumer Traversal Implementation**: `/home/pizani/Projects/rust-sped-analitycs/crates/ecd-graph/src/bfs.rs`
5. **E2E Tests**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/e2e_pipeline_test.rs`

---

## Adversarial Challenge Objectives
Empirically stress-test the societary graph generation:
1. **Empty / Incomplete CNPJ Database**:
   - Run `build-rede` when `cnpj.db` exists but has 0 rows in some or all tables (`socios`, `estabelecimento`, `empresas`).
   - Does it succeed gracefully with 0 rows in `ligacao` and `entidades`, or does it fail with SQLite errors?
2. **Graph Directionality & Edge Validation**:
   - Verify every type of link (Filial->Matriz, PJ->PJ, PF->PJ, PE->PJ, PF->PE rep, PF->PJ rep, PF->PF rep).
   - Verify that edge lookup `WHERE id1 = ?1 OR id2 = ?1` returns the expected edges and that indices `idx_ligacao_origem` and `idx_ligacao_destino` are used (`EXPLAIN QUERY PLAN`).
3. **Circular & Self-Referencing Links**:
   - What happens if an establishment has `matriz_filial = '2'` but matches its own CNPJ as matriz?
   - What happens if partner and company have reciprocal links?
   - Verify that deduplication `GROUP BY origem, destino, tipo, base` avoids duplicate edges.
4. **Verification**:
   - Run full test suite: `cargo test --test e2e_pipeline_test`
   - Run unit tests: `cargo test -p rfb-cnpj-etl --lib --all-targets`

Record empirical results and verdict (**APPROVE** or **REQUEST_CHANGES**) in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_1/handoff.md`.
Notify orchestrator when done via `send_message`.

## 2026-10-25T01:00:02Z
[Message from 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc]
You are challenger_m3_1, assigned as Graph Traversal & Directionality Challenger for Milestone 3 (Societary Network Graph & FTS5 Search) in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_1.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_1/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

Empirically test graph derivation, edge directions, index usage, and edge cases, write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_1/handoff.md with a clear verdict (APPROVE or REQUEST_CHANGES), and send a message back to the orchestrator.
