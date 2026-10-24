//! # ecd-analytics
//!
//! Agregações analíticas e séries temporais de indicadores de empresas
//! (CNAE, porte, regime tributário, capital social, situação cadastral, UF e município)
//! com cache em memória assíncrono `moka`.
//!
//! Este módulo substitui o processamento legado de `apps/indicadores/models.py` e
//! `apps/indicadores/utils.py`, fornecendo paridade de regras de negócio, alta performance
//! e cálculos determinísticos.

pub mod aggregator;
pub mod cache;

pub use aggregator::{
    calcular_percentual, nome_porte, nome_situacao_cadastral, EmpresaRegistro, FaixaCapital,
    IndicadoresAggregator, IndicadoresFiltro, FAIXAS_CAPITAL,
};
pub use cache::{IndicadoresCache, IndicadoresCacheConfig};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Erros originados nas operações analíticas e de persistência.
#[derive(thiserror::Error, Debug)]
pub enum AnalyticsError {
    #[error("Erro de serialização ou desserialização JSON: {0}")]
    Serialization(#[from] serde_json::Error),

    #[cfg(feature = "sqlite")]
    #[error("Erro de consulta no banco SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Parâmetro inválido fornecido: {0}")]
    ParametroInvalido(String),

    #[error("Erro interno ao processar indicadores: {0}")]
    Processamento(String),
}

/// Tipos de indicadores empresariais suportados pelo sistema analítico.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoIndicador {
    Cnae,
    Porte,
    Regime,
    Capital,
    Situacao,
    Natureza,
    Uf,
    Municipio,
}

impl TipoIndicador {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cnae => "cnae",
            Self::Porte => "porte",
            Self::Regime => "regime",
            Self::Capital => "capital",
            Self::Situacao => "situacao",
            Self::Natureza => "natureza",
            Self::Uf => "uf",
            Self::Municipio => "municipio",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Cnae => "CNAE - Atividade Econômica",
            Self::Porte => "Porte da Empresa",
            Self::Regime => "Regime Tributário",
            Self::Capital => "Capital Social",
            Self::Situacao => "Situação Cadastral",
            Self::Natureza => "Natureza Jurídica",
            Self::Uf => "Estado (UF)",
            Self::Municipio => "Município",
        }
    }
}

/// Item agregado de uma categoria de indicador.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IndicadorItem {
    pub categoria: String,
    #[serde(default)]
    pub nome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codigo: Option<String>,
    pub quantidade: i64,
    pub percentual: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor_medio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor: Option<f64>,
}

impl IndicadorItem {
    pub fn new(categoria: impl Into<String>, quantidade: i64, percentual: f64) -> Self {
        let cat = categoria.into();
        Self {
            categoria: cat.clone(),
            nome: cat,
            codigo: None,
            quantidade,
            percentual,
            valor_medio: None,
            valor: None,
        }
    }

    pub fn with_codigo(mut self, codigo: impl Into<String>) -> Self {
        self.codigo = Some(codigo.into());
        self
    }

    pub fn with_valor_medio(mut self, valor: f64) -> Self {
        self.valor_medio = Some(valor);
        self.valor = Some(valor);
        self
    }
}

/// Registro compatível com a tabela `indicador_empresa` do banco legado.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IndicadorEmpresa {
    pub data_referencia: NaiveDate,
    pub tipo_indicador: TipoIndicador,
    pub categoria: String,
    pub codigo_categoria: Option<String>,
    pub quantidade: i64,
    pub percentual: f64,
    pub valor_medio: Option<f64>,
    pub hash_dados: Option<String>,
}

/// Resumo consolidado de todas as dimensões de indicadores empresariais.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IndicadoresResumo {
    pub porte: Vec<IndicadorItem>,
    #[serde(default)]
    pub distribuicao_porte: Vec<IndicadorItem>,
    pub cnae_top15: Vec<IndicadorItem>,
    #[serde(default)]
    pub top_cnaes: Vec<IndicadorItem>,
    pub regime_tributario: Vec<IndicadorItem>,
    #[serde(default)]
    pub regimes_tributarios: Vec<IndicadorItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capital_social: Vec<IndicadorItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub situacao_cadastral: Vec<IndicadorItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uf: Vec<IndicadorItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub municipio: Vec<IndicadorItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub natureza_juridica: Vec<IndicadorItem>,
    pub total_empresas: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capital_social_medio: Option<f64>,
    #[serde(default)]
    pub total_receita_apurada: f64,
    #[serde(default)]
    pub total_alertas_sancoes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_referencia: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_dados: Option<String>,
}

impl IndicadoresResumo {
    /// Converte todas as categorias do resumo em registros `IndicadorEmpresa`
    /// para persistência no banco de dados relacional.
    pub fn to_indicadores_empresa(&self, data_referencia: NaiveDate) -> Vec<IndicadorEmpresa> {
        let mut resultado = Vec::new();
        let hash = self.hash_dados.clone();

        let mapeamentos = [
            (TipoIndicador::Porte, &self.porte),
            (TipoIndicador::Cnae, &self.cnae_top15),
            (TipoIndicador::Regime, &self.regime_tributario),
            (TipoIndicador::Capital, &self.capital_social),
            (TipoIndicador::Situacao, &self.situacao_cadastral),
            (TipoIndicador::Uf, &self.uf),
            (TipoIndicador::Municipio, &self.municipio),
            (TipoIndicador::Natureza, &self.natureza_juridica),
        ];

        for (tipo, itens) in mapeamentos {
            for item in itens {
                resultado.push(IndicadorEmpresa {
                    data_referencia,
                    tipo_indicador: tipo,
                    categoria: item.categoria.clone(),
                    codigo_categoria: item.codigo.clone(),
                    quantidade: item.quantidade,
                    percentual: item.percentual,
                    valor_medio: item.valor_medio,
                    hash_dados: hash.clone(),
                });
            }
        }

        resultado
    }

    /// Retorna a lista de itens agregados correspondente ao `TipoIndicador` informado.
    pub fn itens_por_tipo(&self, tipo: TipoIndicador) -> &[IndicadorItem] {
        match tipo {
            TipoIndicador::Porte => &self.porte,
            TipoIndicador::Cnae => &self.cnae_top15,
            TipoIndicador::Regime => &self.regime_tributario,
            TipoIndicador::Capital => &self.capital_social,
            TipoIndicador::Situacao => &self.situacao_cadastral,
            TipoIndicador::Uf => &self.uf,
            TipoIndicador::Municipio => &self.municipio,
            TipoIndicador::Natureza => &self.natureza_juridica,
        }
    }
}
