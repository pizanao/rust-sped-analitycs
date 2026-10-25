# Review Assignment: Milestone 3 (Iteration 2) — Systems & Schema Reviewer

- **Agent**: `reviewer_m3_r2_1`
- **Role**: Systems & Schema Reviewer
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_r2_1`
- **Date**: 2026-10-25T01:35:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Worker Handoff Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m3_r2/handoff.md`
3. **Modified Source Files**:
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/search.rs`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/attachment.rs`

---

## Scope & Review Checklist
Verify the Iteration 2 remediations:
1. **Schema Qualification in `derive_filial_matriz`**:
   - Verify that `has_b2m` correctly queries `cnpj.sqlite_master` first, then checks `cnpj.cnpj_base2matriz` via short-circuit `&&`.
   - Verify that `DERIVE_FILIAL_MATRIZ_SQL` is selected when `cnpj_base2matriz` is populated.
   - Verify that if `cnpj_base2matriz` is missing or empty, it cleanly selects fallback without SQLite preparation errors.
2. **Constraint Integrity & NULL Guards**:
   - Verify all derivation rules (Rules 1-5) and deduplication in `rules.rs` prevent `NULL` values and bare prefix tokens (`'PJ_'`, `'PE_'`).
3. **Verification**:
   - Run `cargo check --all-targets`
   - Run `cargo test -p rfb-cnpj-etl --lib --all-targets`
   - Run `cargo clippy --all-targets -- -D warnings`

Provide a clear verdict: **APPROVE** or **REQUEST_CHANGES** in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_r2_1/handoff.md`.
Notify orchestrator when done via `send_message`.


## 2026-10-25T01:34:59Z
You are reviewer_m3_r2_1, assigned as Systems & Schema Reviewer for Milestone 3 (Iteration 2) in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_r2_1.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_r2_1/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

Review the worker's remediation, execute verification commands, write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_r2_1/handoff.md with a clear verdict (APPROVE or REQUEST_CHANGES), and send a message back to the orchestrator.
