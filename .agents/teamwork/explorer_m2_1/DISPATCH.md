# DISPATCH — explorer_m2_1

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_1
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Scope: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- Target Project Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Milestone 2: High-Performance CNPJ Ingestion Engine
### Subtask: In-Memory Streaming Decompression & In-Flight CSV Transformation
Design the streaming decompression and parsing layer:
1. `zip_stream.rs`: Read `.zip` files directly from disk without decompressing to intermediate CSVs. Read internal `F.K03200$Z.D*.EXT` entry and feed decompressed stream directly to CSV reader.
2. `transforms.rs`:
   - Encoding conversion: `WINDOWS_1252` to UTF-8 using `encoding_rs`.
   - In-flight BRL float conversion: parse `capital_social_str` (`1500,50` -> `1500.50 f64`).
   - In-flight CNPJ synthesis: concatenate `cnpj_basico (8)`, `cnpj_ordem (4)`, `cnpj_dv (2)` into 14-char `cnpj`.
   - Delimiter `;`, quote `"`, no headers.
3. Provide concrete code and architecture in `handoff.md`.


## 2026-10-24T20:51:59Z
You are explorer_m2_1 for Milestone 2 (High-Performance CNPJ Ingestion Engine).
Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_1.
Read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md, /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md, and /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_1/DISPATCH.md.

Design the streaming decompression and in-flight transformation layer in Rust:
1. zip_stream.rs: zero-copy in-memory zip decompression feeding directly to csv::ReaderBuilder (no raw CSV on disk).
2. transforms.rs: Windows-1252 to UTF-8 decoding, in-flight BRL float parsing (replace comma with dot), in-flight 14-char CNPJ concatenation.
Provide concrete code diffs and recommendations in handoff.md.
Do NOT write or modify source code files.
Notify me when complete.
