# Milestone 3 (Iteration 2) Handoff Report: Constraint Integrity & Orphan Partner Architecture

- **Agent**: `explorer_m3_r2_2`
- **Role**: Constraint Integrity & Orphan Partner Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2`
- **Handoff Type**: Hard Handoff
- **Date**: 2026-10-25T01:20:00Z

---

## 1. Observation

Direct line-by-line inspection of `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/rules.rs` and `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/ingestion/loader.rs` revealed that societary graph derivation staging produces `NULL` or corrupt phantom node IDs (`'PJ_'`, `'PE_'`, `'PF_*-'`) whenever orphan or corrupted records occur in the source database:

1. **Origin of `NULL` in `socios.cnpj`**:
   In `src/ingestion/loader.rs:509` and `loader.rs:536`:
   ```sql
   FROM socios_original ts
   LEFT JOIN cnpj_base2matriz te ON te.cnpj_basico = ts.cnpj_basico
   ```
   In official Brazilian Federal Revenue (RFB) open data, partner records frequently reference companies whose establishment records were omitted, purged, or have not yet been registered. Because of this `LEFT JOIN`, `socios.cnpj` is populated as `NULL` for orphan partner rows.

2. **Propagation of `NULL` in Rules 2, 3, and 4**:
   In `src/graph/rules.rs:41-42` (Rule 2), `rules.rs:54-55` (Rule 3), and `rules.rs:67-68` (Rule 4):
   ```sql
   -- Rule 2:
   SELECT 'PJ_' || t.cnpj_cpf_socio AS origem, 'PJ_' || t.cnpj AS destino ... FROM cnpj.socios t WHERE length(t.cnpj_cpf_socio) = 14;

   -- Rule 3:
   SELECT 'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS origem, 'PJ_' || t.cnpj AS destino ... FROM cnpj.socios t WHERE length(t.cnpj_cpf_socio) = 11 AND t.nome_socio <> '';

   -- Rule 4:
   SELECT 'PE_' || t.nome_socio AS origem, 'PJ_' || t.cnpj AS destino ... FROM cnpj.socios t WHERE (t.cnpj_cpf_socio = '' OR t.cnpj_cpf_socio IS NULL) AND t.nome_socio <> '';
   ```
   In SQLite, the string concatenation operator `'PJ_' || NULL` evaluates strictly to `NULL`.
   Consequently, every orphan partner row generates a staging record in `ligacao1` with `destino IS NULL`.
   Furthermore, if `t.cnpj = ''`, `'PJ_' || ''` evaluates to `'PJ_'`, generating corrupt phantom node IDs.

3. **Schema Violation and Fatal Pipeline Crash in `DEDUPLICATE_LIGACAO_SQL`**:
   In `src/graph/rules.rs:128-144`:
   ```sql
   CREATE TABLE IF NOT EXISTS ligacao (
       id1 TEXT NOT NULL,
       id2 TEXT NOT NULL,
       descricao TEXT NOT NULL DEFAULT 'SOCIO',
       comentario TEXT NOT NULL DEFAULT ''
   );

   INSERT INTO ligacao (id1, id2, descricao, comentario)
   SELECT
       origem AS id1,
       destino AS id2,
       COALESCE(tipo, 'Sócio') AS descricao,
       COALESCE(base, '') AS comentario
   FROM ligacao1
   GROUP BY origem, destino, tipo, base;
   ```
   Executing this query with any `destino IS NULL` triggers the verbatim SQLite constraint violation:
   ```text
   sqlite3.IntegrityError: NOT NULL constraint failed: ligacao.id2
   ```
   This was empirically confirmed in Python SQLite test simulation:
   `with_guards=False: CRASH! IntegrityError: NOT NULL constraint failed: ligacao.id2`.

4. **Missing `t.nome_socio` Guards in Rules 5a and 5c**:
   - In `src/graph/rules.rs:77-91` (Rule 5a: Legal representative of foreign partner):
     `'PE_' || t.nome_socio AS destino`
     `t.nome_socio` is **completely absent from the WHERE clause**. If `t.nome_socio IS NULL`, `destino` evaluates to `NULL`, triggering `NOT NULL constraint failed: ligacao.id2`.
   - In `src/graph/rules.rs:111-125` (Rule 5c: Legal representative of individual partner):
     `'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS destino`
     `t.nome_socio` is **completely absent from the WHERE clause**. If `t.nome_socio IS NULL`, `'PF_' || t.cnpj_cpf_socio || '-' || NULL` evaluates to `NULL`, triggering `NOT NULL constraint failed: ligacao.id2`.

5. **Fragile Null/Empty Handling in Rule 1 (`DERIVE_FILIAL_MATRIZ_SQL` & Fallback)**:
   In `src/graph/rules.rs:21` and `rules.rs:34`:
   `WHERE tf.matriz_filial = '2' AND tf.cnpj <> tm.cnpj;`
   If `tf.cnpj` or `tm.cnpj` is empty `''` or corrupted/NULL, `origem` or `destino` evaluates to `'PJ_'` or `NULL`.

6. **Degenerate Entries in `MATERIALIZE_ENTIDADES_*_SQL`**:
   In `src/graph/rules.rs:151-157` (`MATERIALIZE_ENTIDADES_PJ_SQL`):
   `te.cnpj` is not filtered with `WHERE te.cnpj IS NOT NULL AND length(te.cnpj) = 14`. An empty string in `te.cnpj` inserts `'PJ_'` into `entidades`.
   In `MATERIALIZE_ENTIDADES_PF_PE_SQL`, `nome_socio` and `nome_representante` are only checked against `<> ''`, allowing pure-whitespace strings (`'   '`) to slip through into node names.

---

## 2. Logic Chain

1. **Step 1 (Source Reality)**: RFB open data is un-normalized and inherently incomplete. During ingestion stage 6 (`transform_socios_and_expand_radicals`), `socios` rows whose `cnpj_basico` does not match an active headquarters establishment in `cnpj_base2matriz` have `cnpj` populated as `NULL`.
2. **Step 2 (SQLite Semantics)**: In SQLite, `string || NULL == NULL`. Whenever `t.cnpj IS NULL`, `'PJ_' || t.cnpj` produces `NULL`.
3. **Step 3 (Staging Permissiveness)**: Staging table `ligacao1` is defined without `NOT NULL` constraints (`origem TEXT, destino TEXT, tipo TEXT, base TEXT`). Thus, `INSERT INTO ligacao1` silently accepts `NULL` values.
4. **Step 4 (Constraint Collision)**: Target table `ligacao` enforces `id1 TEXT NOT NULL, id2 TEXT NOT NULL` per downstream contracts with `crates/ecd-graph`. When `DEDUPLICATE_LIGACAO_SQL` runs, `INSERT INTO ligacao ... SELECT ... FROM ligacao1` attempts to insert `NULL` into `id2`, throwing `NOT NULL constraint failed: ligacao.id2` and terminating `build-rede`.
5. **Step 5 (Defensive Architecture)**: Complete remediation requires two coordinated layers:
   - **Layer 1 (Upstream Source Filtering)**: In each derivation rule (Rules 1-5), add explicit `WHERE` guards requiring all composite key fields (`cnpj`, `nome_socio`, `representante_legal`, `nome_representante`) to be `IS NOT NULL`, non-empty, and valid length.
   - **Layer 2 (Defense-in-Depth at Deduplication)**: In `DEDUPLICATE_LIGACAO_SQL`, filter `origem` and `destino` to ensure neither is `NULL`, empty, or a bare prefix token (`'PJ_'`, `'PF_'`, `'PE_'`, `'PF_-'`).

---

## 3. Caveats

1. **Orphan Edge Dropping**: Partner records that cannot be linked to a valid headquarters establishment (because the establishment is absent from `estabelecimento` and `cnpj_base2matriz`) are omitted from the societary graph. This is the correct, intended behavior of `rede-cnpj` because an edge cannot connect to an unknown, non-existent entity.
2. **Masked CPFs**: Brazilian open data masks individual partner CPFs (e.g., `***123456**`). The `length = 11` check correctly validates these 11-character masked strings.
3. **Self-Referencing Corporate Partners**: If dirty RFB data contains a corporate partner that lists itself as a socio (`t.cnpj_cpf_socio = t.cnpj`), `deduplicate_ligacao` does not forcibly drop this edge because `crates/ecd-graph`'s BFS traversal explicitly tests and supports self-loop tolerance (`PJ_E1 -> PJ_E1`). However, Rule 1 filial derivation explicitly enforces `tf.cnpj <> tm.cnpj` because a company cannot be a branch of itself.

---

## 4. Conclusion

All 7 derivation rules and staging queries in `src/graph/rules.rs` must be updated with the exact SQL WHERE clause guards specified below.

### 4.1 Exact Formulations for All Derivation Rules

#### Rule 1: Filial -> Matriz (`DERIVE_FILIAL_MATRIZ_SQL` & `DERIVE_FILIAL_MATRIZ_FALLBACK_SQL`)
```sql
-- DERIVE_FILIAL_MATRIZ_SQL:
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || tf.cnpj AS origem,
    'PJ_' || tm.cnpj AS destino,
    'filial' AS tipo,
    'estabelecimento' AS base
FROM cnpj.estabelecimento tf
INNER JOIN cnpj.cnpj_base2matriz tm ON tm.cnpj_basico = tf.cnpj_basico
WHERE tf.matriz_filial = '2'
  AND tf.cnpj IS NOT NULL AND length(tf.cnpj) = 14
  AND tm.cnpj IS NOT NULL AND length(tm.cnpj) = 14
  AND tf.cnpj <> tm.cnpj;

-- DERIVE_FILIAL_MATRIZ_FALLBACK_SQL:
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || tf.cnpj AS origem,
    'PJ_' || tm.cnpj AS destino,
    'filial' AS tipo,
    'estabelecimento' AS base
FROM cnpj.estabelecimento tf
INNER JOIN cnpj.estabelecimento tm ON tm.cnpj_basico = tf.cnpj_basico AND tm.matriz_filial = '1'
WHERE tf.matriz_filial = '2'
  AND tf.cnpj IS NOT NULL AND length(tf.cnpj) = 14
  AND tm.cnpj IS NOT NULL AND length(tm.cnpj) = 14
  AND tf.cnpj <> tm.cnpj;
```

#### Rule 2: Corporate Partner PJ -> PJ (`DERIVE_SOCIO_PJ_SQL`)
```sql
INSERT INTO ligacao1 (origem, destino, tipo, base)
SELECT
    'PJ_' || t.cnpj_cpf_socio AS origem,
    'PJ_' || t.cnpj AS destino,
    COALESCE(NULLIF(sq.descricao, ''), 'Sócio') AS tipo,
    'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) = 14
  AND t.cnpj IS NOT NULL
  AND length(t.cnpj) = 14;
```

#### Rule 3: Individual Partner PF -> PJ (`DERIVE_SOCIO_PF_SQL`)
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
  AND t.nome_socio IS NOT NULL
  AND trim(t.nome_socio) <> ''
  AND t.cnpj IS NOT NULL
  AND length(t.cnpj) = 14;
```

#### Rule 4: Foreign Partner PE -> PJ (`DERIVE_SOCIO_ESTRANGEIRO_SQL`)
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
  AND t.nome_socio IS NOT NULL
  AND trim(t.nome_socio) <> ''
  AND t.cnpj IS NOT NULL
  AND length(t.cnpj) = 14;
```

#### Rule 5a: Legal Representative of Foreign Partner PF -> PE (`DERIVE_REP_LEGAL_PE_SQL`)
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
  AND t.nome_socio IS NOT NULL
  AND trim(t.nome_socio) <> ''
  AND t.representante_legal IS NOT NULL
  AND trim(t.representante_legal) <> ''
  AND t.representante_legal <> '***000000**'
  AND t.nome_representante IS NOT NULL
  AND trim(t.nome_representante) <> '';
```

#### Rule 5b: Legal Representative of Corporate Partner PF -> PJ (`DERIVE_REP_LEGAL_PJ_SQL`)
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
  AND t.representante_legal IS NOT NULL
  AND trim(t.representante_legal) <> ''
  AND t.representante_legal <> '***000000**'
  AND t.nome_representante IS NOT NULL
  AND trim(t.nome_representante) <> '';
```

#### Rule 5c: Legal Representative of Individual Partner PF -> PF (`DERIVE_REP_LEGAL_PF_SQL`)
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
  AND t.nome_socio IS NOT NULL
  AND trim(t.nome_socio) <> ''
  AND t.representante_legal IS NOT NULL
  AND trim(t.representante_legal) <> ''
  AND t.representante_legal <> '***000000**'
  AND t.nome_representante IS NOT NULL
  AND trim(t.nome_representante) <> '';
```

#### Deduplication Defense-in-Depth Filter (`DEDUPLICATE_LIGACAO_SQL`)
```sql
CREATE TABLE IF NOT EXISTS ligacao (
    id1 TEXT NOT NULL,
    id2 TEXT NOT NULL,
    descricao TEXT NOT NULL DEFAULT 'SOCIO',
    comentario TEXT NOT NULL DEFAULT ''
);

INSERT INTO ligacao (id1, id2, descricao, comentario)
SELECT
    origem AS id1,
    destino AS id2,
    COALESCE(tipo, 'Sócio') AS descricao,
    COALESCE(base, '') AS comentario
FROM ligacao1
WHERE origem IS NOT NULL
  AND destino IS NOT NULL
  AND trim(origem) <> ''
  AND trim(destino) <> ''
  AND origem NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
  AND destino NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
GROUP BY origem, destino, tipo, base;

DROP TABLE IF EXISTS ligacao1;
```

#### Entity Materialization (`MATERIALIZE_ENTIDADES_PJ_SQL` & `MATERIALIZE_ENTIDADES_PF_PE_SQL`)
- In `MATERIALIZE_ENTIDADES_PJ_SQL`, append:
  ```sql
  WHERE te.cnpj IS NOT NULL AND length(te.cnpj) = 14;
  ```
- In `MATERIALIZE_ENTIDADES_PF_PE_SQL`, ensure `nome_socio` and `nome_representante` have `IS NOT NULL AND trim(...) <> ''`, and fallback query filters out invalid prefixes:
  ```sql
  WHERE substr(id1, 1, 3) <> 'PJ_' AND id1 IS NOT NULL AND trim(id1) <> '' AND id1 NOT IN ('PF_', 'PE_', 'PF_-')
  ```

### 4.2 Machine-Applicable Artifacts Created
- Patch file: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2/rules_null_guards.patch`
- Replacement file: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_2/proposed_rules.rs`

---

## 5. Verification Method

To independently verify these fixes:

1. **Run Dedicated Adversarial Unit Test**:
   Include `test_derivation_orphan_and_corrupt_partner_resilience` (provided in `proposed_rules.rs`) in `src/graph/rules.rs` and execute:
   ```bash
   cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --lib test_derivation_orphan_and_corrupt_partner_resilience
   ```

2. **Verify SQLite Clean Execution on Dirty Data**:
   Execute the following simulation script:
   ```bash
   python3 -c "
   import sqlite3
   c = sqlite3.connect(':memory:')
   c.execute('CREATE TABLE ligacao1 (origem TEXT, destino TEXT, tipo TEXT, base TEXT);')
   c.execute('CREATE TABLE ligacao (id1 TEXT NOT NULL, id2 TEXT NOT NULL, descricao TEXT NOT NULL DEFAULT \"SOCIO\", comentario TEXT NOT NULL DEFAULT \"\");')
   # Insert dirty data
   c.execute(\"INSERT INTO ligacao1 VALUES ('PJ_11111111000191', 'PJ_' || NULL, 'Sócio', 'socios');\")
   c.execute(\"INSERT INTO ligacao1 VALUES ('PJ_11111111000191', 'PJ_', 'Sócio', 'socios');\")
   c.execute(\"INSERT INTO ligacao1 VALUES ('PJ_11111111000191', 'PJ_22222222000180', 'Sócio', 'socios');\")
   # Run deduplication with defense-in-depth WHERE clause
   c.execute('''
   INSERT INTO ligacao (id1, id2, descricao, comentario)
   SELECT origem AS id1, destino AS id2, COALESCE(tipo, 'Sócio') AS descricao, COALESCE(base, '') AS comentario
   FROM ligacao1
   WHERE origem IS NOT NULL AND destino IS NOT NULL
     AND trim(origem) <> '' AND trim(destino) <> ''
     AND origem NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
     AND destino NOT IN ('PJ_', 'PF_', 'PE_', 'PF_-')
   GROUP BY origem, destino, tipo, base;
   ''')
   res = c.execute('SELECT * FROM ligacao;').fetchall()
   assert len(res) == 1, f'Expected 1 valid row, got {len(res)}'
   assert res[0][0] == 'PJ_11111111000191' and res[0][1] == 'PJ_22222222000180'
   print('Verification passed: 100% constraint integrity maintained!')
   "
   ```

3. **Run Full Test Suite**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   cargo check --all-targets
   cargo clippy --all-targets -- -D warnings
   cargo test --lib --all-targets
   cargo test --test e2e_pipeline_test
   cargo test --test adversarial_graph_traversal_test
   ```

4. **Invalidation Condition**:
   If an orphan partner record (`socios.cnpj IS NULL` or `socios.nome_socio IS NULL` in Rule 5) ever causes `deduplicate_ligacao` to return an `IntegrityError` or causes `ligacao` to contain `id1 = 'PJ_'` or `id2 = 'PJ_'`, this finding is invalidated.
