# Task Assignment: Milestone 3 (Iteration 2) — Schema Qualification & Fast-Path Architect

- **Agent**: `explorer_m3_r2_1`
- **Role**: Schema Qualification & Fast-Path Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_1`
- **Date**: 2026-10-25T01:12:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Gate Failure Status**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/GATE_STATUS.md`
3. **Reviewer 1 Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_1/handoff.md`
4. **Reviewer 2 Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_2/handoff.md`
5. **Challenger 1 Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_1/handoff.md`
6. **Target Source**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs`

---

## Defect to Investigate
In `src/graph/rules.rs:203-216`:
`derive_filial_matriz` queries `sqlite_master` in `main` (`rede.db`) instead of `cnpj.sqlite_master`.
Because `cnpj_base2matriz` is in attached database `cnpj`, `has_b2m` evaluates to `false` 100% of the time, making `DERIVE_FILIAL_MATRIZ_SQL` dead code.
Also, if `cnpj_base2matriz` does not exist, querying `SELECT 1 FROM cnpj.cnpj_base2matriz` inside the same statement fails during preparation with `no such table`.

Formulate the exact, safest fix for `src/graph/rules.rs:203-216`.
Write your handoff report to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_1/handoff.md` and notify orchestrator when done via `send_message`.


## 2026-10-25T01:09:29Z
You are explorer_m3_r2_1, assigned as Schema Qualification & Fast-Path Architect for Milestone 3 (Iteration 2) in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_1.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_1/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

Analyze the defect in src/graph/rules.rs:205, formulate the exact fix strategy, write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_1/handoff.md, and send a message back to the orchestrator.
