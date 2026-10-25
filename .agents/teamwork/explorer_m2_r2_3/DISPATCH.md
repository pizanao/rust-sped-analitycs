# Dispatch Assignment — explorer_m2_r2_3

- **Role**: Test Suite & Date Parsing Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Scope**: Regression test design for empty/partial datasets and validation of date token parsing
- **Mandatory References**:
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m2_1/handoff.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m2_2/handoff.md`

## Problem Context
Reviewer M2-1 reported:
1. Finding 1: Pipeline crash on empty/partial datasets due to missing `socios` table. Needs an automated regression test covering empty directory execution.
2. Finding 2: `parse_rfb_reference_code` in `src/ingestion/transforms.rs` matches any 6-character part starting with `'D'` without checking if `part[1..]` are digits.

## Your Task
1. Inspect `src/ingestion/transforms.rs` around line 184. Formulate the precise check for `parse_rfb_reference_code`.
2. Design a concrete regression test in `tests/e2e_pipeline_test.rs` or `tests/adversarial_stress_test.rs` that runs `rfb-cnpj-etl build-cnpj` against an empty temporary directory and asserts:
   - Command exits with 0 (success).
   - Database is created.
   - All 12 tables exist.
   - All expected indices exist.
   - Staging table `socios_original` does NOT exist.
   - PRAGMA journal_mode is `wal`.
3. Recommend exact code changes and test code. Do NOT implement the fix.
4. Deliver your handoff report to `handoff.md` and notify parent.


## 2026-10-25T00:09:41Z
You are explorer_m2_r2_3 (Role: Test Suite & Date Parsing Architect).
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_r2_3.
Read your task description in DISPATCH.md in your working directory.
Also read:
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m2_1/handoff.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m2_2/handoff.md

Investigate date token validation in src/ingestion/transforms.rs and design automated regression tests for empty/partial input directory execution.
Do NOT modify source code.
Deliver your handoff report to handoff.md in your working directory and message parent (orchestrator_1).
