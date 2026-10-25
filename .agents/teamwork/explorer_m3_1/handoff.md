# Handoff Report — Graph Derivation SQL & Schema Architect (Milestone 3)

- **Agent**: `explorer_m3_1` (Role: Graph Derivation SQL & Schema Architect)
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Consumer Codebase**: `/home/pizani/Projects/rust-sped-analitycs/crates/ecd-graph`
- **Output Databases**:
  - `dados-publicos/rede.db` (`ligacao`, `entidades` + indices)
  - `dados-publicos/rede_search.db` (`id_search` FTS5 virtual table)
  - Compatibility with `dados-publicos/links.db` (GRC sanctions)
- **Date**: 2026-10-25T00:42:00Z

---

## 1. Observation

### 1.1 Existing Implementations and Test Specifications
1. **Legacy Derivation Reference (`rede_cria_tabela_rede.db.py`)**:
   - Located at `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/rede_cria_tabela_rede.db.py:49-144`.
   - Uses a temporary staging table `ligacao1 (origem, destino, tipo, base)` to stage links from `cnpj.socios`, `cnpj.estabelecimento`, and `cnpj.qualificacao_socio`.
   - Populates final table via:
     ```sql
     CREATE TABLE ligacao AS
     SELECT origem as id1, destino as id2, tipo as descricao, base as comentario
     FROM ligacao1
     GROUP BY origem, destino, tipo, base;
     ```
   - Creates indexes: `CREATE INDEX idx_ligacao_origem ON ligacao (id1);` and `CREATE INDEX idx_ligacao_destino ON ligacao (id2);`.
   - In lines 189-216, creates virtual table `id_search USING fts5 (id, descricao)` in `rede_search.db`.

2. **Automated Test Contract (`tests/e2e_pipeline_test.rs`)**:
   - Located at `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/tests/e2e_pipeline_test.rs:527-621`.
   - Verifies the derivation into `ligacao1` and `ligacao`:
     - Line 532: PJ -> PJ (socios)
     - Line 539: PF -> PJ (socios)
     - Line 546: PE -> PJ (foreign partner)
     - Line 553: PF -> PE (legal representative of foreign partner)
     - Line 560: PF -> PJ (legal representative of corporate partner)
     - Line 567: PF -> PF (legal representative of individual partner)
     - Line 574: Filial -> Matriz (`estabelecimento`)
     - Lines 586-587: `idx_ligacao_origem (id1)` and `idx_ligacao_destino (id2)`.
     - Lines 592-616: Verifies exact edge lookups:
       - `edge_exists(&conn, "PJ_11111111000191", "PJ_22222222000182")` with details `("Sócio", "socios")`.
       - `edge_exists(&conn, "PF_***123456**-JOAO DA SILVA", "PJ_11111111000191")`.
       - `edge_exists(&conn, "PE_GLOBAL VENTURES LLC", "PJ_44444444000164")`.
       - `edge_exists(&conn, "PF_***987654**-MARIA REP PE", "PE_GLOBAL VENTURES LLC")`.
       - `edge_exists(&conn, "PF_***555444**-ROBERTO REP PJ", "PJ_11111111000191")`.
       - `edge_exists(&conn, "PF_***111222**-PAULO PAI", "PF_***333222**-PEDRO MENOR")`.
       - `edge_exists(&conn, "PJ_33333333000254", "PJ_33333333000173")` with details `("filial", "estabelecimento")`.

3. **Consumer Contract in `crates/ecd-graph`**:
   - `crates/ecd-graph/src/db.rs:199-224`:
     - Table `entidades (id TEXT PRIMARY KEY, nome TEXT NOT NULL, tipo TEXT NOT NULL DEFAULT 'PJ')`
     - Table `ligacao (id1 TEXT NOT NULL, id2 TEXT NOT NULL, descricao TEXT NOT NULL DEFAULT 'SOCIO', comentario TEXT NOT NULL DEFAULT '')`
     - Table `links (id1 TEXT NOT NULL, id2 TEXT NOT NULL, descricao TEXT NOT NULL DEFAULT '', comentario TEXT NOT NULL DEFAULT '')`
     - Indices: `idx_ligacao_id1 (id1)`, `idx_ligacao_id2 (id2)`, `idx_links_id1 (id1)`, `idx_links_id2 (id2)`.
   - `crates/ecd-graph/src/bfs.rs:125-129`:
     ```sql
     SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
     FROM ligacao
     WHERE id1 = ?1 OR id2 = ?1
     ```
   - `crates/ecd-graph/src/bfs.rs:322`:
     ```sql
     SELECT nome, tipo FROM {schema}.entidades WHERE id = ?1
     ```
   - `crates/ecd-graph/src/bfs.rs:445-454`:
     ```rust
     pub(crate) fn classificar_tipo_vinculo(descricao: &str, comentario: &str) -> String {
         let combined = format!("{} {}", descricao.to_uppercase(), comentario.to_uppercase());
         if combined.contains("FILIAL") || combined.contains("MATRIZ") {
             "MATRIZ_FILIAL".to_string()
         } else if combined.contains("PROCURADOR") || combined.contains("REPRESENTANTE") {
             "PROCURADOR".to_string()
         } else {
             "SOCIO".to_string()
         }
     }
     ```
   - `crates/ecd-graph/src/sanctions.rs:211-222`:
     ```sql
     SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
     FROM main.links (or links_db.links)
     WHERE id1 = ?1 OR id2 = ?1
     ```

4. **Source Schema in `dados-publicos/cnpj.db` (Milestone 2)**:
   - `src/ingestion/schema.rs:27-145`:
     - `empresas (cnpj_basico, razao_social, natureza_juridica, qualificacao_responsavel, capital_social_str, porte_empresa, ente_federativo_responsavel, capital_social)`
     - `estabelecimento (cnpj_basico, cnpj_ordem, cnpj_dv, matriz_filial, nome_fantasia, ..., cnpj)`
     - `socios (cnpj, cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio, qualificacao_socio, data_entrada_sociedade, pais, representante_legal, nome_representante, qualificacao_representante_legal, faixa_etaria)`
     - `qualificacao_socio (codigo PRIMARY KEY, descricao)`
     - `cnpj_base2matriz (cnpj_basico PRIMARY KEY, cnpj)` (already created and indexed)

---

## 2. Logic Chain

1. **Undirected Traversal vs Direction in Edge Storage**:
   - `ecd-graph`'s BFS traversal (`bfs.rs:128`) executes `WHERE id1 = ?1 OR id2 = ?1` and builds an undirected graph (`RedeUnGraph = UnGraph<NoGrafo, ArestaGrafo>`).
   - However, specific tests in `tests/e2e_pipeline_test.rs:592-616` assert directed lookups (`id1 = origem, id2 = destino`).
   - Therefore, the edge orientations in `ligacao` must strictly adhere to the canonical orientation:
     - Filial (`PJ_<cnpj_filial>`) -> Matriz (`PJ_<cnpj_matriz>`).
     - Sócio PJ (`PJ_<cnpj_socio>`) -> Empresa (`PJ_<cnpj_empresa>`).
     - Sócio PF (`PF_<cpf>-<nome>`) -> Empresa (`PJ_<cnpj_empresa>`).
     - Sócio Estrangeiro (`PE_<nome>`) -> Empresa (`PJ_<cnpj_empresa>`).
     - Representante Legal (`PF_<cpf>-<nome>`) -> Sócio Representado (PE, PJ, or PF).

2. **Deduplication Strategy**:
   - Staging table `ligacao1 (origem TEXT, destino TEXT, tipo TEXT, base TEXT)` without indexes allows maximum bulk insert speed during the 7 derivation SELECTs.
   - Executing `CREATE TABLE ligacao AS SELECT origem AS id1, destino AS id2, tipo AS descricao, base AS comentario FROM ligacao1 GROUP BY origem, destino, tipo, base;` deduplicates across multiple filings and partition overlaps.
   - Indices `idx_ligacao_origem ON ligacao(id1)` and `idx_ligacao_destino ON ligacao(id2)` created post-deduplication provide optimal query performance for `WHERE id1 = ?1 OR id2 = ?1`.

3. **Performance Optimization for Filial -> Matriz Link**:
   - In `rede_cria_tabela_rede.db.py`, the filial link originally suffered from a slow self-join on `estabelecimento`.
   - Because `cnpj_base2matriz(cnpj_basico PRIMARY KEY, cnpj TEXT)` was already materialized and indexed in Milestone 2 (`loader.rs:457-474`), joining `estabelecimento tf` with `cnpj_base2matriz tm ON tm.cnpj_basico = tf.cnpj_basico WHERE tf.matriz_filial = '2'` performs an O(1) B-tree lookup per filial, cutting join time from tens of minutes to seconds.

4. **Entidades Materialization (Rules 6 & 7)**:
   - `crates/ecd-graph/src/bfs.rs:322` looks up `SELECT nome, tipo FROM {schema}.entidades WHERE id = ?1`.
   - By materializing `entidades (id PRIMARY KEY, nome NOT NULL, tipo NOT NULL DEFAULT 'PJ')` during `build-rede`:
     - Rule 6 populates PJ entities (`id = 'PJ_' || cnpj`, `nome = razao_social / nome_fantasia`, `tipo = 'PJ'` or `'FILIAL'`).
     - Rule 7 populates PF and PE entities (`id = 'PF_...'` or `'PE_...'`, `nome = nome_socio / nome_representante`, `tipo = 'PF'` or `'PE'`).
   - `PRIMARY KEY (id)` provides immediate O(1) indexed access, eliminating expensive runtime fallbacks to `empresas`.

5. **GRC Sanctions (`links.db`) Interoperability**:
   - `sanctions.rs` in `ecd-graph` expects `links` to have columns `id1`, `id2`, `descricao`, `comentario`.
   - `idx_links_id1` and `idx_links_id2` ensure that `WHERE id1 = ?1 OR id2 = ?1` completes in sub-millisecond time.

---

## 3. Caveats

1. **Foreign Partners (`PE`) Identifiers**:
   - Foreign partner entries in RFB data lack CNPJ/CPF (`cnpj_cpf_socio = ''` or NULL). They are keyed as `'PE_' || nome_socio`. In rare cases where two distinct foreign entities have identical names, they merge as a single node in the network. This is the official behavior of `rede-cnpj`.
2. **August 2026 Radical Expansion**:
   - In August 2026 RFB files, PJ partners in `socios` were truncated to 8-digit radicals. Milestone 2's `transform_socios_and_expand_radicals` in `loader.rs` already restored them to 14-digit CNPJs in `cnpj.socios`. Rule 1/2 therefore operates cleanly on 14-digit strings (`length(cnpj_cpf_socio) = 14`).
3. **Index Names Compatibility**:
   - Tests in `rede_cria_tabelas/tests/e2e_pipeline_test.rs:619` verify `idx_ligacao_origem` and `idx_ligacao_destino`.
   - `ecd-graph/src/db.rs:212` creates `idx_ligacao_id1` and `idx_ligacao_id2`.
   - Both sets of names index the exact same columns (`id1` and `id2`). Creating `idx_ligacao_origem (id1)` and `idx_ligacao_destino (id2)` ensures 100% compliance with both the tests and the SQLite query planner.

---

## 4. Conclusion & Actionable Specifications

### 4.1 The Exact 7 Derivation SQL Statements

#### Step 0: Staging Table Creation & Attached DB Connection
```sql
-- Executed on rede.db with attached cnpj.db (AS cnpj)
CREATE TABLE IF NOT EXISTS ligacao1 (
    origem TEXT,
    destino TEXT,
    tipo TEXT,
    base TEXT
);
```

---

#### Rule 1: Filial -> Matriz (Hierarchy Links)
Links every branch establishment (`matriz_filial = '2'`) to its headquarters (`matriz_filial = '1'`).
```sql
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || tf.cnpj AS origem,
    'PJ_' || tm.cnpj AS destino,
    'filial' AS tipo,
    'estabelecimento' AS base
FROM cnpj.estabelecimento tf
INNER JOIN cnpj.cnpj_base2matriz tm ON tm.cnpj_basico = tf.cnpj_basico
WHERE tf.matriz_filial = '2'
  AND tf.cnpj <> tm.cnpj;
```
*Note: If `cnpj_base2matriz` is not present, the fallback query is:*
```sql
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || tf.cnpj AS origem,
    'PJ_' || tm.cnpj AS destino,
    'filial' AS tipo,
    'estabelecimento' AS base
FROM cnpj.estabelecimento tf
INNER JOIN cnpj.estabelecimento tm ON tm.cnpj_basico = tf.cnpj_basico AND tm.matriz_filial = '1'
WHERE tf.matriz_filial = '2'
  AND tf.cnpj <> tm.cnpj;
```

---

#### Rule 2: Empresa -> Sócio PJ (Corporate Partner)
Links corporate partners (14-digit CNPJ) to the invested company.
```sql
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || t.cnpj_cpf_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) = 14;
```

---

#### Rule 3: Empresa -> Sócio PF (Individual Partner)
Links individual partners (11-digit masked CPF + name) to the invested company.
```sql
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) = 11
  AND t.nome_socio <> '';
```

---

#### Rule 4: Empresa -> Sócio Estrangeiro (Foreign Partner)
Links foreign corporate or individual partners (no CPF/CNPJ) to the invested company.
```sql
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PE_' || t.nome_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE (t.cnpj_cpf_socio = '' OR t.cnpj_cpf_socio IS NULL)
  AND t.nome_socio <> '';
```

---

#### Rule 5: Sócio -> Representante Legal (Legal Representatives)
Links legal representatives to their represented partners (foreign partners, corporate partners, and individual partners).

*5a. Legal Representative of Foreign Partner (PF -> PE):*
```sql
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
    'PE_' || t.nome_socio AS destino,
    'rep-sócio-' || COALESCE(sq.descricao, '') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE (t.cnpj_cpf_socio = '' OR t.cnpj_cpf_socio IS NULL)
  AND t.representante_legal <> '***000000**'
  AND t.representante_legal <> ''
  AND t.representante_legal IS NOT NULL;
```

*5b. Legal Representative of Corporate Partner (PF -> PJ):*
```sql
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
    'PJ_' || t.cnpj_cpf_socio AS destino,
    'rep-sócio-' || COALESCE(sq.descricao, '') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE length(t.cnpj_cpf_socio) = 14
  AND t.representante_legal <> '***000000**'
  AND t.representante_legal <> ''
  AND t.representante_legal IS NOT NULL;
```

*5c. Legal Representative of Individual Partner (PF -> PF):*
```sql
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
    'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS destino,
    'rep-sócio-' || COALESCE(sq.descricao, '') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE length(t.cnpj_cpf_socio) = 11
  AND t.representante_legal <> '***000000**'
  AND t.representante_legal <> ''
  AND t.representante_legal IS NOT NULL;
```

---

#### Deduplication & Index Creation for `ligacao`
```sql
CREATE TABLE IF NOT EXISTS ligacao (
    id1 TEXT NOT NULL,
    id2 TEXT NOT NULL,
    descricao TEXT NOT NULL,
    comentario TEXT NOT NULL
);

INSERT INTO ligacao (id1, id2, descricao, comentario)
SELECT
    origem AS id1,
    destino AS id2,
    tipo AS descricao,
    base AS comentario
FROM ligacao1
GROUP BY origem, destino, tipo, base;

DROP TABLE IF EXISTS ligacao1;

CREATE INDEX IF NOT EXISTS idx_ligacao_origem ON ligacao (id1);
CREATE INDEX IF NOT EXISTS idx_ligacao_destino ON ligacao (id2);
```

---

#### Rule 6: Materialization of Corporate Entities (`entidades` - PJ)
Materializes all Matrizes and Filiais with primary legal name and fallback trade name.
```sql
CREATE TABLE IF NOT EXISTS entidades (
    id TEXT PRIMARY KEY,
    nome TEXT NOT NULL,
    tipo TEXT NOT NULL DEFAULT 'PJ'
);

INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT
    'PJ_' || te.cnpj AS id,
    COALESCE(NULLIF(em.razao_social, ''), NULLIF(te.nome_fantasia, ''), 'EMPRESA') AS nome,
    CASE WHEN te.matriz_filial = '2' THEN 'FILIAL' ELSE 'PJ' END AS tipo
FROM cnpj.estabelecimento te
LEFT JOIN cnpj.empresas em ON em.cnpj_basico = te.cnpj_basico;
```

---

#### Rule 7: Materialization of Individual & Foreign Entities (`entidades` - PF & PE)
Materializes all unique individual partners, foreign partners, and legal representatives.
```sql
-- 7a. Individual Partners (PF)
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT DISTINCT
    'PF_' || ts.cnpj_cpf_socio || '-' || ts.nome_socio AS id,
    ts.nome_socio AS nome,
    'PF' AS tipo
FROM cnpj.socios ts
WHERE length(ts.cnpj_cpf_socio) = 11
  AND ts.nome_socio <> '';

-- 7b. Foreign Partners (PE)
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT DISTINCT
    'PE_' || ts.nome_socio AS id,
    ts.nome_socio AS nome,
    'PE' AS tipo
FROM cnpj.socios ts
WHERE (ts.cnpj_cpf_socio = '' OR ts.cnpj_cpf_socio IS NULL)
  AND ts.nome_socio <> '';

-- 7c. Legal Representatives (PF)
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT DISTINCT
    'PF_' || ts.representante_legal || '-' || ts.nome_representante AS id,
    ts.nome_representante AS nome,
    'PF' AS tipo
FROM cnpj.socios ts
WHERE ts.representante_legal <> '***000000**'
  AND ts.representante_legal <> ''
  AND ts.representante_legal IS NOT NULL
  AND ts.nome_representante <> '';

CREATE INDEX IF NOT EXISTS idx_entidades_tipo ON entidades (tipo);
```

---

#### Auxiliary: Full-Text Search Virtual Table (`rede_search.db`)
```sql
CREATE VIRTUAL TABLE IF NOT EXISTS id_search USING fts5 (
    id,
    descricao,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO id_search (id, descricao)
SELECT id, descricao
FROM (
    SELECT 'PJ_' || te.cnpj AS id, em.razao_social AS descricao
    FROM cnpj.estabelecimento te
    LEFT JOIN cnpj.empresas em ON em.cnpj_basico = te.cnpj_basico
    WHERE te.matriz_filial = '1' AND em.razao_social IS NOT NULL AND em.razao_social <> ''
    UNION ALL
    SELECT 'PJ_' || te.cnpj AS id, te.nome_fantasia AS descricao
    FROM cnpj.estabelecimento te
    WHERE te.nome_fantasia IS NOT NULL AND te.nome_fantasia <> ''
    UNION ALL
    SELECT id1 AS id, '' AS descricao
    FROM rede.ligacao
    WHERE substr(id1, 1, 3) <> 'PJ_'
    UNION ALL
    SELECT id2 AS id, '' AS descricao
    FROM rede.ligacao
    WHERE substr(id2, 1, 3) <> 'PJ_'
) AS tunion
GROUP BY id, descricao;
```

---

### 4.2 Proposed Rust Module Architecture for `rede_cria_tabelas/src/graph/`

```
rede_cria_tabelas/src/graph/
├── mod.rs      # High-level entry point `run(args: BuildRedeArgs)`
├── schema.rs   # Table DDLs, index SQL, table creation helpers
├── rules.rs    # Derivation queries, batch execution, ligacao & entidades materialization
└── search.rs   # FTS5 search index builder for rede_search.db
```

#### File 1: `src/graph/schema.rs`
```rust
//! SQLite DDL specifications and post-load index statements for `rede.db`.

use rusqlite::{Connection, Result};

pub const LIGACAO1_STAGING_DDL: &str = "
CREATE TABLE IF NOT EXISTS ligacao1 (
    origem TEXT,
    destino TEXT,
    tipo TEXT,
    base TEXT
);";

pub const LIGACAO_FINAL_DDL: &str = "
CREATE TABLE IF NOT EXISTS ligacao (
    id1 TEXT NOT NULL,
    id2 TEXT NOT NULL,
    descricao TEXT NOT NULL DEFAULT 'SOCIO',
    comentario TEXT NOT NULL DEFAULT ''
);";

pub const ENTIDADES_DDL: &str = "
CREATE TABLE IF NOT EXISTS entidades (
    id TEXT PRIMARY KEY,
    nome TEXT NOT NULL,
    tipo TEXT NOT NULL DEFAULT 'PJ'
);";

pub const POST_LOAD_INDICES_LIGACAO_SQL: &str = "
CREATE INDEX IF NOT EXISTS idx_ligacao_origem ON ligacao (id1);
CREATE INDEX IF NOT EXISTS idx_ligacao_destino ON ligacao (id2);
CREATE INDEX IF NOT EXISTS idx_entidades_tipo ON entidades (tipo);
";

pub fn create_graph_staging_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(LIGACAO1_STAGING_DDL)?;
    conn.execute_batch(ENTIDADES_DDL)?;
    Ok(())
}

pub fn create_post_load_indices(conn: &Connection) -> Result<()> {
    conn.execute_batch(POST_LOAD_INDICES_LIGACAO_SQL)?;
    Ok(())
}
```

#### File 2: `src/graph/rules.rs`
```rust
//! Societary graph derivation queries and entity materialization.

use rusqlite::{Connection, Result};
use tracing::info;

pub const DERIVE_FILIAL_MATRIZ_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || tf.cnpj AS origem,
    'PJ_' || tm.cnpj AS destino,
    'filial' AS tipo,
    'estabelecimento' AS base
FROM cnpj.estabelecimento tf
INNER JOIN cnpj.cnpj_base2matriz tm ON tm.cnpj_basico = tf.cnpj_basico
WHERE tf.matriz_filial = '2' AND tf.cnpj <> tm.cnpj;
";

pub const DERIVE_SOCIO_PJ_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || t.cnpj_cpf_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) = 14;
";

pub const DERIVE_SOCIO_PF_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) = 11 AND t.nome_socio <> '';
";

pub const DERIVE_SOCIO_ESTRANGEIRO_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PE_' || t.nome_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE (t.cnpj_cpf_socio = '' OR t.cnpj_cpf_socio IS NULL) AND t.nome_socio <> '';
";

pub const DERIVE_REP_LEGAL_PE_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
    'PE_' || t.nome_socio AS destino,
    'rep-sócio-' || COALESCE(sq.descricao, '') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE (t.cnpj_cpf_socio = '' OR t.cnpj_cpf_socio IS NULL)
  AND t.representante_legal <> '***000000**'
  AND t.representante_legal <> ''
  AND t.representante_legal IS NOT NULL;
";

pub const DERIVE_REP_LEGAL_PJ_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
    'PJ_' || t.cnpj_cpf_socio AS destino,
    'rep-sócio-' || COALESCE(sq.descricao, '') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE length(t.cnpj_cpf_socio) = 14
  AND t.representante_legal <> '***000000**'
  AND t.representante_legal <> ''
  AND t.representante_legal IS NOT NULL;
";

pub const DERIVE_REP_LEGAL_PF_SQL: &str = "
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
    'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS destino,
    'rep-sócio-' || COALESCE(sq.descricao, '') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE length(t.cnpj_cpf_socio) = 11
  AND t.representante_legal <> '***000000**'
  AND t.representante_legal <> ''
  AND t.representante_legal IS NOT NULL;
";

pub const DEDUPLICATE_LIGACAO_SQL: &str = "
CREATE TABLE IF NOT EXISTS ligacao (
    id1 TEXT NOT NULL,
    id2 TEXT NOT NULL,
    descricao TEXT NOT NULL,
    comentario TEXT NOT NULL
);

INSERT INTO ligacao (id1, id2, descricao, comentario)
SELECT
    origem AS id1,
    destino AS id2,
    tipo AS descricao,
    base AS comentario
FROM ligacao1
GROUP BY origem, destino, tipo, base;

DROP TABLE IF EXISTS ligacao1;
";

pub const MATERIALIZE_ENTIDADES_PJ_SQL: &str = "
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT
    'PJ_' || te.cnpj AS id,
    COALESCE(NULLIF(em.razao_social, ''), NULLIF(te.nome_fantasia, ''), 'EMPRESA') AS nome,
    CASE WHEN te.matriz_filial = '2' THEN 'FILIAL' ELSE 'PJ' END AS tipo
FROM cnpj.estabelecimento te
LEFT JOIN cnpj.empresas em ON em.cnpj_basico = te.cnpj_basico;
";

pub const MATERIALIZE_ENTIDADES_PF_PE_SQL: &str = "
INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT DISTINCT
    'PF_' || ts.cnpj_cpf_socio || '-' || ts.nome_socio AS id,
    ts.nome_socio AS nome,
    'PF' AS tipo
FROM cnpj.socios ts
WHERE length(ts.cnpj_cpf_socio) = 11 AND ts.nome_socio <> '';

INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT DISTINCT
    'PE_' || ts.nome_socio AS id,
    ts.nome_socio AS nome,
    'PE' AS tipo
FROM cnpj.socios ts
WHERE (ts.cnpj_cpf_socio = '' OR ts.cnpj_cpf_socio IS NULL) AND ts.nome_socio <> '';

INSERT OR IGNORE INTO entidades (id, nome, tipo)
SELECT DISTINCT
    'PF_' || ts.representante_legal || '-' || ts.nome_representante AS id,
    ts.nome_representante AS nome,
    'PF' AS tipo
FROM cnpj.socios ts
WHERE ts.representante_legal <> '***000000**'
  AND ts.representante_legal <> ''
  AND ts.representante_legal IS NOT NULL
  AND ts.nome_representante <> '';
";

pub fn derive_all_edges(conn: &Connection) -> Result<()> {
    info!("Deriving Rule 1: Filial -> Matriz links");
    conn.execute_batch(DERIVE_FILIAL_MATRIZ_SQL)?;

    info!("Deriving Rule 2: Empresa -> Sócio PJ links");
    conn.execute_batch(DERIVE_SOCIO_PJ_SQL)?;

    info!("Deriving Rule 3: Empresa -> Sócio PF links");
    conn.execute_batch(DERIVE_SOCIO_PF_SQL)?;

    info!("Deriving Rule 4: Empresa -> Sócio Estrangeiro links");
    conn.execute_batch(DERIVE_SOCIO_ESTRANGEIRO_SQL)?;

    info!("Deriving Rule 5a-c: Legal representative links");
    conn.execute_batch(DERIVE_REP_LEGAL_PE_SQL)?;
    conn.execute_batch(DERIVE_REP_LEGAL_PJ_SQL)?;
    conn.execute_batch(DERIVE_REP_LEGAL_PF_SQL)?;

    info!("Deduplicating and materializing ligacao table");
    conn.execute_batch(DEDUPLICATE_LIGACAO_SQL)?;

    Ok(())
}

pub fn materialize_entidades(conn: &Connection) -> Result<()> {
    info!("Materializing Rule 6: PJ Entities into entidades table");
    conn.execute_batch(MATERIALIZE_ENTIDADES_PJ_SQL)?;

    info!("Materializing Rule 7: PF & PE Entities into entidades table");
    conn.execute_batch(MATERIALIZE_ENTIDADES_PF_PE_SQL)?;

    Ok(())
}
```

#### File 3: `src/graph/search.rs`
```rust
//! Full-text search (FTS5) virtual table derivation for `rede_search.db`.

use rusqlite::{Connection, Result};
use std::path::Path;
use tracing::info;

pub fn build_search_database(rede_db_path: &Path, cnpj_db_path: &Path, search_db_path: &Path) -> Result<u64> {
    if let Some(parent) = search_db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }

    let conn = Connection::open(search_db_path)?;

    // PRAGMAs for fast virtual table generation
    conn.execute_batch(
        "PRAGMA journal_mode = OFF;
         PRAGMA synchronous = OFF;
         PRAGMA temp_store = MEMORY;"
    )?;

    let rede_str = rede_db_path.to_string_lossy().replace('\'', "''");
    let cnpj_str = cnpj_db_path.to_string_lossy().replace('\'', "''");

    conn.execute_batch(&format!("ATTACH DATABASE '{rede_str}' AS rede;"))?;
    conn.execute_batch(&format!("ATTACH DATABASE '{cnpj_str}' AS cnpj;"))?;

    info!("Creating FTS5 id_search virtual table in {:?}", search_db_path);
    conn.execute_batch(
        "DROP TABLE IF EXISTS id_search;
         CREATE VIRTUAL TABLE id_search USING fts5 (
             id,
             descricao,
             tokenize = 'unicode61 remove_diacritics 2'
         );

         INSERT INTO id_search (id, descricao)
         SELECT id, descricao
         FROM (
             SELECT 'PJ_' || te.cnpj AS id, em.razao_social AS descricao
             FROM cnpj.estabelecimento te
             LEFT JOIN cnpj.empresas em ON em.cnpj_basico = te.cnpj_basico
             WHERE te.matriz_filial = '1' AND em.razao_social IS NOT NULL AND em.razao_social <> ''
             UNION ALL
             SELECT 'PJ_' || te.cnpj AS id, te.nome_fantasia AS descricao
             FROM cnpj.estabelecimento te
             WHERE te.nome_fantasia IS NOT NULL AND te.nome_fantasia <> ''
             UNION ALL
             SELECT id1 AS id, '' AS descricao
             FROM rede.ligacao
             WHERE substr(id1, 1, 3) <> 'PJ_'
             UNION ALL
             SELECT id2 AS id, '' AS descricao
             FROM rede.ligacao
             WHERE substr(id2, 1, 3) <> 'PJ_'
         ) AS tunion
         GROUP BY id, descricao;"
    )?;

    let total: i64 = conn.query_row("SELECT count(*) FROM id_search;", [], |r| r.get(0))?;

    conn.execute_batch(
        "PRAGMA optimize;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;"
    )?;

    Ok(total as u64)
}
```

#### File 4: `src/graph/mod.rs`
```rust
//! Milestone 3: Societary Relationship Graph Derivation (`rede.db` and `rede_search.db`).

pub mod rules;
pub mod schema;
pub mod search;

use crate::cli::BuildRedeArgs;
use crate::config::IngestionPragmas;
use rusqlite::Connection;
use std::time::Instant;
use tracing::info;

pub fn run(args: BuildRedeArgs) -> anyhow::Result<()> {
    let start_time = Instant::now();
    info!("Starting Milestone 3: Societary Graph Derivation (build-rede)");

    if args.drop_existing {
        if args.output_rede.exists() {
            info!("Removing existing {:?}", args.output_rede);
            let _ = std::fs::remove_file(&args.output_rede);
        }
        if args.output_search.exists() {
            info!("Removing existing {:?}", args.output_search);
            let _ = std::fs::remove_file(&args.output_search);
        }
    }

    if let Some(parent) = args.output_rede.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(&args.output_rede)?;

    let pragmas = IngestionPragmas::from_memory_limit(args.memory_limit_mb, 4);
    pragmas.apply_to(&conn)?;

    let cnpj_path_str = args.cnpj_db.to_string_lossy().replace('\'', "''");
    info!("Attaching source database: {:?}", args.cnpj_db);
    conn.execute_batch(&format!("ATTACH DATABASE '{cnpj_path_str}' AS cnpj;"))?;

    info!("Creating graph staging tables in rede.db");
    schema::create_graph_staging_tables(&conn)?;

    info!("Executing 7 derivation SQL rules");
    rules::derive_all_edges(&conn)?;

    info!("Materializing entidades table");
    rules::materialize_entidades(&conn)?;

    info!("Creating post-load indices on ligacao and entidades");
    schema::create_post_load_indices(&conn)?;

    IngestionPragmas::apply_post_load(&conn)?;

    let edge_count: i64 = conn.query_row("SELECT count(*) FROM ligacao;", [], |r| r.get(0))?;
    let entity_count: i64 = conn.query_row("SELECT count(*) FROM entidades;", [], |r| r.get(0))?;

    info!("Built ligacao with {} edges and entidades with {} entities", edge_count, entity_count);

    // Build FTS5 search database
    let fts_count = search::build_search_database(&args.output_rede, &args.cnpj_db, &args.output_search)?;
    info!("Built id_search FTS5 index with {} tokens in {:?}", fts_count, args.output_search);

    info!("Milestone 3 completed successfully in {:?}", start_time.elapsed());
    Ok(())
}
```

---

## 5. Verification Method

### 5.1 Test Execution Commands
Once Milestone 3 implementation is deployed by the coder agent:

1. **Verify Unit & E2E Pipeline Tests**:
   ```bash
   cargo test --test e2e_pipeline_test test_tier1_f11_societary_links_graph_derivation -- --nocapture
   cargo test --test e2e_pipeline_test test_tier1_f12_entities_and_fts5_search -- --nocapture
   cargo test --test e2e_pipeline_test test_tier1_f13_grc_sanctions_compatibility -- --nocapture
   cargo test --test e2e_pipeline_test -- --nocapture
   ```

2. **Verify Consumer `crates/ecd-graph` Tests**:
   ```bash
   cargo test -p ecd-graph -- --nocapture
   ```

3. **Verify CLI Derivation Command**:
   ```bash
   cargo run --bin rfb-cnpj-etl -- build-rede --help
   cargo run --bin rfb-cnpj-etl -- build-rede \
     --cnpj-db dados-publicos/cnpj.db \
     --output-rede dados-publicos/rede.db \
     --output-search dados-publicos/rede_search.db \
     --drop-existing
   ```

4. **Verify Database Integrity via SQLite CLI**:
   ```bash
   sqlite3 dados-publicos/rede.db ".schema"
   sqlite3 dados-publicos/rede.db "SELECT count(*) FROM ligacao;"
   sqlite3 dados-publicos/rede.db "SELECT count(*) FROM entidades;"
   sqlite3 dados-publicos/rede.db "PRAGMA index_list('ligacao');"
   sqlite3 dados-publicos/rede_search.db "SELECT count(*) FROM id_search;"
   ```

### 5.2 Invalidation Conditions
- Any query in `ecd-graph` (`SELECT ... FROM ligacao WHERE id1 = ?1 OR id2 = ?1` or `SELECT nome, tipo FROM entidades WHERE id = ?1`) returning column errors or missing types.
- Missing `idx_ligacao_origem` or `idx_ligacao_destino` in `ligacao`.
- Edge orientation mismatch (e.g., swapping Filial <-> Matriz) causing `test_tier1_f11` to fail.
