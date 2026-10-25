# DISPATCH — worker_m2

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m2
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Scope: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- Target Project Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Assigned Milestone: Milestone 2 (High-Performance CNPJ Ingestion Engine)
Implement the high-performance native Rust streaming ingestion engine into `dados-publicos/cnpj.db`.

### Exclusive File Ownership
You own and have exclusive write permission to:
- `src/ingestion/mod.rs`
- `src/ingestion/zip_stream.rs`
- `src/ingestion/schema.rs`
- `src/ingestion/transforms.rs`
- `src/ingestion/loader.rs`
- `src/lib.rs` (export `pub mod ingestion;`)
- `src/cli.rs` (wire `build-cnpj` subcommand)

### Input Technical Designs
Read and implement according to the designs provided by the Explorers:
1. `explorer_m2_1`: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_1/handoff.md` (`zip_stream.rs` streaming decompression and `transforms.rs` Windows-1252 to UTF-8, in-flight BRL float parsing, and in-flight 14-char CNPJ concatenation).
2. `explorer_m2_2`: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_2/handoff.md` (`schema.rs` 12-table DDL and `loader.rs` prepared statement batch transactions, `cnpj_base2matriz` materialization, August 2026 radical expansion in `socios`, `_referencia` metadata, and post-insert index creation sequencing).
3. `explorer_m2_3`: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_3/handoff.md` (`ingestion/mod.rs` bounded MPSC crossbeam channel, memory bounding < 1.8 GB RAM, Rayon workers, indicatif progress reporting, and CLI options wiring).

### Verification Requirements
1. `cargo check`
2. `cargo clippy --all-targets -- -D warnings` (MUST pass with 0 warnings)
3. `cargo test --all-targets` (all tests MUST pass)
4. CLI verification: `./target/debug/rfb-cnpj-etl build-cnpj --help`
5. Dry-run or test with synthetic fixtures / temporary SQLite DB: verify that tables and indices are created correctly, rows are ingested, `cnpj_base2matriz` is populated, and post-load PRAGMAs are applied.
6. Document all command outputs in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m2/handoff.md`.

### Integrity Warning
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

## 2026-10-24T21:00:05Z
You are worker_m2, the Rust Database Engineer implementing Milestone 2 (High-Performance CNPJ Ingestion Engine).
Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m2.
Target Project Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.

Read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md, /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md, and /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m2/DISPATCH.md.

Read the concrete specifications from:
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_1/handoff.md (zip_stream.rs and transforms.rs)
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_2/handoff.md (schema.rs and loader.rs)
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_3/handoff.md (ingestion/mod.rs)

Exclusive file ownership:
- src/ingestion/mod.rs
- src/ingestion/zip_stream.rs
- src/ingestion/schema.rs
- src/ingestion/transforms.rs
- src/ingestion/loader.rs
- src/lib.rs
- src/cli.rs

Implement the complete, high-performance streaming ingestion pipeline:
1. Stream decompress zip directly into csv::ReaderBuilder (zero raw CSV on disk).
2. Windows-1252 to UTF-8 decoding, in-flight BRL float parsing, in-flight 14-char CNPJ synthesis.
3. Prepared statement batch transactions, PRAGMAs (journal_mode=OFF, synchronous=OFF, cache_size=-500000), post-indexing sequencing.
4. Materialization of cnpj_base2matriz and August 2026 socios radical expansion.
5. Memory bounded strictly < 1.8 GB RAM using crossbeam bounded channels.
6. Progress tracking via indicatif and CLI wiring for build-cnpj.

Verify:
- cargo check
- cargo clippy --all-targets -- -D warnings (0 warnings)
- cargo test --all-targets
- Run ./target/debug/rfb-cnpj-etl build-cnpj --help
Document all outputs in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m2/handoff.md.

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Notify me when complete.
