# DISPATCH — explorer_m1_1

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_1
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Scope: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- Target Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Milestone 1: Scaffold & Resumable Downloader
### Subtask: Cargo Architecture & CLI Specification
Design the exact `Cargo.toml` dependency set and CLI interface for `rfb-cnpj-etl`:
1. Dependencies needed for the entire project lifecycle (rusqlite with bundled+fts5, zip, reqwest with rustls-tls+stream, indicatif, tokio, clap derive, csv, encoding_rs, crossbeam-channel, rayon, anyhow/thiserror).
2. Binary name and CLI structure (`main.rs`, `clap` subcommands: `download`, `build-cnpj`, `build-rede`, `status`).
3. CLI flags for `download`: `--output-dir` (default: `dados-publicos-zip`), `--share-token`, `--year-month`, `--dry-run`, `--concurrency`.
4. Produce implementation recommendations in `handoff.md`.

## 2026-10-24T19:49:37Z
You are explorer_m1_1 for Milestone 1 (Scaffold & Resumable Downloader).
Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_1.
Read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md, /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md, and /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_1/DISPATCH.md.

Design the exact Cargo.toml dependencies and CLI interface (clap derive) for rfb-cnpj-etl in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Provide concrete code recommendations and structure in your handoff.md.
Do NOT write or modify source code files.
Notify me when complete.
