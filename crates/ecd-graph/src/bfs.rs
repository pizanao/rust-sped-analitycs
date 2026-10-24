//! Algoritmo de Busca em Largura (BFS) em camadas (1 a 10) utilizando `petgraph::graph::UnGraph`
//! com prevenção de ciclos e poda por camada (`K_LIMITE_CAMADA = 1000`).

use crate::{
    db::SqliteGraphStore,
    sanctions::{chaves_candidatas_entidade, cruzar_sancoes_em_conexao},
    ArestaGrafo, GrafoRedeResult, GraphError, NoGrafo, Result,
};
use petgraph::graph::{NodeIndex, UnGraph};
use rusqlite::{Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};

/// Limite máximo de itens (arestas/nós expandidos) por camada para evitar explosão combinatória
/// em holdings de altíssima conectividade.
pub const K_LIMITE_CAMADA: usize = 1_000;

/// Profundidade máxima permitida na expansão BFS societária.
pub const K_MAX_CAMADAS: usize = 10;

/// Grafo não-direcionado em memória construído pelo motor BFS.
pub type RedeUnGraph = UnGraph<NoGrafo, ArestaGrafo>;

#[derive(Clone, Copy)]
pub(crate) struct SchemaCapabilities {
    pub main_entidades: bool,
    pub cnpj_entidades: bool,
    pub main_empresas: bool,
    pub cnpj_empresas: bool,
    pub main_ligacao: bool,
}

impl SchemaCapabilities {
    pub(crate) fn detect(conn: &Connection) -> Self {
        Self {
            main_entidades: tabela_existe_em(conn, "main", "entidades"),
            cnpj_entidades: tabela_existe_em(conn, "cnpj_db", "entidades"),
            main_empresas: tabela_existe_em(conn, "main", "empresas"),
            cnpj_empresas: tabela_existe_em(conn, "cnpj_db", "empresas"),
            main_ligacao: tabela_existe_em(conn, "main", "ligacao"),
        }
    }
}

impl SqliteGraphStore {
    /// Expande a rede societária a partir do nó `raiz` em até `max_camadas` (limitado a 1..=10),
    /// podando cada camada em no máximo `limite_itens` (teto `K_LIMITE_CAMADA = 1000`).
    ///
    /// Utiliza internamente [`petgraph::graph::UnGraph`] e executa o cruzamento atômico de sanções
    /// na base `links`.
    pub fn buscar_camadas(
        &self,
        raiz: &str,
        max_camadas: usize,
        limite_itens: usize,
    ) -> Result<GrafoRedeResult> {
        let (graph, _node_map, camadas_alcancadas, alertas_encontrados) =
            self.construir_ungraph(raiz, max_camadas, limite_itens)?;

        let nodes: Vec<NoGrafo> = graph
            .node_indices()
            .filter_map(|idx| graph.node_weight(idx).cloned())
            .collect();

        let links: Vec<ArestaGrafo> = graph
            .edge_indices()
            .filter_map(|idx| graph.edge_weight(idx).cloned())
            .collect();

        Ok(GrafoRedeResult {
            nodes,
            links,
            total_camadas: camadas_alcancadas,
            alertas_encontrados,
        })
    }

    /// Constrói o [`petgraph::graph::UnGraph`] correspondente à expansão BFS em camadas.
    pub fn construir_ungraph(
        &self,
        raiz: &str,
        max_camadas: usize,
        limite_itens: usize,
    ) -> Result<(RedeUnGraph, HashMap<String, NodeIndex>, usize, usize)> {
        let raiz_trim = raiz.trim();
        if raiz_trim.is_empty() {
            return Err(GraphError::InvalidInput(
                "Identificador raiz não pode ser vazio".to_string(),
            ));
        }

        let camadas_limite = if max_camadas == 0 {
            0
        } else {
            max_camadas.min(K_MAX_CAMADAS)
        };

        let limite_camada = if limite_itens == 0 {
            K_LIMITE_CAMADA
        } else {
            limite_itens.min(K_LIMITE_CAMADA)
        };

        let conn = self.get_conn()?;
        let caps = SchemaCapabilities::detect(&conn);
        let raiz_id = resolver_id_entidade(&conn, raiz_trim, caps);

        let mut graph = RedeUnGraph::new_undirected();
        let mut node_map: HashMap<String, NodeIndex> = HashMap::new();
        let mut seen_edges: HashSet<(String, String, String)> = HashSet::new();

        // Insere nó raiz (Camada 0)
        let no_raiz = carregar_no(&conn, &raiz_id, caps)?;
        let raiz_idx = graph.add_node(no_raiz);
        node_map.insert(raiz_id.clone(), raiz_idx);

        if !caps.main_ligacao {
            let todos_ids: Vec<String> = node_map.keys().cloned().collect();
            let alertas = aplicar_sancoes_no_grafo(&conn, &mut graph, &todos_ids)?;
            return Ok((graph, node_map, 0, alertas));
        }

        let mut fronteira_atual = vec![raiz_id];
        let mut camadas_alcancadas = 0usize;

        let mut stmt_ligacao = conn.prepare_cached(
            "SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
             FROM ligacao
             WHERE id1 = ?1 OR id2 = ?1",
        )?;

        for camada in 1..=camadas_limite {
            if fronteira_atual.is_empty() {
                break;
            }

            let mut proxima_fronteira: Vec<String> = Vec::new();
            let mut itens_na_camada = 0usize;

            'camada_loop: for atual_id in &fronteira_atual {
                if itens_na_camada >= limite_camada {
                    break 'camada_loop;
                }

                let rows = stmt_ligacao.query_map([atual_id], |row| {
                    let id1: String = row.get(0)?;
                    let id2: String = row.get(1)?;
                    let descricao: String = row.get(2)?;
                    let comentario: String = row.get(3)?;
                    Ok((id1, id2, descricao, comentario))
                })?;

                for row in rows {
                    if itens_na_camada >= limite_camada {
                        break 'camada_loop;
                    }

                    let (id1, id2, descricao, comentario) = row?;
                    let id1_trim = id1.trim();
                    let id2_trim = id2.trim();
                    let vizinho_id = if id1_trim == atual_id {
                        id2_trim.to_string()
                    } else {
                        id1_trim.to_string()
                    };

                    // Ignora auto-loops (id1 == id2) ou identificadores vazios
                    if vizinho_id.is_empty() || vizinho_id == *atual_id {
                        continue;
                    }

                    let tipo_vinculo = classificar_tipo_vinculo(&descricao, &comentario);
                    let edge_key = chave_aresta_canonica(atual_id, &vizinho_id, &tipo_vinculo);
                    if seen_edges.contains(&edge_key) {
                        continue;
                    }

                    // Garante que cada nó visitado seja expandido no máximo uma única vez (prevenção de ciclos)
                    let vizinho_idx = if let Some(&idx) = node_map.get(&vizinho_id) {
                        idx
                    } else {
                        let novo_no = carregar_no(&conn, &vizinho_id, caps)?;
                        let idx = graph.add_node(novo_no);
                        node_map.insert(vizinho_id.clone(), idx);
                        proxima_fronteira.push(vizinho_id.clone());
                        idx
                    };

                    let atual_idx = node_map[atual_id];
                    let label = if descricao.trim().is_empty() {
                        tipo_vinculo.clone()
                    } else {
                        descricao.trim().to_string()
                    };

                    graph.add_edge(
                        atual_idx,
                        vizinho_idx,
                        ArestaGrafo {
                            source: id1_trim.to_string(),
                            target: id2_trim.to_string(),
                            label,
                            tipo_vinculo,
                        },
                    );

                    seen_edges.insert(edge_key);
                    itens_na_camada += 1;
                }
            }

            if !proxima_fronteira.is_empty() {
                camadas_alcancadas = camada;
            }
            fronteira_atual = proxima_fronteira;
        }

        drop(stmt_ligacao);

        // Cruzamento atômico de sanções para todos os nós presentes no subgrafo
        let todos_ids: Vec<String> = node_map.keys().cloned().collect();
        let alertas_encontrados = aplicar_sancoes_no_grafo(&conn, &mut graph, &todos_ids)?;

        Ok((graph, node_map, camadas_alcancadas, alertas_encontrados))
    }
}

pub(crate) fn aplicar_sancoes_no_grafo(
    conn: &Connection,
    graph: &mut RedeUnGraph,
    todos_ids: &[String],
) -> Result<usize> {
    let mapa_sancoes = cruzar_sancoes_em_conexao(conn, todos_ids)?;
    let mut alertas_encontrados = 0usize;
    for idx in graph.node_indices() {
        if let Some(node) = graph.node_weight_mut(idx) {
            if let Some(flags) = mapa_sancoes.get(&node.id) {
                alertas_encontrados += flags.len();
                node.flags_sancionatorias = flags.clone();
                node.cor = Some("#ef4444".to_string());
            }
        }
    }
    Ok(alertas_encontrados)
}

fn tabela_existe_em(conn: &Connection, schema: &str, nome_tabela: &str) -> bool {
    let sql = format!(
        "SELECT EXISTS(SELECT 1 FROM {schema}.sqlite_master WHERE type='table' AND name=?1)"
    );
    conn.query_row(&sql, [nome_tabela], |row| row.get(0))
        .unwrap_or(false)
}

/// Resolve um identificador de entrada (ex: CNPJ puro `"11111111000191"`, formatado `"11.111.111/0001-91"`
/// ou prefixado `"PJ_11111111000191"`) para a chave canônica efetivamente armazenada em `ligacao` / `entidades`.
pub(crate) fn resolver_id_entidade(
    conn: &Connection,
    raw_id: &str,
    caps: SchemaCapabilities,
) -> String {
    let candidates = chaves_candidatas_entidade(raw_id);
    if candidates.is_empty() {
        return raw_id.trim().to_string();
    }

    for candidate in &candidates {
        if caps.main_ligacao {
            let exists_in_ligacao: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM ligacao WHERE id1 = ?1 OR id2 = ?1)",
                    [candidate],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if exists_in_ligacao {
                return candidate.clone();
            }
        }
        if caps.main_entidades {
            let exists_in_entidades: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM main.entidades WHERE id = ?1)",
                    [candidate],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if exists_in_entidades {
                return candidate.clone();
            }
        }
        if caps.cnpj_entidades {
            let exists_in_cnpj_entidades: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM cnpj_db.entidades WHERE id = ?1)",
                    [candidate],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if exists_in_cnpj_entidades {
                return candidate.clone();
            }
        }
    }

    candidates[0].clone()
}

pub(crate) fn carregar_no(
    conn: &Connection,
    node_id: &str,
    caps: SchemaCapabilities,
) -> Result<NoGrafo> {
    let candidates = chaves_candidatas_entidade(node_id);

    for schema in [
        caps.main_entidades.then_some("main"),
        caps.cnpj_entidades.then_some("cnpj_db"),
    ]
    .into_iter()
    .flatten()
    {
        let sql = format!("SELECT nome, tipo FROM {schema}.entidades WHERE id = ?1");
        let mut stmt = conn.prepare_cached(&sql)?;
        for key in &candidates {
            let meta: Option<(String, String)> = stmt
                .query_row([key], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()?;
            if let Some((nome, tipo)) = meta {
                let tipo_norm = normalizar_tipo_no(&tipo, node_id);
                let cor = Some(cor_padrao_por_tipo(&tipo_norm).to_string());
                return Ok(NoGrafo {
                    id: node_id.to_string(),
                    label: nome,
                    tipo: tipo_norm,
                    cor,
                    flags_sancionatorias: Vec::new(),
                });
            }
        }
    }

    // Fallback para tabela `empresas` (em `main` ou `cnpj_db` anexado) caso exista
    for schema in [
        caps.main_empresas.then_some("main"),
        caps.cnpj_empresas.then_some("cnpj_db"),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(razao_social) = buscar_razao_social_empresas(conn, schema, &candidates) {
            let (_, tipo_inferido) = inferir_label_e_tipo(node_id);
            let cor = Some(cor_padrao_por_tipo(&tipo_inferido).to_string());
            return Ok(NoGrafo {
                id: node_id.to_string(),
                label: razao_social,
                tipo: tipo_inferido,
                cor,
                flags_sancionatorias: Vec::new(),
            });
        }
    }

    let (label, tipo) = inferir_label_e_tipo(node_id);
    let cor = Some(cor_padrao_por_tipo(&tipo).to_string());

    Ok(NoGrafo {
        id: node_id.to_string(),
        label,
        tipo,
        cor,
        flags_sancionatorias: Vec::new(),
    })
}

fn buscar_razao_social_empresas(
    conn: &Connection,
    schema: &str,
    candidates: &[String],
) -> Option<String> {
    let sql_cnpj_basico =
        format!("SELECT razao_social FROM {schema}.empresas WHERE cnpj_basico = ?1 LIMIT 1");
    for key in candidates {
        let digits: String = key.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() == 14 {
            let basico = &digits[0..8];
            if let Ok(Some(razao)) = conn
                .query_row(&sql_cnpj_basico, [basico], |row| row.get::<_, String>(0))
                .optional()
            {
                if !razao.trim().is_empty() {
                    return Some(razao);
                }
            }
        }
    }
    None
}

fn inferir_label_e_tipo(node_id: &str) -> (String, String) {
    let trimmed = node_id.trim();
    if let Some(rest) = trimmed.strip_prefix("FILIAL_") {
        return (rest.to_string(), "FILIAL".to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("PF_") {
        return (rest.to_string(), "PF".to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("PJ_") {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.len() == 14 && &digits[8..12] != "0001" {
            return (rest.to_string(), "FILIAL".to_string());
        }
        return (rest.to_string(), "PJ".to_string());
    }

    if trimmed.len() == 11 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return (trimmed.to_string(), "PF".to_string());
    }
    if trimmed.len() == 14 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        if &trimmed[8..12] != "0001" {
            return (trimmed.to_string(), "FILIAL".to_string());
        }
        return (trimmed.to_string(), "PJ".to_string());
    }

    (trimmed.to_string(), "PJ".to_string())
}

fn normalizar_tipo_no(tipo: &str, node_id: &str) -> String {
    match tipo.trim().to_uppercase().as_str() {
        "PF" | "SOCIO_PF" | "PESSOA_FISICA" => "PF".to_string(),
        "FILIAL" => "FILIAL".to_string(),
        "PJ" | "EMPRESA" | "PESSOA_JURIDICA" => "PJ".to_string(),
        _ => inferir_label_e_tipo(node_id).1,
    }
}

fn cor_padrao_por_tipo(tipo: &str) -> &'static str {
    match tipo {
        "PF" => "#10b981",
        "FILIAL" => "#6366f1",
        _ => "#3b82f6",
    }
}

pub(crate) fn classificar_tipo_vinculo(descricao: &str, comentario: &str) -> String {
    let combined = format!("{} {}", descricao.to_uppercase(), comentario.to_uppercase());
    if combined.contains("FILIAL") || combined.contains("MATRIZ") {
        "MATRIZ_FILIAL".to_string()
    } else if combined.contains("PROCURADOR") || combined.contains("REPRESENTANTE") {
        "PROCURADOR".to_string()
    } else {
        "SOCIO".to_string()
    }
}

pub(crate) fn chave_aresta_canonica(
    a: &str,
    b: &str,
    tipo_vinculo: &str,
) -> (String, String, String) {
    if a <= b {
        (a.to_string(), b.to_string(), tipo_vinculo.to_string())
    } else {
        (b.to_string(), a.to_string(), tipo_vinculo.to_string())
    }
}
