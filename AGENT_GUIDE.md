# Guia de Operação para Agentes Paralelos (Multi-Agent Guide)

Este documento estabelece o protocolo de trabalho para que múltiplos subagentes autônomos ou desenvolvedores trabalhem em paralelo neste repositório sem gerar conflitos de merge, sem quebrar contratos de interface e mantendo 100% de integridade e paridade com o legado.

---

## 1. Princípios de Isolamento e Concorrência

1. **Arquitetura Orientada a Contratos (Contract-First)**:
   * A crate [`crates/ecd-domain`](crates/ecd-domain) define todas as entidades contábeis oficiais da RFB (`Registro0000`, `RegistroI010`, `RegistroI030`, `RegistroI050`, `RegistroI051`, `RegistroI052`, `RegistroI053`, `RegI050`, `RegistroI150`, `RegistroI155`, `RegI355`, `RegistroJ005`, `RegistroJ100`, `RegJ150`, `RegistroJ210`, `RegistroJ215`), DTOs fiscais e contratos de fila Redis (`ImportSpedJobPayload`, `JobStatusEvent`).
   * **Regra de Ouro**: Nenhum agente das Trilhas 1 a 8 pode alterar ou remover campos existentes em `crates/ecd-domain/` ou alterar `crates/ecd-storage/src/lib.rs`.
2. **Mapeamento Estrito de Agentes por Pasta (Zero Conflito de Merge)**:
   * O projeto está dividido em 8 trilhas de agentes + 1 trilha de homologação final conforme [`TASKS.md`](TASKS.md):

| Agente | Trilha | Onda | Escopo Exclusivo de Escrita (Arquivos/Pastas) |
| :--- | :--- | :--- | :--- |
| **Agente 1** | Trilha 1 (Parser SPED) | Onda 1 | `crates/ecd-parser/**` |
| **Agente 2** | Trilha 2 (Grafos / Rede CNPJ) | Onda 1 | `crates/ecd-graph/**` |
| **Agente 3** | Trilha 3 (Fiscal & XLSX) | Onda 1 | `crates/ecd-fiscal/**` |
| **Agente 4** | Trilha 4 (Indicadores & Cache) | Onda 1 | `crates/ecd-analytics/**` |
| **Agente 5** | Trilha 5 (Frontend SPA) | Onda 1 | `apps/web/**` |
| **Agente 6** | Trilha 6 (AI Gateway) | Onda 1 | `services/ai-gateway/**` |
| **Agente 7** | Trilha 7 (Worker & Batch SQL) | Onda 2 | `apps/worker/**` e `crates/ecd-storage/src/sped_repository.rs` |
| **Agente 8** | Trilha 8 (API Server Axum) | Onda 2 | `apps/server/**`, `crates/ecd-storage/src/auth_repository.rs` e `crates/ecd-storage/src/fiscal_repository.rs` |

3. **Sem Estado Compartilhado em Tempo de Desenvolvimento**:
   * Cada crate do workspace Rust possui seu próprio `Cargo.toml` e suite de testes unitários isolada (`cargo test -p ecd-parser`, `cargo test -p ecd-fiscal`, etc.).
   * Teste sempre seu crate isoladamente antes de rodar o teste global do workspace.

---

## 2. Protocolo de Reivindicação de Tarefas (Claim Protocol)

Antes de iniciar qualquer trabalho:
1. Abra [`TASKS.md`](TASKS.md).
2. Verifique se a tarefa desejada está com todas as dependências marcadas como concluídas (`[x]`).
3. Marque a tarefa como em andamento `[~]` e adicione o identificador do seu agente (`Agente 1` a `Agente 8`).
4. Execute apenas as modificações dentro do seu escopo de diretórios exclusivo.
5. Ao concluir com todos os testes passando, altere para `[x]` e faça commit atômico no git.

---

## 3. Comandos de Verificação Mandatórios

Nenhuma tarefa pode ser considerada concluída sem passar na verificação local:

### Para Crates e Binários Rust (Agentes 1, 2, 3, 4, 7, 8):
```bash
# 1. Checagem rápida de tipos do pacote
cargo check -p <nome-do-pacote>

# 2. Testes unitários do pacote
cargo test -p <nome-do-pacote>

# 3. Formatação e linter sem avisos
cargo fmt --check
cargo clippy -p <nome-do-pacote> -- -D warnings
```

### Para o Frontend SPA React (Agente 5):
```bash
cd apps/web
npm run build
```

### Para o AI Gateway Python (Agente 6):
```bash
cd services/ai-gateway
pytest
```

---

## 4. Matriz de Homologação (Estado Final)

Todas as trilhas estão concluídas. A tabela abaixo reflete o estado de cobertura após a Onda 3 e a Trilha 10:

| Componente | Testes | Status |
| :--- | :---: | :---: |
| Workspace Rust (todos os crates) | **82/82** | ✅ |
| Paridade Django legado | **7/7** | ✅ |
| AI Gateway (FastAPI / pytest) | **43/43** | ✅ |
| Frontend SPA | **15/15** | ✅ |
| ETL RFB CNPJ (`rfb-cnpj-etl`) | **142/142** | ✅ |

Benchmarks de performance validados:
- Parsing SPED 100 MB: **~1,85 s** | 500 MB: **~9,60 s** (throughput: 52–54 MB/s, ~930.000 linhas/s)
- Fuzzy Matching DRE: **>265.000 avaliações/s** (3,48 µs/operação)
- ETL CNPJ: consumo de pico **< 110 MB RSS**

---

## 5. Pipeline de Dados Abertos RFB CNPJ (`crates/rfb-cnpj-etl`)

O módulo `crates/ecd-graph` consome bancos SQLite gerados pela crate nativa `rfb-cnpj-etl`, integrada ao workspace em:

```
crates/rfb-cnpj-etl/
```

Essa crate expõe tanto biblioteca (`rfb_cnpj_etl`) quanto binário CLI (`rfb-cnpj-etl`) e alvos no `Makefile` raiz (`make cnpj-download`, `make cnpj-build`, `make cnpj-rede`, `make cnpj-status`, `make cnpj-all`). Ela:
- Baixa os dados abertos da RFB via HTTP 206 com crawler WebDAV (detecção automática do diretório mensal).
- Gera `cnpj.db` (dados cadastrais, sócios, CNAEs, expansão PJ via `cnpj_base2matriz`) e `rede.db` (grafo + FTS5).
- Opera com zero-disk durante a ingestão (descompressão streaming em memória).

Não modifique nem faça consultas diretas nos arquivos `.db` manualmente; gere-os via `crates/rfb-cnpj-etl` e consuma-os apenas via `crates/ecd-graph`.

---

## 6. Regras de Git e Commits

* Utilize commits semânticos no padrão Conventional Commits:
  * `feat(parser): implementar iterador de registros J100 e J150`
  * `test(fiscal): adicionar teste de similaridade partial_ratio com 24 termos`
  * `feat(analytics): implementar agregador CNAE e cache moka`
  * `fix(storage): ajustar pool r2d2_sqlite para modo read-only`
* Nunca force o push (`git push -f`).
* Nunca comite arquivos de dados (`*.db`, `media/*.txt`, `.env`).
