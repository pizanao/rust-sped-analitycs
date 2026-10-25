# Task Assignment: Milestone 3 (Iteration 2) — FTS5 Input Sanitization & Test Alignment Architect

- **Agent**: `explorer_m3_r2_3`
- **Role**: FTS5 Input Sanitization & Test Alignment Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3`
- **Date**: 2026-10-25T01:12:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Gate Failure Status**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/GATE_STATUS.md`
3. **Reviewer 2 Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m3_2/handoff.md` (Section 1.3 on FTS5 syntax errors)
4. **Challenger 1 Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_1/handoff.md` (Section 3 on test suite column names)
5. **Target Sources**:
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/search.rs`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/adversarial_memory_bounds_test.rs`

---

## Defect to Investigate
1. In `src/graph/search.rs`:
   Helper search functions `search_by_term` and `search_by_cnpj_basico` pass raw user terms directly into FTS5 `MATCH ?1`. Punctuation like `.`, `/`, `-` or unmatched quotes triggers SQLite `fts5: syntax error` or `no such column`.
   Formulate a clean, safe input sanitization helper (e.g., stripping dangerous FTS punctuation or phrase-quoting terms) so that formatted CNPJs (`11.223.344/0001-91`) or hyphenated trade names match smoothly.
2. In `tests/adversarial_memory_bounds_test.rs`:
   Check if any test queries use column name `identificador_socio` instead of `identificador_de_socio` or similar quirks, and formulate the fix.

Formulate the exact changes needed in `src/graph/search.rs` and the test files.
Write your handoff report to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/handoff.md` and notify orchestrator when done via `send_message`.


## 2026-10-25T01:09:40Z
You are explorer_m3_r2_3, assigned as FTS5 Input Sanitization & Test Alignment Architect for Milestone 3 (Iteration 2) in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

Analyze FTS5 input sanitization in src/graph/search.rs and test suite column alignments in tests/adversarial_memory_bounds_test.rs, formulate the exact fix strategy, write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/handoff.md, and send a message back to the orchestrator.


## 2026-10-25T01:20:02Z
**Context**: Milestone 3 Iteration 2 Exploration
**Content**: Checking in on your progress regarding FTS5 input sanitization in `src/graph/search.rs` and test suite alignments. Both peer explorers (`explorer_m3_r2_1` and `explorer_m3_r2_2`) have completed their handoffs.
**Action**: Please provide your status update or complete your handoff report in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/handoff.md`.
