# Project: RFB CNPJ Open Data Native Rust Ingestion & Network Graph Pipeline

## Architecture
- Target Project Directory: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- Output Databases:
  - `dados-publicos/cnpj.db` (SQLite, 12 tables + indices)
  - `dados-publicos/rede.db` (SQLite, `ligacao`, `entidades` + indices)
  - `dados-publicos/rede_search.db` (SQLite, `id_search` FTS5 virtual table)
  - `dados-publicos-zip/*.zip` (37 compressed official files)
- Technology Stack:
  - Rust 2021 Edition
  - `rusqlite` (bundled, fts5, backup)
  - `zip` (deflate streaming)
  - `csv` + `encoding_rs` (Windows-1252 / ISO-8859-1 streaming)
  - `crossbeam-channel` + `rayon` (bounded pipeline, < 1.8 GB RAM)
  - `reqwest` (stream, rustls-tls) + `indicatif` (progress bars)
  - `clap` (CLI parser with subcommands)
  - `Makefile` (root automation)

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | F01: WebDAV Range Downloader | Discover latest YYYY-MM directory, download 37 zips with HTTP 206 resume and WAF bypass | M1 | Survey |
| 2 | F02: Direct Streaming Decompression | Stream decompress from .zip in-memory directly to CSV reader without writing raw CSVs to disk | M2 | Survey |
| 3 | F03: Domain Tables Ingestion | Ingest 6 code lookup tables (cnae, motivo, municipio, etc.) into cnpj.db + indices | M2 | Survey |
| 4 | F04: Enterprises Ingestion | Ingest 10 Empresas*.zip, parse BRL floats in-flight, populate empresas + indices | M2 | Survey |
| 5 | F05: Establishments Ingestion | Ingest 10 Estabelecimentos*.zip, synthesize 14-digit CNPJ in memory, populate estabelecimento + indices | M2 | Survey |
| 6 | F06: Simples & MEI Ingestion | Ingest Simples.zip into simples table + index | M2 | Survey |
| 7 | F07: Matriz Lookup Cache | Materialize cnpj_base2matriz from empresas and estabelecimento (matriz_filial='1') + index | M2 | Survey |
| 8 | F08: Socios Ingestion & Aug/2026 Quirk | Ingest 10 Socios*.zip, join cnpj_base2matriz to expand 8-digit PJ partner radical to 14-digit Matriz CNPJ + indices | M2 | Survey |
| 9 | F09: Audit Metadata | Insert reference date and establishment count into _referencia table | M2 | Survey |
| 10 | F10: SQLite Ingestion PRAGMAs & Memory Bounding | High-throughput PRAGMAs, bounded 5k-row MPSC batches (< 1.8 GB RAM), post-insert index creation | M2 | Survey |
| 11 | F11: Societary Links Graph | Generate ligacao table in rede.db via 7 derivation SQL rules on attached cnpj.db + indices | M3 | Survey |
| 12 | F12: Entities & FTS5 Search | Populate entidades(id, nome, tipo) and virtual table id_search using fts5(id, descricao) | M3 | Survey |
| 13 | F13: GRC Sanctions Compatibility | Ensure links table compatibility for links.db (id1, id2, descricao, valor, comentario) | M3 | Survey |
| 14 | F14: Unified Rust CLI | Single binary rfb-cnpj-etl with subcommands (download, build-cnpj, build-rede, status) | M1, M2, M3, M4 | Survey |
| 15 | F15: Root Makefile Automation | Idempotent non-interactive Makefile targets (download, build-cnpj, build-rede, all, test, status, clean, help) | M4 | Survey |
| 16 | F16: Synthetic Fixture & E2E Test Suite | 37 fixture zip files, opaque-box test runner validating Tiers 1-4 | E2E Track | Survey |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Scaffold & Resumable Downloader | Cargo scaffold, clap CLI, WebDAV discovery, HTTP 206 resume, WAF User-Agent | none | DONE |
| M2 | High-Performance CNPJ Ingestion Engine | Streaming zip->CSV, Windows-1252, in-flight transforms, 12 tables, post-indexing | M1 | DONE |
| M3 | Societary Network Graph & FTS5 Search | Attached DB joins, 7 derivation rules, ligacao, entidades, id_search FTS5 | M2 | DONE |
| M4 | Status & Root Makefile Automation | rfb-cnpj-etl status command, root Makefile with all targets | M1, M2, M3 | DONE |
| E2E | E2E Testing Suite Track | Synthetic 37-file fixture generator, test runner (Tiers 1-4) | none (parallel) | DONE |
| M5 | Final Milestone: Integration & Hardening | Pass 100% of E2E tests (Phase 1) + Adversarial hardening (Phase 2) | M4, E2E | DONE |

## Interface Contracts

### CLI Binary Interface: `rfb-cnpj-etl`
- `rfb-cnpj-etl download [--output-dir <DIR>] [--share-token <TOKEN>] [--year-month <YYYY-MM>]`
  - Exit code 0 on success. Non-interactive.
  - Downloads exactly 37 zip files to specified directory (default: `dados-publicos-zip/`).
- `rfb-cnpj-etl build-cnpj [--input-dir <DIR>] [--output-db <PATH>] [--memory-limit-mb <MB>]`
  - Ingests all 37 zip files into SQLite database (default: `dados-publicos/cnpj.db`).
  - Creates 12 tables + post-load indices + `_referencia`.
- `rfb-cnpj-etl build-rede [--cnpj-db <PATH>] [--output-rede <PATH>] [--output-search <PATH>]`
  - Derives `dados-publicos/rede.db` and `dados-publicos/rede_search.db`.
  - Creates `ligacao(id1, id2, descricao, comentario)`, `idx_ligacao_origem`, `idx_ligacao_destino`, and `id_search` FTS5.
- `rfb-cnpj-etl status [--cnpj-db <PATH>] [--rede-db <PATH>]`
  - Prints human-readable and machine-parseable table counts and sizes.

### Database Schema Contracts
#### `dados-publicos/cnpj.db`
- `cnae(codigo TEXT PRIMARY KEY, descricao TEXT)`
- `motivo(codigo TEXT PRIMARY KEY, descricao TEXT)`
- `municipio(codigo TEXT PRIMARY KEY, descricao TEXT)`
- `natureza_juridica(codigo TEXT PRIMARY KEY, descricao TEXT)`
- `pais(codigo TEXT PRIMARY KEY, descricao TEXT)`
- `qualificacao_socio(codigo TEXT PRIMARY KEY, descricao TEXT)`
- `empresas(cnpj_basico TEXT, razao_social TEXT, natureza_juridica TEXT, qualificacao_responsavel TEXT, capital_social_str TEXT, porte_empresa TEXT, ente_federativo_responsavel TEXT, capital_social REAL)`
  - Indices: `idx_empresas_cnpj_basico (cnpj_basico)`, `idx_empresas_razao_social (razao_social)`
- `estabelecimento(cnpj_basico TEXT, cnpj_ordem TEXT, cnpj_dv TEXT, matriz_filial TEXT, nome_fantasia TEXT, situacao_cadastral TEXT, data_situacao_cadastral TEXT, motivo_situacao_cadastral TEXT, nome_cidade_exterior TEXT, pais TEXT, data_inicio_atividades TEXT, cnae_fiscal TEXT, cnae_fiscal_secundaria TEXT, tipo_logradouro TEXT, logradouro TEXT, numero TEXT, complemento TEXT, bairro TEXT, cep TEXT, uf TEXT, municipio TEXT, ddd1 TEXT, telefone1 TEXT, ddd2 TEXT, telefone2 TEXT, ddd_fax TEXT, fax TEXT, correio_eletronico TEXT, situacao_especial TEXT, data_situacao_especial TEXT, cnpj TEXT)`
  - Indices: `idx_estabelecimento_cnpj_basico (cnpj_basico)`, `idx_estabelecimento_cnpj (cnpj)`, `idx_estabelecimento_nomefantasia (nome_fantasia)`
- `socios(cnpj TEXT, cnpj_basico TEXT, identificador_de_socio TEXT, nome_socio TEXT, cnpj_cpf_socio TEXT, qualificacao_socio TEXT, data_entrada_sociedade TEXT, pais TEXT, representante_legal TEXT, nome_representante TEXT, qualificacao_representante_legal TEXT, faixa_etaria TEXT)`
  - Indices: `idx_socios_cnpj (cnpj)`, `idx_socios_cnpj_cpf_socio (cnpj_cpf_socio)`, `idx_socios_nome_socio (nome_socio)`, `idx_socios_representante (representante_legal)`, `idx_socios_representante_nome (nome_representante)`
- `simples(cnpj_basico TEXT, opcao_simples TEXT, data_opcao_simples TEXT, data_exclusao_simples TEXT, opcao_mei TEXT, data_opcao_mei TEXT, data_exclusao_mei TEXT)`
  - Index: `idx_simples_cnpj_basico (cnpj_basico)`
- `cnpj_base2matriz(cnpj_basico TEXT PRIMARY KEY, cnpj TEXT)`
  - Index: `idx_cnpj_base2matriz (cnpj_basico)`
- `_referencia(referencia TEXT, valor TEXT)`

#### `dados-publicos/rede.db`
- `ligacao(id1 TEXT, id2 TEXT, descricao TEXT, comentario TEXT)`
  - Indices: `idx_ligacao_origem (id1)`, `idx_ligacao_destino (id2)`
- `entidades(id TEXT PRIMARY KEY, nome TEXT NOT NULL, tipo TEXT NOT NULL DEFAULT 'PJ')`

#### `dados-publicos/rede_search.db`
- `id_search USING fts5(id, descricao)`

## Code Layout
Target directory: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
```
rede_cria_tabelas/
├── Cargo.toml
├── Makefile
├── src/
│   ├── main.rs                 # CLI entry point (clap)
│   ├── config.rs               # Paths, PRAGMAs, concurrency limits
│   ├── downloader/             # Milestone 1: WebDAV, range requests, resume
│   │   ├── mod.rs
│   │   ├── webdav.rs
│   │   └── range_client.rs
│   ├── ingestion/              # Milestone 2: streaming zip, CSV parsing, SQLite loader
│   │   ├── mod.rs
│   │   ├── zip_stream.rs
│   │   ├── schema.rs
│   │   ├── transforms.rs
│   │   └── loader.rs
│   ├── graph/                  # Milestone 3: rede.db, attached DB queries, FTS5
│   │   ├── mod.rs
│   │   ├── rules.rs
│   │   └── search.rs
│   └── status/                 # Milestone 4: database inspection & reporting
│       ├── mod.rs
│       └── reporter.rs
├── tests/                      # E2E Testing Track
│   ├── fixtures/
│   │   └── generator.rs        # Synthetic 37-file zip generator
│   ├── common/
│   └── e2e_pipeline_test.rs    # Opaque-box tests (Tiers 1-4)
```
