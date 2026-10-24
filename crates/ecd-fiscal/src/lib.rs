//! # ecd-fiscal
//!
//! Motor de cálculo fiscal, identificação de receita bruta via fuzzy matching
//! e exportação de planilhas contábeis multi-aba formatadas.

use rust_decimal::Decimal;
use strsim::normalized_damerau_levenshtein;

pub struct ReceitaApurada {
    pub receita_bruta: Decimal,
    pub receita_servico: Decimal,
    pub outras_receitas: Decimal,
    pub linhas_identificadas: Vec<LinhaReceita>,
}

pub struct LinhaReceita {
    pub cod_agl: String,
    pub descricao: String,
    pub valor: Decimal,
    pub similaridade: f64,
    pub termo_correspondente: String,
}

pub struct ReceitaExtractor {
    termos_padrao: Vec<String>,
}

impl Default for ReceitaExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl ReceitaExtractor {
    pub fn new() -> Self {
        Self {
            termos_padrao: vec![
                "receita bruta venda".to_string(),
                "vendas e servicos".to_string(),
                "mensalidades contribuicoes".to_string(),
                "receita bruta da prestacao de servicos".to_string(),
                "receita bruta de gestao patrimonial".to_string(),
                "receita bruta de locacao".to_string(),
                "receita bruta de servicos".to_string(),
                "receita bruta de venda e prestacao de servicos".to_string(),
                "receita bruta operacional".to_string(),
                "receitas operacional bruta".to_string(),
                "receita total bruta".to_string(),
                "venda bruta operacional".to_string(),
                "receita bruta".to_string(),
                "ingressos".to_string(),
            ],
        }
    }

    pub fn normalizar(texto: &str) -> String {
        texto
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn avaliar_linha(&self, descricao: &str) -> Option<(f64, String)> {
        let limpa = Self::normalizar(descricao);
        for termo in &self.termos_padrao {
            let score = normalized_damerau_levenshtein(&limpa, termo);
            if score >= 0.80 {
                return Some((score, termo.clone()));
            }
        }
        None
    }
}
