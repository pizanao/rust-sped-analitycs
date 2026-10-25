# Final Workspace Forensic Audit Assignment

- **Agent**: `auditor_final`
- **Role**: Forensic Integrity Auditor (Workspace-Wide)
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Consumer Codebase**: `/home/pizani/Projects/rust-sped-analitycs/crates/ecd-graph`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/auditor_final`
- **Date**: 2026-10-25T05:55:00Z
- **Integrity Mode**: Development (from `ORIGINAL_REQUEST.md`)

---

## Mandatory Reading & Reference Documents
1. **Authoritative User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Global Project Specification**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
3. **E2E Test Specification**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/TEST_READY.md`

---

## Forensic Integrity Audit Scope
Conduct a comprehensive, workspace-wide forensic audit across the completed deliverables:
1. **R1 (CLI Ingestion / `cnpj.db`)**:
   - Verify native Rust implementation with zero external subprocess calls (no python, no sqlite3 CLI, no curl).
   - Verify all 12 tables, in-flight transforms (BRL float normalization, 14-digit CNPJ synthesis, August 2026 radical expansion, `_referencia` metadata), and 18 post-load indices.
   - Verify streaming decompression without intermediate raw CSVs on disk.
2. **R2 (Societary Graph / `rede.db` & FTS5 Search / `rede_search.db`)**:
   - Verify all 7 derivation rules, NULL defense, fast-path join on `cnpj_base2matriz`, fallback join, and FTS5 `id_search` virtual table.
   - Verify 100% schema and query compatibility with `crates/ecd-graph`.
3. **R3 (Streaming Downloader)**:
   - Verify HTTP 206 range client, `.part` staging, atomic rename upon verifying exact file size, WAF User-Agent bypass, exponential backoff, and non-retryable fatal error classification.
4. **R4 (Root Makefile)**:
   - Verify all targets (`all`, `download`, `build-cnpj`, `build-rede`, `status`, `build`, `check`, `lint`, `fmt`, `test`, `test-e2e`, `test-quick`, `clean`, `clean-db`, `clean-zips`, `clean-all`).
   - Verify `.NOTPARALLEL`, `.SHELLFLAGS := -eu -o pipefail -c`, `.DEFAULT_GOAL := help`, and zero interactive prompts.
5. **Memory Bound & Performance**:
   - Verify peak RSS < 1.8 GB RAM constraint.
6. **Code Hygiene & Full Test Suite**:
   - Run `make check`, `make lint`, `make test`, `cargo test --test e2e_pipeline_test`.
   - Run `cargo test -p ecd-graph` in `/home/pizani/Projects/rust-sped-analitycs`.
   - Verify 0 clippy warnings with `-D warnings` and 0 test failures.

---

## Deliverables
Write your hard handoff report to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/auditor_final/handoff.md`. Include:
- Observation: exhaustive checks conducted, grep results, test execution summaries
- Logic Chain: verification of authenticity, absence of facades or stubs, compliance with user request
- Caveats: any technical details to report
- Conclusion: definitive binary verdict (**CLEAN** or **INTEGRITY VIOLATION**)
- Verification Method: reproduction commands

Send a message to orchestrator_1 when complete.
