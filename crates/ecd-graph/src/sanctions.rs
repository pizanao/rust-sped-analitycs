//! Cruzamento atômico de entidades societárias (PJ/PF) contra a base `links.db`
//! de sanções e restrições públicas (PEP, CEIS, CEPIM, CNEP, Dívida PGFN).

use crate::{db::SqliteGraphStore, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

/// Categorias oficiais de sanções e alertas monitorados no módulo GRC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SanctionType {
    /// Pessoas Politicamente Expostas
    Pep,
    /// Cadastro de Empresas Inidôneas e Suspensas
    Ceis,
    /// Cadastro de Entidades Privadas Sem Fins Lucrativos Impedidas
    Cepim,
    /// Cadastro Nacional de Empresas Punidas
    Cnep,
    /// Dívidas Ativas Tributárias e Previdenciárias (PGFN)
    Pgfn,
}

impl SanctionType {
    /// Código padronizado do badge sancionatório.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pep => "PEP",
            Self::Ceis => "CEIS",
            Self::Cepim => "CEPIM",
            Self::Cnep => "CNEP",
            Self::Pgfn => "PGFN",
        }
    }

    /// Identifica todas as categorias de sanção presentes nos campos de um registro de `links`.
    pub fn detect_all(id_aux: &str, descricao: &str, comentario: &str) -> Vec<Self> {
        let combined = format!(
            "{} {} {}",
            id_aux.to_uppercase(),
            descricao.to_uppercase(),
            comentario.to_uppercase()
        );
        let mut found = Vec::new();

        if combined.contains("PEP") || combined.contains("POLITICAMENTE EXPOST") {
            found.push(Self::Pep);
        }
        if combined.contains("CEIS") || combined.contains("INIDONE") || combined.contains("INIDÔNE")
        {
            found.push(Self::Ceis);
        }
        if combined.contains("CEPIM") || combined.contains("IMPEDID") {
            found.push(Self::Cepim);
        }
        if combined.contains("CNEP")
            || combined.contains("PUNID")
            || combined.contains("LENIENCIA")
            || combined.contains("LENIÊNCIA")
        {
            found.push(Self::Cnep);
        }
        if combined.contains("PGFN")
            || combined.contains("DIVIDA")
            || combined.contains("DÍVIDA")
            || combined.contains("DAU")
        {
            found.push(Self::Pgfn);
        }

        found
    }
}

/// Registro detalhado de vínculo sancionatório apurado na base `links.db`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanctionRecord {
    pub entidade_id: String,
    pub tipo_sancao: SanctionType,
    pub codigo_sancao: String,
    pub descricao: String,
    pub comentario: String,
}

/// Gera chaves candidatas (com e sem prefixo `PJ_`/`PF_`/`FILIAL_` e com/sem máscara de CNPJ/CPF)
/// para busca flexível nas tabelas `entidades`, `ligacao` e `links`.
pub fn chaves_candidatas_entidade(entidade_id: &str) -> Vec<String> {
    let trimmed = entidade_id.trim();
    let mut keys = Vec::new();
    if trimmed.is_empty() {
        return keys;
    }

    let push_unique = |vec: &mut Vec<String>, val: String| {
        if !val.is_empty() && !vec.contains(&val) {
            vec.push(val);
        }
    };

    push_unique(&mut keys, trimmed.to_string());

    let mut base = trimmed;
    for prefix in ["PJ_", "PF_", "FILIAL_", "PE_"] {
        if let Some(stripped) = trimmed.strip_prefix(prefix) {
            push_unique(&mut keys, stripped.to_string());
            base = stripped;
            break;
        }
    }

    let only_digits_and_mask = base
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '/' | '-'));
    if only_digits_and_mask {
        let digits: String = base.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() == 14 {
            push_unique(&mut keys, digits.clone());
            push_unique(&mut keys, format!("PJ_{digits}"));
            if &digits[8..12] != "0001" {
                push_unique(&mut keys, format!("FILIAL_{digits}"));
            }
        } else if digits.len() == 11 {
            push_unique(&mut keys, digits.clone());
            push_unique(&mut keys, format!("PF_{digits}"));
        }
    }

    keys
}

fn tabela_links_main_existe(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='links')",
        [],
        |row| row.get(0),
    )
    .unwrap_or(false)
}

fn tabela_links_attached_existe(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM links_db.sqlite_master WHERE type='table' AND name='links')",
        [],
        |row| row.get(0),
    )
    .unwrap_or(false)
}

/// Verifica se a tabela `links` está disponível no banco principal ou em banco anexado (`links_db`).
fn tabela_links_existe(conn: &Connection) -> bool {
    tabela_links_main_existe(conn) || tabela_links_attached_existe(conn)
}

impl SqliteGraphStore {
    /// Consulta todas as sanções detalhadas vinculadas a uma única entidade (`PJ` ou `PF`).
    pub fn consultar_sancoes(&self, entidade_id: &str) -> Result<Vec<SanctionRecord>> {
        let conn = self.get_conn()?;
        if !tabela_links_existe(&conn) {
            return Ok(Vec::new());
        }

        let tx = conn.unchecked_transaction()?;
        let records = consultar_sancoes_conn(&tx, entidade_id)?;
        tx.commit()?;
        Ok(records)
    }

    /// Realiza o cruzamento atômico em lote de uma lista de IDs de entidades contra a base `links`,
    /// retornando o mapa `entidade_id -> Vec<String>` com as siglas sancionatórias (`PEP`, `CEIS`, `CEPIM`, `CNEP`, `PGFN`).
    pub fn cruzar_sancoes_lote(
        &self,
        entidade_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>> {
        let conn = self.get_conn()?;
        cruzar_sancoes_em_conexao(&conn, entidade_ids)
    }
}

/// Executa o cruzamento atômico usando uma transação consistente sobre a conexão SQLite fornecida.
pub(crate) fn cruzar_sancoes_em_conexao(
    conn: &Connection,
    entidade_ids: &[String],
) -> Result<HashMap<String, Vec<String>>> {
    let mut resultado: HashMap<String, Vec<String>> = HashMap::with_capacity(entidade_ids.len());
    if entidade_ids.is_empty() || !tabela_links_existe(conn) {
        return Ok(resultado);
    }

    let tx = conn.unchecked_transaction()?;
    for entidade_id in entidade_ids {
        let records = consultar_sancoes_conn(&tx, entidade_id)?;
        if !records.is_empty() {
            let mut flags = BTreeSet::new();
            for rec in records {
                flags.insert(rec.tipo_sancao.as_str().to_string());
            }
            resultado.insert(entidade_id.clone(), flags.into_iter().collect());
        }
    }
    tx.commit()?;

    Ok(resultado)
}

fn consultar_sancoes_conn(conn: &Connection, entidade_id: &str) -> Result<Vec<SanctionRecord>> {
    let candidates = chaves_candidatas_entidade(entidade_id);
    let mut queries = Vec::with_capacity(2);

    if tabela_links_main_existe(conn) {
        queries.push(
            "SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
             FROM main.links
             WHERE id1 = ?1 OR id2 = ?1",
        );
    }
    if tabela_links_attached_existe(conn) {
        queries.push(
            "SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
             FROM links_db.links
             WHERE id1 = ?1 OR id2 = ?1",
        );
    }

    let mut records = Vec::new();
    for sql in queries {
        let mut stmt = conn.prepare_cached(sql)?;
        for key in &candidates {
            let rows = stmt.query_map([key], |row| {
                let id1: String = row.get(0)?;
                let id2: String = row.get(1)?;
                let descricao: String = row.get(2)?;
                let comentario: String = row.get(3)?;
                Ok((id1, id2, descricao, comentario))
            })?;

            for row in rows {
                let (id1, id2, descricao, comentario) = row?;
                let contraparte = if id1 == *key { &id2 } else { &id1 };
                let tipos = SanctionType::detect_all(contraparte, &descricao, &comentario);
                for tipo in tipos {
                    let rec = SanctionRecord {
                        entidade_id: entidade_id.to_string(),
                        tipo_sancao: tipo,
                        codigo_sancao: contraparte.clone(),
                        descricao: descricao.clone(),
                        comentario: comentario.clone(),
                    };
                    if !records.contains(&rec) {
                        records.push(rec);
                    }
                }
            }
        }
    }

    Ok(records)
}
