# DISPATCH — explorer_m1_r2_1

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_r2_1
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Scope: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- Reviewer Report: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/reviewer_m1_2/handoff.md
- Challenger Report: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m1_1/handoff.md
- Target Project Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Milestone 1 Iteration 2: Remediation Strategy
### Subtask: VULN-01 (Expected Size Validation & BufWriter Flush)
Formulate the exact fix strategy for `src/downloader/range_client.rs`:
1. VULN-01: Verify file length matches `target.expected_size` before renaming `.part` -> `.zip`.
2. VULN-02: Flush `BufWriter` upon stream error to preserve all received bytes.
3. VULN-03: Respect `--base-url` host in fallback catalog.
Produce exact diff / recommendations in `handoff.md`.

## 2026-10-24T20:23:55Z
You are explorer_m1_r2_1 for Milestone 1 Iteration 2.
Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_r2_1.
Read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md, /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md, /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m1_1/handoff.md, and /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_r2_1/DISPATCH.md.

Design the exact fix for VULN-01 (expected size validation before atomic rename), VULN-02 (BufWriter flush on error), and VULN-03 (respect base-url in fallback catalog).
Provide concrete code diffs in handoff.md.
Do NOT write or modify source code files.
Notify me when complete.
