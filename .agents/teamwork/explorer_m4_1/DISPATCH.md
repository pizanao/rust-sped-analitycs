# Task Assignment: Milestone 4 — CLI Status Subcommand & Diagnostics Architect

- **Agent**: `explorer_m4_1`
- **Role**: CLI Status Subcommand & Diagnostics Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_1`
- **Date**: 2026-10-25T01:45:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Project Specification**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
3. **Existing CLI Definition**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/cli.rs`
4. **Existing Main & Lib**:
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/main.rs`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/lib.rs`
5. **Database Schemas**:
   - `dados-publicos/cnpj.db`: 12 tables + `_referencia`
   - `dados-publicos/rede.db`: `ligacao`, `entidades`, `links`
   - `dados-publicos/rede_search.db`: `id_search` FTS5

---

## Architecture Objectives
Design the `rfb-cnpj-etl status` subcommand:
1. **CLI Parameters (`src/cli.rs`)**:
   - `StatusArgs`: `--cnpj-db` (default: `dados-publicos/cnpj.db`), `--rede-db` (default: `dados-publicos/rede.db`), `--search-db` (default: `dados-publicos/rede_search.db`), `--json` (boolean flag for machine output).
2. **Diagnostics & Metrics Collection**:
   - For each database file: check existence, file size on disk (bytes / formatted MB/GB), PRAGMA `journal_mode`, PRAGMA `user_version`, SQLite integrity check (`PRAGMA quick_check`).
   - For `cnpj.db`: query row counts for all 12 tables (`empresas`, `estabelecimento`, `socios`, `simples`, `cnae`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio`, `cnpj_base2matriz`, `_referencia`), query reference date from `_referencia`.
   - For `rede.db`: query row counts for `ligacao`, `entidades`, `links`, and edge counts by `comentario` (`estabelecimento` vs `socios`).
   - For `rede_search.db`: query row count for `id_search`.
3. **Display Formatting**:
   - Clean, aligned terminal table output (ANSI-colored if terminal, plain if piped).
   - Structured JSON output when `--json` is specified.
   - Graceful handling when database files do not exist yet (reports "Not found / not generated" rather than failing with fatal error).
4. **Code Structure**:
   - Design `src/status/mod.rs` (or `src/status.rs`) and wiring in `src/lib.rs` and `src/main.rs`.

Write a comprehensive handoff report to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_1/handoff.md` and notify orchestrator when done via `send_message`.


## 2026-10-25T01:44:29Z
You are explorer_m4_1, assigned as CLI Status Subcommand & Diagnostics Architect for Milestone 4 in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_1.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_1/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

Design the status subcommand and diagnostics module, write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m4_1/handoff.md, and send a message back to the orchestrator.
