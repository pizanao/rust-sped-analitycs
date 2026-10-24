# Plano de Implementação: Rearquitetura ECD Analytics (Rust + React SPA + AI Gateway)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Construir a nova plataforma de auditoria contábil, apuração fiscal e inteligência societária com backend em Rust de alta performance, frontend desacoplado em React + Shadcn UI e AI Gateway em LangChain, preservando 100% o schema das tabelas contábeis do SPED ECD da Receita Federal.

**Architecture:** Monorepo com Cargo Workspace modular separando a API HTTP leve (`apps/server`) do worker assíncrono consumidor de fila Redis (`apps/worker`), compartilhando crates nativas (`ecd-domain`, `ecd-parser`, `ecd-fiscal`, `ecd-graph`, `ecd-analytics`, `ecd-storage`). Comunicação com o frontend SPA via REST e Server-Sent Events (SSE), e orquestração de IA via microsserviço FastAPI + LangGraph.

**Tech Stack:** Rust 1.80+ (Axum, Tokio, SQLx, Rusqlite, Petgraph, Memmap2, Umya-spreadsheet, Moka), React 18+ (Vite, TypeScript, Tailwind CSS, Shadcn UI, TanStack Table, Recharts, Vis-Network), Python 3.11+ (FastAPI, LangChain, LangGraph), PostgreSQL 16, Redis 7.

**Spec:** [`docs/specs/2026-10-24-rearquitetura-ecd-analytics-rust-spa-design.md`](../specs/2026-10-24-rearquitetura-ecd-analytics-rust-spa-design.md)

---

## Global Constraints

1. **Inviolabilidade do Core**: Nenhuma coluna, tipo ou nome de tabela das 14 entidades contábeis (`core_registro0000`, `core_registroi010`, `core_registroi030`, `core_registroi050`, `core_registroi051`, `core_registroi052`, `core_registroi053`, `regi050_plano_contas`, `core_registroi150`, `core_registroi155`, `regi355_detalhes_contas_resultado`, `core_registroj005`, `core_registroj100`, `regj150_demonstracao_resultado`, `core_registroj210`, `core_registroj215`) pode ser alterada no PostgreSQL.
2. **Zero-Copy Streaming**: O leitor de arquivos SPED no `ecd-parser` deve utilizar `memmap2` e fatiamento direto em bytes (`&[u8]`), sem carregar arquivos completos no heap da aplicação.
3. **Persistência em Lote**: As inserções das linhas contábeis devem ser agrupadas em lotes de 5.000 registros por query parametrizada via SQLx em transação atômica.
4. **Isolamento de Carga**: O servidor HTTP `apps/server` nunca pode executar o parsing de arquivos SPED na thread da requisição; toda ingestão deve ser despachada para a fila do Redis via `ImportSpedJobPayload`.
5. **Autenticação Segura**: Senhas hasheadas exclusivamente com Argon2id; tokens JWT armazenados exclusivamente em cookies `HttpOnly; Secure; SameSite=Lax`.
6. **Paridade Aritmética Fiscal**: O cálculo de Receita Bruta no `ecd-fiscal` deve usar similaridade de melhor substring (`partial_ratio >= 0.80`) sobre o catálogo de 24 termos normalizados e aplicar os 12 filtros negativos do legado (`apps/fiscal/utils.py` e `utils_recfuze.py`).

## Review Focus

1. **Arquivos SPED Gigantes (1GB+) com estouro de memória**: Verificar que o leitor `memmap2` consome menos de 100MB de RAM mesmo com arquivos de 2GB.
2. **Caracteres especiais e acentuação no SPED**: Garantir que a decodificação `WINDOWS_1252` preserve caracteres latinos em razões sociais e descrições sem truncamento.
3. **Falhas no meio de um arquivo SPED**: Confirmar que erros de parsing acionam rollback atômico no PostgreSQL, sem deixar registros órfãos nas tabelas `core_*` e `reg*`.
4. **Consultas a grafos com nós de altíssima conectividade (holdings com 10.000+ sócios)**: Confirmar que a busca em largura limita o número de arestas por camada (`kLimiteCamada = 1000`) evitando explosão combinatória.
5. **Geração dinâmica de gráficos pelo Copiloto de IA**: Validar que a resposta JSON do LangChain sempre se ajusta ao schema estrito aceito pelo componente `DynamicChartRenderer`.

---

## Trilha 1: Motor de Parsing SPED (`crates/ecd-parser`) — [Agente 1 / Onda 1]

### Tarefa 1.1 (TASK-101): Implementação do Iterador Streaming Zero-Copy
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

### Tarefa 1.2 (TASK-102): Decodificadores dos Blocos 0000, I010, I030, I050, I051, I052, I053, I150 e I155
**Arquivos:**
- Criar: `crates/ecd-parser/src/records/bloco_0.rs`
- Criar: `crates/ecd-parser/src/records/bloco_i.rs`
- Modificar: `crates/ecd-parser/src/lib.rs`
- Criar: `crates/ecd-parser/tests/decoder_test.rs`

**Passos:**
1. Escrever testes unitários em `decoder_test.rs` decodificando linhas reais dos blocos `0000`, `I010`, `I030`, `I050` (`RegistroI050` + consolidação `RegI050` com `I051`/`I052`), `I053`, `I150` e `I155`.
2. Implementar funções de conversão tipadas retornando as structs de `ecd-domain`.
3. Implementar parsing de datas no formato `DDMMAAAA` / `AAAAMMDD` e números decimais com vírgula (`1234,56`).
4. Executar `cargo test -p ecd-parser` e confirmar aprovação.
5. Commit: `feat(parser): adicionar decodificadores tipados dos blocos 0000 e I`

### Tarefa 1.3 (TASK-103 & TASK-104): Decodificadores dos Blocos I355, J005, J100, J150, J210 e J215
**Arquivos:**
- Criar: `crates/ecd-parser/src/records/bloco_j.rs`
- Modificar: `crates/ecd-parser/src/lib.rs`
- Criar: `crates/ecd-parser/tests/dre_decoder_test.rs`

**Passos:**
1. Escrever testes com amostras de linhas `I355` (`RegI355`), `J005`, `J100` (Balanço), `J150` (`RegJ150` DRE), `J210` e `J215`.
2. Implementar conversão para `RegI355`, `RegistroJ005`, `RegistroJ100`, `RegJ150`, `RegistroJ210` e `RegistroJ215`.
3. Garantir parsing correto de campos de aglutinação (`ind_cod_agl`, `ind_grp_bal`, `ind_grp_dre`) e indicadores de débito/crédito (`D`/`C`).
4. Executar `cargo test -p ecd-parser` e confirmar aprovação.
5. Commit: `feat(parser): adicionar decodificadores tipados dos blocos I355 e J`

---

## Trilha 2: Motor de Grafos GRC (`crates/ecd-graph`) — [Agente 2 / Onda 1]

### Tarefa 2.1 (TASK-201): Pool SQLite e Conectores de Leitura
**Arquivos:**
- Modificar: `crates/ecd-graph/src/lib.rs`
- Criar: `crates/ecd-graph/src/db.rs`
- Criar: `crates/ecd-graph/tests/sqlite_pool_test.rs`

**Passos:**
1. Escrever teste em `sqlite_pool_test.rs` criando um banco SQLite de teste com PRAGMAs otimizados (`query_only = ON`, `mmap_size = 268435456`, `cache_size = -64000`).
2. Implementar `SqliteGraphStore` com pooling thread-safe `r2d2_sqlite`.
3. Executar `cargo test -p ecd-graph` e confirmar aprovação.
4. Commit: `feat(graph): configurar pool SQLite r2d2 com pragmas de alta performance`

### Tarefa 2.2 (TASK-202): Algoritmo de Expansão BFS em Camadas
**Arquivos:**
- Criar: `crates/ecd-graph/src/bfs.rs`
- Modificar: `crates/ecd-graph/src/lib.rs`
- Criar: `crates/ecd-graph/tests/bfs_traversal_test.rs`

**Passos:**
1. Criar fixture com grafo fictício de 3 empresas e 4 sócios em SQLite.
2. Implementar função `buscar_camadas(&self, raiz: &str, max_camadas: usize, limite_itens: usize) -> Result<GrafoRedeResult>`.
3. Validar que nós visitados não geram ciclos infinitos e que o limite de itens por camada (`kLimiteCamada = 1000`) é respeitado.
4. Executar `cargo test -p ecd-graph` e verificar aprovação.
5. Commit: `feat(graph): implementar busca em largura BFS com controle de camadas`

### Tarefa 2.3 (TASK-203, TASK-204 & TASK-205): Cruzamento de Sanções e Busca de Caminho Mínimo
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

## Trilha 3: Motor Fiscal & Apuração de Receitas (`crates/ecd-fiscal`) — [Agente 3 / Onda 1]

### Tarefa 3.1 (TASK-301 & TASK-302): Fuzzy Matching (`partial_ratio`) e Classificador de DRE
**Arquivos:**
- Modificar: `crates/ecd-fiscal/src/lib.rs`
- Criar: `crates/ecd-fiscal/src/revenue.rs`
- Criar: `crates/ecd-fiscal/tests/revenue_test.rs`

**Passos:**
1. Escrever teste validando a classificação de receitas a partir de descrições reais longas, com erros de grafia e acentuação, e garantindo que rubricas de deduções/impostos/receita líquida sejam rejeitadas pelos 12 filtros negativos.
2. Implementar algoritmo `ReceitaExtractor::classificar_dre` aplicando `elegivel_receita_j150` e pontuação `partial_ratio >= 0.80` sobre os 24 termos do legado.
3. Executar `cargo test -p ecd-fiscal` e confirmar aprovação.
4. Commit: `feat(fiscal): implementar extrator de receita bruta com partial_ratio e filtros negativos`

### Tarefa 3.2 (TASK-303 & TASK-304): Gerador de Planilhas Excel Multi-Aba
**Arquivos:**
- Criar: `crates/ecd-fiscal/src/xlsx.rs`
- Modificar: `crates/ecd-fiscal/src/lib.rs`
- Criar: `crates/ecd-fiscal/tests/xlsx_test.rs`

**Passos:**
1. Escrever teste gerando uma planilha XLSX com abas `0000`, `J100`, `J150` e `I155` em buffer de memória.
2. Implementar estilização de tabelas, cabeçalhos azuis corporativos e formatos numéricos monetários (`R$ #,##0.00`) e datas (`DD/MM/YYYY`).
3. Executar `cargo test -p ecd-fiscal` e validar que o buffer gerado é um arquivo XLSX válido.
4. Commit: `feat(fiscal): implementar exportador XLSX multi-aba com formatação financeira`

---

## Trilha 4: Motor de Indicadores Econômicos (`crates/ecd-analytics`) — [Agente 4 / Onda 1]

### Tarefa 4.1 (TASK-401, TASK-402 & TASK-403): Agregações Econômicas e Cache `moka`
**Arquivos:**
- Modificar: `crates/ecd-analytics/src/lib.rs`
- Criar: `crates/ecd-analytics/src/aggregator.rs`
- Criar: `crates/ecd-analytics/src/cache.rs`
- Criar: `crates/ecd-analytics/tests/analytics_test.rs`

**Passos:**
1. Implementar cálculo de distribuições agregadas (CNAE Top 15, Porte Empresarial, Regime Tributário Simples/MEI, Situação Cadastral e Capital Social Médio) compatível com `apps/indicadores/models.py`.
2. Configurar cache assíncrono em memória com `moka::future::Cache` e expiração configurável.
3. Executar `cargo test -p ecd-analytics` e verificar aprovação.
4. Commit: `feat(analytics): implementar agregador de indicadores econômicos e cache moka`

---

## Trilha 5: Frontend SPA React + Shadcn UI (`apps/web`) — [Agente 5 / Onda 1]

### Tarefa 5.1 (TASK-501): Setup de Layout e Navegação
**Arquivos:**
- Modificar: `apps/web/src/App.tsx`
- Criar: `apps/web/src/components/layout/AppLayout.tsx`
- Criar: `apps/web/src/components/layout/Sidebar.tsx`
- Criar: `apps/web/src/services/api.ts`

**Passos:**
1. Implementar layout com Sidebar retrátil, Header e navegação entre abas.
2. Configurar cliente Axios com credenciais habilitadas (`withCredentials: true`).
3. Commit: `feat(web): implementar layout principal e cliente de API`

### Tarefa 5.2 (TASK-502): Tela de Ingestão e Monitor de Logs SSE
**Arquivos:**
- Criar: `apps/web/src/features/importacao/FileDropzone.tsx`
- Criar: `apps/web/src/features/importacao/ImportJobsTable.tsx`
- Criar: `apps/web/src/features/importacao/LiveLogDrawer.tsx`

**Passos:**
1. Implementar upload com barra de progresso.
2. Implementar tabela com status em tempo real alimentada via SSE (`EventSource`).
3. Commit: `feat(web): implementar tela de ingestão de arquivos e monitor de logs SSE`

### Tarefa 5.3 (TASK-503): Tela de Auditoria Contábil e DRE
**Arquivos:**
- Criar: `apps/web/src/features/fiscal/CnpjSearchBar.tsx`
- Criar: `apps/web/src/features/fiscal/SpedBlocksTabs.tsx`
- Criar: `apps/web/src/features/fiscal/DreViewer.tsx`

**Passos:**
1. Implementar busca por CNPJ com máscara e validação de dígitos.
2. Implementar abas para Balanço (`J100`), DRE (`J150`) e Saldos (`I155`) com TanStack Table.
3. Adicionar botão de exportação XLSX.
4. Commit: `feat(web): implementar visualizador de demonstrativos contábeis e auditoria DRE`

### Tarefa 5.4 (TASK-504): Canvas Interativo da Rede CNPJ (Vis-Network) e Dashboard de Indicadores
**Arquivos:**
- Criar: `apps/web/src/features/grc/NetworkGraphCanvas.tsx`
- Criar: `apps/web/src/features/grc/NodeDetailDrawer.tsx`
- Criar: `apps/web/src/features/indicadores/EconomicIndicatorsDashboard.tsx`

**Passos:**
1. Integrar biblioteca `vis-network` para renderizar o grafo de nós societários com filtros de 1 a 10 camadas e realce de sanções.
2. Implementar gráficos de distribuição de CNAE, Porte e Regime Tributário com Recharts.
3. Commit: `feat(web): implementar grafo societário Vis-Network e dashboard de indicadores`

### Tarefa 5.5 (TASK-505): Painel do Copiloto de IA e Gráficos Dinâmicos
**Arquivos:**
- Criar: `apps/web/src/features/copilot/AiChatDrawer.tsx`
- Criar: `apps/web/src/features/copilot/DynamicChartRenderer.tsx`

**Passos:**
1. Implementar chat lateral expansível com streaming de texto em Markdown.
2. Implementar renderizador dinâmico de gráficos Recharts (Barras, Linhas, Pizza) a partir do JSON do agente.
3. Commit: `feat(web): implementar copiloto de IA com renderizador de dashboards em tempo real`

---

## Trilha 6: AI Gateway com LangChain (`services/ai-gateway`) — [Agente 6 / Onda 1]

### Tarefa 6.1 (TASK-601 & TASK-602): Catálogo de Ferramentas (*Tool Calling*)
**Arquivos:**
- Criar: `services/ai-gateway/app/tools/fiscal_tools.py`
- Criar: `services/ai-gateway/app/tools/graph_tools.py`
- Modificar: `services/ai-gateway/app/main.py`

**Passos:**
1. Criar ferramentas `@tool` no LangChain para consultar endpoints do Axum (`buscar_dados_cadastrais`, `apurar_demonstrativo_dre`, `analisar_grafo_societario`, `consultar_indicadores_setor`, `construir_grafico_dashboard`).
2. Criar cliente `httpx.AsyncClient` com autenticação interna.
3. Commit: `feat(ai): implementar catálogo de ferramentas fiscais e societárias no LangChain`

### Tarefa 6.2 (TASK-603 & TASK-604): Agente LangGraph & Gerador de Dashboards
**Arquivos:**
- Criar: `services/ai-gateway/app/agent/fiscal_agent.py`
- Criar: `services/ai-gateway/tests/test_agent_schema.py`
- Modificar: `services/ai-gateway/app/main.py`

**Passos:**
1. Construir grafo de estado com LangGraph conectando modelo LLM às ferramentas.
2. Implementar gerador de especificações de gráficos Recharts em JSON e testes de contrato com `pytest`.
3. Commit: `feat(ai): implementar agente analista fiscal e gerador dinâmico de gráficos`

---

## Trilha 7: Worker Assíncrono & Batch Storage (`apps/worker` + `sped_repository.rs`) — [Agente 7 / Onda 2]

### Tarefa 7.1 (TASK-701 & TASK-702): Consumer da Fila Redis e Repositório de Lote
**Arquivos:**
- Modificar: `crates/ecd-storage/src/sped_repository.rs`
- Modificar: `apps/worker/src/main.rs`
- Criar: `apps/worker/src/consumer.rs`

**Passos:**
1. Implementar `bulk_insert_registros` em `crates/ecd-storage/src/sped_repository.rs` usando queries parametrizadas com `QueryBuilder` (lotes de 5.000 linhas) em transação atômica.
2. Implementar loop assíncrono no `apps/worker` com `BRPOP ecd:queue:jobs 5` decodificando `ImportSpedJobPayload`.
3. Executar `cargo check -p ecd-worker`.
4. Commit: `feat(worker): implementar consumer Redis e repositório batch SQLx`

### Tarefa 7.2 (TASK-703 & TASK-704): Pipeline Completo de Ingestão e Notificações Pub/Sub
**Arquivos:**
- Modificar: `apps/worker/src/consumer.rs`

**Passos:**
1. Integrar `ecd-parser` -> `sped_repository` -> `ecd-fiscal` (`resultados_exercicio`).
2. Publicar `JobStatusEvent` no canal Redis Pub/Sub `ecd:events:status`.
3. Executar `cargo check -p ecd-worker`.
4. Commit: `feat(worker): integrar pipeline completo SPED, apuração DRE e eventos Pub/Sub`

---

## Trilha 8: Servidor HTTP Axum (`apps/server` + `auth/fiscal_repository.rs`) — [Agente 8 / Onda 2]

### Tarefa 8.1 (TASK-801): Autenticação JWT com Cookies HttpOnly
**Arquivos:**
- Modificar: `crates/ecd-storage/src/auth_repository.rs`
- Criar: `apps/server/src/auth.rs`
- Criar: `apps/server/src/handlers/auth_handler.rs`
- Modificar: `apps/server/src/main.rs`

**Passos:**
1. Implementar verificação de senha com Argon2id e geração de token JWT assinado em cookie `HttpOnly`.
2. Implementar rotas `/api/auth/login`, `/api/auth/me`, `/api/auth/logout` e extractor `AuthUser`.
3. Executar `cargo check -p ecd-server`.
4. Commit: `feat(server): implementar autenticação JWT com cookies HttpOnly e RBAC`

### Tarefa 8.2 (TASK-802 & TASK-803): Upload Streaming e Server-Sent Events (SSE)
**Arquivos:**
- Criar: `apps/server/src/handlers/upload_handler.rs`
- Criar: `apps/server/src/handlers/sse_handler.rs`
- Modificar: `apps/server/src/main.rs`

**Passos:**
1. Implementar endpoint `/api/importacoes/upload` gravando em `media/ecd_stage/aguardando/` e enfileirando `ImportSpedJobPayload` no Redis.
2. Implementar endpoint `/api/jobs/{id}/stream` retornando canal `Sse<impl Stream>` subscrito em `ecd:events:status`.
3. Executar `cargo check -p ecd-server`.
4. Commit: `feat(server): implementar upload multipart streaming e stream SSE`

### Tarefa 8.3 (TASK-804): Endpoints Fiscais, Rede Societária, Indicadores e Proxy IA
**Arquivos:**
- Modificar: `crates/ecd-storage/src/fiscal_repository.rs`
- Criar: `apps/server/src/handlers/fiscal_handler.rs`
- Criar: `apps/server/src/handlers/grc_handler.rs`
- Criar: `apps/server/src/handlers/indicadores_handler.rs`
- Modificar: `apps/server/src/main.rs`

**Passos:**
1. Implementar `/api/fiscal/consulta-cnpj/{cnpj}` e `/api/fiscal/exportar-xlsx/{cnpj}`.
2. Implementar `/api/grc/rede`, `/api/grc/caminho`, `/api/indicadores/resumo` e proxy reverso `/api/ai/chat`.
3. Executar `cargo check -p ecd-server`.
4. Commit: `feat(server): implementar rotas fiscais, grafos, indicadores e proxy de IA`

---

## Trilha 9: Homologação Final, Benchmarks & Docker Compose — [Onda 3]

### Tarefa 9.1 (TASK-901): Dockerfiles Multi-Stage
**Arquivos:**
- Criar: `Dockerfile.server`, `Dockerfile.worker`, `apps/web/Dockerfile`, `services/ai-gateway/Dockerfile`

### Tarefa 9.2 (TASK-902): Testes de Paridade com Django Legado
**Arquivos:**
- Criar: `tests/parity/test_sped_parity.py`

### Tarefa 9.3 (TASK-903): Benchmarks de Performance e Memória
**Arquivos:**
- Criar: `tests/benchmarks/benchmark_summary.md`
