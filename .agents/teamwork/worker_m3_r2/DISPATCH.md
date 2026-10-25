# Task Assignment: Milestone 3 (Iteration 2) — Societary Graph & FTS5 Remediation

- **Agent**: `worker_m3_r2`
- **Role**: Rust Societary Graph & FTS5 Remediation Engineer
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m3_r2`
- **Date**: 2026-10-25T01:25:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Project Blueprint**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
3. **Explorer 1 Report & Patch (Schema Qualification)**:
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_1/handoff.md`
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_1/rules_patch.diff`
4. **Explorer 2 Report & Patch (Constraint Integrity & NULL Guards)**:
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2/handoff.md`
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2/rules_null_guards.patch`
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2/proposed_rules.rs`
5. **Explorer 3 Report & Patch (FTS5 Sanitization & Test Alignment)**:
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/handoff.md`
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/search_sanitization.patch`
   - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/tests_alignment.patch`

---

## Mandatory Integrity Warning
> DO NOT CHEAT. All implementations must be genuine. DO NOT
> hardcode test results, create dummy/facade implementations, or
> circumvent the intended task. A teamwork_preview_auditor will independently
> verify your work. Integrity violations WILL be detected and your
> work WILL be rejected.

---

## Write Ownership Boundaries
You have exclusive write ownership of:
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs`
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/search.rs`
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/adversarial_graph_traversal_test.rs`

---

## Implementation Requirements

Apply the unified remediation designed by the 3 Explorers:

### 1. `src/graph/rules.rs`
- **Schema Qualification & Fast Path (Explorer 1)**:
  In `derive_filial_matriz` (lines 201–216), implement the two-step short-circuiting check:
  ```rust
  let has_b2m: bool = conn
      .query_row(
          "SELECT EXISTS(SELECT 1 FROM cnpj.sqlite_master WHERE type IN ('table', 'view') AND name='cnpj_base2matriz');",
          [],
          |r| r.get(0),
      )
      .unwrap_or(false)
      && conn
          .query_row(
              "SELECT EXISTS(SELECT 1 FROM cnpj.cnpj_base2matriz LIMIT 1);",
              [],
              |r| r.get(0),
          )
          .unwrap_or(false);
  ```
- **Constraint Integrity & NULL Guards (Explorer 2)**:
  Apply the NULL and empty string guards across all SQL derivation constants:
  - Rule 1 (`DERIVE_FILIAL_MATRIZ_SQL` and `DERIVE_FILIAL_MATRIZ_FALLBACK_SQL`): Ensure `tf.cnpj` and `tm.cnpj` are `IS NOT NULL AND length(tf.cnpj) = 14 AND length(tm.cnpj) = 14`.
  - Rules 2, 3, 4: Guard `t.cnpj IS NOT NULL AND length(t.cnpj) = 14` so orphan partner rows without establishment CNPJ never insert `NULL` or `'PJ_'` into `destino`.
  - Rule 5a: Guard `t.nome_socio IS NOT NULL AND trim(t.nome_socio) <> ''`.
  - Rule 5b: Guard `t.cnpj_cpf_socio IS NOT NULL AND length(t.cnpj_cpf_socio) = 14`.
  - Rule 5c: Guard `t.nome_socio IS NOT NULL AND trim(t.nome_socio) <> ''`.
  - Deduplication (`DEDUPLICATE_LIGACAO_SQL`): Filter out corrupt/null IDs with `WHERE origem IS NOT NULL AND destino IS NOT NULL AND length(origem) > 3 AND length(destino) > 3`.
  - Entity materialization (`MATERIALIZE_ENTIDADES_*_SQL`): Guard valid lengths and non-empty trimmed names.

### 2. `src/graph/search.rs` (Explorer 3)
- Implement `sanitize_fts5_term` helper function to safely sanitize FTS5 search queries.
- Update `search_by_term` and `search_by_cnpj_basico` to use the sanitized terms, properly handling formatted CNPJs (`11.223.344/0001-91` -> `"11223344000191"`, `11.223.344` -> `"11223344"*`), hyphenated trade names, quotes, and boolean operator keywords without syntax errors.

### 3. `tests/adversarial_graph_traversal_test.rs` (Explorer 3)
- Update lines 418–422 where raw boolean operator test previously asserted `is_err()` to assert clean handling with `assert!(r_syntax.is_ok());` now that inputs are sanitized.

---

## Verification Commands
Execute the following verification commands and report exact outputs:
1. `cargo check --all-targets`
2. `cargo test -p rfb-cnpj-etl --lib --all-targets`
3. `cargo test --test adversarial_graph_traversal_test`
4. `cargo test --test adversarial_memory_bounds_test`
5. `cargo test --test e2e_pipeline_test`
6. `cargo clippy --all-targets -- -D warnings`
7. `cargo test -p ecd-graph` in `/home/pizani/Projects/rust-sped-analitycs`

Write a comprehensive `handoff.md` to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/worker_m3_r2/handoff.md` and notify orchestrator when done via `send_message`.
