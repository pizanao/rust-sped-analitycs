# DISPATCH — worker_m1

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m1
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Scope: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- Target Project Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Assigned Milestone: Milestone 1 (Scaffold & Resumable Downloader)
Implement the high-performance native Rust crate scaffold and the resumable streaming downloader CLI command for `rfb-cnpj-etl`.

### Exclusive File Ownership
You own and have exclusive write permission to:
- `Cargo.toml`
- `src/lib.rs`
- `src/main.rs`
- `src/cli.rs`
- `src/config.rs`
- `src/downloader/mod.rs`
- `src/downloader/webdav.rs`
- `src/downloader/range_client.rs`

### Inputs and Technical References
You MUST read and implement according to the designs provided by the Explorers:
1. `explorer_m1_1`: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_1/handoff.md` (Cargo.toml dependencies, src/cli.rs, src/config.rs, src/main.rs, src/lib.rs).
2. `explorer_m1_2`: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_2/handoff.md` and reference code at `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_2/proposed_webdav.rs` (WebDAV PROPFIND discovery, 37 files listing, static fallback catalog).
3. `explorer_m1_3`: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_3/handoff.md` and reference code at `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_3/proposed_range_client.rs` and `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_3/proposed_download_manager.rs` (HTTP 206 Range resume, WAF User-Agent and cookie store, indicatif MultiProgress, .part staging, retry loop).

### Verification Requirements
1. Build check: `cargo check` and `cargo build` in `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`.
2. Unit tests: `cargo test` in `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`.
3. CLI verification: Test CLI help (`./target/debug/rfb-cnpj-etl --help` and `./target/debug/rfb-cnpj-etl download --help`).
4. Dry-run or discovery verification: Test discovery of the 37 files (`./target/debug/rfb-cnpj-etl download --dry-run`).
5. Include all command outputs in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m1/handoff.md`.

### Integrity Warning
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.
