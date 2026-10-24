//! # ecd-graph
//!
//! Motor de inteligência societária e análise de grafos de relacionamento corporativo
//! sobre as bases SQLite locais da Rede CNPJ / Turmalina com algoritmo BFS e cruzamento de sanções.

pub mod bfs;
pub mod db;
pub mod sanctions;
pub mod shortest_path;

pub use bfs::{RedeUnGraph, K_LIMITE_CAMADA, K_MAX_CAMADAS};
pub use db::{
    PragmaStatus, SqliteGraphConfig, SqliteGraphStore, DEFAULT_CACHE_SIZE, DEFAULT_MMAP_SIZE,
    DEFAULT_POOL_MAX_SIZE,
};
pub use sanctions::{SanctionRecord, SanctionType};
pub use shortest_path::{calcular_caminho_minimo_ungraph, CaminhoMinimoResult};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Erros do motor de grafos societários (`ecd-graph`).
#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Erro no banco SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Erro no pool de conexões SQLite (r2d2): {0}")]
    Pool(#[from] r2d2::Error),

    #[error("Parâmetro de entrada inválido: {0}")]
    InvalidInput(String),
}

/// Alias padrão de resultado para operações de grafo.
pub type Result<T> = std::result::Result<T, GraphError>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoGrafo {
    pub id: String,
    pub label: String,
    pub tipo: String, // "PJ", "PF", "FILIAL"
    pub cor: Option<String>,
    pub flags_sancionatorias: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArestaGrafo {
    pub source: String,
    pub target: String,
    pub label: String,
    pub tipo_vinculo: String, // "SOCIO", "MATRIZ_FILIAL", "PROCURADOR"
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrafoRedeResult {
    pub nodes: Vec<NoGrafo>,
    pub links: Vec<ArestaGrafo>,
    pub total_camadas: usize,
    pub alertas_encontrados: usize,
}
