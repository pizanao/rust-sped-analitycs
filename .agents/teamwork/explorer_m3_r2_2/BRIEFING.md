# BRIEFING — 2026-10-25T01:18:30Z

## Mission
Analyze NULL / empty guard needs across all derivation rules in src/graph/rules.rs, formulate exact SQL WHERE clause guards for constraint integrity, and produce a self-contained handoff report.

## 🔒 My Identity
- Archetype: explorer
- Roles: Constraint Integrity & Orphan Partner Architect
- Working directory: /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2
- Original parent: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc
- Milestone: Milestone 3 (Iteration 2)

## 🔒 Key Constraints
- Read-only investigation — do NOT implement
- Analyze NULL / empty guard needs across all derivation rules in src/graph/rules.rs
- Formulate exact SQL WHERE clause guards for constraint integrity
- Write handoff report to /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2/handoff.md
- Notify orchestrator (0ebc4e6f-ccdc-4481-a769-92fd7f6528cc) via send_message

## Current Parent
- Conversation ID: 0ebc4e6f-ccdc-4481-a769-92fd7f6528cc
- Updated: not yet

## Investigation State
- **Explored paths**:
  - `src/graph/rules.rs` (all 11 SQL constants, derivation functions, unit tests)
  - `src/graph/schema.rs` (`ligacao1`, `ligacao`, `entidades`, `links` DDL and indices)
  - `src/ingestion/loader.rs` (`transform_socios_and_expand_radicals` left joins causing NULL `cnpj`)
  - `dados_cnpj_para_sqlite.py` & `rede_cria_tabela_rede.db.py` (legacy baseline)
  - Reviewer 2 & Challenger 1 reports
- **Key findings**:
  - `socios.cnpj` is populated via `LEFT JOIN cnpj_base2matriz te ON te.cnpj_basico = ts.cnpj_basico`. If establishment is omitted/missing, `socios.cnpj` is NULL.
  - In Rules 2, 3, and 4, `destino` is `'PJ_' || t.cnpj`. When `t.cnpj IS NULL`, SQLite evaluates to `NULL`.
  - In `DEDUPLICATE_LIGACAO_SQL`, inserting `NULL` into `ligacao.id2` triggers `IntegrityError: NOT NULL constraint failed: ligacao.id2`.
  - In Rule 5a, `destino` is `'PE_' || t.nome_socio`. `t.nome_socio` was completely unguarded in WHERE. If NULL, triggers `NOT NULL constraint failed: ligacao.id2`.
  - In Rule 5c, `destino` is `'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio`. `t.nome_socio` was completely unguarded in WHERE. If NULL, triggers `NOT NULL constraint failed: ligacao.id2`.
  - In Rule 1, `tf.cnpj` or `tm.cnpj` could be NULL or truncated if establishment data is dirty.
  - In `entidades`, materializing PJ with empty `te.cnpj` inserts phantom `'PJ_'` entity.
  - Formulated complete SQL WHERE guards and deduplication filters, verified via Python SQLite simulation.
- **Unexplored areas**: None within this subagent's scope.

## Key Decisions Made
- Formulated exact SQL WHERE clauses for all 7 derivation rules.
- Added defense-in-depth filter in `DEDUPLICATE_LIGACAO_SQL` and `MATERIALIZE_ENTIDADES_*.SQL`.
- Authored `.patch` file (`rules_null_guards.patch`) and full replacement file (`proposed_rules.rs`).
- Designed empirical unit test `test_derivation_orphan_and_corrupt_partner_resilience`.

## Artifact Index
- `DISPATCH.md` — Task assignment and instructions
- `BRIEFING.md` — Working memory and identity index
- `progress.md` — Heartbeat and activity log
- `rules_null_guards.patch` — Unified diff patch for `src/graph/rules.rs`
- `proposed_rules.rs` — Full proposed replacement file for `src/graph/rules.rs`
- `handoff.md` — 5-component handoff report for the orchestrator and worker
