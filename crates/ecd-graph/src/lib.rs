//! # ecd-graph
//!
//! Motor de inteligência societária e análise de grafos de relacionamento corporativo
//! sobre as bases SQLite locais da Rede CNPJ / Turmalina com algoritmo BFS e cruzamento de sanções.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoGrafo {
    pub id: String,
    pub label: String,
    pub tipo: String, // "PJ", "PF", "FILIAL"
    pub cor: Option<String>,
    pub flags_sancionatorias: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArestaGrafo {
    pub source: String,
    pub target: String,
    pub label: String,
    pub tipo_vinculo: String, // "SOCIO", "MATRIZ_FILIAL", "PROCURADOR"
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrafoRedeResult {
    pub nodes: Vec<NoGrafo>,
    pub links: Vec<ArestaGrafo>,
    pub total_camadas: usize,
    pub alertas_encontrados: usize,
}
