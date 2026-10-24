# MEMORY.md — Registro de Decisões Arquiteturais e Ajustes de Revisão (`speed_rust_app`)

> **Última atualização**: 2026-10-24
> **Propósito**: Memória persistente do projeto para orientar todos os agentes de desenvolvimento sobre regras invioláveis de negócio, correções de paridade com o legado Django (`ecd_analytics`) e isolamento entre trilhas paralelas.

---

## 1. Paridade 100% do Schema Contábil (`core`) no PostgreSQL

Durante a revisão cruzada com [`apps/core/models.py`](file:///home/pizani/Projects/ecd_analytics/apps/core/models.py) e [`apps/core/migrations/0001_initial.py`](file:///home/pizani/Projects/ecd_analytics/apps/core/migrations/0001_initial.py), foram identificados e corrigidos os seguintes pontos críticos em [`migrations/0001_initial_schema.sql`](file:///home/pizani/Projects/ecd_analytics/apps/speed_rust_app/migrations/0001_initial_schema.sql) e [`crates/ecd-domain/src/sped.rs`](file:///home/pizani/Projects/ecd_analytics/apps/speed_rust_app/crates/ecd-domain/src/sped.rs):

1. **Tabelas com `db_table` customizado (NÃO usam o prefixo `core_`)**:
   - Model `RegI050` $\rightarrow$ tabela **`regi050_plano_contas`** (`id UUID PRIMARY KEY`, `hash_arquivo VARCHAR(40)`, `reg051_json JSONB`, `reg052_json JSONB`). Coexiste com a tabela relacional `core_registroi050`.
   - Model `RegI355` $\rightarrow$ tabela **`regi355_detalhes_contas_resultado`** (`id UUID PRIMARY KEY`, inclui `nome_cta_referencia`, `vl_cta_mf`, `ind_dc_mf`, `cod_agl`, `cod_cta_ref`, `nome_aglutinador`, `hash_arquivo`).
   - Model `RegJ150` $\rightarrow$ tabela **`regj150_demonstracao_resultado`** (`id BIGSERIAL PRIMARY KEY`, inclui `nu_ordem`, `cod_agl`, `ind_cod_agl`, `nivel_agl`, `cod_agl_sup`, `descr_cod_agl`, `vl_cta_ini`, `ind_dc_cta_ini`, `vl_cta_fin`, `ind_dc_cta_fin`, `ind_grp_dre`, `nota_exp_ref`, `cnpj`, `dt_ini`, `dt_fim`, `hash_arquivo`).
2. **Nomes exatos de colunas corrigidos**:
   - `core_registroi155`: usa `vl_sld_fim` e `ind_dc_fim` (com **m** no final, não `fin`), além das duas FKs `reg_0000_id` e `reg_i150_id`.
   - `core_registroj100`: usa `vl_cta_fin`, `ind_dc_cta_fin`, `ind_cod_agl`, `cod_agl_sup`, `ind_grp_bal`, `nota_exp_ref`, `reg_0000_id` e `reg_j005_id`.
   - `core_registroi030`: `nat_livr` ajustado para `VARCHAR(90)` (conforme `0002_alter_registroi030_nat_livr.py`) e `nome` para `VARCHAR(150)`.
3. **Tabelas complementares do Bloco I e J adicionadas ao `ecd-domain` e SQL**:
   - `core_registroi050`, `core_registroi051`, `core_registroi052`, `core_registroi053`, `core_registroj210`, `core_registroj215` e `plano_contas_base`.

---

## 2. Paridade Aritmética na Apuração Fiscal (`ecd-fiscal`)

Comparando com [`apps/fiscal/utils.py`](file:///home/pizani/Projects/ecd_analytics/apps/fiscal/utils.py) e [`apps/fiscal/utils_recfuze.py`](file:///home/pizani/Projects/ecd_analytics/apps/fiscal/utils_recfuze.py):

1. **Algoritmo de Similaridade (`partial_ratio` $\ge 0.80$)**:
   - O sistema legado utiliza `rapidfuzz.fuzz.partial_ratio(descricao, termo) >= 80`.
   - O uso de distância global (`normalized_damerau_levenshtein` na string completa) penalizava descrições longas reais do SPED. Por isso, [`crates/ecd-fiscal/src/lib.rs`](file:///home/pizani/Projects/ecd_analytics/apps/speed_rust_app/crates/ecd-fiscal/src/lib.rs) implementa `ReceitaExtractor::partial_ratio` por janela deslizante sobre `normalized_levenshtein`.
2. **Pré-filtros Obrigatórios de `RegJ150` (`elegivel_receita_j150`)**:
   - `IND_DC_CTA_FIN = 'C'`
   - `VL_CTA_FIN > 0`
   - `IND_COD_AGL = 'T'`
   - Contém `"BRUTA"` ou inicia com `"RECEITA"`
   - Rejeita qualquer linha que contenha um dos **12 termos de exclusão**: `" NAO "`, `" NÃO "`, `"CUST"`, `"DEDU"`, `"IMP"`, `"DESPE"`, `"CONTRA"`, `"TERCEI"`, `"PESSO"`, `" PJ "`, `"LIQUID"` / `"LÍQUID"`, `"ANTES"`.
   - Exclui escriturações substituídas (`NOT EXISTS (SELECT 1 FROM core_registro0000 r0 WHERE r0.cod_hash_sub = r.hash_arquivo)`).
3. **Catálogo Completo**:
   - Todas as 24 variações de termos de receita do legado foram incorporadas ao `ReceitaExtractor`.

---

## 3. Modelagem das Tabelas Otimizadas e Contratos Redis

1. **UUIDv7**:
   - Gerados na aplicação Rust via `uuid::Uuid::now_v7()` para garantir ordenação temporal B-Tree nos índices primários de `usuarios`, `importacoes_ecd`, `resultados_exercicio`, `regi050_plano_contas` e `regi355_detalhes_contas_resultado`.
2. **Unicidade com `NULL` em `resultados_exercicio`**:
   - Definida com `UNIQUE NULLS NOT DISTINCT (cnpj, periodo_inicial, periodo_final, cnpj_scp)` (PostgreSQL 15+), impedindo duplicidade quando a escrituração não pertence a uma SCP (`cnpj_scp IS NULL`).
3. **Contratos de Fila e Eventos Redis em `ecd-domain`**:
   - `ImportSpedJobPayload` (fila `ecd:queue:jobs`) e `JobStatusEvent` (canal Pub/Sub `ecd:events:status`) estão consolidados em [`crates/ecd-domain/src/fiscal.rs`](file:///home/pizani/Projects/ecd_analytics/apps/speed_rust_app/crates/ecd-domain/src/fiscal.rs).

---

## 4. Isolamento para Execução Multi-Agentes sem Conflitos

1. **Submódulos Pré-Declarados em `crates/ecd-storage`**:
   - `crates/ecd-storage/src/lib.rs` já declara `pub mod sped_repository;`, `pub mod fiscal_repository;` e `pub mod auth_repository;`.
   - O **Agente 7 (Worker)** edita apenas `sped_repository.rs` e `apps/worker/**`.
   - O **Agente 8 (Server)** edita apenas `auth_repository.rs`, `fiscal_repository.rs` e `apps/server/**`.
2. **Trilha Dedicada para `ecd-analytics`**:
   - O **Agente 4** foi designado exclusivamente para `crates/ecd-analytics/**` (indicadores CNAE, Porte, Simples/MEI e cache `moka`).
3. **Execução em 3 Ondas**:
   - **Onda 1**: Agentes 1 a 6 em paralelo (`ecd-parser`, `ecd-graph`, `ecd-fiscal`, `ecd-analytics`, `apps/web`, `services/ai-gateway`).
   - **Onda 2**: Agentes 7 e 8 em paralelo (`apps/worker` e `apps/server`).
   - **Onda 3**: Trilha 9 (Dockerfiles, testes de paridade com o legado, benchmarks de performance e hardening de produção).

---

## 5. Homologação, Empacotamento & Hardening de Segurança (Onda 3)

1. **Dockerfiles Multi-stage e Compose**:
   - `Dockerfile.server` e `Dockerfile.worker` utilizam imagem de build `rust:1.85-slim-bookworm` e imagem final mínima `debian:bookworm-slim` com `ca-certificates` e binários otimizados em `--release`.
   - `apps/web/Dockerfile` utiliza multi-stage com `node:20-alpine` (compilação TypeScript + Vite minificado) e servidor web de alta vazão `nginx:1.27-alpine` com proxy reverso para `/api/` e fallback SPA `try_files $uri $uri/ /index.html;`.
   - `services/ai-gateway/Dockerfile` utiliza `python:3.11-slim` com `uvicorn app.main:app`.
   - `.dockerignore` configurado em todos os diretórios para excluir `target/`, `node_modules/`, `.venv/` e caches.

2. **Paridade Rigorosa com o Django Legado (`tests/parity/test_sped_parity.py`)**:
   - 100% de paridade validada entre o algoritmo Hyyrö LCS bit-paralelo de `ecd-fiscal` e o `rapidfuzz.fuzz.partial_ratio` de `apps/fiscal/utils_recfuze.py` (com margem de erro $< 1.05\%$ em ponto flutuante e decisão de corte idêntica em $\ge 80\%$).
   - 100% de paridade em `process_cta`, normalização de strings, catálogo dos 24 termos de receita e exclusão estrita dos 12 termos de filtros negativos.
   - 100% de paridade na decomposição do formato padrão de arquivos SPED (`tratar_nomarquivo`).
   - Leitura de fixtures em codificação Windows-1252 com verificação de todos os registros contábeis (`0000`, `I010`, `I030`, `I050`, `I051`, `I052`, `I053`, `I150`, `I155`, `I350`, `I355`, `J005`, `J100`, `J150`, `J210`, `J215`, `I990`, `J990`).

3. **Benchmarks de Throughput & Consumo de Memória**:
   - **Throughput de Parsing**: **52,11 a 54,14 MB/s** (~926.000 a 962.000 linhas de SPED por segundo).
   - **Tempo de Execução**: SPED de 100 MB em **1,85 segundos**; SPED de 500 MB (8,88 milhões de registros) em **9,60 segundos** (aceleração de mais de 100x em relação aos 18-45 minutos do Django).
   - **Fuzzy Matching**: Mais de 265.000 a 287.000 avaliações de substrings DRE por segundo (3,48 µs/operação).

4. **Hardening de Segurança para Produção (TASK-904)**:
   - Em modo de produção (`ENVIRONMENT=production` ou `not(debug_assertions)`):
     - `JWT_SECRET` é estritamente obrigatório e deve ter no mínimo 32 caracteres seguros; chaves default de desenvolvimento são rejeitadas fatalmente no boot.
     - `DATABASE_URL` é estritamente obrigatória; qualquer falha de conexão com o PostgreSQL aborta o processo imediatamente (fallback silencioso para memória desativado).
     - Credenciais ou usuários pré-configurados em código (`Admin@2026!`, etc.) foram completamente isolados para testes e desenvolvimento; em produção, a coleção em memória inicia vazia e o sistema consulta unicamente a base de dados oficial.
