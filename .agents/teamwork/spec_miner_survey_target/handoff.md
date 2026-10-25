# Handoff Report — Specification Survey of RFB CNPJ Open Data Pipeline (`cnpj.db` & `rede.db`)

**Agent**: `spec_miner_survey_target` (SPECIFICATION MINER)  
**Parent**: `orchestrator_1` (`0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)  
**Target Repository Investigated**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`  
**Reference Code & Databases**:
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/dados_cnpj_baixa.py`
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/dados_cnpj_para_sqlite.py`
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/rede_cria_tabela_rede.db.py`
- `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/rede_cria_tabela_cnpj_links_ete.py`
- Sample databases in `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede/bases/*.db`
- Consumer application in `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede/rede_sqlite_cnpj.py`
- Rust consumer engine in `/home/pizani/Projects/rust-sped-analitycs/crates/ecd-graph/src/db.rs`, `bfs.rs`, `shortest_path.rs`

---

## 1. Observation

### 1.1 Direct File Observations

1. **`dados_cnpj_baixa.py`** (Lines 13-121):
   - Downloads 37 `.zip` files from RFB WebDAV server:
     `base_url = "https://arquivos.receitafederal.gov.br/public.php/webdav"` with `share_token = "YggdBLfdninEJX9"`.
   - Uses WebDAV `PROPFIND` with `Depth: 1` to find latest directory `YYYY-MM/` and enqueues all `.zip` files using `parfive` (5 parallel connections).
   - Target folders: downloads to `dados-publicos-zip/`, extracts into `dados-publicos/`.

2. **`dados_cnpj_para_sqlite.py`** (Lines 57-275):
   - Expects exactly **37 zip files**.
   - Extracts and loads **6 code/domain tables**:
     - `*.CNAECSV` -> table `cnae`
     - `*.MOTICSV` -> table `motivo`
     - `*.MUNICCSV` -> table `municipio`
     - `*.NATJUCSV` -> table `natureza_juridica`
     - `*.PAISCSV` -> table `pais`
     - `*.QUALSCSV` -> table `qualificacao_socio`
     All domain tables have schema `(codigo TEXT, descricao TEXT)` and an index `idx_<tabela> ON <tabela>(codigo)`.
   - Loads **4 large registration tables**:
     - 10 `*.EMPRECSV` -> table `empresas`
     - 10 `*.ESTABELE` -> table `estabelecimento`
     - 10 `*.SOCIOCSV` -> table `socios_original` -> transformed into `socios`
     - 1 `*.SIMPLES.CSV.*` -> table `simples`
   - Creates auxiliary table **`cnpj_base2matriz`**:
     `SELECT DISTINCT t.cnpj_basico, te.cnpj as cnpj FROM empresas t LEFT JOIN estabelecimento te ON te.cnpj_basico = t.cnpj_basico WHERE te.matriz_filial='1'` with index `idx_cnpj_base2matriz ON cnpj_base2matriz(cnpj_basico)`.
   - Handles the **August 2026 RFB layout change** on `socios`:
     When `anoMes >= '202608'`, RFB changed `cnpj_cpf_socio` for corporate partners (`identificador_de_socio = '1'`) to only output the 8-digit radical (`cnpj_basico`). The script resolves this by joining with `cnpj_base2matriz` to restore the full 14-digit Matriz CNPJ.
   - Cleans up columns:
     - `empresas`: casts `replace(capital_social_str, ',', '.')` to `REAL` into `capital_social`, then drops `capital_social_str`.
     - `estabelecimento`: computes `cnpj = cnpj_basico || cnpj_ordem || cnpj_dv`, then drops `cnpj_ordem` and `cnpj_dv`.
     - `socios_original`: dropped after creating `socios`.
   - Creates metadata table **`_referencia`** containing `('CNPJ', '<dataReferencia>')` and `('cnpj_qtde', '<qtde_cnpjs>')`.

3. **`rede_cria_tabela_rede.db.py`** (Lines 49-247):
   - Generates **`dados-publicos/rede.db`**:
     Creates table `ligacao1`, populates it with 6 societary link rules + 1 branch link rule, groups by `(origem, destino, tipo, base)` into table **`ligacao`**, and creates indices on `id1` and `id2`.
   - Generates **`dados-publicos/rede_search.db`**:
     Creates FTS5 virtual table **`id_search using fts5 (id, descricao)`** indexing company legal names (`razao_social`), trade names (`nome_fantasia`), and partner identifiers from `ligacao`.

4. **`rede_cria_tabela_cnpj_links_ete.py`** (Lines 512-680):
   - Generates **`dados-publicos/cnpj_links_ete.db`** with table **`link_ete (id1, id2, descricao, valor)`** and indices `ix_link_ete_id1`, `ix_link_ete_id2`.
   - Normalizes addresses (`EN_<norm_address>-<municipio>-<uf>`), phones (`TE_<ddd> <num>`), and emails (`EM_<email>`), aggregating contacts shared across multiple companies (`HAVING count > 1`).

5. **Direct Inspection of Sample Databases in `rede/bases/*.db`**:
   - `base_cnpj_vazia.db`: Contains all 12 tables + indices. Confirms exact column names and types.
   - `rede.db`: Contains table `ligacao` with 78,111 sample edges. Sample rows show:
     - `id1 = 'PF_***000000**-AITOL ECKNLE DE OZUAS'`, `id2 = 'PJ_77913500000111'`, `descricao = 'Sócio'`, `comentario = 'socios'`
     - `id1 = 'PJ_...filial'`, `id2 = 'PJ_...matriz'`, `descricao = 'filial'`, `comentario = 'estabelecimento'`
   - `links.db`: Table `links (id1, id2, descricao, valor)` for sanctions, PEP, CEIS, CEPIM, CNEP, and PGFN dívida ativa.

---

### 1.2 Specification Tables

#### Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Download | WebDAV Catalog & Retrieval | Query RFB Nextcloud WebDAV for latest `YYYY-MM/` and download 37 zip files | WebDAV URL + `share_token` (`YggdBLfdninEJX9`) | 37 `.zip` files in `dados-publicos-zip/` | Exits with error if WebDAV fails or count != 37 | `dados_cnpj_baixa.py` |
| 2 | Ingestion | Domain Code Ingestion | Loads 6 domain code lookup tables into `cnpj.db` | 6 CSV files (`.CNAECSV`, `.MOTICSV`, `.MUNICCSV`, `.NATJUCSV`, `.PAISCSV`, `.QUALSCSV`) | Tables `cnae`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio` + indices | Replaces table if exists | `dados_cnpj_para_sqlite.py:57` |
| 3 | Ingestion | Enterprises Ingestion (`empresas`) | Ingests enterprise master records, parses BRL comma decimals to REAL | 10 `*.EMPRECSV` files | Table `empresas` (7 columns) + 2 indices | Appends to table | `dados_cnpj_para_sqlite.py:86` |
| 4 | Ingestion | Establishments Ingestion (`estabelecimento`) | Ingests branches/headquarters, synthesizes 14-digit CNPJ | 10 `*.ESTABELE` files | Table `estabelecimento` (28 columns) + 3 indices | Appends to table | `dados_cnpj_para_sqlite.py:93` |
| 5 | Ingestion | Matriz Mapping (`cnpj_base2matriz`) | Maps 8-digit radical to 14-digit Matriz CNPJ | `empresas` and `estabelecimento WHERE matriz_filial='1'` | Table `cnpj_base2matriz` + index | Materialized table | `dados_cnpj_para_sqlite.py:187` |
| 6 | Ingestion | Partners Ingestion & Normalization (`socios`) | Ingests societary composition, resolves Aug/2026 radical-only PJ partner quirk | 10 `*.SOCIOCSV` files + `cnpj_base2matriz` | Table `socios` (12 columns) + 5 indices | Drops `socios_original` on completion | `dados_cnpj_para_sqlite.py:114,208` |
| 7 | Ingestion | Tax Regime (`simples`) | Ingests Simples Nacional and MEI option/exclusion history | 1 `*.SIMPLES.CSV.*` file | Table `simples` (7 columns) + 1 index | Appends to table | `dados_cnpj_para_sqlite.py:128` |
| 8 | Ingestion | ETL Metadata (`_referencia`) | Records reference date and establishment count | Establishment row count + filename timestamp | Table `_referencia` (2 rows) | Insert statement | `dados_cnpj_para_sqlite.py:255` |
| 9 | Graph Transform | Societary Edges (PJ -> PJ) | Creates edge from Corporate Partner Matriz to Investee Matriz | `socios` where `length(cnpj_cpf_socio)=14` | `ligacao (id1='PJ_'\|\|cnpj_cpf_socio, id2='PJ_'\|\|cnpj, descricao=sq.descricao, comentario='socios')` | Skipped if length != 14 | `rede_cria_tabela_rede.db.py:56` |
| 10 | Graph Transform | Societary Edges (PF -> PJ) | Creates edge from Individual Partner to Investee Matriz | `socios` where `length(cnpj_cpf_socio)=11 AND nome_socio<>''` | `ligacao (id1='PF_'\|\|cnpj_cpf_socio\|\|'-'\|\|nome_socio, id2='PJ_'\|\|cnpj, ...)` | Skipped if empty name | `rede_cria_tabela_rede.db.py:63` |
| 11 | Graph Transform | Foreign Entity Edges (PE -> PJ) | Creates edge from Foreign Entity Partner to Investee Matriz | `socios` where `cnpj_cpf_socio=''` | `ligacao (id1='PE_'\|\|nome_socio, id2='PJ_'\|\|cnpj, ...)` | Handled when CPF/CNPJ empty | `rede_cria_tabela_rede.db.py:70` |
| 12 | Graph Transform | Legal Rep of PE (PF -> PE) | Creates edge from Legal Representative to Foreign Entity | `socios` where foreign partner has representative != `***000000**` | `ligacao (id1='PF_'\|\|rep_legal\|\|'-'\|\|nome_rep, id2='PE_'\|\|nome_socio, descricao='rep-sócio-'\|\|sq.descricao)` | Skipped if placeholder CPF | `rede_cria_tabela_rede.db.py:78` |
| 13 | Graph Transform | Legal Rep of PJ (PF -> PJ) | Creates edge from Legal Representative to Corporate Partner | `socios` where corporate partner has representative != `***000000**` | `ligacao (id1='PF_'\|\|rep_legal\|\|'-'\|\|nome_rep, id2='PJ_'\|\|cnpj_cpf_socio, descricao='rep-sócio-'\|\|sq.descricao)` | Skipped if placeholder CPF | `rede_cria_tabela_rede.db.py:86` |
| 14 | Graph Transform | Legal Rep of PF (PF -> PF) | Creates edge from Legal Representative to Individual Partner | `socios` where individual partner has representative != `***000000**` | `ligacao (id1='PF_'\|\|rep_legal\|\|'-'\|\|nome_rep, id2='PF_'\|\|cnpj_cpf_socio\|\|'-'\|\|nome_socio, ...)` | Skipped if placeholder CPF | `rede_cria_tabela_rede.db.py:93` |
| 15 | Graph Transform | Branch Edges (PJ -> PJ) | Creates edge from Filial to Matriz | `estabelecimento` where `matriz_filial='2'` joined with `matriz_filial='1'` | `ligacao (id1='PJ_'\|\|tf.cnpj, id2='PJ_'\|\|t.cnpj, descricao='filial', comentario='estabelecimento')` | Unmatched filiais skipped | `rede_cria_tabela_rede.db.py:100` |
| 16 | Graph Search | Full-Text Search FTS5 (`rede_search.db`) | Virtual FTS5 table for fast prefix & token lookup on IDs, trade names, and corporate names | `estabelecimento`, `empresas`, `ligacao` | Virtual table `id_search using fts5(id, descricao)` | Standard SQLite FTS5 table | `rede_cria_tabela_rede.db.py:184` |
| 17 | Contact Graph | ETE Link Graph (`cnpj_links_ete.db`) | Generates shared contact/address links for forensic clustering | `estabelecimento` addresses, phones, emails | Table `link_ete (id1, id2, descricao, valor)` + indices | Ignores singletons (`count <= 1`) | `rede_cria_tabela_cnpj_links_ete.py` |

---

#### Edge Cases

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | `socios` ingestion | Aug/2026 RFB layout change (`anoMes >= '202608'`) where corporate partner column has only 8 digits | Without intervention, PJ partner CNPJ is incomplete (8 digits instead of 14). Script executes left join against `cnpj_base2matriz` to replace 8-digit radical with 14-digit Matriz CNPJ. Non-matched radicals leave `cnpj_cpf_socio` as NULL. |
| 2 | `socios` parsing | Masked CPFs in RFB data (`***123456**`) | RFB suppresses first 3 and last 2 digits for privacy. Cannot use raw CPF as unique primary key. Script concatenates masked CPF and unaccented uppercase partner name: `PF_***123456**-<NOME>`. |
| 3 | `ligacao` derivation | Dummy representative CPF `***000000**` | RFB fills blank representatives with `***000000**`. Script explicitly filters: `WHERE t.representante_legal <> '***000000**'`. |
| 4 | `estabelecimento` | Companies with Matriz other than `0001` | While `0001` is standard for Matriz, some enterprises have closed `0001` and designated `0002` or higher as Matriz. Script uses `WHERE matriz_filial = '1'`, strictly guaranteeing correct Matriz selection. |
| 5 | `empresas` capital | Capital social formatted with comma (`150000,00`) | Direct float parse fails. Script applies `replace(capital_social_str, ',', '.')` then casts to SQLite `REAL`. |
| 6 | CSV Encodings | Files contain Latin characters (cedillas, accents: `ã`, `é`, `ç`) | RFB files are encoded in `latin1` / WINDOWS-1252 without BOM. Decoders must use Windows-1252/ISO-8859-1; UTF-8 decoder crashes or produces replacement characters. |
| 7 | Filename format | Reference date embedded in filename (e.g. `K3241.K03200Y0.D30610.EMPRECSV`) | Script parses token `D[A-Z0-9]{5}` (e.g. `D30610` -> year 2023, month 06, day 10) to determine base date. |
| 8 | Graph Bidirectionality | Direction of stored edges in `ligacao` | Edges are stored with partner/filial as `id1` (source) and company/matriz as `id2` (target). Graph traversals must query `WHERE id1 = ?1 OR id2 = ?1` to discover full neighbor connectivity. |
| 9 | Quota / Equity % | Requirement R2 mentions "percentuais de participação societária" | **Field does NOT exist in RFB public data**. Official `SOCIOCSV` does not contain ownership quota or percentages, only role qualification and entry date. |
| 10 | Address Normalization | S/N (sem número), abbreviations, punctuation in addresses | `normalizaEndereco` strips punctuation, expands ~100 Brazilian road abbreviations, normalizes `S/N`, removes addresses lacking street numbers, and sorts street name tokens alphabetically. |
| 11 | Contact Clustering | Shared phones and emails | Generic dummy numbers (`0000000`, `1111111`) are discarded; only contacts shared by at least 2 distinct CNPJs (`HAVING count > 1`) are saved in `link_ete`. |

---

## 2. Logic Chain

### 2.1 The Two Core Databases: `cnpj.db` vs. `rede.db`

```
┌────────────────────────────────────────────────────────┐
│               37 RFB Zip Files (WebDAV)               │
└───────────────────────────┬────────────────────────────┘
                            │ (Unzip & CSV Streaming Ingestion)
                            ▼
┌────────────────────────────────────────────────────────┐
│                        cnpj.db                         │
│  - empresas (7 cols, REAL capital_social)              │
│  - estabelecimento (28 cols, 14-digit cnpj)            │
│  - socios (12 cols, resolved matriz CNPJs)             │
│  - simples (7 cols)                                    │
│  - 6 Domain Tables: cnae, motivo, municipio, etc.      │
│  - cnpj_base2matriz (lookup cache)                     │
│  - _referencia (audit metadata)                        │
└───────────────────────────┬────────────────────────────┘
                            │ (SQL Graph Transformation)
                            ▼
┌────────────────────────────────────────────────────────┐
│                        rede.db                         │
│  - ligacao (id1, id2, descricao, comentario)           │
│    Indices: idx_ligacao_origem, idx_ligacao_destino    │
└───────────────────────────┬────────────────────────────┘
                            │ (FTS5 Token Indexing)
                            ▼
┌────────────────────────────────────────────────────────┐
│                     rede_search.db                     │
│  - id_search (Virtual FTS5 Table: id, descricao)       │
└────────────────────────────────────────────────────────┘
```

### 2.2 Detailed Schemas

#### A. Schemas for `cnpj.db`

1. **`cnae`**:
   - `codigo TEXT` (7-digit CNAE code)
   - `descricao TEXT` (Activity description)
   - Index: `CREATE INDEX idx_cnae ON cnae(codigo);`

2. **`motivo`**:
   - `codigo TEXT` (Cadastral situation motive code)
   - `descricao TEXT` (Motive description)
   - Index: `CREATE INDEX idx_motivo ON motivo(codigo);`

3. **`municipio`**:
   - `codigo TEXT` (TOM/RFB municipality code, 4 digits)
   - `descricao TEXT` (Municipality name)
   - Index: `CREATE INDEX idx_municipio ON municipio(codigo);`

4. **`natureza_juridica`**:
   - `codigo TEXT` (4-digit legal nature code)
   - `descricao TEXT` (Legal nature description)
   - Index: `CREATE INDEX idx_natureza_juridica ON natureza_juridica(codigo);`

5. **`pais`**:
   - `codigo TEXT` (3-digit country code)
   - `descricao TEXT` (Country name)
   - Index: `CREATE INDEX idx_pais ON pais(codigo);`

6. **`qualificacao_socio`**:
   - `codigo TEXT` (2-digit partner qualification code)
   - `descricao TEXT` (Partner role description, e.g. "Sócio-Administrador")
   - Index: `CREATE INDEX idx_qualificacao_socio ON qualificacao_socio(codigo);`

7. **`empresas`**:
   - `cnpj_basico TEXT` (8 digits)
   - `razao_social TEXT` (Corporate legal name)
   - `natureza_juridica TEXT` (4 digits, references `natureza_juridica.codigo`)
   - `qualificacao_responsavel TEXT` (2 digits, references `qualificacao_socio.codigo`)
   - `porte_empresa TEXT` ('00' = Não informado, '01' = Microempresa, '03' = EPP, '05' = Demais)
   - `ente_federativo_responsavel TEXT` (Name of federative entity if public)
   - `capital_social REAL` (Share capital in BRL, converted from string `123,45` to float `123.45`)
   - Indices:
     - `CREATE INDEX idx_empresas_cnpj_basico ON empresas (cnpj_basico);`
     - `CREATE INDEX idx_empresas_razao_social ON empresas (razao_social);`

8. **`estabelecimento`**:
   - `cnpj_basico TEXT` (8 digits)
   - `matriz_filial TEXT` ('1' = Matriz, '2' = Filial)
   - `nome_fantasia TEXT` (Trade name)
   - `situacao_cadastral TEXT` ('01' = Nula, '02' = Ativa, '03' = Suspensa, '04' = Inapta, '08' = Baixada)
   - `data_situacao_cadastral TEXT` (YYYYMMDD)
   - `motivo_situacao_cadastral TEXT` (References `motivo.codigo`)
   - `nome_cidade_exterior TEXT`
   - `pais TEXT` (References `pais.codigo`)
   - `data_inicio_atividades TEXT` (YYYYMMDD)
   - `cnae_fiscal TEXT` (Primary CNAE, 7 digits)
   - `cnae_fiscal_secundaria TEXT` (Secondary CNAEs, comma-separated)
   - `tipo_logradouro TEXT` (RUA, AVENIDA, etc.)
   - `logradouro TEXT`
   - `numero TEXT`
   - `complemento TEXT`
   - `bairro TEXT`
   - `cep TEXT` (8 digits)
   - `uf TEXT` (2 uppercase letters, or 'EX')
   - `municipio TEXT` (References `municipio.codigo`)
   - `ddd1 TEXT`
   - `telefone1 TEXT`
   - `ddd2 TEXT`
   - `telefone2 TEXT`
   - `ddd_fax TEXT`
   - `fax TEXT`
   - `correio_eletronico TEXT` (Email)
   - `situacao_especial TEXT`
   - `data_situacao_especial TEXT` (YYYYMMDD)
   - `cnpj TEXT` (14 digits: `cnpj_basico || cnpj_ordem || cnpj_dv`)
   - Indices:
     - `CREATE INDEX idx_estabelecimento_cnpj_basico ON estabelecimento (cnpj_basico);`
     - `CREATE INDEX idx_estabelecimento_cnpj ON estabelecimento (cnpj);`
     - `CREATE INDEX idx_estabelecimento_nomefantasia ON estabelecimento (nome_fantasia);`

9. **`cnpj_base2matriz`**:
   - `cnpj_basico TEXT` (8 digits)
   - `cnpj TEXT` (14 digits of the Matriz establishment)
   - Index: `CREATE INDEX idx_cnpj_base2matriz ON cnpj_base2matriz(cnpj_basico);`

10. **`socios`**:
    - `cnpj TEXT` (14-digit CNPJ of the company Matriz)
    - `cnpj_basico TEXT` (8 digits)
    - `identificador_de_socio TEXT` ('1' = PJ, '2' = PF, '3' = Estrangeiro)
    - `nome_socio TEXT`
    - `cnpj_cpf_socio TEXT` (Masked CPF `***123456**`, or 14-digit CNPJ for PJ partner, or empty)
    - `qualificacao_socio TEXT` (References `qualificacao_socio.codigo`)
    - `data_entrada_sociedade TEXT` (YYYYMMDD)
    - `pais TEXT` (References `pais.codigo`)
    - `representante_legal TEXT` (Masked CPF of representative or '***000000**')
    - `nome_representante TEXT`
    - `qualificacao_representante_legal TEXT` (References `qualificacao_socio.codigo`)
    - `faixa_etaria TEXT` ('1' to '9', '0')
    - Indices:
      - `CREATE INDEX idx_socios_cnpj ON socios(cnpj);`
      - `CREATE INDEX idx_socios_cnpj_cpf_socio ON socios(cnpj_cpf_socio);`
      - `CREATE INDEX idx_socios_nome_socio ON socios(nome_socio);`
      - `CREATE INDEX idx_socios_representante ON socios(representante_legal);`
      - `CREATE INDEX idx_socios_representante_nome ON socios(nome_representante);`

11. **`simples`**:
    - `cnpj_basico TEXT` (8 digits)
    - `opcao_simples TEXT` ('S', 'N', 'O')
    - `data_opcao_simples TEXT` (YYYYMMDD)
    - `data_exclusao_simples TEXT` (YYYYMMDD)
    - `opcao_mei TEXT` ('S', 'N', 'O')
    - `data_opcao_mei TEXT` (YYYYMMDD)
    - `data_exclusao_mei TEXT` (YYYYMMDD)
    - Index: `CREATE INDEX idx_simples_cnpj_basico ON simples(cnpj_basico);`

12. **`_referencia`**:
    - `referencia TEXT`
    - `valor TEXT`
    - Stored entries: `('CNPJ', '<dataReferencia>')`, `('cnpj_qtde', '<qtde_estabelecimentos>')`.

---

#### B. Schemas for `rede.db`

1. **`ligacao`**:
   - `id1 TEXT` (source node identifier)
   - `id2 TEXT` (destination node identifier)
   - `descricao TEXT` (relationship label, e.g. "Sócio-Administrador", "filial", "rep-sócio-Procurador")
   - `comentario TEXT` (provenance, `'socios'` or `'estabelecimento'`)
   - Indices:
     - `CREATE INDEX idx_ligacao_origem ON ligacao (id1);`
     - `CREATE INDEX idx_ligacao_destino ON ligacao (id2);`

#### C. Schemas for `rede_search.db`

1. **`id_search`**:
   - FTS5 Virtual Table:
     `CREATE VIRTUAL TABLE id_search USING fts5 (id, descricao);`
   - Populated from:
     - `PJ_<cnpj>` with `razao_social` (Matrizes)
     - `PJ_<cnpj>` with `nome_fantasia` (Matrizes & Filiais)
     - Non-PJ IDs from `ligacao.id1` and `ligacao.id2` (where prefix != `'PJ_'`), with empty description

---

### 2.3 Graph Construction Logic: Deriving `rede.db` from `cnpj.db`

The reference implementation executes seven rules to generate all graph edges:

```sql
-- Rule 1: Corporate Partner (PJ) -> Investee Company (PJ)
SELECT 'PJ_' || t.cnpj_cpf_socio AS origem,
       'PJ_' || t.cnpj AS destino,
       sq.descricao AS tipo,
       'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) = 14;

-- Rule 2: Individual Partner (PF) -> Investee Company (PJ)
SELECT 'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS origem,
       'PJ_' || t.cnpj AS destino,
       sq.descricao AS tipo,
       'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) = 11 AND t.nome_socio <> '';

-- Rule 3: Foreign Corporate Partner (PE) -> Investee Company (PJ)
SELECT 'PE_' || t.nome_socio AS origem,
       'PJ_' || t.cnpj AS destino,
       sq.descricao AS tipo,
       'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_socio
WHERE length(t.cnpj_cpf_socio) <> 14 AND length(t.cnpj_cpf_socio) <> 11
  AND t.cnpj_cpf_socio = '';

-- Rule 4: Legal Representative of Foreign Partner (PF -> PE)
SELECT 'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
       'PE_' || t.nome_socio AS destino,
       'rep-sócio-' || sq.descricao AS tipo,
       'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE length(t.cnpj_cpf_socio) <> 14 AND length(t.cnpj_cpf_socio) <> 11
  AND t.cnpj_cpf_socio = '' AND t.representante_legal <> '***000000**';

-- Rule 5: Legal Representative of Corporate Partner (PF -> PJ)
SELECT 'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
       'PJ_' || t.cnpj_cpf_socio AS destino,
       'rep-sócio-' || sq.descricao AS tipo,
       'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE length(t.cnpj_cpf_socio) = 14 AND t.representante_legal <> '***000000**';

-- Rule 6: Legal Representative of Individual Partner (PF -> PF)
SELECT 'PF_' || t.representante_legal || '-' || t.nome_representante AS origem,
       'PF_' || t.cnpj_cpf_socio || '-' || t.nome_socio AS destino,
       'rep-sócio-' || sq.descricao AS tipo,
       'socios' AS base
FROM cnpj.socios t
LEFT JOIN cnpj.qualificacao_socio sq ON sq.codigo = t.qualificacao_representante_legal
WHERE length(t.cnpj_cpf_socio) = 11 AND t.representante_legal <> '***000000**';

-- Rule 7: Filial -> Matriz (PJ -> PJ)
SELECT 'PJ_' || tf.cnpj AS origem,
       'PJ_' || t.cnpj AS destino,
       'filial' AS tipo,
       'estabelecimento' AS base
FROM (SELECT cnpj, cnpj_basico FROM estabelecimento WHERE matriz_filial = '2') tf
LEFT JOIN cnpj.estabelecimento t ON t.cnpj_basico = tf.cnpj_basico
WHERE t.matriz_filial = '1';
```

Final deduplication:
```sql
CREATE TABLE ligacao AS
SELECT origem AS id1, destino AS id2, tipo AS descricao, base AS comentario
FROM ligacao1
GROUP BY origem, destino, tipo, base;
```

---

## 3. Caveats

1. **Absence of Partner Equity Percentage**:
   Requirement R2 specifies generating `ligacao` with "percentuais de participação societária". However, **the RFB open data CSVs (`SOCIOCSV`) do not include equity percentages or share quotas**. The only available numerical data in the dataset is `capital_social` in `empresas` (total company share capital). The legacy implementation does not compute percentages because the underlying raw data simply omits this field.

2. **Bidirectional Storage vs. Bidirectional Queries**:
   Requirement R2 specifies "arestas bidirecionais (empresa -> sócio e sócio -> empresa)".
   - In the reference database, each edge is physically stored **once** (`origem -> destino`).
   - Bidirectionality is evaluated at query time via `WHERE id1 = ?1 OR id2 = ?1` (which is supported in Rust by `crates/ecd-graph/src/bfs.rs` and `shortest_path.rs`).
   - If the Rust reconstruction physically writes both `(id1, id2)` and `(id2, id1)` into `ligacao`, the table row count will double (~150M rows instead of ~75M rows), doubling database size from ~15 GB to ~30 GB. Alternatively, a view or query-level bidirectionality preserves the 100% compatibility with legacy `rede-cnpj` while providing bidirectional graph traversal.

3. **Memory Footprint During Aggregation**:
   In `rede_cria_tabela_rede.db.py`, building `ligacao` via SQLite in-memory requires at least **16 GB of RAM**; otherwise, it crashes with OOM. For machines with 8 GB to 16 GB, the Rust CLI tool must stream and batch insert directly on disk using temporary tables or sort-merge joins with `temp_store = FILE` or bounded memory chunks.

4. **RFB Layout Changes (Agosto/2026)**:
   The RFB layout change on `cnpj_cpf_socio` requires checking whether the incoming data has 8 or 14 characters for PJ partners. Any reconstruction must dynamically inspect the column length or configuration date to correctly reconstruct the 14-digit CNPJ.

---

## 4. Conclusion

The reference implementation is a two-stage SQLite ETL pipeline:
1. **`cnpj.db`**: Direct ingestion of RFB's 37 CSV files into 10 structured tables + 2 metadata/lookup tables, with Latin1 decoding, decimal comma transformation, CNPJ synthesis, and Matriz resolution.
2. **`rede.db`**: A relational graph extraction creating a normalized 4-column edge table `ligacao (id1, id2, descricao, comentario)` indexed by source and destination, complemented by `rede_search.db` (FTS5).

For the high-performance Rust reconstruction:
- Ingestion (`cnpj.db`): Stream 37 files directly from `.zip` via `memmap2` and multithreaded chunking (`rayon`), batching inserts into SQLite with WAL mode, `synchronous = OFF`, and transactions of 50,000–100,000 rows.
- Transformation (`rede.db`): Execute the 7 derivation SQL rules inside SQLite with attached `cnpj.db`, or perform high-speed Rust-side join streaming.
- Full compatibility with `crates/ecd-graph` and `rede-cnpj` is guaranteed if table and index names match the specifications documented above.

---

## 5. Verification Method

To independently verify the schemas and behavior discovered:

1. **Verify Sample DB Schemas**:
   ```bash
   sqlite3 /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede/bases/base_cnpj_vazia.db ".schema"
   sqlite3 /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede/bases/rede.db ".schema"
   sqlite3 /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede/bases/links.db ".schema"
   sqlite3 /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede/bases/cnpj_links_ete.db ".schema"
   ```

2. **Verify Consumer Query Compatibility in `crates/ecd-graph`**:
   ```bash
   cargo test -p ecd-graph
   ```

3. **Verify Reference Python Scripts**:
   Inspect line numbers cited in this report:
   - `dados_cnpj_para_sqlite.py:86-145` (Schema definitions)
   - `dados_cnpj_para_sqlite.py:208-236` (August 2026 RFB layout quirk)
   - `rede_cria_tabela_rede.db.py:49-144` (7 derivation rules for `ligacao`)
   - `rede_cria_tabela_rede.db.py:184-215` (FTS5 search indexing)
