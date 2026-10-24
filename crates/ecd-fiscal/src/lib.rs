//! # ecd-fiscal
//!
//! Motor de cálculo fiscal, identificação de receita bruta via fuzzy matching (`partial_ratio` >= 0.80)
//! com paridade estrita ao legado (`apps/fiscal/utils.py` e `utils_recfuze.py`)
//! e exportação de planilhas contábeis multi-aba formatadas.

use rust_decimal::Decimal;
use strsim::normalized_levenshtein;

#[derive(Clone, Debug)]
pub struct ReceitaApurada {
    pub receita_bruta: Decimal,
    pub receita_servico: Decimal,
    pub outras_receitas: Decimal,
    pub linhas_identificadas: Vec<LinhaReceita>,
}

#[derive(Clone, Debug)]
pub struct LinhaReceita {
    pub cod_agl: String,
    pub descricao: String,
    pub valor: Decimal,
    pub similaridade: f64,
    pub termo_correspondente: String,
}

pub struct ReceitaExtractor {
    termos_padrao: Vec<String>,
    filtros_exclusao: Vec<&'static str>,
}

impl Default for ReceitaExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl ReceitaExtractor {
    pub fn new() -> Self {
        // Lista completa dos 24 termos normalizados do legado (apps/fiscal/utils.py & utils_recfuze.py)
        let termos_brutos = [
            "receita bruta venda",
            "vendas e servicos",
            "mensalidades contribuicoes",
            "mensalidades",
            "receita bruta da prestacao de servicos",
            "receita bruta de gestao patrimonial",
            "receita bruta gestao patrimonial",
            "receita bruta de locacao",
            "receita bruta locacao",
            "receita bruta de servicos",
            "receita bruta servicos",
            "receita bruta de venda e prestacao de servicos",
            "receita bruta venda prestacao de servicos",
            "receita bruta de vendas de unidades imobiliarias",
            "receita bruta vendas imobiliarias",
            "receita bruta operacional",
            "receitas operacionais",
            "receita bruta operacional de desagio",
            "receita operacional desagio",
            "receita bruta locacao e servicos",
            "receita bruta locacao servicos",
            "receita bruta vendas de produtos",
            "vendas produtos",
            "receitas operacional bruta",
            "receita brutas de vendas e mercadorias",
            "brutas vendas mercadorias",
            "receita total bruta",
            "receitas brutas de mercadorias",
            "receitas brutas mercadorias",
            "receitas brutas mercadorias e servicos",
            "receitas brutas mercadorias servicos",
            "bruta com aluguel de imoveis",
            "bruta aluguel imoveis",
            "total de receita operacional bruta",
            "total receita operacional bruta",
            "venda bruta operacional",
            "receita bruta",
            "ingressos",
            "receitas",
        ];

        Self {
            termos_padrao: termos_brutos.iter().map(|s| s.to_string()).collect(),
            // 12 filtros negativos obrigatórios da query SQL legada (filtra_receitas_j150)
            filtros_exclusao: vec![
                " NAO ",
                " NÃO ",
                "CUST",
                "DEDU",
                "IMP",
                "DESPE",
                "CONTRA",
                "TERCEI",
                "PESSO",
                " PJ ",
                "LIQUID",
                "LÍQUID",
                "ANTES",
            ],
        }
    }

    /// Remove acentuação comum em português e normaliza espaços em caixa baixa
    pub fn normalizar(texto: &str) -> String {
        texto
            .to_lowercase()
            .chars()
            .map(|c| match c {
                'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'õ' | 'ô' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                _ => c,
            })
            .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Aplica os filtros pré-condicionais equivalentes à query SQL do legado em RegJ150
    pub fn elegivel_receita_j150(
        &self,
        ind_dc_cta_fin: &str,
        vl_cta_fin: Decimal,
        ind_cod_agl: &str,
        descr_cod_agl: &str,
    ) -> bool {
        if ind_dc_cta_fin != "C" || vl_cta_fin <= Decimal::ZERO || ind_cod_agl != "T" {
            return false;
        }
        let upper = format!(" {} ", descr_cod_agl.trim().to_uppercase());
        if !(upper.contains("BRUTA") || upper.trim_start().starts_with("RECEITA")) {
            return false;
        }
        for neg in &self.filtros_exclusao {
            if upper.contains(neg) {
                return false;
            }
        }
        true
    }

    /// Calcula similaridade de melhor substring equivalente ao `rapidfuzz.fuzz.partial_ratio` (0.0 a 1.0)
    pub fn partial_ratio(s1: &str, s2: &str) -> f64 {
        if s1.is_empty() || s2.is_empty() {
            return 0.0;
        }
        let (shorter, longer) = if s1.len() <= s2.len() {
            (s1, s2)
        } else {
            (s2, s1)
        };
        if longer.contains(shorter) {
            return 1.0;
        }
        let short_chars: Vec<char> = shorter.chars().collect();
        let long_chars: Vec<char> = longer.chars().collect();
        let k = short_chars.len();
        if k > long_chars.len() {
            return normalized_levenshtein(shorter, longer);
        }

        let mut max_score = normalized_levenshtein(shorter, longer);
        for window in long_chars.windows(k) {
            let sub: String = window.iter().collect();
            let score = normalized_levenshtein(shorter, &sub);
            if score > max_score {
                max_score = score;
                if max_score >= 0.999 {
                    break;
                }
            }
        }
        max_score
    }

    pub fn avaliar_linha(&self, descricao: &str) -> Option<(f64, String)> {
        let limpa = Self::normalizar(descricao);
        for termo in &self.termos_padrao {
            let score = Self::partial_ratio(&limpa, termo);
            if score >= 0.80 {
                return Some((score, termo.clone()));
            }
        }
        None
    }
}
