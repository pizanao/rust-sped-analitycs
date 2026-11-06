# Rust Sped Analytics

<p align="center">
  <img src="docs/assets/social-preview.png" alt="Rust Sped Analytics - Banner de Social Media e Apresentação" width="100%" />
</p>

> **Plataforma Corporativa de Alta Performance para Auditoria Fiscal Contábil (SPED ECD), Apuração de Base Tributável do ISSQN e Inteligência Societária Municipal.**

---

## Stack Tecnológica

<p align="left">
  <img src="https://img.shields.io/badge/Rust-1.85-orange?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Axum-0.7-black?style=for-the-badge&logo=rust&logoColor=white" alt="Axum" />
  <img src="https://img.shields.io/badge/React-18.3-61DAFB?style=for-the-badge&logo=react&logoColor=black" alt="React" />
  <img src="https://img.shields.io/badge/TypeScript-5.6-3178C6?style=for-the-badge&logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/Vite-5.4-646CFF?style=for-the-badge&logo=vite&logoColor=white" alt="Vite" />
  <img src="https://img.shields.io/badge/Tailwind_CSS-3.4-38B2AC?style=for-the-badge&logo=tailwind-css&logoColor=white" alt="Tailwind CSS" />
  <img src="https://img.shields.io/badge/Python-3.11-3776AB?style=for-the-badge&logo=python&logoColor=white" alt="Python" />
  <img src="https://img.shields.io/badge/FastAPI-0.115-009688?style=for-the-badge&logo=fastapi&logoColor=white" alt="FastAPI" />
  <img src="https://img.shields.io/badge/LangGraph-ReAct-FF6F00?style=for-the-badge&logo=langchain&logoColor=white" alt="LangGraph" />
  <img src="https://img.shields.io/badge/PostgreSQL-16-336791?style=for-the-badge&logo=postgresql&logoColor=white" alt="PostgreSQL" />
  <img src="https://img.shields.io/badge/Redis-7-DC382D?style=for-the-badge&logo=redis&logoColor=white" alt="Redis" />
  <img src="https://img.shields.io/badge/SQLite-3-003B57?style=for-the-badge&logo=sqlite&logoColor=white" alt="SQLite" />
  <img src="https://img.shields.io/badge/Docker-Compose-2496ED?style=for-the-badge&logo=docker&logoColor=white" alt="Docker" />
  <img src="https://img.shields.io/badge/GNU_Make-4-00599C?style=for-the-badge&logo=gnu&logoColor=white" alt="GNU Make" />
</p>

---

## Apresentação para o Auditor Fiscal Tributário Municipal

Caro(a) **Auditor(a) Fiscal de Tributos Municipais**,

A fiscalização eficiente do **ISSQN** (Imposto Sobre Serviços de Qualquer Natureza) e a identificação de fraudes fiscais demandam cada vez mais o cruzamento das declarações emitidas pelo contribuinte (NFS-e, PGMEI, DeSTDA) com as suas escriturações contábeis oficiais transmitidas à Receita Federal do Brasil.

O **SPED ECD** (*Escrituração Contábil Digital*) contém os lançamentos analíticos (Livro Diário) e sintéticos (Razão, Balancete e Demonstrações Contábeis) da pessoa jurídica. No entanto, arquivos de escrituração contábil frequentemente ultrapassam centenas de megabytes (100 MB a 2 GB), tornando a abertura e a auditoria em softwares convencionais (como Excel ou ferramentas legadas) inviáveis por esgotamento de memória.

### O que esta plataforma entrega para a sua rotina fiscal?

1. **Processamento em Segundos**: Arquivos SPED ECD de 500 MB+ são fatiados e gravados no banco de dados em poucos segundos usando streaming de baixo nível em **Rust**, sem travar a máquina.
2. **Apuração Automatizada da Receita de Serviços (ISSQN)**:
   - Extrai automaticamente as contas de receita a partir da **DRE** (Demonstração do Resultado do Exercício — Registro `J150`) e das Contas de Resultado (Registro `I355`).
   - Identifica se o contribuinte faturou receitas de prestação de serviços não declaradas nas notas fiscais do município.
   - Exporta relatórios analíticos em **planilhas Excel multi-aba** para anexar diretamente aos autos de infração e notificações fiscais.
3. **Rede Societária Municipal & Grupos Econômicos (Rede CNPJ / Turmalina)**:
   - Identifica em até 10 camadas de profundidade se a empresa fiscalizada possui sócios em comum, empresas coligadas ou interpostas pessoas ("laranjas").
   - Cruza automaticamente os sócios com bases nacionais de sanções: **PEP** (Pessoas Expostas Politicamente), **CEIS**, **CEPIM**, **CNEP** e **Dívida Ativa da PGFN**.
   - Fundamental para caracterização de grupo econômico de fato e desconsideração da personalidade jurídica.
4. **Copiloto de Inteligência Artificial Especialista em SPED**:
   - Faça perguntas em português claro (*ex: "Qual foi o faturamento bruto e a receita de serviços no primeiro trimestre de 2025?"*).
   - O assistente consulta a base de dados do SPED e gera relatórios com gráficos dinâmicos para subsidiar seu parecer fiscal.

---

## Guia Rápido: Como Executar a Aplicação Localmente

Você não precisa ser um programador experiente para rodar e testar esta plataforma. Desenvolvemos comandos simplificados via **`make`** que preparam todo o ambiente automaticamente.

### 1. Pré-requisitos Básicos no seu Computador
* **Docker** e **Docker Compose** instalados e em execução.
* Ferramenta de linha de comando **`make`** (nativa no Linux/macOS ou via WSL2 no Windows).
* **Python 3** (para o gerador de arquivos de teste SPED).

---

### 2. Iniciando Todos os Serviços

Abra o terminal na pasta do projeto e execute:

```bash
make start
```

> **O que esse comando faz nos bastidores?**
> 1. Cria as pastas de trabalho (`media/ecd_stage/` para arquivos aguardando, processados e com erro).
> 2. Configura automaticamente o arquivo de ambiente `.env` com configurações padrão seguras.
> 3. Inicializa todos os containers em segundo plano:
>    - **PostgreSQL 16**: Banco de dados relacional com o schema oficial do SPED ECD.
>    - **Redis 7**: Fila assíncrona de alto desempenho.
>    - **Server (Rust / Axum)**: API central de autenticação e consultas.
>    - **Worker (Rust)**: Motor de parsing veloz que processa as escriturações.
>    - **AI Gateway (FastAPI)**: Assistente cognitivo com LangChain/LangGraph.
>    - **Web (React SPA)**: Painel web acessível no seu navegador.

Para verificar se todos os serviços estão saudáveis:
```bash
make status
```

---

### 3. Gerando Arquivos SPED Fictícios para Teste de Auditoria

Para experimentar a plataforma sem precisar utilizar arquivos reais de contribuintes em fase de homologação, utilize o gerador de massa de dados oficial do projeto:

```bash
make generate-mock
```
*(ou se preferir executar diretamente com Python: `python3 scripts/gerar_massa_sped.py`)*

Esse script gera automaticamente na pasta `tests/fixtures/sped/` uma coleção de arquivos SPED válidos:
* **`ecd_minimo_pva.txt`**: Estrutura mínima compatível com o Programa Validador e Assinador da RFB (PVA).
* **`ecd_homologacao_completa.txt`**: Arquivo completo contendo Balanço Patrimonial (`J100`), DRE com Receita de Serviços (`J150`) e Balancete Analítico (`I155`).
* **`ecd_substituta.txt`**: Escrituração retificadora que substitui a anterior (testando controle de escrituração ativa).
* **`ecd_volume_10k.txt`**: Arquivo com 10.000 lançamentos contábeis para teste de velocidade e estresse.

> **Nota de Conformidade Legal**: Todos os CNPJs gerados possuem **Dígito Verificador matematicamente válido** (módulo 11), codificação `Windows-1252` e totalizadores do Bloco 9 (`9900`, `9990`, `9999`) rigorosamente balanceados.

---

### 4. Acessando a Interface no Navegador

Após iniciar o sistema, abra seu navegador de internet no endereço:

**[http://localhost:3000](http://localhost:3000)**

#### Roteiro Prático de Auditoria na Interface:

1. **Ingestão de Escrituração**:
   - Acesse o menu **Ingestão**.
   - Arraste para o campo de upload o arquivo `tests/fixtures/sped/ecd_homologacao_completa.txt`.
   - Acompanhe o processamento do arquivo em tempo real pelo log de eventos na tela.
2. **Auditoria Contábil**:
   - Acesse o menu **Auditoria**.
   - Digite o CNPJ da empresa presente no arquivo ou pesquise na listagem.
   - Visualize as abas oficiais:
     - **0000**: Dados cadastrais da empresa e período da escrituração.
     - **J100**: Balanço Patrimonial (Ativo, Passivo e Patrimônio Líquido).
     - **J150**: DRE com destaque automático para as contas de **Receita de Serviços**.
     - **I155**: Balancete mensal analítico com saldos iniciais, débitos, créditos e saldos finais.
   - Clique em **Exportar Excel** para baixar o relatório completo consolidado em `.xlsx`.
3. **Inteligência Societária (GRC / Turmalina)**:
   - Acesse o menu **Rede Societária**.
   - Insira um CNPJ para visualizar o grafo interativo de relacionamentos, sócios comuns e participações societárias.
   - Verifique alertas de restrições ou sanções governamentais associadas aos sócios.
4. **Copiloto de IA**:
   - Abra a gaveta do **Copiloto de IA** e faça perguntas analíticas sobre os números auditados.

---

### 5. Acompanhando Logs do Sistema

Se desejar inspecionar o que está acontecendo no processamento em tempo real:

```bash
# Ver logs gerais de todos os serviços
make logs

# Ver apenas os logs do processador de SPED (Worker)
make logs-worker

# Ver apenas os logs do Servidor de APIs
make logs-server
```
*(Para sair da tela de logs a qualquer momento, pressione `Ctrl + C`)*

---

### 6. Como Parar ou Reiniciar o Sistema

Quando terminar os seus testes ou expediente de auditoria:

```bash
# Para pausar a execução
make stop

# Para reiniciar caso precise
make restart

# Para limpar o ambiente completamente (incluindo dados do banco)
make clean-all
```

---

## Tabela de Referência de Comandos `make`

| Comando | Finalidade para o Auditor / Desenvolvedor |
| :--- | :--- |
| `make help` | Exibe a lista completa de comandos explicados |
| `make start` | Sobe a aplicação inteira em Docker em background |
| `make stop` | Para com segurança todos os serviços |
| `make restart` | Reinicia todos os containers |
| `make status` | Informa se os serviços e bancos de dados estão online |
| `make generate-mock`| Cria os arquivos de teste SPED na pasta `tests/fixtures/sped/` |
| `make logs` | Mostra os registros de processamento e auditoria em tempo real |
| `make package` | Gera o pacote compactado de distribuição (`dist/*.tar.gz`) |
| `make test` | Roda a bateria de testes automatizados de validação contábil |
| `make db-shell` | Abre o terminal interativo do PostgreSQL (`psql`) |

---

## Métricas de Performance Homologadas

| Métrica | Resultado |
| :--- | :--- |
| Throughput de Parsing | **52 – 54 MB/s** (~930.000 linhas/s) |
| SPED de 100 MB | **~1,85 s** (vs. 18–45 min no Django) |
| SPED de 500 MB (8,88 M registros) | **~9,60 s** (100× mais rápido) |
| Fuzzy Matching DRE | **>265.000 avaliações/s** (3,48 µs/op) |
| Testes Rust (workspace) | **82/82 aprovados** |
| Testes Paridade Django | **7/7 aprovados** |
| Testes AI Gateway | **43/43 aprovados** |
| Testes Frontend SPA | **15/15 aprovados** |
| Testes ETL RFB CNPJ | **142/142 aprovados** |
| Consumo de memória (ETL) | **< 110 MB RSS** (teto: 1,8 GB) |

---

## Estrutura de Pastas e Módulos

```text
rust-sped-analitycs/
├── Makefile                       # Comandos simplificados de execução, build e testes
├── Cargo.toml                     # Configuração central do workspace Rust
├── docker-compose.yml             # Orquestrador dos containers (Postgres, Redis, Web, API)
├── AGENTS.md                      # Protocolo multi-agentes e guia de concorrência
├── AGENT_GUIDE.md                 # Protocolo detalhado de reivindicação e verificação de tarefas
├── TASKS.md                       # Quadro de tarefas por trilha (concluídas)
├── MEMORY.md                      # Registro de decisões arquiteturais invioláveis
├── migrations/
│   └── 0001_initial_schema.sql    # Schema PostgreSQL 16 com paridade 100% ao Django legado
├── apps/
│   ├── server/                    # API REST + SSE (Axum 0.7, JWT/Argon2id, RBAC)
│   ├── worker/                    # Consumer Redis BRPOP + pipeline de parsing batch
│   └── web/                       # SPA (React 18 + Vite + TypeScript + Tailwind + Shadcn UI)
├── crates/
│   ├── ecd-domain/                # Contratos: structs SPED ECD e eventos Redis
│   ├── ecd-parser/                # Streaming zero-copy memmap2 (WINDOWS-1252, CRLF)
│   ├── ecd-fiscal/                # Apuração DRE: partial_ratio, 24 termos, 12 filtros, XLSX
│   ├── ecd-graph/                 # BFS 1-10 camadas, Dijkstra, sanções PEP/CEIS/CEPIM/CNEP/PGFN
│   ├── ecd-analytics/             # Indicadores CNAE/Porte/Simples + cache moka TTL
│   ├── ecd-storage/               # Pool SQLx PostgreSQL + repositories isolados por trilha
│   └── rfb-cnpj-etl/              # Pipeline ETL RFB CNPJ (HTTP 206, streaming zip, cnpj.db/rede.db)
├── services/
│   └── ai-gateway/                # Copiloto IA: FastAPI + LangGraph ReAct + ChartSpec JSON
├── scripts/
│   ├── gerar_massa_sped.py        # Gerador de arquivos SPED sintéticos (CNPJ DV-válido)
│   └── package.sh                 # Empacotamento de distribuição (dist/*.tar.gz)
└── tests/
    ├── fixtures/sped/             # Arquivos SPED gerados para testes e homologação
    └── parity/
        └── test_sped_parity.py    # Paridade aritmética estrita vs. Django legado (7/7)
```

---

## Pipeline de Dados Abertos RFB CNPJ (`crates/rfb-cnpj-etl`)

A inteligência societária do módulo `ecd-graph` depende de bases SQLite geradas pela crate nativa integrada ao workspace em:

```
crates/rfb-cnpj-etl/
```

Expõe tanto biblioteca (`rfb_cnpj_etl`) quanto binário CLI (`rfb-cnpj-etl`) e alvos no `Makefile` raiz (`make cnpj-download`, `make cnpj-build`, `make cnpj-rede`, `make cnpj-status`, `make cnpj-all`). Essa ferramenta realiza:
- **Download resumível HTTP 206** com crawler WebDAV para detectar o diretório mensal mais recente dos dados abertos da RFB.
- **Descompressão streaming zero-disk** de arquivos `.zip` diretamente em memória (sem extrair GBs de CSV para disco).
- **Geração de `cnpj.db`**: 12 tabelas SQLite com 18 índices — dados cadastrais, estabelecimentos, sócios, CNAEs, expansão de sócios PJ via `cnpj_base2matriz`.
- **Geração de `rede.db`**: 7 regras de vínculos societários + índice FTS5 (`unicode61 remove_diacritics 2`) para busca insensível a acentos.
- Consumo de memória de pico mantido **< 110 MB RSS** (limite de projeto: 1,8 GB).
- **142 testes automatizados** aprovados (134 no ETL + 8 de compatibilidade em `crates/ecd-graph`).

---

## Desenvolvimento e Contribuição

Para orientações sobre desenvolvimento com múltiplos agentes de IA em paralelo e especificações técnicas de cada módulo, consulte [`AGENTS.md`](AGENTS.md), [`AGENT_GUIDE.md`](AGENT_GUIDE.md) e [`TASKS.md`](TASKS.md).
