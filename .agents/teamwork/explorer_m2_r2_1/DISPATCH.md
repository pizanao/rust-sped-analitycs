# Dispatch Assignment — explorer_m2_r2_1

- **Role**: Schema & DDL Lifecycle Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Scope**: Investigate remediation for VULN-M2-01 (Missing `socios` table causing Stage 8 crash on empty/partial datasets)
- **Mandatory References**:
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m2_1/handoff.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m2_2/handoff.md`

## Problem Context
During Milestone 2 Gate verification, `reviewer_m2_1` and `challenger_m2_2` reported that running `rfb-cnpj-etl build-cnpj` on an empty directory or any dataset without `Socios*.zip` crashes fatally at Stage 8 (`Post-Load Index Creation`) with `no such table: main.socios`.
Root cause: `create_initial_tables()` in `src/ingestion/schema.rs` creates `socios_original` but does not create `socios`. The DDL for `socios` (`SOCIOS_FINAL_DDL`) is only executed inside `transform_socios_and_expand_radicals()`.

## Your Task
1. Inspect `src/ingestion/schema.rs` and `src/ingestion/loader.rs`.
2. Formulate the exact, robust fix strategy for DDL creation:
   - Ensure all public tables (especially `socios` and `cnpj_base2matriz`) are guaranteed to exist before indexing, even when partitions are missing.
   - Clarify whether `create_initial_tables` should execute `SOCIOS_FINAL_DDL` upfront, or how `socios_original` and `socios` should be managed.
3. Recommend exact code changes with rationale. Do NOT implement the fix.
4. Deliver your handoff report to `handoff.md` and notify parent.

## 2026-10-25T00:09:29Z
You are explorer_m2_r2_1 (Role: Schema & DDL Lifecycle Architect).
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_r2_1.
Read your task description in DISPATCH.md in your working directory.
Also read:
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m2_1/handoff.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m2_2/handoff.md

Investigate schema lifecycle in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/ingestion/schema.rs and loader.rs.
Formulate a robust fix strategy for VULN-M2-01 so that table socios (and cnpj_base2matriz) exist even on empty/partial datasets without crashing stage 8 post-indexing.
Do NOT modify source code.
Deliver your handoff report to handoff.md in your working directory and message parent (orchestrator_1).
