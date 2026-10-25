# E2E Test Infra: RFB CNPJ Open Data Pipeline

## Test Philosophy
- Opaque-box, requirement-driven. No dependency on internal implementation structures.
- Tests exercise the compiled CLI binary `rfb-cnpj-etl` and root `Makefile` targets against isolated temporary directories with synthetic fixtures.
- Methodology: Category-Partition + Boundary Value Analysis + Pairwise Combinatorial + Real-World Workload Testing.

## Feature Inventory & Test Coverage Goals
| # | Feature | Requirement | Tier 1 (Coverage) | Tier 2 (Boundary) | Tier 3 (Pairwise) | Tier 4 (Application) |
|---|---------|-------------|:-----------------:|:-----------------:|:-----------------:|:---------------------:|
| 1 | F01: WebDAV Range Downloader | R3 | 5 | 5 | ✓ | ✓ |
| 2 | F02: Direct Streaming Decompression | R1 | 5 | 5 | ✓ | ✓ |
| 3 | F03: Domain Tables Ingestion | R1 | 5 | 5 | ✓ | ✓ |
| 4 | F04: Empresas Ingestion | R1 | 5 | 5 | ✓ | ✓ |
| 5 | F05: Estabelecimento Ingestion | R1 | 5 | 5 | ✓ | ✓ |
| 6 | F06: Simples & MEI Ingestion | R1 | 5 | 5 | ✓ | ✓ |
| 7 | F07: Matriz Lookup Cache | R1 | 5 | 5 | ✓ | ✓ |
| 8 | F08: Socios Ingestion (Aug/2026) | R1 | 5 | 5 | ✓ | ✓ |
| 9 | F09: Audit Metadata | R1 | 5 | 5 | ✓ | ✓ |
| 10 | F10: SQLite PRAGMAs & Memory Bounding | R1 | 5 | 5 | ✓ | ✓ |
| 11 | F11: Societary Links Graph | R2 | 5 | 5 | ✓ | ✓ |
| 12 | F12: Entities & FTS5 Search | R2 | 5 | 5 | ✓ | ✓ |
| 13 | F13: GRC Sanctions Compatibility | R2 | 5 | 5 | ✓ | ✓ |
| 14 | F14: Unified Rust CLI | R1, R2, R3 | 5 | 5 | ✓ | ✓ |
| 15 | F15: Root Makefile Automation | R4 | 5 | 5 | ✓ | ✓ |
| 16 | F16: Synthetic Fixture Dataset | R4 | 5 | 5 | ✓ | ✓ |

## Test Architecture
- Fixture Generator: `tests/fixtures/generator.rs` builds minimal, valid 37 `.zip` files containing synthetic RFC/RFB-compliant Windows-1252 semicolon CSV data.
- Test Runner: Invokes `make test` or `cargo test --test e2e_pipeline_test`.
- Test Verification:
  - Validates SQLite schema via `sqlite3` PRAGMA table_info and index_list.
  - Validates row counts and referential consistency.
  - Validates graph connectivity (BFS queries on `ligacao`).
  - Validates FTS5 match queries on `id_search`.
  - Validates memory and process exit codes.

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Expected Outcome |
|---|----------|--------------------|------------------|
| 1 | Full Cold Pipeline Execution | F01, F02, F03, F04, F05, F06, F07, F08, F09, F11, F12, F15 | `make all` runs end-to-end, creates `cnpj.db` and `rede.db` with exit code 0 |
| 2 | August 2026 Corporate Partner Expansion | F04, F05, F07, F08, F11 | 8-digit radical PJ partner correctly expands to 14-digit Matriz CNPJ in `socios` and `ligacao` |
| 3 | Multilevel Societary Graph Traversal | F11, F12 | BFS query traversal across 3 hops (PF -> PJ1 -> PJ2 -> PJ3) via `crates/ecd-graph` algorithm |
| 4 | Resumable Download Recovery | F01, F14 | Interrupted download with partial zip resumes via HTTP 206 without re-downloading entire file |
| 5 | Clean Environment Status & Help | F14, F15 | `make status`, `make help`, `make clean` execute idempotently in empty and populated directories |

## Coverage Thresholds
- Tier 1: ≥ 80 test cases across 16 features.
- Tier 2: ≥ 80 boundary/corner cases (empty fields, max lengths, accents, comma floats, missing matrices, dummy CPFs).
- Tier 3: ≥ 16 cross-feature interaction tests (download+ingest, ingest+graph, graph+BFS, FTS+search).
- Tier 4: ≥ 8 realistic end-to-end workload tests.
- Total Target: ≥ 184 test assertions.
