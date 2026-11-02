# RFB CNPJ ETL Workspace Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the `rfb-cnpj-etl` Rust library, CLI binary, and 142-test suite from `/apps/rede-cnpj-master-v26/rede_cria_tabelas/` into `crates/rfb-cnpj-etl` within the `rust-sped-analitycs` Cargo workspace.

**Architecture:** `crates/rfb-cnpj-etl` is integrated as a hybrid Cargo package exposing both `rfb_cnpj_etl` (`[lib]`) for upcoming `ecd-server`/`ecd-worker` consumption and `rfb-cnpj-etl` (`[[bin]]`) for standalone CLI execution and E2E testing. Dependencies are unified in the root `Cargo.toml` (`[workspace.dependencies]`), and root `Makefile` and `scripts/package.sh` are updated to build, run, and package the ETL binary.

**Tech Stack:** Rust 2021, Cargo Workspace, `rusqlite` 0.32 (`bundled-full`, `backup`, `blob`), `reqwest` 0.12 (`rustls-tls`, `stream`, `json`, `cookies`), `clap` 4.5, `rayon` 1.10, `zip` 2.2, `csv` 1.3.

**Spec:** `docs/superpowers/specs/2026-11-02-rfb-cnpj-etl-migration-design.md`

## Global Constraints

- Migrate only the Rust project (`src/`, `tests/`, `Cargo.toml`); do not copy legacy Python scripts (`*.py`, `requirements.txt`) or local data directories (`dados-publicos/`, `dados-publicos-zip/`).
- Unify `rusqlite = { version = "0.32", features = ["bundled-full", "backup", "blob"] }` and `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "stream", "cookies"] }` in root `[workspace.dependencies]`.
- Preserve both `[lib]` (`name = "rfb_cnpj_etl"`) and `[[bin]]` (`name = "rfb-cnpj-etl"`) in `crates/rfb-cnpj-etl/Cargo.toml` so `assert_cmd::Command::cargo_bin("rfb-cnpj-etl")` resolves `CARGO_BIN_EXE_rfb-cnpj-etl` during `cargo test --workspace`.
- All 142 tests in `rfb-cnpj-etl` and 82 existing workspace tests must pass (`cargo test --workspace`) with zero clippy warnings (`cargo clippy --workspace -- -D warnings`).

## Review Focus

- `rusqlite` feature unification (`bundled-full` + `backup` + `blob`) must compile cleanly across both `ecd-graph` (which uses `r2d2_sqlite`) and `rfb-cnpj-etl` without duplicate SQLite C symbol conflicts.
- `tracing-subscriber` in workspace dependencies must include the `"fmt"` feature required by `rfb-cnpj-etl/src/main.rs` alongside `"env-filter"` and `"json"`.
- `Command::cargo_bin("rfb-cnpj-etl")` in `crates/rfb-cnpj-etl/tests/common/mod.rs` must resolve the binary when invoked from the workspace root via `cargo test --workspace`.
- Root `Makefile` targets for the CNPJ pipeline must use the `cnpj-` prefix (`cnpj-download`, `cnpj-build`, `cnpj-rede`, `cnpj-status`, `cnpj-all`) so they do not shadow existing workspace `build`, `status`, or `all` targets.
- `scripts/package.sh` must build and copy `target/release/rfb-cnpj-etl` into `dist/rust-sped-analytics-<VERSION>/bin/`.

---

### Task 1: Migrate `rfb-cnpj-etl` Crate and Unify Cargo Workspace Dependencies

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/rfb-cnpj-etl/Cargo.toml`
- Create: `crates/rfb-cnpj-etl/src/**` (copied verbatim from `/apps/rede-cnpj-master-v26/rede_cria_tabelas/src/`)
- Create: `crates/rfb-cnpj-etl/tests/**` (copied verbatim from `/apps/rede-cnpj-master-v26/rede_cria_tabelas/tests/`)

**Interfaces:**
- Consumes: `/apps/rede-cnpj-master-v26/rede_cria_tabelas/src` and `/apps/rede-cnpj-master-v26/rede_cria_tabelas/tests`
- Produces: Workspace member `rfb-cnpj-etl` (`lib` crate `rfb_cnpj_etl` + `bin` `rfb-cnpj-etl`) registered in root `Cargo.toml`

- [ ] **Step 1: Copy `src/` and `tests/` from `/apps/rede-cnpj-master-v26/rede_cria_tabelas/` into `crates/rfb-cnpj-etl/`**

Run:
```bash
mkdir -p crates/rfb-cnpj-etl
cp -r /apps/rede-cnpj-master-v26/rede_cria_tabelas/src crates/rfb-cnpj-etl/
cp -r /apps/rede-cnpj-master-v26/rede_cria_tabelas/tests crates/rfb-cnpj-etl/
```
Expected: `crates/rfb-cnpj-etl/src/lib.rs`, `crates/rfb-cnpj-etl/src/main.rs`, and 7 test files in `crates/rfb-cnpj-etl/tests/` exist.

- [ ] **Step 2: Create `crates/rfb-cnpj-etl/Cargo.toml` referencing workspace metadata and dependencies**

Write `crates/rfb-cnpj-etl/Cargo.toml`:
```toml
[package]
name = "rfb-cnpj-etl"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "High-performance native Rust ETL and SQLite database generator for RFB CNPJ open data"
keywords = ["cnpj", "rfb", "sqlite", "etl", "graph"]
categories = ["command-line-utilities", "database"]

[lib]
name = "rfb_cnpj_etl"
path = "src/lib.rs"

[[bin]]
name = "rfb-cnpj-etl"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
chrono.workspace = true
clap.workspace = true
crossbeam-channel.workspace = true
csv.workspace = true
encoding_rs.workspace = true
futures-util.workspace = true
indicatif.workspace = true
memmap2.workspace = true
quick-xml.workspace = true
rayon.workspace = true
regex.workspace = true
reqwest.workspace = true
rusqlite.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
zip.workspace = true

[dev-dependencies]
assert_cmd.workspace = true
predicates.workspace = true
tempfile.workspace = true
```

- [ ] **Step 3: Update root `Cargo.toml` to register `crates/rfb-cnpj-etl` and unify `[workspace.dependencies]`**

In `Cargo.toml`:
- Add `"crates/rfb-cnpj-etl"` to `[workspace].members`.
- Add `rfb-cnpj-etl = { path = "crates/rfb-cnpj-etl" }` under `# Internal crates`.
- Update `reqwest`, `rusqlite`, and `tracing-subscriber` features and add the ETL dependencies in `[workspace.dependencies]`:
  - `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "stream", "cookies"] }`
  - `rusqlite = { version = "0.32", features = ["bundled-full", "backup", "blob"] }`
  - `tracing-subscriber = { version = "0.3", features = ["env-filter", "json", "fmt"] }`
  - `clap = { version = "4.5", features = ["derive", "env"] }`
  - `futures-util = "0.3"`
  - `indicatif = { version = "0.17", features = ["rayon"] }`
  - `zip = { version = "2.2", default-features = false, features = ["deflate"] }`
  - `csv = "1.3"`
  - `crossbeam-channel = "0.5"`
  - `rayon = "1.10"`
  - `regex = "1.11"`
  - `quick-xml = { version = "0.37", features = ["serialize"] }`
  - `tempfile = "3.12"`
  - `assert_cmd = "2.0"`
  - `predicates = "3.1"`

- [ ] **Step 4: Run workspace check, clippy, formatting, and full test suite**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p rfb-cnpj-etl
cargo test -p ecd-graph
```
Expected: All 142 tests in `rfb-cnpj-etl` (unit + 7 integration test suites) and all tests in `ecd-graph` pass with 0 clippy warnings.

- [ ] **Step 5: Commit Task 1**

```bash
git add Cargo.toml crates/rfb-cnpj-etl
git commit -m "feat(etl): migrate rfb-cnpj-etl crate into cargo workspace"
```

---

### Task 2: Integrate Makefile Targets, Packaging Script, Gitignore, and Documentation

**Files:**
- Modify: `Makefile`
- Modify: `scripts/package.sh`
- Modify: `.gitignore`
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: `rfb-cnpj-etl` binary and package from Task 1 (`cargo build --release -p rfb-cnpj-etl`, `cargo run --release -p rfb-cnpj-etl --bin rfb-cnpj-etl -- <subcommand>`)
- Produces: Root `Makefile` targets (`build-etl`, `cnpj-download`, `cnpj-build`, `cnpj-rede`, `cnpj-status`, `cnpj-all`), packaged `bin/rfb-cnpj-etl` in `scripts/package.sh`, updated `.gitignore` and `AGENTS.md`

- [ ] **Step 1: Update `.gitignore` to ignore ETL data directories and SQLite/download temporaries**

Append to `.gitignore`:
```gitignore
# RFB CNPJ ETL Data & Temporaries
dados-publicos/
dados-publicos-zip/
*.db-shm
*.part
*.tmp*
```

- [ ] **Step 2: Add `build-etl` and `cnpj-*` pipeline targets to root `Makefile`**

In `Makefile`:
- Add configurable variables near the top:
  ```makefile
  CNPJ_ZIP_DIR   ?= dados-publicos-zip
  CNPJ_DATA_DIR  ?= dados-publicos
  CNPJ_DB        ?= $(CNPJ_DATA_DIR)/cnpj.db
  REDE_DB        ?= $(CNPJ_DATA_DIR)/rede.db
  SEARCH_DB      ?= $(CNPJ_DATA_DIR)/rede_search.db
  CNPJ_MEM_LIMIT ?= 8096
  CNPJ_BATCH     ?= 500000
  CNPJ_THREADS   ?= 12
  CNPJ_CONC      ?= 6
  ```
- Include `build-etl` in `build-native: build-server build-worker build-etl` and define:
  ```makefile
  build-etl: ## Compila o binário Rust nativo do rfb-cnpj-etl em modo --release
  	@echo -e "${CYAN}Compilando rfb-cnpj-etl (--release)...${RESET}"
  	@cargo build --release -p rfb-cnpj-etl
  ```
- Add a section `# PIPELINE RFB CNPJ ETL (DADOS ABERTOS)` with targets:
  - `cnpj-download`: runs `cargo run --release -p rfb-cnpj-etl --bin rfb-cnpj-etl -- download --output-dir $(CNPJ_ZIP_DIR) --concurrency $(CNPJ_CONC)`
  - `cnpj-build`: runs `cargo run --release -p rfb-cnpj-etl --bin rfb-cnpj-etl -- build-cnpj --input-dir $(CNPJ_ZIP_DIR) --output-db $(CNPJ_DB) --memory-limit-mb $(CNPJ_MEM_LIMIT) --batch-size $(CNPJ_BATCH) --threads $(CNPJ_THREADS) --drop-existing`
  - `cnpj-rede`: runs `cargo run --release -p rfb-cnpj-etl --bin rfb-cnpj-etl -- build-rede --cnpj-db $(CNPJ_DB) --output-rede $(REDE_DB) --output-search $(SEARCH_DB) --memory-limit-mb $(CNPJ_MEM_LIMIT) --drop-existing`
  - `cnpj-status`: runs `cargo run --release -p rfb-cnpj-etl --bin rfb-cnpj-etl -- status --cnpj-db $(CNPJ_DB) --rede-db $(REDE_DB) --search-db $(SEARCH_DB) --zip-dir $(CNPJ_ZIP_DIR)`
  - `cnpj-all: cnpj-download cnpj-build cnpj-rede cnpj-status`

- [ ] **Step 3: Update `scripts/package.sh` to build and bundle `rfb-cnpj-etl`**

In `scripts/package.sh`:
- Add `ETL_BIN="${ROOT_DIR}/target/release/rfb-cnpj-etl"` alongside `SERVER_BIN` and `WORKER_BIN`.
- Check `[[ ! -f "${SERVER_BIN}" || ! -f "${WORKER_BIN}" || ! -f "${ETL_BIN}" || "${FORCE_BUILD:-0}" == "1" ]]` and build with `cargo build --release -p ecd-server -p ecd-worker -p rfb-cnpj-etl`.
- Copy `cp -f "${ETL_BIN}" "${STAGE_DIR}/bin/"`.

- [ ] **Step 4: Update `AGENTS.md` to reflect `crates/rfb-cnpj-etl` inside the workspace**

In `AGENTS.md`:
- In Section 2.2 (Isolamento Estrito de Diretórios), include `crates/rfb-cnpj-etl/**` under Trilha 2 (or dedicated ETL scope).
- In Section 6 (Pipeline de Dados Abertos RFB CNPJ), replace the external path reference with `crates/rfb-cnpj-etl` inside this workspace and document the `make cnpj-*` targets.

- [ ] **Step 5: Run full verification checklist (`make check`, `make lint`, `make test`, `make cnpj-status`)**

Run:
```bash
make help
make check
make lint
make test
```
Expected: `make help` lists `build-etl`, `cnpj-download`, `cnpj-build`, `cnpj-rede`, `cnpj-status`, `cnpj-all`; `make check`, `make lint`, and `make test` pass 100% of workspace tests with zero warnings.

- [ ] **Step 6: Commit Task 2**

```bash
git add Makefile scripts/package.sh .gitignore AGENTS.md
git commit -m "chore(workspace): wire rfb-cnpj-etl into Makefile, packaging, and docs"
```
