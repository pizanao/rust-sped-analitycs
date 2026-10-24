//! # ecd-analytics
//!
//! Agregações analíticas e séries temporais de indicadores de empresas
//! (CNAE, porte, regime tributário, capital social) com cache em memória `moka`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndicadorItem {
    pub categoria: String,
    pub codigo: Option<String>,
    pub quantidade: i64,
    pub percentual: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndicadoresResumo {
    pub porte: Vec<IndicadorItem>,
    pub cnae_top15: Vec<IndicadorItem>,
    pub regime_tributario: Vec<IndicadorItem>,
    pub total_empresas: i64,
}
