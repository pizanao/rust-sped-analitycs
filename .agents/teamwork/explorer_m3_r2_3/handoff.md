# Milestone 3 (Iteration 2) Handoff Report: FTS5 Input Sanitization & Test Alignment Architecture

- **Agent**: `explorer_m3_r2_3`
- **Role**: FTS5 Input Sanitization & Test Alignment Architect
- **Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`
- **Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3`
- **Handoff Type**: Hard Handoff
- **Date**: 2026-10-25T01:25:00Z
- **Verdict**: Analysis Complete — Exact Fix Strategy Formulated & Validated

---

## Executive Summary

In Milestone 3 (Iteration 1), `src/graph/search.rs` introduced the FTS5 virtual table `id_search` and query helpers `search_by_term` and `search_by_cnpj_basico`. However, both functions passed raw user inputs directly into SQLite FTS5 `MATCH ?1` clauses without sanitization. Consequently, standard Brazilian tax queries—such as formatted CNPJs (`11.223.344/0001-91` or `11.223.344`), hyphenated trade names (`ALFA-TECH`), abbreviations (`S.A.`), and boolean operators (`AND`, `OR`)—trigger fatal SQLite syntax errors (`fts5: syntax error near "."`, `no such column: TECH`).

We have performed a complete analysis of FTS5 query mechanics under `unicode61 remove_diacritics 2`, designed and validated a robust sanitization helper (`sanitize_fts5_term`), and verified test suite column alignments across `tests/adversarial_memory_bounds_test.rs` and `tests/adversarial_graph_traversal_test.rs`.

---

## 1. Observation

### 1.1 FTS5 Query Defect in `src/graph/search.rs` (lines 121–150)
In `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/search.rs:121-150`:
```rust
pub fn search_by_term(conn: &Connection, term: &str, limit: usize) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, descricao FROM id_search WHERE id_search MATCH ?1 LIMIT ?2;",
    )?;
    let rows = stmt.query_map(params![term, limit as i64], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    ...
}

pub fn search_by_cnpj_basico(conn: &Connection, cnpj_basico: &str, limit: usize) -> rusqlite::Result<Vec<String>> {
    let match_pattern = format!("PJ_{}*", cnpj_basico.trim());
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT id FROM id_search WHERE id MATCH ?1 LIMIT ?2;",
    )?;
    ...
}
```

### 1.2 Empirical Failure Reproduction Across Standard User Inputs
Executing SQLite FTS5 queries against the `id_search` table with raw inputs:
```bash
python3 -c "
import sqlite3
c = sqlite3.connect(':memory:')
c.execute(\"CREATE VIRTUAL TABLE id_search USING fts5(id, descricao, tokenize = 'unicode61 remove_diacritics 2');\")
c.execute(\"INSERT INTO id_search VALUES ('PJ_11223344000191', 'ALFA TECNOLOGIA LTDA');\")
c.execute(\"INSERT INTO id_search VALUES ('PJ_11223344000191', 'ALFA-TECH');\")
c.execute(\"INSERT INTO id_search VALUES ('PF_***123456**-JOAO DA SILVA', '');\")

queries = ['11.223.344/0001-91', '11.223.344', 'ALFA-TECH', '\"unclosed', '***123456**', 'AND', 'NOT', 'OR', 'S.A.', 'PJ_11111111**']
for q in queries:
    try:
        c.execute('SELECT id, descricao FROM id_search WHERE id_search MATCH ?', (q,)).fetchall()
    except Exception as e:
        print(f'{q:22} -> ERROR: {e}')
"
```
Verbatim tool output:
```text
11.223.344/0001-91     -> ERROR: fts5: syntax error near "."
11.223.344             -> ERROR: fts5: syntax error near "."
ALFA-TECH              -> ERROR: no such column: TECH
"unclosed              -> ERROR: unterminated string
***123456**            -> ERROR: unknown special query: **123456**
AND                    -> ERROR: fts5: syntax error near "AND"
NOT                    -> ERROR: fts5: syntax error near "NOT"
OR                     -> ERROR: fts5: syntax error near "OR"
S.A.                   -> ERROR: fts5: syntax error near "."
PJ_11111111**          -> ERROR: fts5: syntax error near "*"
```

### 1.3 FTS5 Token Storage in `id_search`
Inspecting the indexed tokens via `fts5vocab` on `id_search` containing `'PJ_11223344000191'` and `'ALFA TECNOLOGIA LTDA'`:
```text
('11223344000191', 2, 2)
('alfa', 2, 2)
('pj', 2, 2)
('tecnologia', 1, 1)
```
- In `id_search`, CNPJs are stored in `id` as uninterrupted 14-digit strings: `PJ_11223344000191`.
- The `unicode61` tokenizer splits `PJ_11223344000191` into two tokens: `pj` and `11223344000191`.
- If an input like `11.223.344/0001-91` simply has punctuation replaced with spaces (`11 223 344 0001 91`), FTS5 searches for 5 separate token matches (`11`, `223`, `344`, `0001`, `91`), which returns **zero results** because the stored token is `11223344000191`.

### 1.4 Test Suite Column Names in `tests/adversarial_memory_bounds_test.rs`
Inspecting `tests/adversarial_memory_bounds_test.rs`:
- Line 344:
  ```rust
  INSERT INTO socios (cnpj, identificador_de_socio, nome_socio, cnpj_cpf_socio, qualificacao_socio) VALUES ('11111111000191', '2', 'MARCOS ALVES', '***123456**', '49');
  ```
- Line 458:
  ```rust
  let mut stmt_soc = conn
      .prepare("INSERT INTO socios (cnpj, identificador_de_socio, nome_socio, cnpj_cpf_socio, qualificacao_socio) VALUES (?1, ?2, ?3, ?4, ?5);")
  ```
- In `src/ingestion/schema.rs:107`:
  ```sql
  CREATE TABLE IF NOT EXISTS socios (
      cnpj TEXT,
      cnpj_basico TEXT,
      identificador_de_socio TEXT,
      ...
  ```
The column name `identificador_de_socio` is already 100% compliant with `SOCIOS_FINAL_DDL`. No instances of `identificador_socio` exist in the codebase.

### 1.5 Downstream Test Suite Assertion in `tests/adversarial_graph_traversal_test.rs`
In `tests/adversarial_graph_traversal_test.rs:418-422`:
```rust
    // What happens if user provides raw unescaped boolean operators or unclosed quotes?
    let r_syntax = search_by_term(&conn, "MODAS OR", 10);
    println!("Dangling OR search result: {:?}", r_syntax);
    assert!(r_syntax.is_err(), "Raw FTS5 syntax errors (e.g. trailing 'OR') are returned as errors by rusqlite");
```
In Milestone 3 (Iteration 1), `challenger_m3_1` asserted that raw unescaped boolean operators returned `Err`. Once `search_by_term` is sanitized, `"MODAS OR"` is safely queried without a syntax error and returns `Ok(vec![])`. If this test assertion is not aligned, `cargo test --test adversarial_graph_traversal_test` will fail on `assert!(r_syntax.is_err())`.

---

## 2. Logic Chain

1. **Why FTS5 `MATCH` Fails on Raw Input**:
   - In SQLite FTS5, characters such as `.`, `/`, `-`, `:`, `*`, and `"` are reserved query syntax operators.
   - For example, `11.223.344` is parsed as column `11` dot column `223` dot token `344`, which triggers `fts5: syntax error near "."`.
   - `ALFA-TECH` is parsed as token `ALFA` followed by column minus `TECH`, triggering `no such column: TECH`.
   - Bare boolean operators (`AND`, `OR`, `NOT`, `NEAR`) without valid surrounding expressions trigger unary/binary operator syntax errors.
   - An empty query string `""` passed to `MATCH ""` triggers `fts5: syntax error near ""`.

2. **Why Quoting Tokens Neutralizes Operator Syntax**:
   - In SQLite FTS5, any token enclosed in double quotes (e.g. `"ALFA-TECH"`, `"S.A."`, `"AND"`, `"OR"`) is treated strictly as a string literal.
   - FTS5 passes the contents of double quotes directly to the tokenizer (`unicode61 remove_diacritics 2`), which splits punctuation according to Unicode word boundaries while neutralizing all query-level boolean/column operators.
   - In FTS5, wildcard prefix search is expressed by placing the asterisk *outside* the quotes: `"TECNOL"*`.

3. **Why CNPJs Require Dedicated Digit Extraction**:
   - In Brazilian open data, establishment CNPJs are 14 digits (e.g. `11223344000191`).
   - If a user passes a formatted CNPJ `11.223.344/0001-91`, the dots and slashes represent formatting, not word boundaries.
   - Extracting digits produces `"11223344000191"`. When queried as `\"11223344000191\"`, FTS5 matches the `id` token `11223344000191` with 100% precision.
   - If a user passes an 8-digit radical `11.223.344` (or `11223344`), querying `\"11223344\"*` performs an index prefix scan matching the company's headquarters and all its branches (`PJ_11223344000191`, `PJ_11223344000272`, etc.).

4. **Why `search_by_cnpj_basico` Must Filter Digits**:
   - `search_by_cnpj_basico` constructs `format!("PJ_{}*", cnpj_basico.trim())`.
   - If `cnpj_basico` contains punctuation (e.g. `11.223.344`), `PJ_11.223.344*` crashes FTS5 on `.`.
   - Filtering digits to the 8-digit radical (`&digits[..8]`) guarantees the pattern is always valid alphanumeric `PJ_{radical}*`.
   - If no digits are present (e.g. empty string or punctuation), returning `Ok(Vec::new())` immediately avoids querying `PJ_*` (which would scan the entire database).

5. **Why `tests/adversarial_graph_traversal_test.rs` Must Be Aligned**:
   - `challenger_m3_1` documented the Iteration 1 defect by asserting `assert!(r_syntax.is_err())`.
   - Once input sanitization is active, trailing boolean operators like `"MODAS OR"` are sanitized into `"\"MODAS\" \"OR\""`.
   - The query executes cleanly and returns `Ok(vec![])`.
   - Updating the test to `assert!(r_syntax.is_ok()); assert_eq!(r_syntax.unwrap().len(), 0);` aligns the test suite with the intended sanitized behavior.

---

## 3. Caveats

1. **Exact Quoted Search**: If a user explicitly wraps an entire phrase in double quotes (e.g. `"PJ_11223344000191"`), the sanitizer preserves the phrase search while escaping any inner quotes (`replace('"', "\"\"")`).
2. **Masked CPFs**: In Brazilian open data, partner CPFs are masked as `***123456**`. The sanitizer strips leading asterisks and preserves trailing wildcards, converting `***123456**` into `"123456"*`, which matches `PF_***123456**-JOAO DA SILVA`.
3. **No Project Source Modification**: Per explorer agent rules, no production source files in `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/` were modified directly. All proposed changes are provided as diff patches and reference files in this directory.

---

## 4. Conclusion & Proposed Code Fixes

### 4.1 Exact Code Implementation for `src/graph/search.rs`

Replace lines 121–150 of `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/src/graph/search.rs` with the following implementation:

```rust
/// Sanitizes an input search term for safe execution against an SQLite FTS5 MATCH expression.
///
/// Prevents SQLite FTS5 syntax errors from:
/// - Formatted CNPJs (`11.223.344/0001-91` -> `"11223344000191"`, `11.223.344` -> `"11223344"*`)
/// - Hyphenated trade names (`ALFA-TECH` -> `"ALFA-TECH"`)
/// - Punctuation / abbreviations (`C&A` -> `"C&A"`, `S.A.` -> `"S.A."`, `D'AVILA` -> `"D'AVILA"`)
/// - Prefix wildcards (`TECNOL*` -> `"TECNOL"*`)
/// - Reserved operators (`AND`, `OR`, `NOT`, `NEAR` -> escaped safely inside double quotes)
/// - Unbalanced quotes, colons (`id:col`), and asterisks
pub fn sanitize_fts5_term(term: &str) -> String {
    let trimmed = term.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // 1. Exact quoted string: if user explicitly provided a fully double-quoted string
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 1 {
        let inner = trimmed[1..trimmed.len() - 1].replace('"', "\"\"");
        let inner_trimmed = inner.trim();
        if !inner_trimmed.is_empty() {
            return format!("\"{inner_trimmed}\"");
        }
        return String::new();
    }

    // 2. Formatted CNPJ: check if input is formatted CNPJ (digits with ., /, -, spaces)
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    let is_cnpj_like = !digits.is_empty()
        && trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '/' || c == '-' || c.is_whitespace());

    if is_cnpj_like && trimmed.contains(['.', '/']) {
        if digits.len() >= 14 {
            return format!("\"{}\"", &digits[..14]);
        } else if digits.len() >= 8 {
            return format!("\"{}\"*", &digits[..8]);
        } else {
            return format!("\"{digits}\"*");
        }
    }

    // 3. Tokenize general words separated by whitespace
    let mut tokens = Vec::new();
    for word in trimmed.split_whitespace() {
        // Strip leading asterisks to avoid invalid FTS5 syntax
        let without_leading_star = word.trim_start_matches('*');
        let has_wildcard = without_leading_star.ends_with('*');
        let clean_word = without_leading_star.trim_end_matches('*').replace('"', "\"\"");

        // Ignore tokens that contain no alphanumeric characters (e.g. lone punctuation)
        if clean_word.is_empty() || !clean_word.chars().any(|c| c.is_alphanumeric()) {
            continue;
        }

        if has_wildcard {
            tokens.push(format!("\"{clean_word}\"*"));
        } else {
            tokens.push(format!("\"{clean_word}\""));
        }
    }

    tokens.join(" ")
}

/// Query helper to search by full-text token matching both id and descricao.
pub fn search_by_term(conn: &Connection, term: &str, limit: usize) -> rusqlite::Result<Vec<(String, String)>> {
    let sanitized = sanitize_fts5_term(term);
    if sanitized.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare_cached(
        "SELECT id, descricao FROM id_search WHERE id_search MATCH ?1 LIMIT ?2;",
    )?;
    let rows = stmt.query_map(params![sanitized, limit as i64], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Query helper to search by CNPJ radical prefix (e.g. '11223344' -> matches 'PJ_11223344*').
pub fn search_by_cnpj_basico(conn: &Connection, cnpj_basico: &str, limit: usize) -> rusqlite::Result<Vec<String>> {
    let digits: String = cnpj_basico.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Ok(Vec::new());
    }
    let radical = if digits.len() >= 8 {
        &digits[..8]
    } else {
        &digits[..]
    };
    let match_pattern = format!("PJ_{radical}*");
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT id FROM id_search WHERE id MATCH ?1 LIMIT ?2;",
    )?;
    let rows = stmt.query_map(params![match_pattern, limit as i64], |r| r.get::<_, String>(0))?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}
```

### 4.2 Proposed Unit Tests for `src/graph/search.rs`
Add to `src/graph/search.rs` in `mod tests`:
```rust
    #[test]
    fn test_fts5_sanitization_formatted_cnpj_and_hyphens() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(FTS5_SEARCH_TABLE_DDL).unwrap();

        conn.execute_batch(
            "INSERT INTO id_search VALUES ('PJ_11223344000191', 'ALFA-TECH LTDA');
             INSERT INTO id_search VALUES ('PJ_11223344000272', 'ALFA FILIAL');
             INSERT INTO id_search VALUES ('PJ_001', 'C&A MODAS S.A.');
             INSERT INTO id_search VALUES ('PF_001', 'PF_***123456**-D''AVILA SILVA');",
        )
        .unwrap();

        // 1. Formatted CNPJ
        let r1 = search_by_term(&conn, "11.223.344/0001-91", 10).unwrap();
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].0, "PJ_11223344000191");

        // 2. Formatted CNPJ radical
        let r2 = search_by_term(&conn, "11.223.344", 10).unwrap();
        assert_eq!(r2.len(), 2);

        // 3. search_by_cnpj_basico with formatted input
        let r3 = search_by_cnpj_basico(&conn, "11.223.344/0001-91", 10).unwrap();
        assert_eq!(r3.len(), 2);

        // 4. Hyphenated trade name
        let r4 = search_by_term(&conn, "ALFA-TECH", 10).unwrap();
        assert_eq!(r4.len(), 1);

        // 5. Special punctuation: &, ., '
        let r5 = search_by_term(&conn, "C&A", 10).unwrap();
        assert_eq!(r5.len(), 1);

        let r6 = search_by_term(&conn, "S.A.", 10).unwrap();
        assert_eq!(r6.len(), 1);

        let r7 = search_by_term(&conn, "D'AVILA", 10).unwrap();
        assert_eq!(r7.len(), 1);

        // 6. Trailing boolean operator and malformed input
        let r8 = search_by_term(&conn, "MODAS OR", 10).unwrap();
        assert_eq!(r8.len(), 0);

        let r9 = search_by_term(&conn, "", 10).unwrap();
        assert_eq!(r9.len(), 0);

        let r10 = search_by_cnpj_basico(&conn, "", 10).unwrap();
        assert_eq!(r10.len(), 0);
    }
```

### 4.3 Proposed Test Alignment in `tests/adversarial_graph_traversal_test.rs`
Update lines 418–422 of `tests/adversarial_graph_traversal_test.rs`:
```rust
    // Resilient search with trailing boolean operator (sanitized into quoted tokens, no syntax error)
    let r_syntax = search_by_term(&conn, "MODAS OR", 10);
    assert!(r_syntax.is_ok(), "Sanitized search safely handles trailing boolean operators without SQLite syntax error");
    assert_eq!(r_syntax.unwrap().len(), 0, "Non-matching tokens must return empty result");
```

### 4.4 Machine-Applicable Artifacts Created
- Patch file for `src/graph/search.rs`: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/search_sanitization.patch`
- Patch file for `tests/adversarial_graph_traversal_test.rs`: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/tests_alignment.patch`
- Full proposed `search.rs` file: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/proposed_search.rs`

---

## 5. Verification Method

To independently verify these fixes:

1. **Apply the patch or inspect the proposed search.rs file**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   patch -p1 < /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/search_sanitization.patch
   patch -p1 < /home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m3_r2_3/tests_alignment.patch
   ```

2. **Run FTS5 Unit Tests**:
   ```bash
   cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --lib graph::search::tests
   ```
   Must pass all tests:
   - `test_fts5_virtual_table_and_queries`
   - `test_fts5_sanitization_formatted_cnpj_and_hyphens`
   - `test_fts5_portuguese_diacritics_and_special_chars`
   - `test_build_search_database_end_to_end`

3. **Run Adversarial Challenge Suites**:
   ```bash
   cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --test adversarial_memory_bounds_test
   cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --test adversarial_graph_traversal_test
   ```

4. **Verify Linter Cleanliness**:
   ```bash
   cargo clippy --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml --all-targets -- -D warnings
   ```

### Invalidation Conditions:
- `search_by_term(&conn, "11.223.344/0001-91", 10)` returning `fts5: syntax error near "."` or returning 0 hits when the company exists.
- `search_by_term(&conn, "ALFA-TECH", 10)` returning `no such column: TECH`.
- `search_by_term(&conn, "MODAS OR", 10)` returning `fts5: syntax error near "OR"`.
- `search_by_cnpj_basico(&conn, "11.223.344", 10)` failing with syntax error.
