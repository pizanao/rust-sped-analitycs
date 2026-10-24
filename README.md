# Speed Rust App — ECD Analytics Platform

> **Plataforma de Alta Performance em Rust para Auditoria Contábil, Ingestão SPED ECD, Rede CNPJ e Copiloto de IA.**

---

## 1. Visão Geral

Este repositório contém a nova arquitetura desacoplada e de alto desempenho do **ECD Analytics**, projetada para substituir a pilha Django/Celery legada por um ecossistema nativo moderno:

* **Backend Rust (`crates/*` e `apps/server`, `apps/worker`)**:
  * Parser de arquivos SPED ECD com I/O mapeado em memória virtual (`memmap2`) e fatiamento zero-copy de bytes.
  * Inserções em lote de dezenas de milhares de registros contábeis por segundo via `SQLx`.
  * Motor de grafos corporativos sobre a base da Receita Federal com `petgraph` e `rusqlite`.
  * Fila de tarefas assíncrona dedicada com Redis.
* **Frontend SPA (`apps/web`)**:
  * React + Vite + TypeScript + Tailwind CSS + Shadcn UI.
  * Tabelas virtualizadas com `TanStack Table`, visualizador interativo de grafos com `Vis-Network` e gráficos analíticos com `Recharts`.
* **AI Gateway (`services/ai-gateway`)**:
  * Microsserviço FastAPI com LangChain e LangGraph provendo ferramentas (*tool calling*) para análise fiscal e geração dinâmica de dashboards em linguagem natural.

---

## 2. Estrutura do Workspace

```
.
├── Cargo.toml                       # Workspace Cargo raiz
├── AGENT_GUIDE.md                   # Manual de trabalho concorrente para subagentes
├── TASKS.md                         # Quadro de tarefas e dependências
├── docker-compose.yml               # Orquestração local
├── migrations/                      # Migrações SQL do PostgreSQL
├── docs/
│   ├── specs/                       # Especificações arquiteturais completas
│   └── plans/                       # Planos de implementação detalhados
├── crates/
│   ├── ecd-domain/                  # Modelos oficiais do SPED RFB e DTOs
│   ├── ecd-parser/                  # Leitor zero-copy streaming de SPED
│   ├── ecd-fiscal/                  # Motor fiscal, fuzzy matching e XLSX
│   ├── ecd-graph/                   # Motor de grafos em SQLite (Rede CNPJ)
│   ├── ecd-analytics/               # Agregação de indicadores econômicos
│   └── ecd-storage/                 # Conexões SQLx (Postgres) e Rusqlite
├── apps/
│   ├── server/                      # Servidor HTTP Axum (REST, JWT, SSE)
│   ├── worker/                      # Consumer de tarefas pesadas com Redis
│   └── web/                         # Frontend SPA React + Vite + Shadcn UI
└── services/
    └── ai-gateway/                  # Microsserviço FastAPI + LangChain
```

---

## 3. Guia Rápido de Execução

### Pré-requisitos
* Rust 1.80+ (`cargo`)
* Node.js 20+ (`npm`)
* Python 3.11+
* Docker e Docker Compose

### Compilação do Backend Rust
```bash
cargo check
cargo test
cargo build --release
```

### Inicialização do Frontend
```bash
cd apps/web
npm install
npm run dev
```

### Inicialização do AI Gateway
```bash
cd services/ai-gateway
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
uvicorn app.main:app --reload --port 8000
```
