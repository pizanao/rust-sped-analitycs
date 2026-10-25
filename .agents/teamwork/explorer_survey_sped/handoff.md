# Relatório de Investigação Técnica: Requisitos e Expectativas do `rust-sped-analytics` para `cnpj.db`, `rede.db` e `links.db`

- **Data**: 2026-10-24
- **Autor**: Codebase Researcher (`explorer_survey_sped`)
- **Destinatário**: Orchestrator (`0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)
- **Status**: Concluído

---

## 1. Observation (Observações Diretas do Código e Sistema)

### 1.1 Arquitetura de Conexão e PRAGMAs SQLite
- **Arquivo**: `crates/ecd-graph/src/db.rs`, linhas 12-27, 88-128
  - O pool de conexões é gerenciado via `r2d2_sqlite::SqliteConnectionManager` com tamanho máximo padrão de 16 conexões (`DEFAULT_POOL_MAX_SIZE = 16`).
  - O banco principal de abertura é `rede.db`.
  - A conexão executa anexação dinâmica opcional (*database attachment*):
    ```sql
    ATTACH DATABASE '{cnpj_path}' AS cnpj_db;
    ATTACH DATABASE '{links_path}' AS links_db;
    ```
  - Em cada conexão do pool, os seguintes PRAGMAs são aplicados obrigatoriamente:
    ```sql
    PRAGMA mmap_size = 268435456; -- 256 MB por conexão
    PRAGMA cache_size = -64000;   -- 64 MB em páginas de memória
    PRAGMA temp_store = MEMORY;
    PRAGMA query_only = ON;       -- Modo estrito somente-leitura
    ```
  - Linhas 234-248 de `apps/server/src/lib.rs`: o caminho das bases é lido da variável de ambiente `TURMALINA_BASES_PATH`. Se existir, busca `rede.db`, `cnpj.db` e `links.db`.
  - Linhas 56 e 62 de `docker-compose.yml`: `TURMALINA_BASES_PATH: /apps/turmalina/bases` montado como volume `:ro`.

### 1.2 Todas as Consultas SQL Executadas no Sistema

#### A. Em `crates/ecd-graph/src/bfs.rs` (Expansão BFS e Descoberta de Nós)
1. **Detecção Dinâmica de Capacidades do Schema** (linhas 33-41, 246-252):
   ```sql
   SELECT EXISTS(SELECT 1 FROM main.sqlite_master WHERE type='table' AND name='entidades')
   SELECT EXISTS(SELECT 1 FROM cnpj_db.sqlite_master WHERE type='table' AND name='entidades')
   SELECT EXISTS(SELECT 1 FROM main.sqlite_master WHERE type='table' AND name='empresas')
   SELECT EXISTS(SELECT 1 FROM cnpj_db.sqlite_master WHERE type='table' AND name='empresas')
   SELECT EXISTS(SELECT 1 FROM main.sqlite_master WHERE type='table' AND name='ligacao')
   ```
2. **Resolução do Identificador Canônico de Entidade** (linhas 268-302):
   ```sql
   SELECT EXISTS(SELECT 1 FROM ligacao WHERE id1 = ?1 OR id2 = ?1)
   SELECT EXISTS(SELECT 1 FROM main.entidades WHERE id = ?1)
   SELECT EXISTS(SELECT 1 FROM cnpj_db.entidades WHERE id = ?1)
   ```
3. **Expansão de Arestas Societárias BFS por Camada** (linhas 125-129):
   ```sql
   SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
   FROM ligacao
   WHERE id1 = ?1 OR id2 = ?1
   ```
   *Nota: Executada com `prepare_cached` a cada nó expandido na fronteira.*
4. **Carga de Metadados de Entidade em `entidades`** (linhas 322-328):
   ```sql
   SELECT nome, tipo FROM main.entidades WHERE id = ?1
   SELECT nome, tipo FROM cnpj_db.entidades WHERE id = ?1
   ```
5. **Fallback para Razão Social em `empresas`** (linhas 380-395):
   ```sql
   SELECT razao_social FROM main.empresas WHERE cnpj_basico = ?1 LIMIT 1
   SELECT razao_social FROM cnpj_db.empresas WHERE cnpj_basico = ?1 LIMIT 1
   ```

#### B. Em `crates/ecd-graph/src/shortest_path.rs` (Algoritmo de Menor Caminho / Dijkstra)
1. **Verificação de Ligação Direta (1 Salto)** (linhas 114-118):
   ```sql
   SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
   FROM ligacao
   WHERE (id1 = ?1 AND id2 = ?2) OR (id1 = ?2 AND id2 = ?1)
   LIMIT 4
   ```
2. **Verificação de Pontes de 2 Saltos (Sócio/Empresa em Comum)** (linhas 146-156):
   ```sql
   SELECT COALESCE(l1.id1, ''), COALESCE(l1.id2, ''), COALESCE(l1.descricao, ''), COALESCE(l1.comentario, ''),
          COALESCE(l2.id1, ''), COALESCE(l2.id2, ''), COALESCE(l2.descricao, ''), COALESCE(l2.comentario, '')
   FROM ligacao l1
   JOIN ligacao l2
     ON (CASE WHEN l1.id1 = ?1 THEN l1.id2 ELSE l1.id1 END)
      = (CASE WHEN l2.id1 = ?2 THEN l2.id2 ELSE l2.id1 END)
   WHERE (l1.id1 = ?1 OR l1.id2 = ?1)
     AND (l2.id1 = ?2 OR l2.id2 = ?2)
   LIMIT 16
   ```
3. **Expansão Reversa BFS para Conectar Pontes** (linhas 202-206):
   ```sql
   SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
   FROM ligacao
   WHERE id1 = ?1 OR id2 = ?1
   ```

#### C. Em `crates/ecd-graph/src/sanctions.rs` (Cruzamento de Sanções GRC)
1. **Detecção da Tabela `links`** (linhas 132-147):
   ```sql
   SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='links')
   SELECT EXISTS(SELECT 1 FROM links_db.sqlite_master WHERE type='table' AND name='links')
   ```
2. **Consulta de Sanções por Identificador/Contraparte** (linhas 210-222):
   ```sql
   SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
   FROM main.links
   WHERE id1 = ?1 OR id2 = ?1
   ```
   e/ou
   ```sql
   SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
   FROM links_db.links
   WHERE id1 = ?1 OR id2 = ?1
   ```

#### D. Em `crates/ecd-analytics/src/aggregator.rs` (Agregação de Indicadores Econômicos RFB)
1. **Consulta Analítica Consolidada de Empresas RFB** (`SQLITE_QUERY_RFB_EMPRESAS`, linhas 834-852):
   ```sql
   SELECT e.cnpj,
          emp.porte_empresa,
          e.situacao_cadastral,
          e.cnae_fiscal,
          emp.natureza_juridica,
          emp.capital_social,
          s.opcao_mei,
          s.opcao_simples,
          e.uf,
          e.municipio
   FROM estabelecimento e
   LEFT JOIN empresas emp ON emp.cnpj_basico = e.cnpj_basico
   LEFT JOIN simples s ON s.cnpj_basico = e.cnpj_basico
   WHERE (?1 IS NULL OR e.situacao_cadastral != ?1)
     AND (?2 IS NULL OR e.uf = ?2)
     AND (?3 IS NULL OR e.municipio = ?3)
     AND (?4 IS NULL OR e.cnae_fiscal LIKE ?4 || '%')
   ```
2. **Verificação de Tabela em `apps/server/src/handlers/indicadores_handler.rs`** (linhas 95-99):
   ```sql
   SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='estabelecimento')
   ```

### 1.3 Verificação das Bases Físicas Reais do Ambiente
Foi inspecionado o diretório local `/apps/turmalina/bases/` via `sqlite3`:
- `/apps/turmalina/bases/rede.db` (5.787.578.368 bytes ~ 5.8 GB):
  ```sql
  CREATE TABLE ligacao(id1, id2, descricao TEXT, comentario);
  CREATE INDEX idx_ligacao_origem ON ligacao (id1);
  CREATE INDEX idx_ligacao_destino ON ligacao (id2);
  ```
- `/apps/turmalina/bases/cnpj.db` (45.288.509.440 bytes ~ 45 GB):
  - Contém as 10 tabelas oficiais da RFB: `empresas`, `estabelecimento`, `socios`, `simples`, `cnae`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio`.
  - Tabela auxiliar: `cnpj_base2matriz`, `_referencia`.
  - Índices presentes: `idx_empresas_cnpj_basico`, `idx_empresas_razao_social`, `idx_estabelecimento_cnpj_basico`, `idx_estabelecimento_cnpj`, `idx_estabelecimento_nomefantasia`, `estabelecimento_situacao_cadastral_index`, `idx_socios_cnpj`, `idx_socios_cnpj_cpf_socio`, `idx_socios_nome_socio`, `idx_socios_representante`, `idx_socios_representante_nome`, `idx_simples_cnpj_basico`, `idx_cnpj_base2matriz`.
- `/apps/turmalina/bases/links.db` (16.384 bytes):
  ```sql
  CREATE TABLE "links" ("id1", "id2", "descricao", "valor");
  CREATE INDEX "idx_links_id1" ON "links" ("id1");
  CREATE INDEX "idx_links_id2" ON "links" ("id2");
  ```
- `/apps/turmalina/bases/base_dados_modelo.db` (274.432 bytes):
  ```sql
  CREATE TABLE "links" ("id1" TEXT, "id2" TEXT, "descricao" TEXT, "valor" REAL, "comentario" TEXT);
  CREATE INDEX "idx_links_id1" ON "links" ("id1");
  CREATE INDEX "idx_links_id2" ON "links" ("id2");
  ```
- `/apps/turmalina/bases/rede_search.db` (14.921.388.032 bytes ~ 15 GB):
  ```sql
  CREATE VIRTUAL TABLE id_search using fts5 (id, descricao);
  ```

---

## 2. Logic Chain (Cadeia Lógica de Inferência)

1. **A partir da Observação 1.1**:
   O `rust-sped-analytics` utiliza `rede.db` como banco SQLite principal (`main`) na inicialização do `SqliteGraphStore`, anexando `cnpj.db` como `cnpj_db` e `links.db` como `links_db`. Todas as conexões aplicam `PRAGMA query_only = ON;`, `PRAGMA mmap_size = 268435456;` e `PRAGMA cache_size = -64000;`.
2. **A partir da Observação 1.2.A e 1.2.B**:
   As consultas de travessia BFS (`buscar_camadas`) e menor caminho (`buscar_caminho_minimo`) operam sobre a tabela `ligacao` buscando bidirecionalmente por `id1 = ?1 OR id2 = ?1`. A performance crítica dessa operação depende estritamente dos índices `idx_ligacao_origem` (em `id1`) e `idx_ligacao_destino` (em `id2`).
3. **A partir da Observação 1.2.A.4 e 1.2.A.5**:
   O motor BFS tenta carregar o nome/tipo da entidade primeiro na tabela `entidades` (verificando `main.entidades` e `cnpj_db.entidades`). Caso a tabela `entidades` não exista (o que é a realidade do `rede.db` original de 5.8 GB, conforme Observação 1.3), ele realiza fallback automático para `empresas` (`main.empresas` ou `cnpj_db.empresas`) consultando `razao_social WHERE cnpj_basico = ?1`.
4. **A partir da Observação 1.2.C e 1.3**:
   O módulo de sanções (`sanctions.rs`) consulta `links` procurando `(id1, id2, descricao, comentario)`. Na base física `/apps/turmalina/bases/links.db`, a tabela `links` possui as colunas `(id1, id2, descricao, valor)`, enquanto na base modelo `base_dados_modelo.db` possui `(id1, id2, descricao, valor, comentario)`. A presença de `comentario` é esperada pela consulta SQL de `sanctions.rs`.
5. **A partir da Observação 1.2.D e 1.3**:
   A agregação de indicadores econômicos em `crates/ecd-analytics` executa `SQLITE_QUERY_RFB_EMPRESAS`, exigindo as tabelas `estabelecimento`, `empresas` e `simples` com junções em `cnpj_basico`. A coluna `capital_social` é tipada como `real`, mas o parser Rust suporta tolerantemente `Real(f64)`, `Integer(i64)` e `Text(s)` formatado com pontos e vírgula.
6. **A partir da Observação 1.2.D.2**:
   Em `apps/server/src/handlers/indicadores_handler.rs:94-100`, há uma verificação `SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='estabelecimento')`. Como o `graph_store` abre `rede.db` como banco `main`, `sqlite_master` verifica apenas as tabelas de `rede.db` (que só contém `ligacao`). Portanto, quando `cnpj.db` está anexado como `cnpj_db`, a verificação em `main.sqlite_master` não encontra `estabelecimento` a menos que seja qualificado com `cnpj_db.sqlite_master` ou `cnpj.db` seja o banco primário.

---

## 3. Caveats (Ressalvas, Discrepâncias e Limitações Identificadas)

1. **Nome da Tabela de Empresas**:
   - No script legado `dados_cnpj_para_sqlite.py`, a tabela criada no `cnpj.db` chama-se **`empresas`** (no plural).
   - O código do `rust-sped-analytics` em `crates/ecd-graph/src/bfs.rs` e `crates/ecd-analytics/src/aggregator.rs` busca expressamente por **`empresas`** (no plural).
   - Em alguns documentos descritivos (como `ORIGINAL_REQUEST.md`), há menção a "empresa" (no singular). **Atenção**: para 100% de compatibilidade com o código compilado do `rust-sped-analytics`, o nome da tabela deve ser estritamente `empresas` (ou deve ser criado um `VIEW empresa AS SELECT * FROM empresas;`).
2. **Discrepância de Schema na Tabela `links`**:
   - Em `crates/ecd-graph/src/sanctions.rs` (linhas 211 e 219), a consulta executa:
     `SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '') FROM ...links`
   - Na base física encontrada em `/apps/turmalina/bases/links.db`, a tabela `links` possui colunas `("id1", "id2", "descricao", "valor")`, sem a coluna `comentario`. Isso causaria o erro SQLite `no such column: comentario` ao acionar cruzamento de sanções contra aquela base física específica.
   - Na base de modelo `base_dados_modelo.db`, a tabela `links` possui `("id1", "id2", "descricao", "valor", "comentario")`.
   - **Recomendação**: Na geração do novo `links.db` ou `rede.db`, garantir que a tabela `links` contenha ambas as colunas: `valor REAL` e `comentario TEXT`.
3. **Escopo do Handler de Indicadores em `apps/server`**:
   - `indicadores_handler.rs` faz `SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='estabelecimento')`. No SQLite, `sqlite_master` sem prefixo consulta o schema `main`. Como `rede.db` é o `main`, `estabelecimento` (que reside em `cnpj_db`) não é detectado. Para o pipeline funcionar com conexão unificada, deve-se considerar criar views sinônimas em `rede.db` apontando para `cnpj_db` ou ajustar a rota para checar `cnpj_db.sqlite_master`.
4. **Formatação de Chaves na Tabela `ligacao`**:
   - Na geração oficial do `rede-cnpj` (`rede_cria_tabela_rede.db.py`):
     - `id1` para Pessoa Física contém CPF e Nome concatenados: `'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio`.
     - `id1` e `id2` para Pessoa Jurídica recebem o prefixo: `'PJ_' || t.cnpj`.
     - Empresas estrangeiras recebem: `'PE_' || t.nome_socio`.
   - O `rust-sped-analytics` (`crates/ecd-graph/src/sanctions.rs:87-128`) foi projetado com gerador de candidatos (`chaves_candidatas_entidade`) que remove e adiciona prefixos `PJ_`, `PF_`, `FILIAL_`, `PE_` e formatações com e sem pontuação para garantir correspondência.
5. **Tabela `entidades` no `rede.db`**:
   - A base física real `/apps/turmalina/bases/rede.db` possui **apenas** a tabela `ligacao`.
   - O `rust-sped-analytics` suporta essa ausência através do seu mecanismo de detecção (`SchemaCapabilities::detect`) e faz fallback transparente para a tabela `empresas` em `cnpj_db` para recuperar a razão social. No entanto, se uma tabela `entidades(id, nome, tipo)` for adicionada ao `rede.db`, a performance de resolução de nós será ainda mais rápida (evitando consultas cruzadas ao `cnpj.db`).

---

## 4. Conclusion (Conclusão e Especificação Normativa de Schemas)

Para garantir **100% de compatibilidade** com o `rust-sped-analytics`, o pipeline em Rust a ser reconstruído deve produzir as bases SQLite obedecendo rigorosamente à seguinte especificação:

### 4.1 Especificação para `dados-publicos/cnpj.db`

#### Tabelas de Domínio / Apoio:
1. `cnae (codigo TEXT, descricao TEXT)` + `CREATE INDEX idx_cnae ON cnae(codigo);`
2. `motivo (codigo TEXT, descricao TEXT)` + `CREATE INDEX idx_motivo ON motivo(codigo);`
3. `municipio (codigo TEXT, descricao TEXT)` + `CREATE INDEX idx_municipio ON municipio(codigo);`
4. `natureza_juridica (codigo TEXT, descricao TEXT)` + `CREATE INDEX idx_natureza_juridica ON natureza_juridica(codigo);`
5. `pais (codigo TEXT, descricao TEXT)` + `CREATE INDEX idx_pais ON pais(codigo);`
6. `qualificacao_socio (codigo TEXT, descricao TEXT)` + `CREATE INDEX idx_qualificacao_socio ON qualificacao_socio(codigo);`

#### Tabelas Principais de Negócio:
7. **`empresas`** *(Atenção: nome no plural)*:
   ```sql
   CREATE TABLE empresas (
       cnpj_basico TEXT,
       razao_social TEXT,
       natureza_juridica TEXT,
       qualificacao_responsavel TEXT,
       porte_empresa TEXT,
       ente_federativo_responsavel TEXT,
       capital_social REAL
   );
   CREATE INDEX idx_empresas_cnpj_basico ON empresas (cnpj_basico);
   CREATE INDEX idx_empresas_razao_social ON empresas (razao_social);
   ```
8. **`estabelecimento`**:
   ```sql
   CREATE TABLE estabelecimento (
       cnpj_basico TEXT,
       matriz_filial TEXT,
       nome_fantasia TEXT,
       situacao_cadastral TEXT,
       data_situacao_cadastral TEXT,
       motivo_situacao_cadastral TEXT,
       nome_cidade_exterior TEXT,
       pais TEXT,
       data_inicio_atividades TEXT,
       cnae_fiscal TEXT,
       cnae_fiscal_secundaria TEXT,
       tipo_logradouro TEXT,
       logradouro TEXT,
       numero TEXT,
       complemento TEXT,
       bairro TEXT,
       cep TEXT,
       uf TEXT,
       municipio TEXT,
       ddd1 TEXT,
       telefone1 TEXT,
       ddd2 TEXT,
       telefone2 TEXT,
       ddd_fax TEXT,
       fax TEXT,
       correio_eletronico TEXT,
       situacao_especial TEXT,
       data_situacao_especial TEXT,
       cnpj TEXT
   );
   CREATE INDEX idx_estabelecimento_cnpj_basico ON estabelecimento (cnpj_basico);
   CREATE INDEX idx_estabelecimento_cnpj ON estabelecimento (cnpj);
   CREATE INDEX idx_estabelecimento_nomefantasia ON estabelecimento (nome_fantasia);
   CREATE INDEX estabelecimento_situacao_cadastral_index ON estabelecimento (situacao_cadastral, uf);
   ```
9. **`socios`**:
   ```sql
   CREATE TABLE socios (
       cnpj TEXT,
       cnpj_basico TEXT,
       identificador_de_socio TEXT,
       nome_socio TEXT,
       cnpj_cpf_socio TEXT,
       qualificacao_socio TEXT,
       data_entrada_sociedade TEXT,
       pais TEXT,
       representante_legal TEXT,
       nome_representante TEXT,
       qualificacao_representante_legal TEXT,
       faixa_etaria TEXT
   );
   CREATE INDEX idx_socios_cnpj ON socios(cnpj);
   CREATE INDEX idx_socios_cnpj_cpf_socio ON socios(cnpj_cpf_socio);
   CREATE INDEX idx_socios_nome_socio ON socios(nome_socio);
   CREATE INDEX idx_socios_representante ON socios(representante_legal);
   CREATE INDEX idx_socios_representante_nome ON socios(nome_representante);
   ```
10. **`simples`**:
    ```sql
    CREATE TABLE simples (
        cnpj_basico TEXT,
        opcao_simples TEXT,
        data_opcao_simples TEXT,
        data_exclusao_simples TEXT,
        opcao_mei TEXT,
        data_opcao_mei TEXT,
        data_exclusao_mei TEXT
    );
    CREATE INDEX idx_simples_cnpj_basico ON simples (cnpj_basico);
    ```
11. **Tabelas de Referência**:
    - `cnpj_base2matriz (cnpj_basico TEXT, cnpj TEXT)` + `CREATE INDEX idx_cnpj_base2matriz ON cnpj_base2matriz(cnpj_basico);`
    - `_referencia (referencia TEXT, valor TEXT);`

---

### 4.2 Especificação para `dados-publicos/rede.db`

1. **`ligacao`** *(Tabela primária obrigatória de arestas)*:
   ```sql
   CREATE TABLE ligacao (
       id1 TEXT NOT NULL,
       id2 TEXT NOT NULL,
       descricao TEXT NOT NULL DEFAULT 'SOCIO',
       comentario TEXT NOT NULL DEFAULT ''
   );
   CREATE INDEX idx_ligacao_origem ON ligacao (id1);
   CREATE INDEX idx_ligacao_destino ON ligacao (id2);
   ```
2. **`entidades`** *(Opcional na especificação do legado, mas altamente recomendada pelo `ecd-graph`)*:
   ```sql
   CREATE TABLE entidades (
       id TEXT PRIMARY KEY,
       nome TEXT NOT NULL,
       tipo TEXT NOT NULL DEFAULT 'PJ'
   );
   ```
3. **`links`** *(Pode residir em `rede.db` ou `links.db` anexado)*:
   ```sql
   CREATE TABLE links (
       id1 TEXT NOT NULL,
       id2 TEXT NOT NULL,
       descricao TEXT NOT NULL DEFAULT '',
       valor REAL DEFAULT 0.0,
       comentario TEXT NOT NULL DEFAULT ''
   );
   CREATE INDEX idx_links_id1 ON links (id1);
   CREATE INDEX idx_links_id2 ON links (id2);
   ```

---

### 4.3 Especificação para Busca Textual (`rede_search.db`)
```sql
CREATE VIRTUAL TABLE id_search USING fts5 (id, descricao);
```

---

## 5. Verification Method (Método de Verificação Independente)

Para validar de forma automatizada e independente as conclusões deste relatório:

1. **Execução da Bateria de Testes dos Módulos SQLite**:
   ```bash
   cargo test -p ecd-graph -p ecd-analytics -- --nocapture
   ```
   *Condição de Sucesso*: Todos os 32 testes de unidade e integração aprovados (`32 passed; 0 failed`).
2. **Inspeção Estática dos Schemas Físicos de Referência**:
   ```bash
   sqlite3 /apps/turmalina/bases/rede.db ".schema"
   sqlite3 /apps/turmalina/bases/cnpj.db ".schema"
   sqlite3 /apps/turmalina/bases/links.db ".schema"
   ```
3. **Validação das Consultas SQL no Código-Fonte**:
   - `grep -n "SQLITE_QUERY_RFB_EMPRESAS" crates/ecd-analytics/src/aggregator.rs` (confirmação da query analítica de empresas)
   - `grep -n "SELECT COALESCE(id1" crates/ecd-graph/src/bfs.rs` (confirmação da query BFS de arestas)
   - `grep -n "SELECT razao_social FROM" crates/ecd-graph/src/bfs.rs` (confirmação do nome `empresas`)
   - `grep -n "PRAGMA mmap_size" crates/ecd-graph/src/db.rs` (confirmação dos parâmetros de performance)
