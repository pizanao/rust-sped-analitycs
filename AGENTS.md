# Guia de Operação e Contexto para Agentes de IA (AGENTS.md)

Este documento define a arquitetura, o contexto de negócio e as regras operacionais para o desenvolvimento autônomo e colaborativo de múltiplos agentes de IA no projeto **Rust Sped Analytics**.

---

## 1. Visão Geral e Contexto da Solução

O **Rust Sped Analytics** é uma plataforma de engenharia de software de alta performance concebida para **Auditoria Fiscal e Contábil** sobre escriturações da Receita Federal do Brasil (**SPED ECD — Escrituração Contábil Digital**), com foco estratégico na apuração de receitas de serviços para o **ISSQN** (Imposto Sobre Serviços de Qualquer Natureza) e inteligência societária municipal.

### Pilares Arquiteturais:
1. **Motor de Parsing Streaming Zero-Copy (`crates/ecd-parser`)**:
   - Processamento de arquivos massivos (de 100 MB a mais de 2 GB) em frações de segundo.
   - Utiliza mapeamento de memória virtual (`memmap2`) e fatiamento direto em bytes (`&[u8]`) com codificação oficial `WINDOWS_1252` e fim de linha CRLF.
2. **Persistência Transacional Desacoplada (`apps/worker` + `crates/ecd-storage`)**:
   - Inserção em lotes atômicos (`QueryBuilder` do SQLx com 5.000 linhas por query) no PostgreSQL 16.
   - Comunicação desacoplada entre a API Web e o processador via fila Redis (`ecd:queue:jobs`), com transmissão de progresso por Server-Sent Events (SSE).
3. **Motor Fiscal de Apuração DRE (`crates/ecd-fiscal`)**:
   - Classificação e cálculo automático de Receita Bruta, Receita de Serviços e Outras Receitas na DRE (`RegJ150`) e no Bloco `I355`.
   - Algoritmo de paridade aritmética estrita com o sistema legado, catálogo fiscal de 24 termos e 12 filtros negativos, com exportação nativa de planilhas Excel (`umya-spreadsheet`).
4. **Motor de Grafos Societários e Sanções GRC (`crates/ecd-graph`)**:
   - Análise de vínculos corporativos e grupos econômicos sobre a base SQLite da Receita Federal (Turmalina: `cnpj.db`, `rede.db`).
   - Algoritmo de busca em largura (BFS) de 1 a 10 camadas com controle de complexidade e cruzamento atômico de sanções públicas (**PEP, CEIS, CEPIM, CNEP e Dívida Ativa PGFN** da base `links.db`).
5. **AI Gateway Cognitivo (`services/ai-gateway`)**:
   - Microsserviço Python com **FastAPI**, **LangChain** e **LangGraph** (Arquitetura ReAct).
   - Execução de consultas analíticas em linguagem natural por meio de *tool calling* sobre o backend Axum, gerando relatórios Markdown e especificações JSON para renderização de gráficos.
6. **Frontend SPA de Alta Densidade (`apps/web`)**:
   - **React 18 + Vite + TypeScript + Tailwind CSS + Shadcn UI**.
   - Virtualização de tabelas contábeis massivas (`TanStack Table`), grafos societários dinâmicos (`Vis-Network`) e gráficos interativos (`Recharts`).

---

## 2. Princípios de Execução Paralela (Multi-Agent Concurrency)

Para garantir que múltiplos agentes e desenvolvedores operem simultaneamente sem corromper código, gerar conflitos de merge ou desalinhar interfaces:

### 2.1 Arquitetura Orientada a Contratos (Contract-First)
* **Fonte Única da Verdade**: A crate [`crates/ecd-domain/`](crates/ecd-domain) contém a definição tipada das estruturas contábeis oficiais (`Registro0000`, `RegistroI155`, `RegistroJ150`, `RegI050`, etc.) e os contratos de eventos da fila Redis (`ImportSpedJobPayload`, `JobStatusEvent`).
* **Regra de Ouro**: Nenhum agente pode modificar structs compartilhadas em `crates/ecd-domain/` ou o arquivo de inicialização de conexões `crates/ecd-storage/src/lib.rs` sem alinhamento prévio explícito.

### 2.2 Isolamento Estrito de Diretórios (Zero Merge Conflicts)
Cada agente possui autoridade exclusiva sobre um conjunto de diretórios:

| Trilha / Agente | Escopo do Módulo | Diretórios Exclusivos de Escrita |
| :--- | :--- | :--- |
| **Trilha 1** | Motor Parser SPED | `crates/ecd-parser/**` |
| **Trilha 2** | Motor de Grafos e Rede CNPJ | `crates/ecd-graph/**` |
| **Trilha 3** | Motor Fiscal & Planilhas XLSX | `crates/ecd-fiscal/**` |
| **Trilha 4** | Indicadores Econômicos & Cache | `crates/ecd-analytics/**` |
| **Trilha 5** | Frontend SPA React | `apps/web/**` |
| **Trilha 6** | AI Gateway Cognitivo | `services/ai-gateway/**` |
| **Trilha 7** | Worker Assíncrono & Batch SQL | `apps/worker/**` e `crates/ecd-storage/src/sped_repository.rs` |
| **Trilha 8** | API REST/SSE Axum | `apps/server/**`, `crates/ecd-storage/src/{auth,fiscal}_repository.rs` |

### 2.3 Protocolo de Reivindicação de Tarefas (Claim Protocol)
1. Inspecione o quadro de atividades em [`TASKS.md`](TASKS.md).
2. Verifique se as tarefas precedentes no grafo de dependências estão concluídas (`[x]`).
3. Marque a tarefa como em andamento: substitua `[ ]` por `[~]` e informe o identificador do agente.
4. Execute o trabalho restrito ao seu escopo de escrita.
5. Execute os comandos de verificação de qualidade mandatórios.
6. Ao aprovar todos os testes, marque como concluído `[x]` e registre o commit atômico.

---

## 3. Agentes Especializados Disponíveis (`.claude/agents`)

O ecossistema conta com personas especialistas configuradas no diretório [`.claude/agents/`](.claude/agents):

### 3.1 [`rust-axum-architect`](.claude/agents/rust-axum-architect.md)
* **Especialidade**: Arquiteto e Revisor Sênior de Backend Rust.
* **Foco de Atuação**:
  * Projetar rotas Axum REST e streaming Server-Sent Events (SSE).
  * Otimizar inserções em lote no PostgreSQL via SQLx (`sped_repository.rs`).
  * Assegurar conformidade legal rigorosa com o layout contábil do SPED ECD da Receita Federal.
  * Gerenciar ciclo de vida, concorrência e graceful shutdown do worker Redis.
* **Comandos de Verificação do Agente**:
  ```bash
  cargo check -p ecd-server -p ecd-worker
  cargo test -p ecd-server -p ecd-worker
  cargo clippy --workspace -- -D warnings
  ```

### 3.2 [`react-spa-specialist`](.claude/agents/react-spa-specialist.md)
* **Especialidade**: Especialista Sênior em Frontend SPA (React 18, Vite, TypeScript).
* **Foco de Atuação**:
  * Implementação de telas de alta performance contábil com Shadcn UI e Tailwind CSS.
  * Virtualização de balancetes e lançamentos contábeis de 100k+ linhas (`TanStack Table`).
  * Visualização de grafos societários multinível interativos (`Vis-Network`) e painéis de indicadores (`Recharts`).
  * Consumo de streams SSE para acompanhamento em tempo real de uploads e jobs.
* **Comandos de Verificação do Agente**:
  ```bash
  cd apps/web
  npm test
  npm run build
  ```

### 3.3 [`ai-gateway-test-engineer`](.claude/agents/ai-gateway-test-engineer.md)
* **Especialidade**: Engenheiro de IA e Testes Automatizados (FastAPI, LangGraph, Pytest).
* **Foco de Atuação**:
  * Implementação de agentes cognitivos ReAct via LangGraph com injeção segura de contexto fiscal.
  * Auditoria dos contratos JSON (`ChartSpec`) gerados pelo LLM para renderização nativa no Recharts.
  * Manutenção e execução da suíte de paridade aritmética estrita (`tests/parity/test_sped_parity.py`) contra o legado Django.
* **Comandos de Verificação do Agente**:
  ```bash
  cd services/ai-gateway
  pytest tests/ -v
  pytest tests/parity/test_sped_parity.py -v
  ```

---

## 4. Checklist Mandatório de Verificação Pré-Commit

Nenhuma alteração pode ser integrada sem passar pelas verificações automatizadas locais:

```bash
# 1. Compilação e tipagem do workspace completo
make check

# 2. Bateria de testes do workspace Rust
make test

# 3. Formatação e linter rigoroso sem avisos
make lint

# 4. Testes do Frontend SPA
make test-web

# 5. Validação de empacotamento
make package
```
