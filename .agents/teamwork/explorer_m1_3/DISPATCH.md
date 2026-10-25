# DISPATCH — explorer_m1_3

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_3
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Scope: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- Target Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Milestone 1: Scaffold & Resumable Downloader
### Subtask: HTTP 206 Resume Client, WAF Bypass & Progress Reporting
Design the downloading engine:
1. Handling partial downloads via `Range: bytes={existing_bytes}-`.
2. Handling HTTP status codes (206 Partial Content: append to file; 200 OK: overwrite if server doesn't support range or file was 0 bytes; 416 Range Not Satisfiable: file already complete).
3. Browser User-Agent header to bypass F5 BIG-IP WAF challenge cookies.
4. Real-time progress bars via `indicatif::MultiProgress` or single progress bar per file showing speed, ETA, and bytes transferred.
5. Error handling, retries with exponential backoff on network dropouts.
6. Produce implementation recommendations in `handoff.md`.
## 2026-10-24T19:49:53Z
You are explorer_m1_3 for Milestone 1 (Scaffold & Resumable Downloader).
Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_3.
Read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md, /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md, and /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_3/DISPATCH.md.

Design the HTTP 206 range resume downloading client in Rust:
1. Partial content resume logic with Range header.
2. Handling 206, 200, 416 responses.
3. F5 BIG-IP WAF bypass headers (browser User-Agent).
4. Progress bar via indicatif (speed, ETA, bytes).
5. Transient error retry loop with exponential backoff.
Provide concrete code recommendations in your handoff.md.
Do NOT write or modify source code files.
Notify me when complete.
