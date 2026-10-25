# Challenger Assignment: Milestone 3 (Iteration 2) — FTS5 Search & Punctuation Challenger

- **Agent**: `challenger_m3_r2_2`
- **Role**: FTS5 Search & Punctuation Challenger
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_r2_2`
- **Date**: 2026-10-25T01:35:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Worker Handoff Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m3_r2/handoff.md`
3. **Source & Tests**:
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/search.rs`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/adversarial_memory_bounds_test.rs`

---

## Empirical Challenge Objectives
1. **FTS5 Punctuation & Keyword Stress Test**:
   - Query `search_by_term` and `search_by_cnpj_basico` with:
     - Formatted CNPJ `11.223.344/0001-91` and 8-digit radical `11.223.344`
     - Hyphenated trade name `ALFA-TECH`
     - Dangling operators `MODAS OR`, `AND`, `NOT`
     - Special punctuation `C&A`, `S.A.`, `"unclosed`
     - Asterisks `PJ_11111111**`, `***123456**`
   - Verify that 0 SQLite syntax errors are thrown and valid matches are returned.
2. **Memory Bounds & WAL Verification**:
   - Run memory bounds test in `tests/adversarial_memory_bounds_test.rs` and verify peak RSS remains < 1.8 GB.
3. **Verification**:
   - Run `cargo test -p rfb-cnpj-etl --test adversarial_memory_bounds_test`
   - Run `cargo test -p rfb-cnpj-etl --lib --all-targets`

Record empirical results and verdict (**APPROVE** or **REQUEST_CHANGES**) in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_r2_2/handoff.md`.
Notify orchestrator when done via `send_message`.


## 2026-10-25T01:35:28Z
From: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc
Content: You are challenger_m3_r2_2, assigned as FTS5 Search & Punctuation Challenger for Milestone 3 (Iteration 2) in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_r2_2.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_r2_2/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

Empirically test FTS5 punctuation, formatted CNPJs, keywords, and memory bounds, write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_r2_2/handoff.md with a clear verdict (APPROVE or REQUEST_CHANGES), and send a message back to the orchestrator.
