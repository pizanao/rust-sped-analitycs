# Design: Migração e Integração do Módulo `rfb-cnpj-etl`

**Data:** 2026-11-02  
**Tamanho:** Mudança pequena/média (migração direta de crate existente para o workspace, sem alteração de regras de negócio).

## 1. Intenção e Motivação (Why)
Trazer o motor Rust de ETL de Dados Abertos do CNPJ da Receita Federal (`rfb-cnpj-etl`), hoje isolado em `/apps/rede-cnpj-master-v26/rede_cria_tabelas/`, para dentro do monorepo `rust-sped-analitycs`. Essa integração unifica o código em um único workspace Cargo para que:
1. O pipeline (`download`, `build-cnpj`, `build-rede`, `status`) possa ser operado e empacotado nativamente junto ao projeto.
2. Na próxima rodada, o `ecd-server` e o `ecd-worker` possam importar `rfb_cnpj_etl` como biblioteca Rust para oferecer uma API e uma interface web de operação e monitoramento da atualização das bases (`cnpj.db`, `rede.db`, `rede_search.db`).

## 2. Objetivos, Não-Objetivos e Anti-Objetivos
- **Objetivos (Goals):**
  - Migrar `src/` e `tests/` de `/apps/rede-cnpj-master-v26/rede_cria_tabelas/` para `crates/rfb-cnpj-etl`.
  - Unificar as dependências no `Cargo.toml` raiz do workspace (`[workspace.dependencies]`).
  - Manter 100% dos 142 testes de `rfb-cnpj-etl` e dos 82 testes atuais do workspace passando via `cargo test --workspace` e sem avisos no `cargo clippy --workspace -- -D warnings`.
  - Integrar os comandos de build e execução do ETL no `Makefile` raiz e incluir o binário `rfb-cnpj-etl` no pacote de distribuição (`scripts/package.sh`).
  - Atualizar `.gitignore` e `AGENTS.md` refletindo a nova localização interna.
- **Não-Objetivos (Non-Goals):**
  - Implementar rotas HTTP/SSE no `ecd-server`, filas no `ecd-worker` ou telas no `apps/web` nesta etapa (ficam para a próxima rodada).
  - Migrar scripts Python legados (`*.py`, `requirements.txt`) ou diretórios de dados brutos (`dados-publicos/`, `dados-publicos-zip/`).
- **Anti-Objetivos (Anti-Goals):**
  - Quebrar a compatibilidade da suíte E2E (`assert_cmd::Command::cargo_bin("rfb-cnpj-etl")`) ou introduzir duplicação de versões do SQLite (`rusqlite`) no workspace.

## 3. Restrições (Constraints)
- **Unificação de Features no Workspace:**
  - `rusqlite = "0.32"` no workspace deve passar a incluir `features = ["bundled", "backup", "blob"]` (`bundled` já habilita `SQLITE_ENABLE_FTS5` e `unicode61` no `rfb-cnpj-etl` sem ativar `extra_check` de `bundled-full`, preservando compatibilidade com os PRAGMAs de `r2d2_sqlite` em `ecd-graph`).
  - `reqwest = "0.12"` no workspace deve incluir a feature `"cookies"` exigida pelo crawler WebDAV da RFB.
- **Docker Build Context:** `Dockerfile.server` e `Dockerfile.worker` já copiam `COPY crates ./crates`, portanto alojar o pacote em `crates/rfb-cnpj-etl` mantém os builds Docker funcionando sem camadas extras.

## 4. Abordagens Consideradas (Approaches)
1. **Abordagem 1 (Escolhida): Crate Híbrida (`lib` + `bin`) em `crates/rfb-cnpj-etl`**
   - Mantém `[lib]` (`rfb_cnpj_etl`) e `[[bin]]` (`rfb-cnpj-etl`) juntos em `crates/rfb-cnpj-etl`.
   - *Vantagem:* Preserva `CARGO_BIN_EXE_rfb-cnpj-etl` para a suíte de testes E2E em `crates/rfb-cnpj-etl/tests/`, expõe a biblioteca pronta para `ecd-server`/`ecd-worker` e não exige alterar `Dockerfile.server`/`Dockerfile.worker`.
   - *Trade-off:* Um binário CLI reside dentro de `crates/` junto com sua biblioteca.
2. **Abordagem 2: Dividir em `crates/rfb-cnpj-etl` (lib) e `apps/cnpj-etl` (bin)**
   - Separa biblioteca e executável em diretórios distintos.
   - *Trade-off:* Quebra a resolução automática de `Command::cargo_bin("rfb-cnpj-etl")` nos testes de integração da biblioteca para um `main.rs` de apenas 46 linhas.

## 5. Decisões de Implementação
- **Decidido com o usuário:**
  - Escopo restrito à migração do código Rust (`rfb-cnpj-etl`) nesta etapa, preparando terreno para a API e UI na etapa seguinte.
  - Adoção da **Abordagem 1** (`crates/rfb-cnpj-etl` contendo `lib` + `bin` + `tests`).
- **Escolhas técnicas propostas pelo construtor (sujeitas à sua revisão):**
  - **Alvos no `Makefile` raiz:** Adicionar `build-etl` (incluído também em `build-native`), `cnpj-download`, `cnpj-build`, `cnpj-rede`, `cnpj-status` e `cnpj-all` usando o prefixo `cnpj-` para não colidir com os alvos gerais (`build`, `status`, `all`) do monorepo.
  - **Empacotamento (`scripts/package.sh`):** Compilar e copiar `target/release/rfb-cnpj-etl` para `dist/.../bin/` junto com `ecd-server` e `ecd-worker`.
  - **`.gitignore`:** Adicionar `dados-publicos/`, `dados-publicos-zip/`, `*.db-shm` e `*.part` para evitar commits acidentais de artefatos do ETL.
