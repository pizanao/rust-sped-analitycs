# Guia de Operação para Agentes Paralelos (Multi-Agent Guide)

Este documento estabelece o protocolo de trabalho para que múltiplos subagentes autônomos ou desenvolvedores trabalhem em paralelo neste repositório sem gerar conflitos de merge, sem quebrar contratos de interface e mantendo a integridade do código.

---

## 1. Princípios de Isolamento e Concorrência

1. **Arquitetura Orientada a Contratos (Contract-First)**:
   * A crate [`crates/ecd-domain`](crates/ecd-domain) define as entidades centrais e structs canônicos.
   * **Regra de Ouro**: Nenhum agente pode alterar structs em `ecd-domain` sem compatibilidade retroativa. Se um campo novo for necessário, crie um tipo derivado ou extensão no seu próprio crate.
2. **Separação por Faixas de Trabalho (Work Tracks)**:
   * O projeto foi dividido em trilhas independentes descritas em [`TASKS.md`](TASKS.md).
   * Cada agente deve trabalhar **exclusivamente** dentro da sua pasta/crate atribuída:
     * **Agente A (Parser)**: `crates/ecd-parser/`
     * **Agente B (Fiscal & XLSX)**: `crates/ecd-fiscal/`
     * **Agente C (Rede CNPJ & Grafos)**: `crates/ecd-graph/`
     * **Agente D (Storage & Worker)**: `crates/ecd-storage/` e `apps/worker/`
     * **Agente E (API Server Axum)**: `apps/server/`
     * **Agente F (Frontend SPA)**: `apps/web/`
     * **Agente G (AI Gateway)**: `services/ai-gateway/`
3. **Sem Estado Compartilhado em Tempo de Desenvolvimento**:
   * Cada crate do workspace Rust possui seu próprio `Cargo.toml` e suite de testes unitários isolada (`cargo test -p ecd-parser`, `cargo test -p ecd-fiscal`).
   * Teste sempre seu crate isoladamente antes de rodar o teste global do workspace.

---

## 2. Protocolo de Reivindicação de Tarefas (Claim Protocol)

Antes de iniciar qualquer trabalho:
1. Abra [`TASKS.md`](TASKS.md).
2. Verifique se a tarefa desejada está com todas as dependências marcadas como concluídas (`[x]`).
3. Marque a tarefa como em andamento `[~]` e adicione o identificador do seu agente.
4. Execute apenas as modificações necessárias para cumprir os critérios de aceite daquela tarefa.
5. Ao concluir com testes passando, altere para `[x]` e faça commit atômico no git.

---

## 3. Comandos de Verificação Mandatórios

Nenhuma tarefa pode ser considerada concluída sem passar na verificação local:

### Para Crates Rust:
```bash
# 1. Checagem rápida de tipos
cargo check -p <nome-do-crate>

# 2. Testes unitários do crate
cargo test -p <nome-do-crate>

# 3. Formatação e linter
cargo fmt --check
cargo clippy -p <nome-do-crate> -- -D warnings
```

### Para o Frontend SPA (React):
```bash
cd apps/web
npm run build
```

### Para o AI Gateway (Python):
```bash
cd services/ai-gateway
pytest
```

---

## 4. Regras de Git e Commits
* Utilize commits semânticos no padrão conventional commits:
  * `feat(parser): implementar iterador de registros J100 e J150`
  * `test(fiscal): adicionar teste de similaridade Damerau-Levenshtein`
  * `fix(storage): ajustar pool r2d2_sqlite para modo read-only`
* Nunca force o push (`git push -f`).
* Nunca comite arquivos de dados (`*.db`, `media/*.txt`, `.env`).
