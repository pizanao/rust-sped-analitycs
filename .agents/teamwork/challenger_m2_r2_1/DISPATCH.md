# Dispatch Assignment — challenger_m2_r2_1

- **Role**: Empty Directory & Schema Adversarial Challenger
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Scope**: Adversarial verification of VULN-M2-01 fix across dataset variations
- **Mandatory References**:
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m2_r2/handoff.md`

## Your Task
1. Empirically verify that `rfb-cnpj-etl build-cnpj` runs cleanly and exits with code 0 on:
   - A completely empty directory.
   - A directory with only `Empresas0.zip` (missing Socios, Estabelecimentos, Simples).
   - A directory with only `Socios0.zip` (missing Empresas and Estabelecimentos).
2. For each test case, verify SQLite schema:
   - All 12 tables exist.
   - All 18 indices exist.
   - `socios_original` does NOT exist.
   - `PRAGMA journal_mode` is `wal`.
3. Run verification in target project:
   - `cargo test --all-targets`
4. Deliver your handoff report to `handoff.md` with an explicit verdict: `APPROVE` or `REQUEST_CHANGES`.


## 2026-10-25T00:25:29Z
You are challenger_m2_r2_1 (Role: Empty Directory & Schema Adversarial Challenger).
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m2_r2_1.
Read your task description in DISPATCH.md in your working directory.
Also read:
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m2_r2/handoff.md

Empirically test build-cnpj against an empty directory and partial datasets in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Verify that all 12 tables and 18 indices exist, socios_original is absent, and process exits with 0.
Run cargo test --all-targets.

Deliver your handoff report to handoff.md in your working directory and message your parent (orchestrator_1, Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc) with an explicit verdict: APPROVE or REQUEST_CHANGES.
