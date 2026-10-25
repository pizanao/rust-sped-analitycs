# Task Assignment: Milestone 3 (Iteration 2) — Constraint Integrity & Orphan Partner Architect

- **Agent**: `explorer_m3_r2_2`
- **Role**: Constraint Integrity & Orphan Partner Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2`
- **Date**: 2026-10-25T01:12:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Gate Failure Status**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/GATE_STATUS.md`
3. **Reviewer 2 Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_2/handoff.md` (Section 1.2 on orphan socio records)
4. **Target Source**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs`

---

## Defect to Investigate
In `src/graph/rules.rs`:
Examine all 7 derivation queries (Rules 1-5) and deduplication into `ligacao`:
If a partner record has `t.cnpj IS NULL` or `trim(t.cnpj) = ''` (e.g. orphan partner row where establishment was not found), `'PJ_' || t.cnpj` evaluates to `NULL` or `'PJ_'`.
Inserting `NULL` into `ligacao.id2` triggers SQLite `IntegrityError: NOT NULL constraint failed: ligacao.id2`.
Also check for `nome_socio IS NULL` in Rule 4 or Rule 5.

Formulate the exact SQL WHERE clauses to guard against NULL / empty IDs across all derivation rules in `src/graph/rules.rs`.
Write your handoff report to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2/handoff.md` and notify orchestrator when done via `send_message`.


## 2026-10-25T01:09:34Z
You are explorer_m3_r2_2, assigned as Constraint Integrity & Orphan Partner Architect for Milestone 3 (Iteration 2) in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

Analyze NULL / empty guard needs across all derivation rules in src/graph/rules.rs, formulate the exact SQL WHERE clause guards, write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2/handoff.md, and send a message back to the orchestrator.
