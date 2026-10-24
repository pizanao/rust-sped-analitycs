//! Agregador de indicadores cadastrais e econômicos de empresas.
//!
//! Implementa o cálculo de distribuições agregadas para:
//! - Porte Empresarial (Microempresa, EPP, Médio/Grande, Não Informado)
//! - CNAE Top 15 (com códigos normalizados de 7 dígitos e descrições)
//! - Regime Tributário (MEI, Simples Nacional, Lucro Presumido, Lucro Real, Outros)
//! - Capital Social (faixas econômicas e valor médio)
//! - Situação Cadastral (Ativa, Baixada, Inapta, Suspensa, Nula)
//! - Distribuição Geográfica (UF e Município)
//! - Natureza Jurídica
//!
//! Compatível com o schema do legado `apps/indicadores/models.py` e `apps/indicadores/utils.py`.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{IndicadorItem, IndicadoresResumo};

/// Registro individual de empresa/estabelecimento para processamento de indicadores.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct EmpresaRegistro {
    pub cnpj: Option<String>,
    pub porte_empresa: Option<String>,
    pub situacao_cadastral: Option<String>,
    pub cnae_fiscal: Option<String>,
    pub cnae_descricao: Option<String>,
    pub natureza_juridica: Option<String>,
    pub capital_social: Option<Decimal>,
    pub opcao_mei: Option<String>,
    pub opcao_simples: Option<String>,
    pub regime_tributario: Option<String>,
    pub uf: Option<String>,
    pub municipio: Option<String>,
}

impl EmpresaRegistro {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_porte(mut self, porte: impl Into<String>) -> Self {
        self.porte_empresa = Some(porte.into());
        self
    }

    pub fn with_situacao(mut self, situacao: impl Into<String>) -> Self {
        self.situacao_cadastral = Some(situacao.into());
        self
    }

    pub fn with_cnae(mut self, cnae: impl Into<String>) -> Self {
        self.cnae_fiscal = Some(cnae.into());
        self
    }

    pub fn with_natureza(mut self, nat: impl Into<String>) -> Self {
        self.natureza_juridica = Some(nat.into());
        self
    }

    pub fn with_capital(mut self, capital: Decimal) -> Self {
        self.capital_social = Some(capital);
        self
    }

    pub fn with_mei(mut self, opcao_mei: impl Into<String>) -> Self {
        self.opcao_mei = Some(opcao_mei.into());
        self
    }

    pub fn with_simples(mut self, opcao_simples: impl Into<String>) -> Self {
        self.opcao_simples = Some(opcao_simples.into());
        self
    }

    pub fn with_regime(mut self, regime: impl Into<String>) -> Self {
        self.regime_tributario = Some(regime.into());
        self
    }

    pub fn with_uf(mut self, uf: impl Into<String>) -> Self {
        self.uf = Some(uf.into());
        self
    }

    pub fn with_municipio(mut self, mun: impl Into<String>) -> Self {
        self.municipio = Some(mun.into());
        self
    }
}

/// Filtros opcionais para agregação de indicadores.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndicadoresFiltro {
    pub uf: Option<String>,
    pub municipio: Option<String>,
    pub cnae: Option<String>,
    pub ignorar_baixadas: bool,
    pub data_referencia: Option<NaiveDate>,
}

impl Default for IndicadoresFiltro {
    fn default() -> Self {
        Self::new()
    }
}

impl IndicadoresFiltro {
    pub fn new() -> Self {
        Self {
            uf: None,
            municipio: None,
            cnae: None,
            ignorar_baixadas: true,
            data_referencia: None,
        }
    }

    pub fn with_uf(mut self, uf: impl Into<String>) -> Self {
        self.uf = Some(uf.into());
        self
    }

    pub fn with_municipio(mut self, mun: impl Into<String>) -> Self {
        self.municipio = Some(mun.into());
        self
    }

    pub fn with_cnae(mut self, cnae: impl Into<String>) -> Self {
        self.cnae = Some(cnae.into());
        self
    }

    pub fn com_baixadas(mut self) -> Self {
        self.ignorar_baixadas = false;
        self
    }

    pub fn with_data(mut self, data: NaiveDate) -> Self {
        self.data_referencia = Some(data);
        self
    }

    /// Gera um sufixo descritivo e seguro para chaves de cache.
    pub fn cache_key_suffix(&self) -> String {
        let mut partes = Vec::new();
        if let Some(uf) = &self.uf {
            partes.push(format!("uf-{}", uf.trim().to_uppercase()));
        }
        if let Some(mun) = &self.municipio {
            partes.push(format!("mun-{}", mun.trim().to_uppercase()));
        }
        if let Some(cnae) = &self.cnae {
            let digits: String = cnae.chars().filter(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                partes.push(format!("cnae-{}", digits));
            }
        }
        if !self.ignorar_baixadas {
            partes.push("com_baixadas".to_string());
        }
        partes.join("_")
    }
}

/// Faixa de agrupamento de capital social.
#[derive(Clone, Copy, Debug)]
pub struct FaixaCapital {
    pub codigo: &'static str,
    pub descricao: &'static str,
    pub limite_superior: f64,
}

pub const FAIXAS_CAPITAL: &[FaixaCapital] = &[
    FaixaCapital {
        codigo: "0",
        descricao: "Sem capital declarado",
        limite_superior: 0.0,
    },
    FaixaCapital {
        codigo: "1",
        descricao: "Até R$ 1.000",
        limite_superior: 1_000.0,
    },
    FaixaCapital {
        codigo: "2",
        descricao: "R$ 1.001 a R$ 10.000",
        limite_superior: 10_000.0,
    },
    FaixaCapital {
        codigo: "3",
        descricao: "R$ 10.001 a R$ 100.000",
        limite_superior: 100_000.0,
    },
    FaixaCapital {
        codigo: "4",
        descricao: "R$ 100.001 a R$ 1.000.000",
        limite_superior: 1_000_000.0,
    },
    FaixaCapital {
        codigo: "5",
        descricao: "Acima de R$ 1.000.000",
        limite_superior: f64::INFINITY,
    },
];

/// Normaliza o código de porte para padrão de 2 caracteres ("00", "01", "03", "05").
pub fn normalizar_porte(codigo: &str) -> &'static str {
    let clean = codigo.trim();
    match clean {
        "01" | "1" => "01",
        "03" | "3" => "03",
        "05" | "5" => "05",
        "00" | "0" => "00",
        _ => {
            if clean.starts_with("01") || clean.starts_with('1') {
                "01"
            } else if clean.starts_with("03") || clean.starts_with('3') {
                "03"
            } else if clean.starts_with("05") || clean.starts_with('5') {
                "05"
            } else {
                "00"
            }
        }
    }
}

/// Normaliza o código de situação cadastral ("01", "02", "03", "04", "08").
pub fn normalizar_situacao(codigo: &str) -> &'static str {
    let clean = codigo.trim();
    match clean {
        "01" | "1" => "01",
        "02" | "2" => "02",
        "03" | "3" => "03",
        "04" | "4" => "04",
        "08" | "8" => "08",
        _ => "02",
    }
}

/// Dicionário padrão de portes da Receita Federal.
pub fn nome_porte(codigo: &str) -> &'static str {
    match normalizar_porte(codigo) {
        "00" => "Não informado",
        "01" => "Micro empresa",
        "03" => "Empresa de pequeno porte",
        "05" => "Demais (Médio ou Grande porte)",
        _ => "Desconhecido",
    }
}

/// Dicionário padrão de situação cadastral da Receita Federal.
pub fn nome_situacao_cadastral(codigo: &str) -> &'static str {
    match normalizar_situacao(codigo) {
        "01" => "Nula",
        "02" => "Ativa",
        "03" => "Suspensa",
        "04" => "Inapta",
        "08" => "Baixada",
        _ => "Desconhecida",
    }
}

/// Catálogo de descrições dos CNAEs mais comuns no Brasil.
pub fn lookup_cnae_descricao(codigo: &str) -> Option<&'static str> {
    let clean: String = codigo.chars().filter(|c| c.is_ascii_digit()).collect();
    match clean.as_str() {
        "4711302" => Some("Comércio varejista de mercadorias em geral (hipermercados)"),
        "4712100" => Some("Comércio varejista de mercadorias em geral (minimercados e mercearias)"),
        "4744099" => Some("Comércio varejista de materiais de construção em geral"),
        "5611201" => Some("Restaurantes e similares"),
        "5611203" => Some("Lanchonetes, casas de chá, de sucos e similares"),
        "6201501" => Some("Desenvolvimento de programas de computador sob encomenda"),
        "6202300" => Some("Desenvolvimento e licenciamento de programas customizáveis"),
        "6204000" => Some("Consultoria em tecnologia da informação"),
        "6209100" => Some("Suporte técnico, manutenção e outros serviços em TI"),
        "6920601" => Some("Atividades de contabilidade"),
        "7020400" => Some("Atividades de consultoria em gestão empresarial"),
        "4120400" => Some("Construção de edifícios"),
        "4930202" => Some("Transporte rodoviário de carga intermunicipal e interestadual"),
        "8630503" => Some("Atividade médica ambulatorial"),
        "9602501" => Some("Cabeleireiros, manicure e pedicure"),
        "4520001" => Some("Serviços de manutenção e reparação mecânica de veículos"),
        "7319002" => Some("Promoção de vendas e publicidade no local de venda"),
        "8219999" => Some("Preparação de documentos e serviços especializados de apoio"),
        "8599699" => Some("Outras atividades de ensino não especificadas anteriormente"),
        "6810202" => Some("Aluguel de imóveis próprios"),
        "6821801" => Some("Corretagem na compra e venda e avaliação de imóveis"),
        "6462000" => Some("Holdings de instituições não-financeiras"),
        "0115600" => Some("Cultivo de soja e grãos"),
        "4781400" => Some("Comércio varejista de artigos do vestuário e acessórios"),
        "8299799" => Some("Outras atividades de serviços prestados às empresas"),
        "7490104" => Some("Atividades de intermediação e agenciamento de negócios em geral"),
        "5320202" => Some("Serviços de entrega rápida"),
        "4399103" => Some("Obras de alvenaria"),
        "4321500" => Some("Instalação e manutenção elétrica"),
        "4729699" => Some("Comércio varejista de produtos alimentícios em geral"),
        "7711000" => Some("Locação de automóveis sem condutor"),
        _ => None,
    }
}

/// Dicionário com principais Naturezas Jurídicas.
pub fn lookup_natureza_juridica(codigo: &str) -> Option<&'static str> {
    let clean = codigo.replace(['-', ' '], "");
    match clean.as_str() {
        "2062" => Some("Sociedade Empresária Limitada"),
        "2135" => Some("Empresário Individual"),
        "2046" => Some("Sociedade Anônima Aberta"),
        "2054" => Some("Sociedade Anônima Fechada"),
        "2232" => Some("Sociedade Simples Pura"),
        "2240" => Some("Sociedade Simples Limitada"),
        "2305" => Some("Empresa Individual de Responsabilidade Limitada (EIRELI)"),
        "3999" => Some("Associação Privada"),
        "1015" => Some("Órgão Público do Poder Executivo Federal"),
        _ => None,
    }
}

/// Calcula o percentual e arredonda para duas casas decimais.
#[inline]
pub fn calcular_percentual(quantidade: i64, total: i64) -> f64 {
    if total <= 0 || quantidade <= 0 {
        return 0.0;
    }
    let p = (quantidade as f64 / total as f64) * 100.0;
    (p * 100.0).round() / 100.0
}

/// Arredonda número flutuante para duas casas decimais.
#[inline]
pub fn arredondar_2casas(valor: f64) -> f64 {
    (valor * 100.0).round() / 100.0
}

/// Agregador principal de indicadores estatísticos de empresas.
#[derive(Clone, Debug)]
pub struct IndicadoresAggregator {
    cnae_limit: usize,
    municipio_limit: usize,
    natureza_limit: usize,
}

impl Default for IndicadoresAggregator {
    fn default() -> Self {
        Self {
            cnae_limit: 15,
            municipio_limit: 20,
            natureza_limit: 15,
        }
    }
}

impl IndicadoresAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cnae_limit(mut self, limit: usize) -> Self {
        self.cnae_limit = limit;
        self
    }

    pub fn with_municipio_limit(mut self, limit: usize) -> Self {
        self.municipio_limit = limit;
        self
    }

    pub fn with_natureza_limit(mut self, limit: usize) -> Self {
        self.natureza_limit = limit;
        self
    }

    /// Processa uma coleção de registros de empresas e gera o resumo de indicadores.
    pub fn processar(&self, registros: &[EmpresaRegistro]) -> IndicadoresResumo {
        self.processar_com_filtro(registros, &IndicadoresFiltro::default())
    }

    /// Processa os registros de empresas aplicando o filtro especificado.
    pub fn processar_com_filtro(
        &self,
        registros: &[EmpresaRegistro],
        filtro: &IndicadoresFiltro,
    ) -> IndicadoresResumo {
        let clean_cnae_filtro = filtro.cnae.as_ref().map(|c| {
            c.chars()
                .filter(|ch| ch.is_ascii_digit())
                .collect::<String>()
        });

        // Filtragem inicial dos registros
        let registros_filtrados: Vec<&EmpresaRegistro> = registros
            .iter()
            .filter(|emp| {
                if filtro.ignorar_baixadas {
                    let sit = emp.situacao_cadastral.as_deref().unwrap_or("02");
                    let cod_sit = normalizar_situacao(sit);
                    if cod_sit == "08" {
                        return false;
                    }
                }
                if let Some(uf_filtro) = &filtro.uf {
                    if emp.uf.as_deref().map(|s| s.eq_ignore_ascii_case(uf_filtro)) != Some(true) {
                        return false;
                    }
                }
                if let Some(mun_filtro) = &filtro.municipio {
                    if emp
                        .municipio
                        .as_deref()
                        .map(|s| s.eq_ignore_ascii_case(mun_filtro))
                        != Some(true)
                    {
                        return false;
                    }
                }
                if let Some(cnae_f) = &clean_cnae_filtro {
                    if !cnae_f.is_empty() {
                        let emp_cnae_digits: String = emp
                            .cnae_fiscal
                            .as_deref()
                            .unwrap_or("")
                            .chars()
                            .filter(|ch| ch.is_ascii_digit())
                            .collect();
                        if !emp_cnae_digits.starts_with(cnae_f) {
                            return false;
                        }
                    }
                }
                true
            })
            .collect();

        let total_empresas = registros_filtrados.len() as i64;

        let porte = self.processar_porte(&registros_filtrados, total_empresas);
        let cnae_top15 = self.processar_cnae(&registros_filtrados);
        let regime_tributario = self.processar_regime(&registros_filtrados, total_empresas);
        let (capital_social, capital_social_medio) =
            self.processar_capital(&registros_filtrados, total_empresas);
        let situacao_cadastral = self.processar_situacao(&registros_filtrados, total_empresas);
        let uf = self.processar_uf(&registros_filtrados, total_empresas);
        let municipio = self.processar_municipio(&registros_filtrados, total_empresas);
        let natureza_juridica = self.processar_natureza(&registros_filtrados, total_empresas);

        let mut resumo = IndicadoresResumo {
            distribuicao_porte: porte.clone(),
            porte,
            top_cnaes: cnae_top15.clone(),
            cnae_top15,
            regimes_tributarios: regime_tributario.clone(),
            regime_tributario,
            capital_social,
            situacao_cadastral,
            uf,
            municipio,
            natureza_juridica,
            total_empresas,
            capital_social_medio,
            total_receita_apurada: 0.0,
            total_alertas_sancoes: 0,
            data_referencia: filtro.data_referencia,
            hash_dados: None,
        };

        resumo.hash_dados = Some(self.calcular_hash(&resumo));
        resumo
    }

    /// Processa a distribuição de portes empresariais.
    pub fn processar_porte(
        &self,
        empresas: &[&EmpresaRegistro],
        total_empresas: i64,
    ) -> Vec<IndicadorItem> {
        let mut contadores: HashMap<String, i64> = HashMap::new();

        for emp in empresas {
            let porte = emp.porte_empresa.as_deref().unwrap_or("00");
            let codigo = normalizar_porte(porte);
            *contadores.entry(codigo.to_string()).or_insert(0) += 1;
        }

        let mut resultados: Vec<IndicadorItem> = contadores
            .into_iter()
            .map(|(codigo, quantidade)| {
                let nome = nome_porte(&codigo);
                let percentual = calcular_percentual(quantidade, total_empresas);
                IndicadorItem::new(format!("{codigo} - {nome}"), quantidade, percentual)
                    .with_codigo(codigo)
            })
            .collect();

        resultados.sort_by_key(|a| std::cmp::Reverse(a.quantidade));
        resultados
    }

    /// Processa os CNAEs mais frequentes (Top N).
    pub fn processar_cnae(&self, empresas: &[&EmpresaRegistro]) -> Vec<IndicadorItem> {
        let mut contadores: HashMap<String, (i64, Option<String>)> = HashMap::new();
        let mut total_com_cnae: i64 = 0;

        for emp in empresas {
            if let Some(cnae_raw) = &emp.cnae_fiscal {
                let digitos: String = cnae_raw.chars().filter(|c| c.is_ascii_digit()).collect();
                let cnae_key = if digitos.len() >= 7 {
                    digitos[..7].to_string()
                } else if digitos.len() >= 5 {
                    digitos
                } else {
                    continue;
                };

                let entry = contadores
                    .entry(cnae_key)
                    .or_insert((0, emp.cnae_descricao.clone()));
                entry.0 += 1;
                if entry.1.is_none() && emp.cnae_descricao.is_some() {
                    entry.1 = emp.cnae_descricao.clone();
                }
                total_com_cnae += 1;
            }
        }

        let mut sorted_cnaes: Vec<(String, (i64, Option<String>))> =
            contadores.into_iter().collect();
        sorted_cnaes.sort_by_key(|a| std::cmp::Reverse(a.1 .0));

        sorted_cnaes
            .into_iter()
            .take(self.cnae_limit)
            .map(|(cnae, (quantidade, descricao_custom))| {
                let descricao = descricao_custom
                    .as_deref()
                    .or_else(|| lookup_cnae_descricao(&cnae))
                    .unwrap_or("CNAE não identificado");
                let percentual = calcular_percentual(quantidade, total_com_cnae);
                IndicadorItem::new(format!("{cnae} - {descricao}"), quantidade, percentual)
                    .with_codigo(cnae)
            })
            .collect()
    }

    /// Processa a classificação por regime tributário (MEI, Simples, Presumido, Real, Outros).
    pub fn processar_regime(
        &self,
        empresas: &[&EmpresaRegistro],
        total_empresas: i64,
    ) -> Vec<IndicadorItem> {
        let mut contadores: HashMap<&'static str, i64> = HashMap::from([
            ("MEI", 0),
            ("Simples", 0),
            ("Lucro Presumido", 0),
            ("Lucro Real", 0),
            ("Outros", 0),
        ]);

        for emp in empresas {
            let regime = Self::classificar_regime(emp);
            *contadores.entry(regime).or_insert(0) += 1;
        }

        let mut resultados: Vec<IndicadorItem> = contadores
            .into_iter()
            .filter(|(_, qtd)| *qtd > 0)
            .map(|(regime, quantidade)| {
                let percentual = calcular_percentual(quantidade, total_empresas);
                IndicadorItem::new(regime, quantidade, percentual).with_codigo(regime)
            })
            .collect();

        resultados.sort_by_key(|a| std::cmp::Reverse(a.quantidade));
        resultados
    }

    /// Classifica uma empresa em seu regime tributário correspondente.
    pub fn classificar_regime(empresa: &EmpresaRegistro) -> &'static str {
        if empresa.opcao_mei.as_deref() == Some("S") || empresa.opcao_mei.as_deref() == Some("s") {
            return "MEI";
        }
        if let Some(reg) = &empresa.regime_tributario {
            let upper = reg.trim().to_uppercase();
            if upper.contains("SIMPLES") {
                return "Simples";
            } else if upper.contains("PRESUMIDO") {
                return "Lucro Presumido";
            } else if upper.contains("REAL") {
                return "Lucro Real";
            } else if upper == "MEI" {
                return "MEI";
            } else if !upper.is_empty() {
                return "Outros";
            }
        }
        if empresa.opcao_simples.as_deref() == Some("S")
            || empresa.opcao_simples.as_deref() == Some("s")
        {
            "Simples"
        } else if empresa.opcao_simples.as_deref() == Some("N")
            || empresa.opcao_simples.as_deref() == Some("n")
        {
            "Outros"
        } else if empresa.opcao_mei.as_deref() == Some("N")
            || empresa.opcao_mei.as_deref() == Some("n")
        {
            // Paridade estrita com o script Python legado: if opcao_mei != 'S': Simples
            "Simples"
        } else {
            "Outros"
        }
    }

    /// Processa os indicadores de capital social agrupados em faixas e calcula a média.
    pub fn processar_capital(
        &self,
        empresas: &[&EmpresaRegistro],
        total_empresas: i64,
    ) -> (Vec<IndicadorItem>, Option<f64>) {
        struct FaixaAcc {
            faixa: &'static FaixaCapital,
            quantidade: i64,
            total_valor: f64,
        }

        let mut accs: Vec<FaixaAcc> = FAIXAS_CAPITAL
            .iter()
            .map(|f| FaixaAcc {
                faixa: f,
                quantidade: 0,
                total_valor: 0.0,
            })
            .collect();

        let mut soma_geral: f64 = 0.0;

        for emp in empresas {
            let cap_f64 = emp.capital_social.and_then(|d| d.to_f64()).unwrap_or(0.0);

            if cap_f64 > 0.0 {
                soma_geral += cap_f64;
            }

            // Encontra a faixa correspondente
            if cap_f64 <= 0.0 {
                accs[0].quantidade += 1;
            } else if cap_f64 <= 1_000.0 {
                accs[1].quantidade += 1;
                accs[1].total_valor += cap_f64;
            } else if cap_f64 <= 10_000.0 {
                accs[2].quantidade += 1;
                accs[2].total_valor += cap_f64;
            } else if cap_f64 <= 100_000.0 {
                accs[3].quantidade += 1;
                accs[3].total_valor += cap_f64;
            } else if cap_f64 <= 1_000_000.0 {
                accs[4].quantidade += 1;
                accs[4].total_valor += cap_f64;
            } else {
                accs[5].quantidade += 1;
                accs[5].total_valor += cap_f64;
            }
        }

        let mut resultados: Vec<IndicadorItem> = accs
            .into_iter()
            .filter(|acc| acc.quantidade > 0)
            .map(|acc| {
                let percentual = calcular_percentual(acc.quantidade, total_empresas);
                let valor_medio = if acc.quantidade > 0 && acc.total_valor > 0.0 {
                    arredondar_2casas(acc.total_valor / acc.quantidade as f64)
                } else {
                    0.0
                };

                IndicadorItem::new(acc.faixa.descricao, acc.quantidade, percentual)
                    .with_codigo(acc.faixa.codigo)
                    .with_valor_medio(valor_medio)
            })
            .collect();

        resultados.sort_by_key(|a| std::cmp::Reverse(a.quantidade));

        let capital_medio_geral = if total_empresas > 0 {
            Some(arredondar_2casas(soma_geral / total_empresas as f64))
        } else {
            None
        };

        (resultados, capital_medio_geral)
    }

    /// Processa a situação cadastral das empresas.
    pub fn processar_situacao(
        &self,
        empresas: &[&EmpresaRegistro],
        total_empresas: i64,
    ) -> Vec<IndicadorItem> {
        let mut contadores: HashMap<String, i64> = HashMap::new();

        for emp in empresas {
            let situacao = emp.situacao_cadastral.as_deref().unwrap_or("02");
            let codigo = normalizar_situacao(situacao);
            *contadores.entry(codigo.to_string()).or_insert(0) += 1;
        }

        let mut resultados: Vec<IndicadorItem> = contadores
            .into_iter()
            .map(|(codigo, quantidade)| {
                let nome = nome_situacao_cadastral(&codigo);
                let percentual = calcular_percentual(quantidade, total_empresas);
                IndicadorItem::new(format!("{codigo} - {nome}"), quantidade, percentual)
                    .with_codigo(codigo)
            })
            .collect();

        resultados.sort_by_key(|a| std::cmp::Reverse(a.quantidade));
        resultados
    }

    /// Processa a distribuição geográfica por UF.
    pub fn processar_uf(
        &self,
        empresas: &[&EmpresaRegistro],
        total_empresas: i64,
    ) -> Vec<IndicadorItem> {
        let mut contadores: HashMap<String, i64> = HashMap::new();

        for emp in empresas {
            if let Some(uf) = &emp.uf {
                let clean = uf.trim().to_uppercase();
                if !clean.is_empty() {
                    *contadores.entry(clean).or_insert(0) += 1;
                }
            }
        }

        let mut resultados: Vec<IndicadorItem> = contadores
            .into_iter()
            .map(|(uf, quantidade)| {
                let percentual = calcular_percentual(quantidade, total_empresas);
                IndicadorItem::new(uf.clone(), quantidade, percentual).with_codigo(uf)
            })
            .collect();

        resultados.sort_by_key(|a| std::cmp::Reverse(a.quantidade));
        resultados
    }

    /// Processa os municípios com maior densidade de empresas.
    pub fn processar_municipio(
        &self,
        empresas: &[&EmpresaRegistro],
        total_empresas: i64,
    ) -> Vec<IndicadorItem> {
        let mut contadores: HashMap<String, i64> = HashMap::new();

        for emp in empresas {
            if let Some(mun) = &emp.municipio {
                let clean = mun.trim().to_uppercase();
                if !clean.is_empty() {
                    *contadores.entry(clean).or_insert(0) += 1;
                }
            }
        }

        let mut resultados: Vec<IndicadorItem> = contadores
            .into_iter()
            .map(|(mun, quantidade)| {
                let percentual = calcular_percentual(quantidade, total_empresas);
                IndicadorItem::new(mun.clone(), quantidade, percentual).with_codigo(mun)
            })
            .collect();

        resultados.sort_by_key(|a| std::cmp::Reverse(a.quantidade));
        resultados.truncate(self.municipio_limit);
        resultados
    }

    /// Processa a natureza jurídica das empresas.
    pub fn processar_natureza(
        &self,
        empresas: &[&EmpresaRegistro],
        total_empresas: i64,
    ) -> Vec<IndicadorItem> {
        let mut contadores: HashMap<String, i64> = HashMap::new();

        for emp in empresas {
            if let Some(nat) = &emp.natureza_juridica {
                let clean = nat.trim().replace(['-', ' '], "");
                if !clean.is_empty() {
                    *contadores.entry(clean).or_insert(0) += 1;
                }
            }
        }

        let mut resultados: Vec<IndicadorItem> = contadores
            .into_iter()
            .map(|(nat, quantidade)| {
                let desc = lookup_natureza_juridica(&nat).unwrap_or("Outras Naturezas");
                let percentual = calcular_percentual(quantidade, total_empresas);
                IndicadorItem::new(format!("{nat} - {desc}"), quantidade, percentual)
                    .with_codigo(nat)
            })
            .collect();

        resultados.sort_by_key(|a| std::cmp::Reverse(a.quantidade));
        resultados.truncate(self.natureza_limit);
        resultados
    }

    /// Gera um hash SHA-256 estável e determinístico para o resumo de indicadores.
    pub fn calcular_hash(&self, resumo: &IndicadoresResumo) -> String {
        let mut resumo_sem_hash = resumo.clone();
        resumo_sem_hash.hash_dados = None;
        let json_str = serde_json::to_string(&resumo_sem_hash).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json_str.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

// ==============================================================================
// Consultas SQL e Integração com SQLite / PostgreSQL
// ==============================================================================

/// Consulta SQL oficial para busca de estabelecimentos e dados consolidados da Receita Federal.
pub const SQLITE_QUERY_RFB_EMPRESAS: &str = r#"
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
"#;

pub const POSTGRES_QUERY_RFB_EMPRESAS: &str = r#"
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
    WHERE ($1::text IS NULL OR e.situacao_cadastral != $1)
      AND ($2::text IS NULL OR e.uf = $2)
      AND ($3::text IS NULL OR e.municipio = $3)
      AND ($4::text IS NULL OR e.cnae_fiscal LIKE $4 || '%')
"#;

#[cfg(feature = "sqlite")]
impl IndicadoresAggregator {
    /// Carrega registros de empresas diretamente de uma conexão SQLite da Receita Federal.
    pub fn carregar_de_sqlite(
        conn: &rusqlite::Connection,
        filtro: Option<&IndicadoresFiltro>,
    ) -> Result<Vec<EmpresaRegistro>, crate::AnalyticsError> {
        let param_ignorar_baixadas = if filtro.map(|f| f.ignorar_baixadas).unwrap_or(true) {
            Some("08")
        } else {
            None
        };
        let param_uf = filtro.and_then(|f| f.uf.as_deref());
        let param_mun = filtro.and_then(|f| f.municipio.as_deref());
        let param_cnae = filtro.and_then(|f| f.cnae.as_deref());

        let mut stmt = conn.prepare(SQLITE_QUERY_RFB_EMPRESAS)?;
        let rows = stmt.query_map(
            rusqlite::params![param_ignorar_baixadas, param_uf, param_mun, param_cnae],
            |row| {
                let cnpj: Option<String> = row.get(0)?;
                let porte: Option<String> = row.get(1)?;
                let situacao: Option<String> = row.get(2)?;
                let cnae: Option<String> = row.get(3)?;
                let nat: Option<String> = row.get(4)?;
                let cap_val: rusqlite::types::Value = row.get(5)?;
                let mei: Option<String> = row.get(6)?;
                let simples: Option<String> = row.get(7)?;
                let uf: Option<String> = row.get(8)?;
                let mun: Option<String> = row.get(9)?;

                let capital_decimal = match cap_val {
                    rusqlite::types::Value::Real(f) => Decimal::from_f64_retain(f),
                    rusqlite::types::Value::Integer(i) => Some(Decimal::from(i)),
                    rusqlite::types::Value::Text(s) => {
                        let clean = s.trim().replace('.', "").replace(',', ".");
                        Decimal::from_str_exact(&clean).ok()
                    }
                    _ => None,
                };

                Ok(EmpresaRegistro {
                    cnpj,
                    porte_empresa: porte,
                    situacao_cadastral: situacao,
                    cnae_fiscal: cnae,
                    cnae_descricao: None,
                    natureza_juridica: nat,
                    capital_social: capital_decimal,
                    opcao_mei: mei,
                    opcao_simples: simples,
                    regime_tributario: None,
                    uf,
                    municipio: mun,
                })
            },
        )?;

        let mut registros = Vec::new();
        for r in rows {
            registros.push(r?);
        }

        Ok(registros)
    }

    /// Processa os indicadores diretamente de um banco de dados SQLite.
    pub fn processar_sqlite(
        &self,
        conn: &rusqlite::Connection,
        filtro: Option<&IndicadoresFiltro>,
    ) -> Result<IndicadoresResumo, crate::AnalyticsError> {
        let registros = Self::carregar_de_sqlite(conn, filtro)?;
        let default_filtro = IndicadoresFiltro::default();
        let f = filtro.unwrap_or(&default_filtro);
        Ok(self.processar_com_filtro(&registros, f))
    }
}
