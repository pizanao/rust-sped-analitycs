# Speed Rust App — ECD Analytics Platform

> **Plataforma Corporativa de Alta Performance em Rust + React SPA + AI Gateway (LangChain/LangGraph) para Auditoria Contábil SPED ECD, Apuração Fiscal de Receitas e Inteligência Societária (Rede CNPJ / Turmalina).**

---

## 1. Visão Geral da Arquitetura

O **Speed Rust App** é a rearquitetura moderna e desacoplada do **ECD Analytics**, projetada para processar arquivos massivos da Escrituração Contábil Digital (SPED ECD de 100 MB a 2 GB+) em segundos com uso mínimo de RAM, preservando **100% de compatibilidade com o modelo de dados contábil oficial da Receita Federal do Brasil (RFB)**.

### Principais Pilares Técnicos
* **Motor de Ingestão Zero-Copy (`ecd-parser` + `apps/worker`)**:
  * Leitura via mapeamento de memória virtual (`memmap2::Mmap`), fatiamento zero-copy em `&[u8]` e decodificação `WINDOWS_1252`.
  * Persistência transacional em lotes (`batch inserts` de 5.000 registros por query via `SQLx`) desacoplada da API HTTP por fila Redis (`ecd:queue:jobs`).
* **Motor Fiscal de Paridade Estrita (`ecd-fiscal`)**:
  * Identificação automática de Receita Bruta, Receita de Serviços e Outras Receitas na DRE (`regj150_demonstracao_resultado`) e no bloco `I355` (`regi355_detalhes_contas_resultado`) usando `partial_ratio >= 0.80` sobre os 24 termos fiscais e os 12 filtros negativos do sistema legado.
  * Exportação nativa de planilhas Excel multi-aba (`umya-spreadsheet`).
* **Motor de Grafos Societários GRC (`ecd-graph`)**:
  * Consultas somente-leitura ultrarrápidas (`rusqlite` + `r2d2_sqlite` com `mmap_size = 256MB`) sobre as bases locais da Receita Federal (`cnpj.db`, `rede.db`, `links.db`).
  * Expansão em largura (BFS de 1 a 10 camadas com poda anti-explosão), menor caminho via Dijkstra (`petgraph`) e cruzamento atômico de sanções (**PEP, CEIS, CEPIM, CNEP, PGFN**).
* **Motor de Indicadores Econômicos (`ecd-analytics`)**:
  * Agregações estatísticas por CNAE, Porte Empresarial, Regime Tributário (Simples/MEI) e Capital Social com cache assíncrono em memória (`moka`).
* **AI Gateway Cognitivo (`services/ai-gateway`)**:
  * Microsserviço Python (**FastAPI + LangChain + LangGraph**) com *Tool Calling* sobre o servidor Rust para auditoria em linguagem natural e geração dinâmica de gráficos JSON.
* **Frontend SPA Desacoplado (`apps/web`)**:
  * **React 18 + Vite + TypeScript + Tailwind CSS + Shadcn UI**.
  * Tabelas contábeis de alta densidade (`TanStack Table`), grafos interativos (`Vis-Network`), dashboards (`Recharts`) e acompanhamento de ingestão em tempo real via Server-Sent Events (`SSE`).

---

## 2. Estrutura do Monorepo

```text
speed_rust_app/
├── Cargo.toml                       # Workspace Cargo raiz (6 crates + 2 binários)
├── MEMORY.md                        # Memória arquitetural, decisões de paridade e ajustes
├── AGENT_GUIDE.md                   # Protocolo de isolamento para execução multi-agentes
├── TASKS.md                         # Quadro de tarefas (DAG) dividido em Ondas 1, 2 e 3
├── docker-compose.yml               # Orquestração local (Postgres 16, Redis 7, Server, Worker, Web, AI)
├── migrations/
│   └── 0001_initial_schema.sql      # Schema PostgreSQL: 100% paridade Core RFB + tabelas UUIDv7
├── docs/
│   ├── specs/
│   │   └── 2026-10-24-rearquitetura-ecd-analytics-rust-spa-design.md
│   └── plans/
│       └── 2026-10-24-plano-implementacao-multi-agentes.md
├── crates/
│   ├── ecd-domain/                  # Structs SPED (0000, I010..I355, J005..J215), CNPJ e contratos Redis
│   ├── ecd-parser/                  # Leitor streaming zero-copy (memmap2 + encoding_rs)
│   ├── ecd-fiscal/                  # Extrator DRE (partial_ratio >= 0.80) e exportador XLSX
│   ├── ecd-graph/                   # Grafo societário Rede CNPJ (petgraph + rusqlite)
│   ├── ecd-analytics/               # Indicadores econômicos CNAE/Porte e cache moka
│   └── ecd-storage/                 # Repositórios SQLx (sped_repository, fiscal_repository, auth_repository)
├── apps/
│   ├── server/                      # API HTTP Axum (REST, Auth Argon2id/JWT HttpOnly, SSE, Upload)
│   ├── worker/                      # Consumer assíncrono Redis (Parsing SPED + Batch Inserts)
│   └── web/                         # Frontend SPA (React + Vite + TypeScript + Shadcn UI)
└── services/
    └── ai-gateway/                  # Microsserviço FastAPI + LangChain / LangGraph
```

---

## 3. Convivência de Dados Híbrida (PostgreSQL + SQLite)

### 3.1 Tabelas Contábeis Preservadas no PostgreSQL (`core`)
Todas as tabelas contábeis originais mantêm 100% de compatibilidade de nomes e colunas com o sistema legado:
* **Bloco 0**: `core_registro0000`
* **Bloco I**: `core_registroi010`, `core_registroi030`, `core_registroi050`, `core_registroi051`, `core_registroi052`, `core_registroi053`, `regi050_plano_contas`, `core_registroi150`, `core_registroi155`, `regi355_detalhes_contas_resultado`
* **Bloco J**: `core_registroj005`, `core_registroj100`, `regj150_demonstracao_resultado`, `core_registroj210`, `core_registroj215`
* **Catálogo RFB**: `plano_contas_base`

### 3.2 Tabelas de Suporte Otimizadas no PostgreSQL
* `usuarios` (Argon2id, RBAC: `Admin`, `Analista`, `Visualizador`)
* `configuracoes_sistema` (Parâmetros em `JSONB`)
* `importacoes_ecd` (Controle de jobs, métricas de linhas e tempo em ms)
* `resultados_exercicio` (Apuração consolidada de receitas com `JSONB` indexado via GIN e constraint `UNIQUE NULLS NOT DISTINCT`)

### 3.3 Bases SQLite Locais (Rede CNPJ / Turmalina)
* `cnpj.db`: Empresas, estabelecimentos, CNAE, Simples Nacional e sócios.
* `rede.db`: Arestas pré-indexadas de vínculos societários.
* `links.db`: Base consolidada de sanções e restrições (**PEP, CEIS, CEPIM, CNEP, PGFN**).

---

## 4. Guia Rápido de Desenvolvimento e Verificação

### Pré-requisitos
* **Rust**: `1.80+` (`rustup`, `cargo`, `clippy`, `rustfmt`)
* **Node.js**: `20+` (`npm`)
* **Python**: `3.11+`
* **Infraestrutura**: PostgreSQL 16 e Redis 7 (ou via `docker compose`)

### 1. Compilar e Testar o Workspace Rust
```bash
# Checagem rápida de todos os 6 crates e 2 binários
cargo check --workspace

# Executar todos os testes unitários
cargo test --workspace

# Verificar formatação e lints
cargo fmt --check
cargo clippy --workspace -- -D warnings
```

### 2. Executar o Frontend SPA (`apps/web`)
```bash
cd apps/web
npm install
npm run dev
```

### 3. Executar o AI Gateway (`services/ai-gateway`)
```bash
cd services/ai-gateway
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
uvicorn app.main:app --reload --port 8000
```

---

## 5. Execução Paralela por Múltiplos Agentes

O projeto está organizado para desenvolvimento paralelo sem conflitos de arquivos. Antes de iniciar qualquer implementação, consulte:
1. [`MEMORY.md`](MEMORY.md) — Regras de paridade e decisões técnicas.
2. [`AGENT_GUIDE.md`](AGENT_GUIDE.md) — Escopo exclusivo de diretórios por agente (`Agente 1` a `Agente 8`) e protocolo de verificação.
3. [`TASKS.md`](TASKS.md) — Quadro de tarefas e grafo de dependências em 3 ondas.
4. [`docs/plans/2026-10-24-plano-implementacao-multi-agentes.md`](docs/plans/2026-10-24-plano-implementacao-multi-agentes.md) — Passo a passo TDD por tarefa.
