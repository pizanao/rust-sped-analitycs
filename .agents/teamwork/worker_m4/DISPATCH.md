# Task Assignment: Milestone 4 — Status Subcommand & Root Makefile Implementation

- **Agent**: `worker_m4`
- **Role**: Rust Systems & Build Automation Engineer
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m4`
- **Date**: 2026-10-25T05:10:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Project Blueprint**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
3. **Status Architecture & Drop-in Code**:
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_1/handoff.md`
4. **Makefile Specification & Source**:
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_2/proposed_Makefile`
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_2/handoff.md`
5. **E2E Alignment & Test Requirements**:
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_3/handoff.md`

---

## Mandatory Integrity Warning
> DO NOT CHEAT. All implementations must be genuine. DO NOT
> hardcode test results, create dummy/facade implementations, or
> circumvent the intended task. A teamwork_preview_auditor will independently
> verify your work. Integrity violations WILL be detected and your
> work WILL be rejected.

---

## Write Ownership Boundaries
You have exclusive write ownership of:
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/status/**`
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/lib.rs` (to replace the status stub with `pub mod status;`)
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Makefile`
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/e2e_pipeline_test.rs` (if any status test assertion alignment is needed)

---

## Implementation Requirements

### 1. Implement `src/status/` Module
Create the status inspection and diagnostics module following `explorer_m4_1/handoff.md`:
- `src/status/models.rs`: Strongly typed structs (`StatusReport`, `DatabaseReport`, `TableMetric`, `IndexMetric`, `ZipDirectoryReport`, `EdgeBreakdown`, etc.) with `Serialize, Deserialize, Debug, Clone`.
- `src/status/diagnostics.rs`: Collects file size, PRAGMAs (`quick_check(1)`, `journal_mode`, `user_version`, `page_count/size`), row counts for all 12 tables of `cnpj.db`, `_referencia` metadata, 18 post-load indices, `rede.db` (edges by comentário, entities by tipo, 5 indices), `rede_search.db` (`id_search`), and zip file counts.
  - Critical: If database files or zip directory do not exist, report them gracefully with `present: false` rather than failing or panicking (must exit code 0).
- `src/status/reporter.rs`: Renders human-readable formatted terminal tables (ANSI colored if TTY, plain if piped) and structured JSON (`serde_json::to_string_pretty`) when `--format json` or `--json` is specified.
- `src/status/mod.rs`: Exposes `pub fn run(args: StatusArgs) -> anyhow::Result<StatusReport>` and unit tests.
- Update `src/lib.rs`: Replace the inline stub `pub mod status { ... }` with `pub mod status;`.

### 2. Create Root `Makefile`
Create `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Makefile` using the exact design in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_2/proposed_Makefile`:
- Targets: `all`, `download`, `build-cnpj`, `build-rede`, `status`, `build`, `check`, `lint`, `fmt`, `test`, `test-e2e`, `test-quick`, `clean`, `clean-db`, `clean-zips`, `clean-all`, `help`.
- Enforce `.SHELLFLAGS := -eu -o pipefail -c`, `.NOTPARALLEL: all download build-cnpj build-rede status`, and `.DEFAULT_GOAL := help`.
- Zero interactive prompts; fully autonomous and idempotent.

---

## Verification Commands
Execute the following verification commands and record exact outputs:
1. `cargo check --all-targets`
2. `cargo test -p rfb-cnpj-etl --lib --all-targets`
3. `cargo test --test e2e_pipeline_test` (Full 32 tests passing without hitting any status stubs!)
4. `cargo clippy --all-targets -- -D warnings`
5. Test CLI status:
   - `cargo run --bin rfb-cnpj-etl -- status --help`
   - `cargo run --bin rfb-cnpj-etl -- status --json`
6. Test Makefile targets:
   - `make help`
   - `make -n all`
   - `make status`
7. Downstream verification in `/home/pizani/Projects/rust-sped-analitycs`:
   - `cargo test -p ecd-graph`

Write a comprehensive `handoff.md` to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m4/handoff.md` and notify orchestrator when done via `send_message`.


## 2026-10-25T05:08:41Z
You are worker_m4, assigned to implement Milestone 4 (CLI Status Subcommand & Root Makefile Automation) for the RFB CNPJ open data ETL project in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m4.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m4/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Follow all instructions in DISPATCH.md, execute the verification tests, write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m4/handoff.md, and send a message back to the orchestrator when completed.
