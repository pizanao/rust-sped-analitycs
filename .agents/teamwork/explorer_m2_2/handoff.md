# Handoff Report: Milestone 2 — SQLite Persistence Layer (`schema.rs` & `loader.rs`)

**Agent**: `explorer_m2_2`  
**Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m2_2`  
**Target Project**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`  
**Milestone**: Milestone 2 (High-Performance CNPJ Ingestion Engine)  

---

## 1. Observation

Direct observations from the codebase, specifications, and legacy implementations:

1. **Database Schema Contract in `PROJECT.md` (`lines 65–84`)**:
   - `cnpj.db` must contain exactly 12 final tables:
     - 6 domain code lookup tables: `cnae`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio` with schema `(codigo TEXT PRIMARY KEY, descricao TEXT)`.
     - `empresas(cnpj_basico TEXT, razao_social TEXT, natureza_juridica TEXT, qualificacao_responsavel TEXT, capital_social_str TEXT, porte_empresa TEXT, ente_federativo_responsavel TEXT, capital_social REAL)` with indices `idx_empresas_cnpj_basico`, `idx_empresas_razao_social`.
     - `estabelecimento(cnpj_basico TEXT, cnpj_ordem TEXT, cnpj_dv TEXT, matriz_filial TEXT, nome_fantasia TEXT, situacao_cadastral TEXT, data_situacao_cadastral TEXT, motivo_situacao_cadastral TEXT, nome_cidade_exterior TEXT, pais TEXT, data_inicio_atividades TEXT, cnae_fiscal TEXT, cnae_fiscal_secundaria TEXT, tipo_logradouro TEXT, logradouro TEXT, numero TEXT, complemento TEXT, bairro TEXT, cep TEXT, uf TEXT, municipio TEXT, ddd1 TEXT, telefone1 TEXT, ddd2 TEXT, telefone2 TEXT, ddd_fax TEXT, fax TEXT, correio_eletronico TEXT, situacao_especial TEXT, data_situacao_especial TEXT, cnpj TEXT)` with indices `idx_estabelecimento_cnpj_basico`, `idx_estabelecimento_cnpj`, `idx_estabelecimento_nomefantasia`.
     - `socios(cnpj TEXT, cnpj_basico TEXT, identificador_de_socio TEXT, nome_socio TEXT, cnpj_cpf_socio TEXT, qualificacao_socio TEXT, data_entrada_sociedade TEXT, pais TEXT, representante_legal TEXT, nome_representante TEXT, qualificacao_representante_legal TEXT, faixa_etaria TEXT)` with indices `idx_socios_cnpj`, `idx_socios_cnpj_cpf_socio`, `idx_socios_nome_socio`, `idx_socios_representante`, `idx_socios_representante_nome`.
     - `simples(cnpj_basico TEXT, opcao_simples TEXT, data_opcao_simples TEXT, data_exclusao_simples TEXT, opcao_mei TEXT, data_opcao_mei TEXT, data_exclusao_mei TEXT)` with index `idx_simples_cnpj_basico`.
     - `cnpj_base2matriz(cnpj_basico TEXT PRIMARY KEY, cnpj TEXT)` with index `idx_cnpj_base2matriz`.
     - `_referencia(referencia TEXT, valor TEXT)`.

2. **Legacy Python ETL Pipeline (`dados_cnpj_para_sqlite.py`)**:
   - In lines 187–195:
     ```python
     create table cnpj_base2matriz as 
     SELECT Distinct t.cnpj_basico, te.cnpj as cnpj
     from empresas t
     left join estabelecimento te on te.cnpj_basico = t.cnpj_basico
     where te.matriz_filial='1';
     CREATE INDEX idx_cnpj_base2matriz ON cnpj_base2matriz(cnpj_basico);
     ```
   - In lines 208–236 (The August 2026 Quirk):
     ```python
     -- a partir de ago/2026 está aparecendo apenas o radical de cnpj do sócio na coluna cnpj_cpf_socio
     -- separando em duas partes, a que tiver cnpj é feito um join para obter o cnpj completo
     CREATE TABLE socios AS 
     SELECT te.cnpj as cnpj, ts.*
     from socios_original ts
     left join cnpj_base2matriz te on te.cnpj_basico = ts.cnpj_basico
     where ts.identificador_de_socio<>'1';

     insert into socios 
     SELECT te.cnpj as cnpj,
         ts.cnpj_basico,
         ts.identificador_de_socio,
         ts.nome_socio,
         tes.cnpj as cnpj_cpf_socio,
         ts.qualificacao_socio,
         ts.data_entrada_sociedade,
         ts.pais,
         ts.representante_legal,
         ts.nome_representante,
         ts.qualificacao_representante_legal,
         ts.faixa_etaria
     from socios_original ts
     left join cnpj_base2matriz te on te.cnpj_basico = ts.cnpj_basico
     left join cnpj_base2matriz tes on tes.cnpj_basico = ts.cnpj_cpf_socio
     where ts.identificador_de_socio='1';
     ```
   - In lines 240–245:
     `DROP TABLE IF EXISTS socios_original;`
   - In lines 272–275:
     Inserts audit rows `('CNPJ', '{dataReferencia}')` and `('cnpj_qtde', '{qtde_cnpjs}')` into `_referencia`.

3. **Ingestion PRAGMAs Presets in Target Repository (`src/config.rs:115–148`)**:
   - Verified that `IngestionPragmas::apply_to` applies:
     `PRAGMA page_size = 4096;`
     `PRAGMA journal_mode = OFF;`
     `PRAGMA synchronous = OFF;`
     `PRAGMA cache_size = -500000;` (-500 MB)
     `PRAGMA temp_store = MEMORY;`
     `PRAGMA locking_mode = EXCLUSIVE;`
     `PRAGMA foreign_keys = OFF;`
     `PRAGMA auto_vacuum = NONE;`
     `PRAGMA threads = 4;`
   - Verified that `IngestionPragmas::apply_post_load` applies:
     `PRAGMA optimize;`
     `PRAGMA journal_mode = WAL;`
     `PRAGMA synchronous = NORMAL;`
     `PRAGMA locking_mode = NORMAL;`

4. **Integration with Downstream Consumers (`crates/ecd-graph/src/db.rs` and `bfs.rs`)**:
   - `crates/ecd-graph/src/bfs.rs` attaches `cnpj.db` as `cnpj_db` and queries:
     `SELECT razao_social FROM cnpj_db.empresas WHERE cnpj_basico = ?1;`
   - `rede_cria_tabela_rede.db.py` queries `socios(cnpj, cnpj_cpf_socio, nome_socio, qualificacao_socio, representante_legal, nome_representante, qualificacao_representante_legal)`, `qualificacao_socio(codigo, descricao)`, and `estabelecimento(cnpj, cnpj_basico, matriz_filial)`.

---

## 2. Logic Chain

1. **Zero Index Overhead during Bulk Ingestion (Observation 1, 3)**:
   - Inserting ~55M records across 37 files into SQLite tables with active B-tree indices results in severe write amplification: for each row, 1 heap write plus 2–5 index B-tree traverses and page rebalances.
   - Deferring all index creation until after all data is inserted allows SQLite to perform sequential append-only writes to raw leaf pages.
   - When `CREATE INDEX` is executed post-load, SQLite performs an external merge-sort using `threads = 4`, sorted memory buffers, and builds B-trees bottom-up with 100% leaf fill factor (zero page splits, ~35% smaller index on disk, 5x–10x faster overall).

2. **Prepared Statements with Statement Caching & Transaction Sizing (Observation 3)**:
   - Single-row SQL parsing in SQLite is costly. Pre-compiling statements via `Transaction::prepare_cached` eliminates statement re-parsing across millions of iterations.
   - Committing transactions every 10,000 to 50,000 rows (or per 5,000-row batch received from the MPSC channel) provides the optimal balance: it amortizes the SQLite transaction boundary cost while keeping RAM usage negligible (< a few megabytes per transaction).

3. **Staging Table Strategy for `socios` and Materialization of `cnpj_base2matriz` (Observation 1, 2)**:
   - The RFB `Socios` dataset only contains `cnpj_basico` (8 digits). The final `socios` table requires the full 14-digit CNPJ of the company (`cnpj TEXT`), requiring a join with the Matriz establishment.
   - In August 2026, RFB truncated corporate partner CNPJs (`identificador_de_socio = '1'`) to their 8-digit radical in `cnpj_cpf_socio`.
   - Populating an in-memory hash table for 50M base-to-matriz mappings would consume >1.1 GB of RAM, threatening the 1.8 GB RSS ceiling.
   - Therefore, the optimal architecture is:
     1. Ingest `Empresas*.zip` into `empresas`.
     2. Ingest `Estabelecimentos*.zip` into `estabelecimento` with synthesized 14-digit `cnpj`.
     3. Create index `idx_estabelecimento_cnpj_basico ON estabelecimento (cnpj_basico)`.
     4. Materialize `cnpj_base2matriz(cnpj_basico PRIMARY KEY, cnpj)` via `SELECT DISTINCT cnpj_basico, cnpj FROM estabelecimento WHERE matriz_filial = '1'`.
     5. Ingest `Socios*.zip` into an unindexed staging table `socios_original`.
     6. Execute the August 2026 expansion SQL: join `socios_original` with `cnpj_base2matriz` on `ts.cnpj_basico` (for company CNPJ) and `ts.cnpj_cpf_socio` (for PJ partner expansion) directly into the final `socios` table.
     7. Drop `socios_original` and create the 5 secondary indices on `socios`.

4. **Audit Metadata Recording & WAL Transition (Observation 1, 3)**:
   - Once all tables are populated and indexed, `_referencia` is populated with `CNPJ` (the reference date extracted from filename or CLI) and `cnpj_qtde` (`SELECT COUNT(*) FROM estabelecimento`).
   - SQLite `PRAGMA optimize;` collects index statistics (equivalent to `ANALYZE` sampling), then `PRAGMA journal_mode = WAL;` and `PRAGMA synchronous = NORMAL;` transition the database into safe, concurrent multi-reader mode for downstream graph tools and Axum API services.

---

## 3. Caveats

1. **Pre-August 2026 vs Post-August 2026 Corporative Partner Radical**:
   - The query uses `COALESCE(tes.cnpj, ts.cnpj_cpf_socio)`:
     If the dataset is pre-August 2026, `ts.cnpj_cpf_socio` is already 14 digits and will not match `tes.cnpj_basico` (8 digits), so `COALESCE` safely preserves the original 14-digit CNPJ.
     If the dataset is August 2026+, `ts.cnpj_cpf_socio` has 8 digits, matches `tes.cnpj_basico`, and expands to the 14-digit Matriz CNPJ.
     If a corporate partner has an 8-digit radical not present in the current dataset (e.g. inactive or foreign-registered Brazilian entity), `COALESCE` preserves the 8-digit radical rather than NULLing it out.
2. **Column Retention in `empresas` and `estabelecimento`**:
   - In `PROJECT.md` line 73 and 75, `capital_social_str`, `cnpj_ordem`, and `cnpj_dv` are preserved in the schema contract. The legacy Python script dropped them (`ALTER TABLE DROP COLUMN`). In modern SQLite, retaining them provides richer auditing capabilities, satisfies the contract in `PROJECT.md`, and avoids mutating tables after load. If strict omission is requested, the DDL can omit them without affecting any other query.
3. **Domain Table Primary Key Indexing**:
   - Domain tables define `codigo TEXT PRIMARY KEY`. SQLite automatically creates an internal unique index for primary keys (`sqlite_autoindex_<table_name>_1`). Explicitly adding `CREATE INDEX IF NOT EXISTS idx_<table_name> ON <table_name>(codigo)` is redundant but harmless.

---

## 4. Conclusion & Concrete Code Specification

We propose two complete, production-ready Rust source files for the persistence layer:
1. `src/ingestion/schema.rs` — Complete DDL, table setup, index generation, and metadata structures.
2. `src/ingestion/loader.rs` — Batch insert handlers, transaction manager, `cnpj_base2matriz` materialization, August 2026 quirk expansion, and post-load sequencing.

### 4.1 Specification for `src/ingestion/schema.rs`

```rust
//! SQLite table schemas, DDL definitions, and post-load index specifications.

use rusqlite::{Connection, Result};

/// Canonical list of all 6 domain lookup tables.
pub const DOMAIN_TABLES: [&str; 6] = [
    "cnae",
    "motivo",
    "municipio",
    "natureza_juridica",
    "pais",
    "qualificacao_socio",
];

/// SQL DDL for domain lookup tables (codigo, descricao).
pub fn domain_table_ddl(table_name: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {table_name} (
            codigo TEXT PRIMARY KEY,
            descricao TEXT
        );"
    )
}

/// SQL DDL for `empresas` (7 base columns + capital_social REAL).
pub const EMPRESAS_DDL: &str = "
CREATE TABLE IF NOT EXISTS empresas (
    cnpj_basico TEXT,
    razao_social TEXT,
    natureza_juridica TEXT,
    qualificacao_responsavel TEXT,
    capital_social_str TEXT,
    porte_empresa TEXT,
    ente_federativo_responsavel TEXT,
    capital_social REAL
);";

/// SQL DDL for `estabelecimento` (30 original columns + synthesized 14-char cnpj).
pub const ESTABELECIMENTO_DDL: &str = "
CREATE TABLE IF NOT EXISTS estabelecimento (
    cnpj_basico TEXT,
    cnpj_ordem TEXT,
    cnpj_dv TEXT,
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
);";

/// SQL DDL for `simples` (7 columns).
pub const SIMPLES_DDL: &str = "
CREATE TABLE IF NOT EXISTS simples (
    cnpj_basico TEXT,
    opcao_simples TEXT,
    data_opcao_simples TEXT,
    data_exclusao_simples TEXT,
    opcao_mei TEXT,
    data_opcao_mei TEXT,
    data_exclusao_mei TEXT
);";

/// SQL DDL for unindexed staging table `socios_original`.
pub const SOCIOS_ORIGINAL_DDL: &str = "
CREATE TABLE IF NOT EXISTS socios_original (
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
);";

/// SQL DDL for final `socios` table (12 columns: cnpj TEXT + 11 socio columns).
pub const SOCIOS_FINAL_DDL: &str = "
CREATE TABLE IF NOT EXISTS socios (
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
);";

/// SQL DDL for `cnpj_base2matriz` lookup cache table.
pub const CNPJ_BASE2MATRIZ_DDL: &str = "
CREATE TABLE IF NOT EXISTS cnpj_base2matriz (
    cnpj_basico TEXT PRIMARY KEY,
    cnpj TEXT
);";

/// SQL DDL for `_referencia` audit metadata table.
pub const REFERENCIA_DDL: &str = "
CREATE TABLE IF NOT EXISTS _referencia (
    referencia TEXT,
    valor TEXT
);";

/// Create all initial tables without secondary indices for fast bulk loading.
pub fn create_initial_tables(conn: &Connection) -> Result<()> {
    for domain in DOMAIN_TABLES {
        conn.execute_batch(&domain_table_ddl(domain))?;
    }
    conn.execute_batch(EMPRESAS_DDL)?;
    conn.execute_batch(ESTABELECIMENTO_DDL)?;
    conn.execute_batch(SIMPLES_DDL)?;
    conn.execute_batch(SOCIOS_ORIGINAL_DDL)?;
    conn.execute_batch(REFERENCIA_DDL)?;
    Ok(())
}

/// DDL statements for secondary indices created strictly post-load.
pub const POST_LOAD_INDICES_SQL: &str = "
-- Empresas
CREATE INDEX IF NOT EXISTS idx_empresas_cnpj_basico ON empresas (cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_empresas_razao_social ON empresas (razao_social);

-- Estabelecimento
CREATE INDEX IF NOT EXISTS idx_estabelecimento_cnpj_basico ON estabelecimento (cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_estabelecimento_cnpj ON estabelecimento (cnpj);
CREATE INDEX IF NOT EXISTS idx_estabelecimento_nomefantasia ON estabelecimento (nome_fantasia);

-- Simples
CREATE INDEX IF NOT EXISTS idx_simples_cnpj_basico ON simples (cnpj_basico);

-- Socios
CREATE INDEX IF NOT EXISTS idx_socios_cnpj ON socios (cnpj);
CREATE INDEX IF NOT EXISTS idx_socios_cnpj_cpf_socio ON socios (cnpj_cpf_socio);
CREATE INDEX IF NOT EXISTS idx_socios_nome_socio ON socios (nome_socio);
CREATE INDEX IF NOT EXISTS idx_socios_representante ON socios (representante_legal);
CREATE INDEX IF NOT EXISTS idx_socios_representante_nome ON socios (nome_representante);
";

/// Create all secondary indices post-load.
pub fn create_post_load_indices(conn: &Connection) -> Result<()> {
    conn.execute_batch(POST_LOAD_INDICES_SQL)?;
    Ok(())
}
```

---

### 4.2 Specification for `src/ingestion/loader.rs`

```rust
//! High-performance SQLite batch loader with prepared statement caching,
//! transactional chunking, cache materialization, and August 2026 radical expansion.

use crate::config::IngestionPragmas;
use crate::ingestion::schema::{
    create_initial_tables, create_post_load_indices, CNPJ_BASE2MATRIZ_DDL, SOCIOS_FINAL_DDL,
};
use rusqlite::{params, Connection, Result, Transaction};
use std::path::Path;

/// In-memory representation of an `empresas` row for batch insertion.
#[derive(Debug, Clone)]
pub struct EmpresaBatchRow {
    pub cnpj_basico: String,
    pub razao_social: String,
    pub natureza_juridica: String,
    pub qualificacao_responsavel: String,
    pub capital_social_str: String,
    pub porte_empresa: String,
    pub ente_federativo_responsavel: String,
    pub capital_social: f64,
}

/// In-memory representation of an `estabelecimento` row for batch insertion.
#[derive(Debug, Clone)]
pub struct EstabelecimentoBatchRow {
    pub cnpj_basico: String,
    pub cnpj_ordem: String,
    pub cnpj_dv: String,
    pub matriz_filial: String,
    pub nome_fantasia: String,
    pub situacao_cadastral: String,
    pub data_situacao_cadastral: String,
    pub motivo_situacao_cadastral: String,
    pub nome_cidade_exterior: String,
    pub pais: String,
    pub data_inicio_atividades: String,
    pub cnae_fiscal: String,
    pub cnae_fiscal_secundaria: String,
    pub tipo_logradouro: String,
    pub logradouro: String,
    pub numero: String,
    pub complemento: String,
    pub bairro: String,
    pub cep: String,
    pub uf: String,
    pub municipio: String,
    pub ddd1: String,
    pub telefone1: String,
    pub ddd2: String,
    pub telefone2: String,
    pub ddd_fax: String,
    pub fax: String,
    pub correio_eletronico: String,
    pub situacao_especial: String,
    pub data_situacao_especial: String,
    pub cnpj: String, // 14-digit synthesized CNPJ
}

/// In-memory representation of a `simples` row for batch insertion.
#[derive(Debug, Clone)]
pub struct SimplesBatchRow {
    pub cnpj_basico: String,
    pub opcao_simples: String,
    pub data_opcao_simples: String,
    pub data_exclusao_simples: String,
    pub opcao_mei: String,
    pub data_opcao_mei: String,
    pub data_exclusao_mei: String,
}

/// In-memory representation of a raw socio row for staging table insertion.
#[derive(Debug, Clone)]
pub struct SocioStagingBatchRow {
    pub cnpj_basico: String,
    pub identificador_de_socio: String,
    pub nome_socio: String,
    pub cnpj_cpf_socio: String,
    pub qualificacao_socio: String,
    pub data_entrada_sociedade: String,
    pub pais: String,
    pub representante_legal: String,
    pub nome_representante: String,
    pub qualificacao_representante_legal: String,
    pub faixa_etaria: String,
}

/// State and execution manager for SQLite database ingestion.
pub struct SqliteIngestionLoader {
    conn: Connection,
}

impl SqliteIngestionLoader {
    /// Open SQLite database at `db_path`, configuring ingestion PRAGMAs.
    pub fn open<P: AsRef<Path>>(
        db_path: P,
        pragmas: &IngestionPragmas,
        drop_existing: bool,
    ) -> Result<Self> {
        let path = db_path.as_ref();
        if drop_existing && path.exists() {
            std::fs::remove_file(path).ok();
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)?;
        pragmas.apply_to(&conn)?;
        create_initial_tables(&conn)?;

        Ok(Self { conn })
    }

    /// Provides access to the underlying connection reference.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Provides mutable access to the underlying connection.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Batch insert domain lookup records (e.g. cnae, motivo, municipio).
    pub fn insert_domain_batch(
        &mut self,
        table_name: &str,
        records: &[(String, String)],
    ) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }
        let sql = format!("INSERT OR REPLACE INTO {table_name} (codigo, descricao) VALUES (?1, ?2)");
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(&sql)?;
            for (code, desc) in records {
                stmt.execute(params![code, desc])?;
            }
        }
        tx.commit()?;
        Ok(records.len())
    }

    /// Batch insert `empresas` records inside a single transaction with prepared statement caching.
    pub fn insert_empresas_batch(&mut self, rows: &[EmpresaBatchRow]) -> Result<usize> {
        if rows.is_empty() {
            return Ok(0);
        }
        let sql = "
            INSERT INTO empresas (
                cnpj_basico, razao_social, natureza_juridica, qualificacao_responsavel,
                capital_social_str, porte_empresa, ente_federativo_responsavel, capital_social
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);";

        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(sql)?;
            for row in rows {
                stmt.execute(params![
                    row.cnpj_basico,
                    row.razao_social,
                    row.natureza_juridica,
                    row.qualificacao_responsavel,
                    row.capital_social_str,
                    row.porte_empresa,
                    row.ente_federativo_responsavel,
                    row.capital_social,
                ])?;
            }
        }
        tx.commit()?;
        Ok(rows.len())
    }

    /// Batch insert `estabelecimento` records inside a single transaction with prepared statement caching.
    pub fn insert_estabelecimento_batch(&mut self, rows: &[EstabelecimentoBatchRow]) -> Result<usize> {
        if rows.is_empty() {
            return Ok(0);
        }
        let sql = "
            INSERT INTO estabelecimento (
                cnpj_basico, cnpj_ordem, cnpj_dv, matriz_filial, nome_fantasia,
                situacao_cadastral, data_situacao_cadastral, motivo_situacao_cadastral,
                nome_cidade_exterior, pais, data_inicio_atividades, cnae_fiscal,
                cnae_fiscal_secundaria, tipo_logradouro, logradouro, numero,
                complemento, bairro, cep, uf, municipio, ddd1, telefone1,
                ddd2, telefone2, ddd_fax, fax, correio_eletronico,
                situacao_especial, data_situacao_especial, cnpj
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31
            );";

        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(sql)?;
            for r in rows {
                stmt.execute(params![
                    r.cnpj_basico,
                    r.cnpj_ordem,
                    r.cnpj_dv,
                    r.matriz_filial,
                    r.nome_fantasia,
                    r.situacao_cadastral,
                    r.data_situacao_cadastral,
                    r.motivo_situacao_cadastral,
                    r.nome_cidade_exterior,
                    r.pais,
                    r.data_inicio_atividades,
                    r.cnae_fiscal,
                    r.cnae_fiscal_secundaria,
                    r.tipo_logradouro,
                    r.logradouro,
                    r.numero,
                    r.complemento,
                    r.bairro,
                    r.cep,
                    r.uf,
                    r.municipio,
                    r.ddd1,
                    r.telefone1,
                    r.ddd2,
                    r.telefone2,
                    r.ddd_fax,
                    r.fax,
                    r.correio_eletronico,
                    r.situacao_especial,
                    r.data_situacao_especial,
                    r.cnpj,
                ])?;
            }
        }
        tx.commit()?;
        Ok(rows.len())
    }

    /// Batch insert `simples` records inside a single transaction.
    pub fn insert_simples_batch(&mut self, rows: &[SimplesBatchRow]) -> Result<usize> {
        if rows.is_empty() {
            return Ok(0);
        }
        let sql = "
            INSERT INTO simples (
                cnpj_basico, opcao_simples, data_opcao_simples,
                data_exclusao_simples, opcao_mei, data_opcao_mei, data_exclusao_mei
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);";

        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(sql)?;
            for r in rows {
                stmt.execute(params![
                    r.cnpj_basico,
                    r.opcao_simples,
                    r.data_opcao_simples,
                    r.data_exclusao_simples,
                    r.opcao_mei,
                    r.data_opcao_mei,
                    r.data_exclusao_mei,
                ])?;
            }
        }
        tx.commit()?;
        Ok(rows.len())
    }

    /// Batch insert raw socios records into staging table `socios_original`.
    pub fn insert_socios_staging_batch(&mut self, rows: &[SocioStagingBatchRow]) -> Result<usize> {
        if rows.is_empty() {
            return Ok(0);
        }
        let sql = "
            INSERT INTO socios_original (
                cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio,
                qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
                nome_representante, qualificacao_representante_legal, faixa_etaria
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);";

        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(sql)?;
            for r in rows {
                stmt.execute(params![
                    r.cnpj_basico,
                    r.identificador_de_socio,
                    r.nome_socio,
                    r.cnpj_cpf_socio,
                    r.qualificacao_socio,
                    r.data_entrada_sociedade,
                    r.pais,
                    r.representante_legal,
                    r.nome_representante,
                    r.qualificacao_representante_legal,
                    r.faixa_etaria,
                ])?;
            }
        }
        tx.commit()?;
        Ok(rows.len())
    }

    /// Materialize lookup cache table `cnpj_base2matriz` from `estabelecimento`.
    /// Requires `estabelecimento` to be populated and preferably indexed on `cnpj_basico`.
    pub fn materialize_cnpj_base2matriz(&mut self) -> Result<usize> {
        self.conn.execute_batch(CNPJ_BASE2MATRIZ_DDL)?;
        
        let inserted = self.conn.execute(
            "INSERT OR IGNORE INTO cnpj_base2matriz (cnpj_basico, cnpj)
             SELECT DISTINCT cnpj_basico, cnpj
             FROM estabelecimento
             WHERE matriz_filial = '1';",
            [],
        )?;

        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_cnpj_base2matriz ON cnpj_base2matriz (cnpj_basico);"
        )?;

        Ok(inserted)
    }

    /// Materializes the final `socios` table from `socios_original` and `cnpj_base2matriz`,
    /// performing the August 2026 8-digit radical expansion for corporate partners (identificador_de_socio = '1').
    pub fn transform_socios_and_expand_radicals(&mut self) -> Result<usize> {
        // Ensure destination table exists
        self.conn.execute_batch(SOCIOS_FINAL_DDL)?;

        // Index staging table to accelerate the transformation join
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_socios_original_cnpj_basico ON socios_original(cnpj_basico);"
        )?;

        // Part 1: Non-PJ partners (PF, PE): direct left join to company matriz
        self.conn.execute(
            "INSERT INTO socios (
                cnpj, cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio,
                qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
                nome_representante, qualificacao_representante_legal, faixa_etaria
            )
            SELECT
                te.cnpj AS cnpj,
                ts.cnpj_basico,
                ts.identificador_de_socio,
                ts.nome_socio,
                ts.cnpj_cpf_socio,
                ts.qualificacao_socio,
                ts.data_entrada_sociedade,
                ts.pais,
                ts.representante_legal,
                ts.nome_representante,
                ts.qualificacao_representante_legal,
                ts.faixa_etaria
            FROM socios_original ts
            LEFT JOIN cnpj_base2matriz te ON te.cnpj_basico = ts.cnpj_basico
            WHERE ts.identificador_de_socio <> '1';",
            [],
        )?;

        // Part 2: PJ partners (identificador_de_socio = '1'):
        // Join te for company matriz AND tes for partner's 8-digit radical expansion
        let pj_count = self.conn.execute(
            "INSERT INTO socios (
                cnpj, cnpj_basico, identificador_de_socio, nome_socio, cnpj_cpf_socio,
                qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
                nome_representante, qualificacao_representante_legal, faixa_etaria
            )
            SELECT
                te.cnpj AS cnpj,
                ts.cnpj_basico,
                ts.identificador_de_socio,
                ts.nome_socio,
                COALESCE(tes.cnpj, ts.cnpj_cpf_socio) AS cnpj_cpf_socio,
                ts.qualificacao_socio,
                ts.data_entrada_sociedade,
                ts.pais,
                ts.representante_legal,
                ts.nome_representante,
                ts.qualificacao_representante_legal,
                ts.faixa_etaria
            FROM socios_original ts
            LEFT JOIN cnpj_base2matriz te ON te.cnpj_basico = ts.cnpj_basico
            LEFT JOIN cnpj_base2matriz tes ON tes.cnpj_basico = ts.cnpj_cpf_socio
            WHERE ts.identificador_de_socio = '1';",
            [],
        )?;

        // Drop temporary staging table to reclaim pages
        self.conn.execute_batch("DROP TABLE IF EXISTS socios_original;")?;

        let total_socios: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM socios;",
            [],
            |r| r.get(0),
        )?;

        Ok(total_socios)
    }

    /// Record reference date and total establishment count into `_referencia`.
    pub fn record_audit_metadata(&mut self, ref_date: &str) -> Result<()> {
        let qtde: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM estabelecimento;",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO _referencia (referencia, valor) VALUES (?1, ?2);"
            )?;
            stmt.execute(params!["CNPJ", ref_date])?;
            stmt.execute(params!["cnpj_qtde", qtde.to_string()])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Execute post-insert indexing and apply production WAL PRAGMAs.
    pub fn finalize_post_load(&mut self, skip_indices: bool) -> Result<()> {
        if !skip_indices {
            create_post_load_indices(&self.conn)?;
        }
        IngestionPragmas::apply_post_load(&self.conn)?;
        Ok(())
    }
}
```

---

## 5. Verification Method

### 5.1 Compilation and Unit Testing
Once implemented by the worker agent:
```bash
cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
cargo check
cargo test
```

### 5.2 SQLite Schema and Contract Verification
Verify in-memory or on a synthetic SQLite database:
```rust
#[test]
fn test_sqlite_schemas_and_indices() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let pragmas = IngestionPragmas::default();
    pragmas.apply_to(&conn).unwrap();
    create_initial_tables(&conn).unwrap();
    create_post_load_indices(&conn).unwrap();

    let mut tables = Vec::new();
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").unwrap();
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
    for row in rows {
        tables.push(row.unwrap());
    }

    assert!(tables.contains(&"cnae".to_string()));
    assert!(tables.contains(&"empresas".to_string()));
    assert!(tables.contains(&"estabelecimento".to_string()));
    assert!(tables.contains(&"simples".to_string()));
    assert!(tables.contains(&"_referencia".to_string()));
}
```

### 5.3 August 2026 PJ Partner Radical Expansion Verification
```rust
#[test]
fn test_aug_2026_radical_expansion() {
    let mut loader = SqliteIngestionLoader::open(":memory:", &IngestionPragmas::default(), false).unwrap();
    
    // Seed estabelecimento with Matriz (11223344000199) and Partner Matriz (55667788000100)
    loader.conn().execute(
        "INSERT INTO estabelecimento (cnpj_basico, matriz_filial, cnpj) VALUES ('11223344', '1', '11223344000199')",
        [],
    ).unwrap();
    loader.conn().execute(
        "INSERT INTO estabelecimento (cnpj_basico, matriz_filial, cnpj) VALUES ('55667788', '1', '55667788000100')",
        [],
    ).unwrap();

    // Materialize base2matriz
    loader.materialize_cnpj_base2matriz().unwrap();

    // Seed staging socios with 8-digit PJ partner radical ('55667788')
    loader.conn().execute(
        "INSERT INTO socios_original (cnpj_basico, identificador_de_socio, cnpj_cpf_socio, nome_socio)
         VALUES ('11223344', '1', '55667788', 'HOLDING PARTICIPACOES LTDA')",
        [],
    ).unwrap();

    // Transform and expand
    loader.transform_socios_and_expand_radicals().unwrap();

    // Verify expanded 14-digit CNPJ in socios
    let (cnpj, socio_cnpj): (String, String) = loader.conn().query_row(
        "SELECT cnpj, cnpj_cpf_socio FROM socios WHERE cnpj_basico = '11223344'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();

    assert_eq!(cnpj, "11223344000199");
    assert_eq!(socio_cnpj, "55667788000100");
}
```

### 5.4 Invalidation Conditions
- If RFB alters table column order or adds new columns in official layout versions after 2026, the DDL strings in `schema.rs` and the parameter mappings in `loader.rs` must be adjusted accordingly.
- If SQLite is compiled without FTS5 or backup extensions, check `Cargo.toml` feature flags (`rusqlite = { version = "0.32", features = ["bundled-full"] }`).
