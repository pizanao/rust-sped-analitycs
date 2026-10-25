# Challenger Assignment: Milestone 3 — FTS5 Search & Memory Bounds Challenger

- **Agent**: `challenger_m3_2`
- **Role**: FTS5 Search & Memory Bounds Challenger
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_2`
- **Date**: 2026-10-25T01:05:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Worker Handoff Report**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m3/handoff.md`
3. **FTS5 & Memory Management Code**:
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/search.rs`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/attachment.rs`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/mod.rs`
4. **Existing Memory & Stress Tests**:
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/adversarial_memory_bounds_test.rs`

---

## Adversarial Challenge Objectives
Empirically stress-test FTS5 search tokenization and memory bounding:
1. **FTS5 Tokenizer & Diacritic Removal**:
   - Test diacritic-insensitive search with special Portuguese characters: `SÃO PAULO`, `COMÉRCIO`, `INDÚSTRIA`, `SERVIÇO`, `ÁGUA`, `JOÃO`.
   - Test wildcard and prefix matching: `PJ_11111111*`, `PF_ ***123456**`, etc.
   - Verify that FTS5 `optimize` ran without error.
2. **Memory Ceiling (< 1.8 GB RSS)**:
   - Verify cache sizes and PRAGMA settings.
   - Run `build-rede` and measure peak RSS memory or verify memory bounds test in `tests/adversarial_memory_bounds_test.rs`.
3. **Attachment Lifecycle & Lock Avoidance**:
   - Verify that detachment never returns SQLite error 5 (`SQLITE_BUSY` or `database locked`).
   - Verify that WAL mode is permanently enabled on `rede.db` and `rede_search.db`.
4. **Verification**:
   - Run `cargo test -p rfb-cnpj-etl --test adversarial_memory_bounds_test`
   - Run `cargo test -p rfb-cnpj-etl --lib --all-targets`

Record empirical results and verdict (**APPROVE** or **REQUEST_CHANGES**) in `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_2/handoff.md`.
Notify orchestrator when done via `send_message`.

## 2026-10-25T01:00:06Z
[Message] timestamp=2026-10-25T01:00:06Z sender=0ebc4e6f-ccdc-4481-a769-92fd7f6528cc priority=MESSAGE_PRIORITY_HIGH content=You are challenger_m3_2, assigned as FTS5 Search & Memory Bounds Challenger for Milestone 3 (Societary Network Graph & FTS5 Search) in /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_2.
Read your instructions in /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_2/DISPATCH.md.
Also read /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md.

Empirically test FTS5 diacritic and wildcard search, detachment safety, and memory ceilings (< 1.8 GB RAM), write your handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m3_2/handoff.md with a clear verdict (APPROVE or REQUEST_CHANGES), and send a message back to the orchestrator.
