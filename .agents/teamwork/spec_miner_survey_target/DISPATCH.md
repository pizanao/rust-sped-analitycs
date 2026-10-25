# DISPATCH — spec_miner_survey_target

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/spec_miner_survey_target
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Target to Investigate: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Mission
Investigate the existing reference codebase in `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`.
Extract:
1. Exact list of tables and views created for `cnpj.db` and `rede.db`.
2. Exact schemas: column names, data types, nullability, primary keys, indices, and FTS tables.
3. Transformation logic: how `rede.db` (especially `ligacao`) is derived from `cnpj.db` (societary edges, roles, percentages, bidirectionality).
4. Edge cases, data cleaning, encodings, and quirks handled by the existing scripts.
5. Produce a comprehensive report in `handoff.md`.

## 2026-10-24T19:39:41Z
Received message from orchestrator_1 (0ebc4e6f-ccdc-4481-a769-92fd7f6528cc):
"You are the Specification Investigator for the RFB CNPJ open data reconstruction project.
Your working directory is: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/spec_miner_survey_target.
Read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md and /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/spec_miner_survey_target/DISPATCH.md.

Your mission:
Investigate /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Examine all scripts, SQL definitions, python files, docs, and configurations to extract:
1. Exact tables and views created for `cnpj.db` and `rede.db`.
2. Exact schemas: column names, data types, nullability, primary keys, indices, and FTS tables.
3. Transformation logic: how `rede.db` (especially `ligacao`) is derived from `cnpj.db` (societary edges, roles, percentages, bidirectionality, queries).
4. Edge cases, data cleaning, encodings, and quirks handled by the existing scripts.
Write your complete findings to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/spec_miner_survey_target/handoff.md and notify me when complete."

