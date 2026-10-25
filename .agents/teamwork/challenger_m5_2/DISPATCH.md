# Challenger Assignment: Milestone 5 Phase 2 — White-Box Societary Graph, FTS5 & Pipeline Edge Cases Challenger

- **Agent**: `challenger_m5_2`
- **Role**: White-Box Societary Graph, FTS5 & Pipeline Edge Cases Challenger
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m5_2`
- **Date**: 2026-10-25T05:45:00Z

---

## Mandatory Reading & Reference Documents
1. **User Request**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md` (Read FIRST)
2. **Implementation Source**:
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/**`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/status/**`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Makefile`
3. **Existing Adversarial Tests**:
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/adversarial_graph_traversal_test.rs`
   - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/adversarial_fts5_challenge_test.rs`
4. **Downstream Consumer**:
   - `/home/pizani/Projects/rust-sped-analitycs/crates/ecd-graph`

---

## White-Box Coverage Audit & Adversarial Testing Objectives
Perform an exhaustive white-box analysis of the societary graph generator, FTS5 search, and pipeline integration:
1. **Graph Derivation Rules & Join Optimization**:
   - Audit `src/graph/rules.rs` (Rules 1 to 5, entity materialization, deduplication).
   - Test both fast-path execution (when `cnpj_base2matriz` is present in attached schema) and fallback execution (when `cnpj_base2matriz` is missing).
   - Verify NULL defense across all join points and confirm `ligacao.id1` and `ligacao.id2` are never NULL or empty.
2. **FTS5 Adversarial Syntax Stress**:
   - Audit `src/graph/search.rs` and `sanitize_fts5_term`.
   - Stress-test with hostile query strings: unmatched quotes, Boolean operators (`AND`, `OR`, `NOT`, `NEAR`), column filters, wildcard operators (`*`), Unicode accents, punctuation (`/`, `-`, `.`).
   - Confirm zero SQLite FTS5 syntax error crashes.
3. **Downstream BFS Graph Traversal & Sanctions GRC**:
   - Verify that `rede.db` and `rede_search.db` produced by `rfb-cnpj-etl` satisfy the exact query requirements of `crates/ecd-graph`:
     - Multi-hop BFS traversal (1 to 10 layers)
     - Cycle detection and self-loop pruning
     - Integration with `links.db` public sanctions tables (PEP, CEIS, CEPIM, CNEP, Dívida Ativa PGFN)
4. **Adversarial Test Execution**:
   - Run:
     ```bash
     cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
     cargo test --test adversarial_graph_traversal_test
     cargo test --test adversarial_fts5_challenge_test
     make test
     cd /home/pizani/Projects/rust-sped-analitycs
     cargo test -p ecd-graph
     ```

## Deliverables
Write your hard handoff report to `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m5_2/handoff.md`. Include:
- Observation: white-box paths inspected, hostile tests executed, query performance
- Logic Chain: analysis of any coverage gaps or confirmation of complete hardening
- Caveats: any technical limitations noted
- Conclusion: verdict (**APPROVE** if hardened with 0 remaining gaps, or **REQUEST_CHANGES** with gap report)
- Verification Method: reproduction commands

Send a message to orchestrator_1 when complete.
