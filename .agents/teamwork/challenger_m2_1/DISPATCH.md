# Dispatch Assignment — challenger_m2_1

- **Role**: Data Integrity & Edge Case Challenger
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Scope**: Adversarial verification of Milestone 2 (Data transformations, schema integrity, and August 2026 radical resolution)
- **Mandatory References**:
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m2/handoff.md`

## Your Task
1. Empirically verify data transformation correctness:
   - Test `parse_brl_float` against tricky Brazilian float formatting (`"1.234.567,89"`, `"0,00"`, `""`, `"  500,00 "`, negative values, invalid strings).
   - Test `synthesize_cnpj` against edge case padding (`"1"`, `"1"`, `"1"` -> `"00000001000101"`).
   - Test Windows-1252 decoding against Brazilian accented characters (ç, ã, é, ó, etc.).
2. Test August 2026 radical expansion:
   - Pre-2026 PJ partner (14-digit CNPJ already) -> preserves 14 digits.
   - Post-2026 PJ partner (8-digit radical) with matching Matriz in `cnpj_base2matriz` -> correctly expands to Matriz 14-digit CNPJ.
   - Post-2026 PJ partner with NO matching Matriz -> fallback to 8-digit radical via `COALESCE`.
   - PF partner (11-digit CPF with leading asterisks) -> untouched.
   - Foreign partner (identificador 3) -> untouched.
3. Run tests using `cargo test` in `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`.
4. Issue an explicit verdict: `APPROVE` or `REQUEST_CHANGES` in your `handoff.md` and message.


## 2026-10-25T00:00:21Z
You are challenger_m2_1 (Role: Data Integrity & Edge Case Challenger).
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m2_1.
Read your task description in DISPATCH.md in your working directory.
Also read:
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m2/handoff.md

Empirically test data transforms and edge cases (BRL currency formatting, zero-padding 14-char CNPJ, Windows-1252 accented characters, August 2026 PJ partner radical expansion vs fallback).
Target project: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Run cargo test in the target project.

Deliver your handoff report to handoff.md in your working directory and message your parent (orchestrator_1, Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc) with an explicit verdict: APPROVE or REQUEST_CHANGES.
