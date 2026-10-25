# Task Assignment: Milestone 4 — Makefile Automation & Idempotency Architect

- **Agent**: `explorer_m4_2`
- **Role**: Makefile Automation & Idempotency Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_2`
- **Date**: 2026-10-25T01:45:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Project Specification**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
3. **Existing Root Makefile / Target Project Directory**:
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
4. **Automated Test Expectations**:
   - `tests/e2e_pipeline_test.rs` (Tier 4 Real-World scenarios calling CLI / Makefile targets)

---

## Architecture Objectives
Design the root `Makefile` in `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Makefile`:
1. **Target Specification**:
   - `all`: End-to-end orchestration (`download` -> `build-cnpj` -> `build-rede` -> `status`).
   - `download`: Downloads all 37 RFB archives using `cargo run --release --bin rfb-cnpj-etl -- download`. Configurable via `ZIP_DIR ?= dados-publicos-zip`.
   - `build-cnpj`: Ingests zip files into `cnpj.db` using `cargo run --release --bin rfb-cnpj-etl -- build-cnpj`. Configurable via `CNPJ_DB ?= dados-publicos/cnpj.db`, `MEM_LIMIT ?= 1536`.
   - `build-rede`: Derives societary graph and FTS search using `cargo run --release --bin rfb-cnpj-etl -- build-rede`. Configurable via `REDE_DB ?= dados-publicos/rede.db`.
   - `status`: Displays status using `cargo run --release --bin rfb-cnpj-etl -- status`.
   - `test`: Runs unit and integration tests using `cargo test --all-targets`.
   - `lint`: Runs `cargo clippy --all-targets -- -D warnings` and `cargo fmt -- --check`.
   - `clean`: Removes intermediate temporary files and optionally databases (`clean-all`).
   - `help`: Automatically documents all targets with ANSI formatting and descriptions.
2. **Idempotency & Non-Interactive Safety**:
   - Zero prompts: `make all`, `make download`, `make build-cnpj`, `make build-rede` must execute autonomously without stdin.
   - Robust error handling: Proper `.PHONY` declarations, `.DEFAULT_GOAL := help`.
   - Directory creation prerequisites: Ensure destination folders (`dados-publicos/`, `dados-publicos-zip/`) are created if missing.
   - Profile optimization: Build `--release` for high throughput, or support `PROFILE ?= release`.
3. **Draft the Complete Makefile**:
   - Provide the complete, clean, production-ready Makefile text in your report.

Write a comprehensive handoff report to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_2/handoff.md` and notify orchestrator when done via `send_message`.

## 2026-10-25T01:44:36Z
You are explorer_m4_2, assigned as Makefile Automation & Idempotency Architect for Milestone 4 in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_2.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_2/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

Design the root Makefile and automation targets, write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_2/handoff.md, and send a message back to the orchestrator.

## 2026-10-25T05:07:21Z
**Context**: Milestone 4 Exploration
**Content**: Checking in on your status. `explorer_m4_1` and `explorer_m4_3` have completed their handoffs. I saw your `proposed_Makefile` is already generated. Please finalize your handoff report in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_2/handoff.md`.
**Action**: Wrap up handoff.md and report back.
