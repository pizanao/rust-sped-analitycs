//! Handlers HTTP da Rede Societária GRC (`/api/grc/rede` e `/api/grc/caminho`)
//! sobre o motor de grafos `ecd-graph` (`SqliteGraphStore` + `petgraph`).

use crate::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use ecd_graph::{ArestaGrafo, NoGrafo, K_LIMITE_CAMADA, K_MAX_CAMADAS};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Deserialize)]
pub struct RedeQuery {
    pub raiz: String,
    pub camadas: Option<usize>,
    pub limite: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CaminhoQuery {
    pub origem: String,
    pub destino: String,
    pub camadas: Option<usize>,
}

/// Nó enriquecido compatível tanto com `ecd_graph::NoGrafo` quanto com `GrafoNode` do Vis-Network React.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrafoNodeDto {
    pub id: String,
    pub label: String,
    pub nome: String,
    pub tipo: String,
    pub cpf_cnpj: String,
    pub cor: Option<String>,
    pub flags_sancionatorias: Vec<String>,
    pub sancoes: Vec<String>,
}

impl From<&NoGrafo> for GrafoNodeDto {
    fn from(n: &NoGrafo) -> Self {
        let tipo_vis = if n.tipo == "PF" { "PF" } else { "PJ" }.to_string();
        Self {
            id: n.id.clone(),
            label: n.label.clone(),
            nome: n.label.clone(),
            tipo: tipo_vis,
            cpf_cnpj: n.id.clone(),
            cor: n.cor.clone(),
            flags_sancionatorias: n.flags_sancionatorias.clone(),
            sancoes: n.flags_sancionatorias.clone(),
        }
    }
}

/// Aresta enriquecida compatível tanto com `ecd_graph::ArestaGrafo` quanto com `GrafoEdge` do Vis-Network React.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrafoEdgeDto {
    pub id: String,
    pub source: String,
    pub target: String,
    pub from: String,
    pub to: String,
    pub label: String,
    pub tipo_vinculo: String,
    pub qualificacao: String,
}

impl GrafoEdgeDto {
    pub fn from_aresta(idx: usize, a: &ArestaGrafo) -> Self {
        Self {
            id: format!("e-{idx}"),
            source: a.source.clone(),
            target: a.target.clone(),
            from: a.source.clone(),
            to: a.target.clone(),
            label: a.label.clone(),
            tipo_vinculo: a.tipo_vinculo.clone(),
            qualificacao: a.label.clone(),
        }
    }
}

/// `GET /api/grc/rede?raiz=...&camadas=...&limite=...`
/// Expande a rede societária em BFS (1 a 10 camadas) e cruza sanções (`PEP`, `CEIS`, `CEPIM`, `CNEP`, `PGFN`).
pub async fn buscar_rede_handler(
    State(state): State<AppState>,
    Query(query): Query<RedeQuery>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let raiz_trim = query.raiz.trim().to_string();
    if raiz_trim.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "erro": "O parâmetro 'raiz' (CNPJ ou CPF) é obrigatório.",
            })),
        ));
    }

    let camadas = query.camadas.unwrap_or(2).clamp(1, K_MAX_CAMADAS);
    let limite = query
        .limite
        .unwrap_or(K_LIMITE_CAMADA)
        .clamp(1, K_LIMITE_CAMADA);

    let store = state.graph_store.clone();
    let raiz_for_query = raiz_trim.clone();

    let resultado =
        tokio::task::spawn_blocking(move || store.buscar_camadas(&raiz_for_query, camadas, limite))
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "erro": format!("Falha na thread de consulta de grafos: {e}"),
                    })),
                )
            })?
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "erro": e.to_string(),
                    })),
                )
            })?;

    let nos: Vec<GrafoNodeDto> = resultado.nodes.iter().map(GrafoNodeDto::from).collect();
    let arestas: Vec<GrafoEdgeDto> = resultado
        .links
        .iter()
        .enumerate()
        .map(|(i, a)| GrafoEdgeDto::from_aresta(i, a))
        .collect();

    Ok((
        StatusCode::OK,
        Json(json!({
            "raiz": raiz_trim,
            "camadas": resultado.total_camadas,
            "total_camadas": resultado.total_camadas,
            "total_nos": nos.len(),
            "total_arestas": arestas.len(),
            "alertas_sancoes": resultado.alertas_encontrados,
            "alertas_encontrados": resultado.alertas_encontrados,
            "nodes": resultado.nodes,
            "links": resultado.links,
            "nos": nos,
            "arestas": arestas,
        })),
    )
        .into_response())
}

/// `GET /api/grc/caminho?origem=...&destino=...&camadas=...`
/// Calcula o menor caminho societário entre duas entidades via algoritmo de Dijkstra (`petgraph`).
pub async fn buscar_caminho_handler(
    State(state): State<AppState>,
    Query(query): Query<CaminhoQuery>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let origem_trim = query.origem.trim().to_string();
    let destino_trim = query.destino.trim().to_string();
    if origem_trim.is_empty() || destino_trim.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "erro": "Os parâmetros 'origem' e 'destino' são obrigatórios.",
            })),
        ));
    }

    let camadas = query
        .camadas
        .unwrap_or(K_MAX_CAMADAS)
        .clamp(1, K_MAX_CAMADAS);
    let store = state.graph_store.clone();
    let orig_q = origem_trim.clone();
    let dest_q = destino_trim.clone();

    let caminho_opt =
        tokio::task::spawn_blocking(move || store.buscar_caminho_minimo(&orig_q, &dest_q, camadas))
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "erro": format!("Falha na thread de busca de caminho mínimo: {e}"),
                    })),
                )
            })?
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "erro": e.to_string(),
                    })),
                )
            })?;

    match caminho_opt {
        Some(res) => {
            let caminho_nos: Vec<GrafoNodeDto> = res.nodes.iter().map(GrafoNodeDto::from).collect();
            let caminho_arestas: Vec<GrafoEdgeDto> = res
                .links
                .iter()
                .enumerate()
                .map(|(i, a)| GrafoEdgeDto::from_aresta(i, a))
                .collect();

            Ok((
                StatusCode::OK,
                Json(json!({
                    "encontrado": true,
                    "origem": res.origem,
                    "destino": res.destino,
                    "distancia": res.distancia,
                    "nodes": res.nodes,
                    "links": res.links,
                    "caminho_nos": caminho_nos,
                    "caminho_arestas": caminho_arestas,
                })),
            )
                .into_response())
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "encontrado": false,
                "origem": origem_trim,
                "destino": destino_trim,
                "erro": "Nenhum caminho societário encontrado entre as entidades dentro do limite de camadas.",
            })),
        )),
    }
}
