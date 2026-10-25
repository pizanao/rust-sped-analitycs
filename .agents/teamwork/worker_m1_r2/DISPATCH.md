# DISPATCH — worker_m1_r2

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m1_r2
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Scope: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- Target Project Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Milestone 1 Iteration 2: Remediation Implementation
Implement the fixes designed by the Iteration 2 Explorers to address all Reviewer and Challenger feedback:

### Exclusive File Ownership
- `src/downloader/range_client.rs`
- `src/downloader/mod.rs`
- `src/downloader/webdav.rs`
- `src/main.rs`

### Required Fixes to Implement:
1. **VULN-01 (Expected size validation)**:
   In `src/downloader/range_client.rs`, check that `.part` length matches `target.expected_size` before `tokio::fs::rename`. Bail on mismatch. See `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_r2_1/handoff.md` and `remediation_m1_r2.patch`.
2. **VULN-02 (BufWriter flush on error)**:
   In `src/downloader/range_client.rs`, flush `writer` before propagating stream errors.
3. **VULN-03 (Fallback catalog base-url)**:
   In `src/downloader/webdav.rs`, use `extract_base_host(&self.config.base_url)` in `build_static_fallback_catalog` and `supplement_catalog`.
4. **Clippy Len-Zero**:
   In `src/downloader/range_client.rs:488`, replace `assert!(archive.len() > 0)` with `assert!(!archive.is_empty())`.
5. **JoinSet Fail-Fast**:
   In `src/downloader/mod.rs`, replace linear `for task in tasks` loop with `tokio::task::JoinSet` and call `join_set.abort_all()` immediately upon any task error. See `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_r2_2/handoff.md`.
6. **Quiet Flag Propagation**:
   In `src/main.rs` and `src/downloader/mod.rs`, propagate `cli.quiet` to `DownloaderOptions` and suppress `MultiProgress` and non-error output when `-q` is specified. See `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_r2_3/handoff.md`.
7. **StorageFull / ENOSPC Handling**:
   In `src/downloader/range_client.rs`, add `ErrorKind::StorageFull` and OS error code checks in `is_fatal_error` to abort immediately without retries when disk is full.

### Verification Commands to Run:
1. `cargo check`
2. `cargo clippy --all-targets -- -D warnings` (MUST pass with 0 warnings)
3. `cargo test --all-targets` (all tests MUST pass)
4. Test `./target/debug/rfb-cnpj-etl -q download --dry-run` (MUST NOT print progress bars or dry-run listing)
5. Document all outputs in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m1_r2/handoff.md`.

### Integrity Warning
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.
