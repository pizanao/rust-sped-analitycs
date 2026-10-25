# Dispatch Assignment — explorer_m3_1

- **Role**: Graph Derivation SQL & Schema Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Scope**: Milestone 3 — Societary Network Graph Derivation (`dados-publicos/rede.db`)
- **Mandatory References**:
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md`
  - `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md`
  - `/home/pizani/Projects/rust-sped-analitycs/crates/ecd-graph/src/service.rs`
  - `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/cli.rs`

## Your Task
1. Inspect the legacy derivation rules from `rede-cnpj` and the consumer requirements in `rust-sped-analytics/crates/ecd-graph`.
2. Formulate the exact SQL derivation statements for populating table `ligacao(id1 TEXT, id2 TEXT, descricao TEXT, comentario TEXT)`:
   - Rule 1: Matriz -> Filial (linking Matriz 14-char CNPJ to Filial 14-char CNPJ with description 'filial').
   - Rule 2: Empresa -> Sócio PJ (linking Empresa Matriz CNPJ to PJ partner 14-char Matriz CNPJ with description 'socio pj' or qualification).
   - Rule 3: Empresa -> Sócio PF (linking Empresa Matriz CNPJ to PF partner CPF/name with description 'socio pf' or qualification).
   - Rule 4: Empresa -> Sócio Estrangeiro (linking Empresa to foreign partner).
   - Rule 5: Sócio -> Representante Legal (linking partner to their legal representative).
   - Rule 6 & 7: Materialization of table `entidades(id TEXT PRIMARY KEY, nome TEXT NOT NULL, tipo TEXT NOT NULL DEFAULT 'PJ')`.
3. Verify post-load index creation: `idx_ligacao_origem (id1)`, `idx_ligacao_destino (id2)`.
4. Ensure 100% schema compatibility with `rust-sped-analytics/crates/ecd-graph` and GRC `links.db`.
5. Propose the module structure in `src/graph/rules.rs` and `src/graph/schema.rs`.
6. Deliver your handoff report to `handoff.md` and message parent (orchestrator_1).

## 2026-10-25T00:35:17Z
You are explorer_m3_1 (Role: Graph Derivation SQL & Schema Architect).
Your working directory is /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_1.
Read your task description in DISPATCH.md in your working directory.
Also read:
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/ORIGINAL_REQUEST.md
- /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/orchestrator_1/PROJECT.md
- /home/pizani/Projects/rust-sped-analitycs/crates/ecd-graph/src/service.rs

Inspect the target project /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas.
Formulate the exact 7 SQL derivation queries for populating ligacao(id1, id2, descricao, comentario) and entidades(id, nome, tipo) with 100% schema compatibility with ecd-graph and GRC links.db.
Do NOT modify source code.
Deliver your handoff report to handoff.md in your working directory and message parent (orchestrator_1).
