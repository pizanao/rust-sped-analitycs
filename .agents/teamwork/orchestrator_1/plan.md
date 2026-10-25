# Plan — High-Performance Native Rust RFB CNPJ ETL & Graph DB

## Objective
Reconstrução em Rust nativo de alta performance das rotinas de ETL e geração das bases SQLite dos Dados Abertos do CNPJ da Receita Federal (`cnpj.db` e `rede.db`), integradas a um pipeline completo e automatizadas via `make`.

## Phases
1. **Phase 0: Survey & Specification Mining**
   - Survey the existing target directory `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas` (Python scripts, schema definitions, tables, indices, edge cases).
   - Survey RFB open data layout (37 files, zip structure, delimiters, encodings, columns).
   - Survey `rust-sped-analytics` expectations (how `crates/ecd-graph` consumes `cnpj.db` and `rede.db`).
2. **Phase 1: Architecture & Decomposition (PROJECT.md & TEST_INFRA.md)**
   - Define exact table schemas and SQLite indices for `cnpj.db` and `rede.db`.
   - Specify Rust crate/binary architecture (CLI flags, crate layout, dependencies).
   - Define milestone boundaries and interface contracts.
   - Establish E2E Testing methodology (Tiers 1-4).
3. **Phase 2: Dual Track Execution**
   - Track 1 (Implementation):
     - M1: Download & Streaming Unzip module.
     - M2: CNPJ DB high-performance ingestion engine (`memmap2`, `rusqlite`, `rayon`).
     - M3: Rede DB societary link generation & FTS engine.
     - M4: Root Makefile automation (`download`, `build-cnpj`, `build-rede`, `all`, `test`, `status`, `clean`, `help`).
   - Track 2 (E2E Testing):
     - Build synthetic fixture datasets representing RFB CSV layout.
     - Build automated test runner validating schema, row counts, indices, bidirectional links, memory usage.
     - Publish `TEST_READY.md`.
4. **Phase 3: Final Milestone — E2E Passing & Adversarial Hardening**
   - Verify 100% passing E2E tests across Tiers 1-4.
   - Tier 5 adversarial coverage and robustness verification.
5. **Phase 4: Forensic Audit & Completion Report**
   - Forensic auditor integrity check.
   - Synthesize results and notify Sentinel.
