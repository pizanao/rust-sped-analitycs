# DISPATCH — explorer_m1_2

- Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_2
- Parent: orchestrator_1 (Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc)
- Original Request: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Scope: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- Target Directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas

## Milestone 1: Scaffold & Resumable Downloader
### Subtask: WebDAV Discovery & URL Catalog
Design the WebDAV discovery mechanism in Rust:
1. Base URL: `https://arquivos.receitafederal.gov.br/public.php/webdav/` with share token `YggdBLfdninEJX9`.
2. PROPFIND request structure, headers (`Depth: 1`, basic auth), and parsing of XML response (or regex/string scan) to automatically identify the latest `YYYY-MM/` folder and all 37 `.zip` files.
3. Fallback logic: if WebDAV directory listing is unreachable or format changes, provide static/deterministic listing of the 37 canonical filenames (`Cnaes.zip`, `Motivos.zip`, `Municipios.zip`, `Naturezas.zip`, `Paises.zip`, `Qualificacoes.zip`, `Simples.zip`, `Empresas0.zip`..`9.zip`, `Estabelecimentos0.zip`..`9.zip`, `Socios0.zip`..`9.zip`).
4. Produce implementation recommendations in `handoff.md`.

## 2026-10-24T19:49:46Z
You are explorer_m1_2 for Milestone 1 (Scaffold & Resumable Downloader).
Working Directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_2.
Read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md, /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md, and /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_2/DISPATCH.md.

Design the WebDAV discovery mechanism in Rust for querying https://arquivos.receitafederal.gov.br/public.php/webdav/ with share token YggdBLfdninEJX9. Include XML parsing / regex for finding the latest YYYY-MM directory and 37 zip files, and static fallback catalogue.
Provide concrete code recommendations in your handoff.md.
Do NOT write or modify source code files.
Notify me when complete.
