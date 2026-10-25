# DISPATCH — explorer_m2_3

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_3
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Scope: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- Target Project Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Milestone 2: High-Performance CNPJ Ingestion Engine
### Subtask: Memory Bounding & Concurrency Orchestrator
Design the ingestion pipeline coordinator:
1. `ingestion/mod.rs`: Bounded MPSC channels (`crossbeam_channel::bounded(16)`) with 5,000-row chunks.
2. Memory ceiling enforcement: Ensure process RSS stays strictly under 1.8 GB RAM across all operations, safely running on 8GB-16GB machines.
3. Concurrency architecture: Rayon decompression/parsing workers feeding SQLite loader thread.
4. Progress tracking: `indicatif::ProgressBar` showing records inserted, throughput (rows/sec), and elapsed time per table and overall.
5. CLI integration: Connect `build-cnpj` subcommand with options (`--input-dir`, `--output-db`, `--memory-limit-mb`, `--drop-existing`).
6. Provide concrete code and architecture in `handoff.md`.

## 2026-10-24T20:52:14Z
You are explorer_m2_3 for Milestone 2 (High-Performance CNPJ Ingestion Engine).
Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_3.
Read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md, /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md, and /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_3/DISPATCH.md.

Design the memory-bounded concurrency orchestrator for ingestion:
1. ingestion/mod.rs: Bounded MPSC channels (crossbeam_channel::bounded(16)) with 5,000-row chunks.
2. Memory ceiling enforcement: Guarantee process resident memory stays strictly under 1.8 GB RAM across all operations, safely running on 8GB-16GB machines without OOM.
3. Rayon worker pool for decompression/parsing feeding SQLite loader thread.
4. Progress tracking via indicatif, and integration with build-cnpj CLI subcommand.
Provide concrete code diffs and recommendations in handoff.md.
Do NOT write or modify source code files.
Notify me when complete.
