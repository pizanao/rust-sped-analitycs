# DISPATCH — explorer_survey_sped

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_survey_sped
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Target to Investigate: /home/pizani/Projects/rust-sped-analitycs (crates/ecd-graph, crates/ecd-storage, apps/server, etc.)

## Mission
Investigate how `rust-sped-analytics` interacts with and queries `cnpj.db` and `rede.db`.
Extract:
1. All SQL queries executed against `cnpj.db` and `rede.db`.
2. Tables, views, columns, indices, and data types expected by `rust-sped-analytics`.
3. Graph algorithms (BFS in `crates/ecd-graph`), societary links queries, and public sanctions cross-referencing.
4. Specific performance requirements, index requirements, and schema compatibility constraints.
5. Produce a comprehensive report in `handoff.md`.

## 2026-10-24T19:39:46Z
From: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc
You are the Codebase Researcher investigating rust-sped-analytics expectations for CNPJ and Rede databases.
Your working directory is: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_survey_sped.
Read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md and /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_survey_sped/DISPATCH.md.

Your mission:
Investigate /home/pizani/Projects/rust-sped-analitycs, specifically crates/ecd-graph, crates/ecd-storage, apps/server, and any other relevant crates.
Find:
1. All SQL queries executed against `cnpj.db` and `rede.db`.
2. Exact tables, columns, indices, and schemas expected by rust-sped-analytics.
3. Graph traversal queries (BFS in crates/ecd-graph), societary link queries, sanctions queries (links.db/cnpj.db), and performance expectations.
Write your complete findings to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_survey_sped/handoff.md and notify me when complete.
