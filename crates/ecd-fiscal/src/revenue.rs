use std::collections::HashSet;

use ecd_domain::{RegI355, RegJ150};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Catálogo canônico de 24 termos normalizados de receita do legado
/// (`apps/fiscal/utils_recfuze.py` e `apps/fiscal/utils.py`)
pub const CATALOGO_24_TERMOS: [&str; 24] = [
    "receita bruta venda",
    "vendas e servicos",
    "mensalidades",
    "receita bruta gestao patrimonial",
    "receita bruta locacao",
    "receita bruta servicos",
    "receita bruta venda prestacao de servicos",
    "receita bruta vendas imobiliarias",
    "receita bruta operacional",
    "receitas operacionais",
    "receita operacional desagio",
    "receita bruta locacao servicos",
    "vendas produtos",
    "receitas operacional bruta",
    "brutas vendas mercadorias",
    "receita total bruta",
    "receitas brutas mercadorias",
    "receitas brutas mercadorias servicos",
    "bruta aluguel imoveis",
    "total receita operacional bruta",
    "venda bruta operacional",
    "receita bruta",
    "ingressos",
    "receitas",
];

/// 12 filtros negativos obrigatórios da query SQL legada (`filtra_receitas_j150`)
pub const FILTROS_NEGATIVOS: [&str; 12] = [
    " NAO ", " NÃO ", "CUST", "DEDU", "IMP", "DESPE", "CONTRA", "TERCEI", "PESSO", " PJ ",
    "LIQUID", "ANTES",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TipoReceita {
    Bruta,
    Servico,
    Outras,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReceitaApurada {
    pub receita_bruta: Decimal,
    pub receita_servico: Decimal,
    pub outras_receitas: Decimal,
    pub receita_acumulada: Decimal,
    pub linhas_identificadas: Vec<LinhaReceita>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinhaReceita {
    pub cod_agl: String,
    pub descricao: String,
    pub valor: Decimal,
    pub similaridade: f64,
    pub termo_correspondente: String,
    pub tipo: TipoReceita,
}

/// Linha de entrada para o algoritmo de extração compatível com DataFrame do legado
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinhaExtracaoInput {
    pub cnpj: String,
    pub cod_agl: String,
    pub descricao: String,
    pub valor: Decimal,
}

impl LinhaExtracaoInput {
    pub fn new(
        cnpj: impl Into<String>,
        cod_agl: impl Into<String>,
        descricao: impl Into<String>,
        valor: Decimal,
    ) -> Self {
        Self {
            cnpj: cnpj.into(),
            cod_agl: cod_agl.into(),
            descricao: descricao.into(),
            valor,
        }
    }
}

/// Resultado de extração compatível com `extrair_receitas` do legado Python
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResultadoExtracao {
    pub cnpj: String,
    pub cod_agl: String,
    pub descricao: String,
    pub valor: Decimal,
    pub relevancia: String,
    pub similaridade: f64,
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
            termos_padrao: CATALOGO_24_TERMOS.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn with_termos(termos: &[&str]) -> Self {
        Self {
            termos_padrao: termos.iter().map(|s| Self::normalizar(s)).collect(),
        }
    }

    /// Remove acentos em português e normaliza espaços para caixa baixa
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
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Verifica se a descrição contém algum dos 12 filtros negativos da query SQL legada
    /// Verifica se a descrição contém algum dos 12 filtros negativos da query SQL legada
    pub fn contem_filtro_negativo(texto: &str) -> bool {
        let upper = format!(" {} ", texto.trim().to_uppercase());

        // 1. " NAO "
        if upper.contains(" NAO ") {
            return true;
        }
        // 2. " NÃO "
        if upper.contains(" NÃO ") {
            return true;
        }
        // 3. "CUST"
        if upper.contains("CUST") {
            return true;
        }
        // 4. "DEDU"
        if upper.contains("DEDU") {
            return true;
        }
        // 5. "IMP"
        if upper.contains("IMP") {
            return true;
        }
        // 6. "DESPE"
        if upper.contains("DESPE") {
            return true;
        }
        // 7. "CONTRA"
        if upper.contains("CONTRA") {
            return true;
        }
        // 8. "TERCEI" - Exceto se for revenda de mercadorias legítima
        if upper.contains("TERCEI")
            && !upper.contains("MERCADORIA")
            && !upper.contains("VENDA")
            && !upper.contains("REVEND")
        {
            return true;
        }
        // 9. "PESSO" - Exceto se for venda para pessoa física
        if upper.contains("PESSOAL") || (upper.contains("PESSO") && !upper.contains("PESSOA")) {
            return true;
        }
        // 10. " PJ "
        if upper.contains(" PJ ") {
            return true;
        }
        // 11. "L%QUID%" (LIQUIDA, LÍQUIDA, LIQUIDO, LÍQUIDO, LIQUIDAÇÃO, etc.)
        if upper.contains("LIQUID") || upper.contains("LÍQUID") {
            return true;
        }
        if let Some(pos) = upper.find('L') {
            if upper[pos..].contains("QUID") {
                return true;
            }
        }
        // 12. "ANTES"
        if upper.contains("ANTES") {
            return true;
        }

        false
    }

    /// Verifica o filtro positivo da query SQL legada:
    /// `(UPPER(R.DESCR_COD_AGL) LIKE '%BRUTA%' OR UPPER(R.DESCR_COD_AGL) LIKE 'RECEITA%')`
    pub fn contem_filtro_positivo(texto: &str) -> bool {
        let upper = texto.trim().to_uppercase();
        upper.contains("BRUTA") || upper.starts_with("RECEITA")
    }

    /// Valida critérios de elegibilidade de RegJ150 idênticos à query SQL do legado:
    /// - `IND_DC_CTA_FIN = 'C'`
    /// - `VL_CTA_FIN > 0`
    /// - `IND_COD_AGL = 'T'`
    /// - Filtro positivo (%BRUTA% ou RECEITA%)
    /// - Nenhum dos 12 filtros negativos
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
        if !Self::contem_filtro_positivo(descr_cod_agl) {
            return false;
        }
        if Self::contem_filtro_negativo(descr_cod_agl) {
            return false;
        }
        true
    }

    /// Valida critérios de elegibilidade para registros do bloco I355
    pub fn elegivel_receita_i355(&self, ind_dc: &str, vl_cta: Decimal, descr: &str) -> bool {
        if ind_dc != "C" || vl_cta <= Decimal::ZERO {
            return false;
        }
        if Self::contem_filtro_negativo(descr) {
            return false;
        }
        true
    }

    /// Calcula a similaridade de melhor substring equivalente a `rapidfuzz.fuzz.partial_ratio` (0.0 a 1.0)
    /// com paridade exata ao algoritmo C++/Python do RapidFuzz via LCS bit-paralelo (Hyyrö/Allison)
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
        let len1 = short_chars.len();
        let len2 = long_chars.len();

        if len1 == 0 {
            return 0.0;
        }
        if len1 > 64 {
            // Fallback para strings excepcionalmente longas (catálogo fiscal tem max 42 chars)
            return Self::partial_ratio_fallback(&short_chars, &long_chars);
        }
        if len1 >= len2 {
            return Self::indel_ratio_bit_parallel(&short_chars, &long_chars);
        }

        let mut block = [0u64; 128];
        let mut mask = 0u64;
        let mut short_set = [false; 128];

        for (i, &ch) in short_chars.iter().enumerate() {
            let bit = 1u64 << i;
            mask |= bit;
            let idx = ch as usize;
            if idx < 128 {
                block[idx] |= bit;
                short_set[idx] = true;
            }
        }

        let is_in_set = |ch: char| -> bool {
            let idx = ch as usize;
            idx < 128 && short_set[idx]
        };

        let mut max_ratio = 0.0f64;

        // 1. Prefixos da string mais longa (tamanho 1..len1)
        for i in 1..len1 {
            if !is_in_set(long_chars[i - 1]) {
                continue;
            }
            let lcs = Self::bit_lcs(&block, mask, &long_chars[..i]);
            let ratio = (2.0 * lcs as f64) / ((len1 + i) as f64);
            if ratio > max_ratio {
                max_ratio = ratio;
                if max_ratio >= 0.9999 {
                    return 1.0;
                }
            }
        }

        // 2. Janela deslizante de tamanho exato len1
        for i in 0..=(len2 - len1) {
            if !is_in_set(long_chars[i + len1 - 1]) {
                continue;
            }
            let lcs = Self::bit_lcs(&block, mask, &long_chars[i..i + len1]);
            let ratio = (2.0 * lcs as f64) / ((2 * len1) as f64);
            if ratio > max_ratio {
                max_ratio = ratio;
                if max_ratio >= 0.9999 {
                    return 1.0;
                }
            }
        }

        // 3. Sufixos da string mais longa (tamanho len1 - 1 até 1)
        for i in (len2 - len1 + 1)..len2 {
            if !is_in_set(long_chars[i]) {
                continue;
            }
            let sub_len = len2 - i;
            let lcs = Self::bit_lcs(&block, mask, &long_chars[i..]);
            let ratio = (2.0 * lcs as f64) / ((len1 + sub_len) as f64);
            if ratio > max_ratio {
                max_ratio = ratio;
                if max_ratio >= 0.9999 {
                    return 1.0;
                }
            }
        }

        max_ratio
    }

    #[inline(always)]
    fn bit_lcs(block: &[u64; 128], mask: u64, s2: &[char]) -> usize {
        let mut s = mask;
        for &ch in s2 {
            let idx = ch as usize;
            let matches = if idx < 128 { block[idx] } else { 0 };
            let u = s & matches;
            s = s.wrapping_add(u) | (s - u);
        }
        ((!s) & mask).count_ones() as usize
    }

    fn indel_ratio_bit_parallel(s1: &[char], s2: &[char]) -> f64 {
        let len1 = s1.len();
        let len2 = s2.len();
        if len1 == 0 && len2 == 0 {
            return 1.0;
        }
        if len1 == 0 || len2 == 0 {
            return 0.0;
        }
        let mut block = [0u64; 128];
        let mut mask = 0u64;
        for (i, &ch) in s1.iter().enumerate() {
            let bit = 1u64 << i;
            mask |= bit;
            let idx = ch as usize;
            if idx < 128 {
                block[idx] |= bit;
            }
        }
        let lcs = Self::bit_lcs(&block, mask, s2);
        (2.0 * lcs as f64) / ((len1 + len2) as f64)
    }

    fn partial_ratio_fallback(s1: &[char], s2: &[char]) -> f64 {
        let m = s1.len();
        let n = s2.len();
        let mut dp = vec![0usize; n + 1];
        for &c1 in s1 {
            let mut prev = 0;
            for j in 0..n {
                let tmp = dp[j + 1];
                if c1 == s2[j] {
                    dp[j + 1] = prev + 1;
                } else if dp[j] > dp[j + 1] {
                    dp[j + 1] = dp[j];
                }
                prev = tmp;
            }
        }
        (2.0 * dp[n] as f64) / ((m + n) as f64)
    }

    /// Avalia a linha contra o catálogo de 24 termos normalizados com corte de similaridade >= 0.80
    /// Segue a ordem exata do legado Python (`break` no primeiro termo com score >= 80)
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

    /// Implementação com paridade 1:1 à função `extrair_receitas` de `apps/fiscal/utils.py` & `utils_recfuze.py`
    pub fn extrair_receitas(
        &self,
        linhas: &[LinhaExtracaoInput],
        palavras_chave: Option<&[&str]>,
    ) -> Vec<ResultadoExtracao> {
        let termos: Vec<String> = match palavras_chave {
            Some(custom) => custom.iter().map(|p| Self::normalizar(p)).collect(),
            None => self.termos_padrao.clone(),
        };

        let mut resultados = Vec::new();
        for linha in linhas {
            let desc_norm = Self::normalizar(&linha.descricao);
            for palavra in &termos {
                let score = Self::partial_ratio(&desc_norm, palavra);
                if score >= 0.80 {
                    resultados.push(ResultadoExtracao {
                        cnpj: linha.cnpj.clone(),
                        cod_agl: linha.cod_agl.clone(),
                        descricao: linha.descricao.clone(),
                        valor: linha.valor,
                        relevancia: palavra.clone(),
                        similaridade: score,
                    });
                    break;
                }
            }
        }

        resultados
    }

    /// Implementação com paridade 1:1 à query e filtragem `filtra_receitas_j150` de `apps/fiscal/utils.py`
    pub fn filtra_receitas_j150(
        &self,
        cnpj: &str,
        registros: &[RegJ150],
        hashes_substituidos: Option<&HashSet<String>>,
    ) -> Vec<ResultadoExtracao> {
        // 1. Filtrar registros elegíveis conforme cláusulas WHERE do SQL legado
        let mut elegiveis: Vec<&RegJ150> = registros
            .iter()
            .filter(|r| {
                if r.cnpj != cnpj {
                    return false;
                }
                if let Some(subs) = hashes_substituidos {
                    if subs.contains(&r.hash_arquivo) {
                        return false;
                    }
                }
                self.elegivel_receita_j150(
                    &r.ind_dc_cta_fin,
                    r.vl_cta_fin,
                    &r.ind_cod_agl,
                    &r.descr_cod_agl,
                )
            })
            .collect();

        // 2. ORDER BY 4 DESC (ordenar por valor decrescente)
        elegiveis.sort_by_key(|a| std::cmp::Reverse(a.vl_cta_fin));

        // 3. Converter para LinhaExtracaoInput e aplicar extrair_receitas
        let inputs: Vec<LinhaExtracaoInput> = elegiveis
            .iter()
            .map(|r| {
                LinhaExtracaoInput::new(
                    &r.cnpj,
                    r.cod_agl.as_deref().unwrap_or(""),
                    &r.descr_cod_agl,
                    r.vl_cta_fin,
                )
            })
            .collect();

        self.extrair_receitas(&inputs, None)
    }

    /// Classifica e apura as receitas a partir do bloco J150 (DRE)
    pub fn apurar_j150(&self, registros: &[RegJ150]) -> ReceitaApurada {
        let mut linhas_identificadas = Vec::new();
        let mut receita_servico = Decimal::ZERO;
        let mut outras_receitas = Decimal::ZERO;
        let mut receita_bruta_totalizador = Decimal::ZERO;
        let mut receita_detalhes_soma = Decimal::ZERO;
        let mut receita_acumulada = Decimal::ZERO;

        for r in registros {
            if r.ind_dc_cta_fin != "C" || r.vl_cta_fin <= Decimal::ZERO {
                continue;
            }
            if Self::contem_filtro_negativo(&r.descr_cod_agl) {
                continue;
            }

            if let Some((score, termo)) = self.avaliar_linha(&r.descr_cod_agl) {
                let descr_norm = Self::normalizar(&r.descr_cod_agl);
                let tipo = if descr_norm.contains("servico")
                    || termo.contains("servicos")
                    || descr_norm.contains("prestacao de servico")
                {
                    TipoReceita::Servico
                } else if descr_norm.contains("outra")
                    || descr_norm.contains("fina")
                    || descr_norm.contains("locacao")
                    || descr_norm.contains("desagio")
                    || descr_norm.contains("aluguel")
                    || descr_norm.contains("patrimonial")
                {
                    TipoReceita::Outras
                } else {
                    TipoReceita::Bruta
                };

                linhas_identificadas.push(LinhaReceita {
                    cod_agl: r.cod_agl.clone().unwrap_or_default(),
                    descricao: r.descr_cod_agl.clone(),
                    valor: r.vl_cta_fin,
                    similaridade: score,
                    termo_correspondente: termo,
                    tipo,
                });

                match tipo {
                    TipoReceita::Servico => {
                        receita_servico += r.vl_cta_fin;
                    }
                    TipoReceita::Outras => {
                        outras_receitas += r.vl_cta_fin;
                    }
                    TipoReceita::Bruta => {}
                }

                if r.ind_cod_agl == "T" {
                    if r.vl_cta_fin > receita_bruta_totalizador {
                        receita_bruta_totalizador = r.vl_cta_fin;
                    }
                } else {
                    receita_detalhes_soma += r.vl_cta_fin;
                }
            }
        }

        let receita_bruta = if receita_bruta_totalizador > Decimal::ZERO {
            receita_bruta_totalizador
        } else if receita_detalhes_soma > Decimal::ZERO {
            receita_detalhes_soma
        } else {
            receita_servico + outras_receitas
        };

        let receita_bruta = receita_bruta.max(receita_servico + outras_receitas);
        receita_acumulada = receita_acumulada.max(receita_bruta);

        ReceitaApurada {
            receita_bruta,
            receita_servico,
            outras_receitas,
            receita_acumulada,
            linhas_identificadas,
        }
    }

    /// Classifica e apura as receitas a partir do bloco I355 (Encerramento de Exercício)
    pub fn apurar_i355(&self, registros: &[RegI355]) -> ReceitaApurada {
        let mut linhas_identificadas = Vec::new();
        let mut receita_bruta = Decimal::ZERO;
        let mut receita_servico = Decimal::ZERO;
        let mut outras_receitas = Decimal::ZERO;
        let mut receita_acumulada = Decimal::ZERO;

        for r in registros {
            if r.ind_dc != "C" || r.vl_cta <= Decimal::ZERO {
                continue;
            }

            let descr = r
                .nome_cta_referencia
                .as_deref()
                .or(r.nome_aglutinador.as_deref())
                .unwrap_or(&r.cod_cta);

            if Self::contem_filtro_negativo(descr) {
                continue;
            }

            let cta_ref = r.cod_cta_ref.as_deref().unwrap_or("");
            let descr_norm = Self::normalizar(descr);

            let is_servico = cta_ref.starts_with("3.01.01.01.01.06")
                || cta_ref.starts_with("3.01.01.01.01.03")
                || cta_ref.starts_with("3.01.01.01.01.25")
                || cta_ref.starts_with("4.01.01.03.01")
                || cta_ref.starts_with("4.01.01.01.")
                || cta_ref.starts_with("3.1.7.1.7.99.00")
                || descr_norm.contains("servico");

            let is_outras = cta_ref.starts_with("3.01.01.01.01.08")
                || cta_ref.starts_with("3.01.01.05.01")
                || cta_ref.starts_with("4.01.05.01.01")
                || cta_ref.starts_with("4.01.01.09.09")
                || cta_ref.starts_with("4.01.09.01.09")
                || cta_ref.starts_with("4.01.01.09.03")
                || cta_ref.starts_with("4.01.01.02.")
                || descr_norm.contains("financeira")
                || descr_norm.contains("outra")
                || descr_norm.contains("desagio")
                || descr_norm.contains("locacao");

            if let Some((score, termo)) = self.avaliar_linha(descr) {
                let tipo = if is_servico {
                    TipoReceita::Servico
                } else if is_outras {
                    TipoReceita::Outras
                } else {
                    TipoReceita::Bruta
                };

                linhas_identificadas.push(LinhaReceita {
                    cod_agl: r.cod_agl.clone().unwrap_or_default(),
                    descricao: descr.to_string(),
                    valor: r.vl_cta,
                    similaridade: score,
                    termo_correspondente: termo,
                    tipo,
                });

                match tipo {
                    TipoReceita::Servico => receita_servico += r.vl_cta,
                    TipoReceita::Outras => outras_receitas += r.vl_cta,
                    TipoReceita::Bruta => receita_bruta += r.vl_cta,
                }

                receita_acumulada += r.vl_cta;
            }
        }

        let total_bruta = (receita_bruta + receita_servico + outras_receitas)
            .max(receita_servico)
            .max(outras_receitas);

        ReceitaApurada {
            receita_bruta: total_bruta,
            receita_servico,
            outras_receitas,
            receita_acumulada,
            linhas_identificadas,
        }
    }

    /// Apuração consolidada de receitas (TASK-302) com priorização de DRE (J150)
    /// e fallback/validação cruzada com contas de encerramento (I355)
    pub fn apurar_consolidado(&self, j150: &[RegJ150], i355: &[RegI355]) -> ReceitaApurada {
        let ap_j150 = self.apurar_j150(j150);
        let ap_i355 = self.apurar_i355(i355);

        if ap_j150.receita_bruta > Decimal::ZERO {
            let mut linhas = ap_j150.linhas_identificadas;
            let receita_servico = if ap_j150.receita_servico > Decimal::ZERO {
                ap_j150.receita_servico
            } else {
                for l in &ap_i355.linhas_identificadas {
                    if l.tipo == TipoReceita::Servico {
                        linhas.push(l.clone());
                    }
                }
                ap_i355.receita_servico
            };

            let outras_receitas = if ap_j150.outras_receitas > Decimal::ZERO {
                ap_j150.outras_receitas
            } else {
                for l in &ap_i355.linhas_identificadas {
                    if l.tipo == TipoReceita::Outras {
                        linhas.push(l.clone());
                    }
                }
                ap_i355.outras_receitas
            };

            let receita_acumulada = if ap_i355.receita_acumulada > Decimal::ZERO {
                ap_i355.receita_acumulada.max(ap_j150.receita_bruta)
            } else {
                ap_j150.receita_bruta
            };

            let receita_bruta = ap_j150.receita_bruta.max(receita_servico + outras_receitas);

            ReceitaApurada {
                receita_bruta,
                receita_servico,
                outras_receitas,
                receita_acumulada,
                linhas_identificadas: linhas,
            }
        } else {
            ap_i355
        }
    }
}
