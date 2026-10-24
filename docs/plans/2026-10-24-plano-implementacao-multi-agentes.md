# Plano de Implementação: Rearquitetura ECD Analytics (Rust + React SPA + AI Gateway)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Construir a nova plataforma de auditoria contábil, apuração fiscal e inteligência societária com backend em Rust de alta performance, frontend desacoplado em React + Shadcn UI e AI Gateway em LangChain, preservando 100% o schema das tabelas contábeis do SPED ECD da Receita Federal.

**Architecture:** Monorepo com Cargo Workspace modular separando a API HTTP leve (`apps/server`) do worker assíncrono consumidor de fila Redis (`apps/worker`), compartilhando crates nativas (`ecd-domain`, `ecd-parser`, `ecd-fiscal`, `ecd-graph`, `ecd-analytics`, `ecd-storage`). Comunicação com o frontend SPA via REST e Server-Sent Events (SSE), e orquestração de IA via microsserviço FastAPI + LangGraph.

**Tech Stack:** Rust 1.80+ (Axum, Tokio, SQLx, Rusqlite, Petgraph, Memmap2, Umya-spreadsheet), React 18+ (Vite, TypeScript, Tailwind CSS, Shadcn UI, TanStack Table, Recharts, Vis-Network), Python 3.11+ (FastAPI, LangChain, LangGraph), PostgreSQL 16, Redis 7.

**Spec:** [`docs/specs/2026-10-24-rearquitetura-ecd-analytics-rust-spa-design.md`](../specs/2026-10-24-rearquitetura-ecd-analytics-rust-spa-design.md)

---

## Global Constraints

1. **Inviolabilidade do Core**: Nenhuma coluna, tipo ou nome de tabela das entidades contábeis `core_registro0000`, `core_registroi010`, `core_registroi030`, `core_regi050`, `core_registroi150`, `core_registroi155`, `core_regi355`, `core_registroj005`, `core_registroj100`, `core_regj150`, `core_registroj210` pode ser alterada no PostgreSQL.
2. **Zero-Copy Streaming**: O leitor de arquivos SPED no `ecd-parser` deve utilizar `memmap2` e fatiamento direto em bytes (`&[u8]`), sem carregar arquivos completos no heap da aplicação.
3. **Persistência em Lote**: As inserções das linhas contábeis devem ser agrupadas em lotes de 5.000 a 10.000 registros por query parametrizada via SQLx.
4. **Isolamento de Carga**: O servidor HTTP `apps/server` nunca pode executar o parsing de arquivos SPED na thread da requisição; toda ingestão deve ser despachada para a fila do Redis.
5. **Autenticação Segura**: Senhas hasheadas exclusivamente com Argon2id; tokens JWT armazenados exclusivamente em cookies `HttpOnly; Secure; SameSite=Lax`.

## Review Focus

1. **Arquivos SPED Gigantes (1GB+) com estouro de memória**: Verificar que o leitor `memmap2` consome menos de 100MB de RAM mesmo com arquivos de 2GB.
2. **Caracteres especiais e acentuação no SPED**: Garantir que a decodificação `WINDOWS_1252` preserve caracteres latinos em razões sociais e descrições sem truncamento.
3. **Falhas no meio de um arquivo SPED**: Confirmar que erros de parsing acionam rollback atômico no PostgreSQL, sem deixar registros órfãos nas tabelas `core_*`.
4. **Consultas a grafos com nós de altíssima conectividade (holdings com 10.000+ sócios)**: Confirmar que a busca em largura limita o número de arestas por camada (`kLimiteCamada = 1000`) evitando explosão combinatória.
5. **Geração dinâmica de gráficos pelo Copiloto de IA**: Validar que a resposta JSON do LangChain sempre se ajusta ao schema estrito aceito pelo componente `DynamicChartRenderer`.

---

## Fase 1: Motor de Parsing SPED (`ecd-parser`) — [Agente 1]

### Tarefa 1.1: Implementação do Iterador Streaming Zero-Copy
**Arquivos:**
- Modificar: `crates/ecd-parser/src/lib.rs`
- Criar: `crates/ecd-parser/src/reader.rs`
- Criar: `crates/ecd-parser/tests/reader_test.rs`

**Passos:**
1. Escrever teste em `reader_test.rs` que cria um arquivo temporário com linhas de SPED e verifica a iteração correta de campos.
2. Executar `cargo test -p ecd-parser` e confirmar falha.
3. Implementar `SpedStreamReader::iter_records` fatiando fatias de bytes `&[u8]` entre delimitadores `b'|'`.
4. Executar `cargo test -p ecd-parser` e verificar aprovação.
5. Commit: `feat(parser): implementar iterador streaming de fatiamento zero-copy`

### Tarefa 1.2: Decodificadores dos Blocos 0000, I010, I030, I050 e I155
**Arquivos:**
- Criar: `crates/ecd-parser/src/records/bloco_0.rs`
- Criar: `crates/ecd-parser/src/records/bloco_i.rs`
- Modificar: `crates/ecd-parser/src/lib.rs`
- Criar: `crates/ecd-parser/tests/decoder_test.rs`

**Passos:**
1. Escrever testes unitários em `decoder_test.rs` decodificando linhas reais dos blocos 0000, I050 e I155.
2. Implementar funções de conversão `parse_0000(&[&[u8]]) -> Result<Registro0000, ParserError>`.
3. Implementar parsing de datas no formato `AAAAMMDD` e `DDMMAAAA` e números decimais com vírgula (`1234,56`).
4. Executar `cargo test -p ecd-parser` e confirmar aprovação.
5. Commit: `feat(parser): adicionar decodificadores tipados dos blocos 0000 e I`

### Tarefa 1.3: Decodificadores dos Blocos J005, J100, J150 e I355
**Arquivos:**
- Criar: `crates/ecd-parser/src/records/bloco_j.rs`
- Modificar: `crates/ecd-parser/src/lib.rs`
- Criar: `crates/ecd-parser/tests/dre_decoder_test.rs`

**Passos:**
1. Escrever testes com amostras de linhas J100 (Balanço) e J150 (DRE).
2. Implementar conversão para `RegistroJ100` e `RegJ150`.
3. Garantir parsing correto de campos de aglutinação e indicadores de débito/crédito (`D`/`C`).
4. Executar `cargo test -p ecd-parser` e confirmar aprovação.
5. Commit: `feat(parser): adicionar decodificadores tipados dos blocos J e DRE`

---

## Fase 2: Motor de Grafos GRC (Rede CNPJ / Turmalina) — [Agente 2]

### Tarefa 2.1: Pool SQLite e Conectores de Leitura
**Arquivos:**
- Modificar: `crates/ecd-graph/src/lib.rs`
- Criar: `crates/ecd-graph/src/db.rs`
- Criar: `crates/ecd-graph/tests/sqlite_pool_test.rs`

**Passos:**
1. Escrever teste em `sqlite_pool_test.rs` criando um banco SQLite de teste em memória com PRAGMAs otimizados.
2. Implementar `SqliteGraphStore` com pooling thread-safe `r2d2_sqlite` e suporte a `query_only = ON`.
3. Executar `cargo test -p ecd-graph` e confirmar aprovação.
4. Commit: `feat(graph): configurar pool SQLite r2d2 com pragmas de alta performance`

### Tarefa 2.2: Algoritmo de Expansão BFS em Camadas
**Arquivos:**
- Criar: `crates/ecd-graph/src/bfs.rs`
- Modificar: `crates/ecd-graph/src/lib.rs`
- Criar: `crates/ecd-graph/tests/bfs_traversal_test.rs`

**Passos:**
1. Criar fixture com grafo fictício de 3 empresas e 4 sócios em SQLite.
2. Implementar função `buscar_camadas(&self, raiz: &str, max_camadas: usize, limite_itens: usize) -> Result<GrafoRedeResult>`.
3. Validar que nós visitados não geram ciclos infinitos e que o limite de itens por camada é respeitado.
4. Executar `cargo test -p ecd-graph` e verificar aprovação.
5. Commit: `feat(graph): implementar busca em largura BFS com controle de camadas`

### Tarefa 2.3: Cruzamento de Sanções e Busca de Caminho Mínimo
**Arquivos:**
- Criar: `crates/ecd-graph/src/sanctions.rs`
- Criar: `crates/ecd-graph/src/shortest_path.rs`
- Modificar: `crates/ecd-graph/src/lib.rs`

**Passos:**
1. Implementar checagem de sanções cruzando com a tabela `links` (PEP, CEIS, CEPIM, CNEP, PGFN).
2. Implementar busca de caminho mais curto com `petgraph::algo::dijkstra`.
3. Executar `cargo test -p ecd-graph` e confirmar aprovação.
4. Commit: `feat(graph): adicionar cruzamento de sanções e cálculo de menor caminho`

---

## Fase 3: Motor Fiscal & Apuração de Receitas — [Agente 3]

### Tarefa 3.1: Fuzzy Matching e Classificador de DRE
**Arquivos:**
- Modificar: `crates/ecd-fiscal/src/lib.rs`
- Criar: `crates/ecd-fiscal/src/revenue.rs`
- Criar: `crates/ecd-fiscal/tests/revenue_test.rs`

**Passos:**
1. Escrever teste validando a classificação de receitas a partir de descrições reais com erros de grafia e acentuação.
2. Implementar algoritmo `ReceitaExtractor::classificar` aplicando regras de filtro de natureza credora e pontuação `strsim >= 0.80`.
3. Executar `cargo test -p ecd-fiscal` e confirmar aprovação.
4. Commit: `feat(fiscal): implementar extrator de receita bruta com fuzzy matching`

### Tarefa 3.2: Gerador de Planilhas Excel Multi-Aba
**Arquivos:**
- Criar: `crates/ecd-fiscal/src/xlsx.rs`
- Modificar: `crates/ecd-fiscal/src/lib.rs`
- Criar: `crates/ecd-fiscal/tests/xlsx_test.rs`

**Passos:**
1. Escrever teste gerando uma planilha XLSX com abas 0000, J100 e J150 em buffer de memória.
2. Implementar estilização de tabelas, cabeçalhos azuis corporativos e formatos numéricos monetários.
3. Executar `cargo test -p ecd-fiscal` e validar que o buffer gerado é um arquivo XLSX válido.
4. Commit: `feat(fiscal): implementar exportador XLSX multi-aba com formatação financeira`

---

## Fase 4: Worker Assíncrono com Redis (`apps/worker`) — [Agente 4]

### Tarefa 4.1: Conexão e Consumer da Fila Redis
**Arquivos:**
- Modificar: `apps/worker/src/main.rs`
- Criar: `apps/worker/src/consumer.rs`
- Criar: `apps/worker/src/job.rs`

**Passos:**
1. Implementar conexão com Redis e loop assíncrono de escuta com `BRPOP ecd:queue:jobs 5`.
2. Implementar decodificação do payload `ImportSpedJob`.
3. Adicionar publicação de eventos no canal Pub/Sub `ecd:events:status`.
4. Executar `cargo check -p ecd-worker`.
5. Commit: `feat(worker): implementar loop de consumo de fila Redis e notificações Pub/Sub`

### Tarefa 4.2: Persistência em Lote no PostgreSQL
**Arquivos:**
- Criar: `crates/ecd-storage/src/sped_repository.rs`
- Modificar: `apps/worker/src/consumer.rs`

**Passos:**
1. Implementar método `bulk_insert_registros` no `ecd-storage` usando queries parametrizadas com `QueryBuilder`.
2. Conectar o pipeline do `ecd-parser` com o `bulk_insert_registros` no worker.
3. Adicionar cálculo final de encerramento da apuração e atualização de `resultados_exercicio`.
4. Executar `cargo check -p ecd-worker`.
5. Commit: `feat(worker): integrar parser e persistência em massa de registros SPED`

---

## Fase 5: Servidor HTTP Axum (`apps/server`) — [Agente 5]

### Tarefa 5.1: Autenticação JWT com Cookies HttpOnly
**Arquivos:**
- Criar: `apps/server/src/auth.rs`
- Criar: `apps/server/src/handlers/auth_handler.rs`
- Modificar: `apps/server/src/main.rs`

**Passos:**
1. Implementar verificação de senha com Argon2id e geração de token JWT assinado.
2. Implementar rotas `/api/auth/login`, `/api/auth/me`, `/api/auth/logout`.
3. Implementar extractor Axum `AuthUser` para proteger rotas autenticadas.
4. Executar `cargo check -p ecd-server`.
5. Commit: `feat(server): implementar autenticação JWT com cookies HttpOnly e RBAC`

### Tarefa 5.2: Upload Streaming e Server-Sent Events (SSE)
**Arquivos:**
- Criar: `apps/server/src/handlers/upload_handler.rs`
- Criar: `apps/server/src/handlers/sse_handler.rs`
- Modificar: `apps/server/src/main.rs`

**Passos:**
1. Implementar endpoint `/api/importacoes/upload` lendo stream multipart diretamente para `media/ecd_stage/aguardando/`.
2. Implementar enfileiramento do job no Redis.
3. Implementar endpoint `/api/jobs/{id}/stream` retornando canal `Sse<impl Stream>` conectado ao Redis Pub/Sub.
4. Executar `cargo check -p ecd-server`.
5. Commit: `feat(server): implementar upload multipart streaming e stream SSE`

### Tarefa 5.3: Endpoints Fiscais e de Rede Societária
**Arquivos:**
- Criar: `apps/server/src/handlers/fiscal_handler.rs`
- Criar: `apps/server/src/handlers/grc_handler.rs`
- Modificar: `apps/server/src/main.rs`

**Passos:**
1. Implementar `/api/fiscal/consulta-cnpj/{cnpj}` e `/api/fiscal/exportar-xlsx/{cnpj}`.
2. Implementar `/api/grc/rede` e `/api/grc/caminho` integrados com `ecd-graph`.
3. Adicionar proxy reverso para `/api/ai/chat`.
4. Executar `cargo check -p ecd-server`.
5. Commit: `feat(server): implementar rotas de consulta fiscal, grafos e proxy de IA`

---

## Fase 6: AI Gateway com LangChain (`services/ai-gateway`) — [Agente 6]

### Tarefa 6.1: Catálogo de Ferramentas (*Tool Calling*)
**Arquivos:**
- Criar: `services/ai-gateway/app/tools/fiscal_tools.py`
- Criar: `services/ai-gateway/app/tools/graph_tools.py`
- Modificar: `services/ai-gateway/app/main.py`

**Passos:**
1. Criar ferramentas `@tool` no LangChain para consultar endpoints do Axum (`consultar_contabilidade`, `consultar_rede_societaria`).
2. Criar cliente `httpx.AsyncClient` com autenticação interna.
3. Testar execução de ferramentas via mock local.
4. Commit: `feat(ai): implementar catálogo de ferramentas fiscais e societárias no LangChain`

### Tarefa 6.2: Agente LangGraph & Gerador de Dashboards
**Arquivos:**
- Criar: `services/ai-gateway/app/agent/fiscal_agent.py`
- Modificar: `services/ai-gateway/app/main.py`

**Passos:**
1. Construir grafo de estado com LangGraph conectando modelo Groq Llama 3.1 às ferramentas.
2. Implementar gerador de especificações de gráficos Recharts em JSON.
3. Expor endpoint streaming `/api/ai/chat`.
4. Commit: `feat(ai): implementar agente analista fiscal e gerador dinâmico de gráficos`

---

## Fase 7: Frontend SPA React + Shadcn UI (`apps/web`) — [Agente 7]

### Tarefa 7.1: Setup de Layout e Navegação
**Arquivos:**
- Modificar: `apps/web/src/App.tsx`
- Criar: `apps/web/src/components/layout/AppLayout.tsx`
- Criar: `apps/web/src/components/layout/Sidebar.tsx`
- Criar: `apps/web/src/services/api.ts`

**Passos:**
1. Implementar layout com Sidebar retrátil, Header e navegação entre abas.
2. Configurar cliente Axios/Ky com credenciais habilitadas (`withCredentials: true`).
3. Commit: `feat(web): implementar layout principal e cliente de API`

### Tarefa 7.2: Tela de Ingestão e Monitor de Logs SSE
**Arquivos:**
- Criar: `apps/web/src/features/importacao/FileDropzone.tsx`
- Criar: `apps/web/src/features/importacao/ImportJobsTable.tsx`
- Criar: `apps/web/src/features/importacao/LiveLogDrawer.tsx`

**Passos:**
1. Implementar upload com barra de progresso.
2. Implementar tabela com status em tempo real alimentada via SSE (`EventSource`).
3. Commit: `feat(web): implementar tela de ingestão de arquivos e monitor de logs SSE`

### Tarefa 7.3: Tela de Auditoria Contábil e DRE
**Arquivos:**
- Criar: `apps/web/src/features/fiscal/CnpjSearchBar.tsx`
- Criar: `apps/web/src/features/fiscal/SpedBlocksTabs.tsx`
- Criar: `apps/web/src/features/fiscal/DreViewer.tsx`

**Passos:**
1. Implementar busca por CNPJ com máscara e validação de dígitos.
2. Implementar abas para Balanço (J100), DRE (J150) e Saldos (I155) com paginação TanStack Table.
3. Adicionar botão de exportação XLSX.
4. Commit: `feat(web): implementar visualizador de demonstrativos contábeis e auditoria DRE`

### Tarefa 7.4: Canvas Interativo da Rede CNPJ (Vis-Network)
**Arquivos:**
- Criar: `apps/web/src/features/grc/NetworkGraphCanvas.tsx`
- Criar: `apps/web/src/features/grc/NodeDetailDrawer.tsx`

**Passos:**
1. Integrar biblioteca `vis-network` para renderizar o grafo de nós societários.
2. Implementar física de força, zoom, filtros de camadas (1 a 10) e realce de sanções.
3. Implementar drawer lateral de detalhes ao clicar no nó.
4. Commit: `feat(web): implementar tela de grafo societário com Vis-Network e alertas sancionatórios`

### Tarefa 7.5: Painel do Copiloto de IA e Gráficos Dinâmicos
**Arquivos:**
- Criar: `apps/web/src/features/copilot/AiChatDrawer.tsx`
- Criar: `apps/web/src/features/copilot/DynamicChartRenderer.tsx`

**Passos:**
1. Implementar chat lateral expansível com streaming de texto em Markdown.
2. Implementar renderizador dinâmico de gráficos Recharts (Barras, Linhas, Pizza) a partir do JSON do agente.
3. Commit: `feat(web): implementar copiloto de IA com renderizador de dashboards em tempo real`

---

## Fase 8: Homologação Final, Benchmarks & Docker Compose

### Tarefa 8.1: Testes de Paridade com Django Legado
**Arquivos:**
- Criar: `tests/parity/test_sped_parity.py`

**Passos:**
1. Processar 5 arquivos SPED reais simultaneamente na versão Django e na versão Rust.
2. Comparar bit-a-bit os totais de `receita_bruta`, `receita_servico` e saldos contábeis finais.
3. Validar 100% de paridade aritmética.

### Tarefa 8.2: Benchmarks de Performance e Memória
**Arquivos:**
- Criar: `tests/benchmarks/benchmark_summary.md`

**Passos:**
1. Medir tempo de ingestão de arquivos de 500MB e 1GB.
2. Medir consumo de memória RSS do worker Rust vs processo Celery.
3. Registrar ganhos obtidos na documentação.
