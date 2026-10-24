//! Cálculo de menor caminho societário entre duas entidades corporativas (`PJ`) ou sócios (`PF`)
//! utilizando `petgraph::algo::dijkstra`.

use crate::{
    bfs::{
        aplicar_sancoes_no_grafo, carregar_no, chave_aresta_canonica, classificar_tipo_vinculo,
        resolver_id_entidade, RedeUnGraph, SchemaCapabilities, K_LIMITE_CAMADA, K_MAX_CAMADAS,
    },
    db::SqliteGraphStore,
    sanctions::chaves_candidatas_entidade,
    ArestaGrafo, GraphError, NoGrafo, Result,
};
use petgraph::{algo::dijkstra, graph::NodeIndex, visit::EdgeRef};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Resultado da busca de caminho mínimo entre duas entidades no grafo societário.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaminhoMinimoResult {
    pub origem: String,
    pub destino: String,
    pub distancia: usize,
    pub nodes: Vec<NoGrafo>,
    pub links: Vec<ArestaGrafo>,
}

impl SqliteGraphStore {
    /// Calcula o menor caminho societário entre `origem` e `destino` expandindo até `max_camadas`
    /// (limitado a `1..=10`) e executando [`petgraph::algo::dijkstra`].
    ///
    /// Retorna `Ok(None)` caso não exista caminho conectando as duas entidades dentro do raio pesquisado.
    pub fn buscar_caminho_minimo(
        &self,
        origem: &str,
        destino: &str,
        max_camadas: usize,
    ) -> Result<Option<CaminhoMinimoResult>> {
        let orig_trim = origem.trim();
        let dest_trim = destino.trim();
        if orig_trim.is_empty() || dest_trim.is_empty() {
            return Err(GraphError::InvalidInput(
                "Identificadores de origem e destino não podem ser vazios".to_string(),
            ));
        }

        let camadas = if max_camadas == 0 {
            K_MAX_CAMADAS
        } else {
            max_camadas.min(K_MAX_CAMADAS)
        };

        let conn = self.get_conn()?;
        let caps = SchemaCapabilities::detect(&conn);
        let orig_id = resolver_id_entidade(&conn, orig_trim, caps);
        let dest_id = resolver_id_entidade(&conn, dest_trim, caps);

        if orig_id == dest_id {
            let mut graph = RedeUnGraph::new_undirected();
            let mut node_map = HashMap::new();
            let root = carregar_no(&conn, &orig_id, caps)?;
            let idx = graph.add_node(root);
            node_map.insert(orig_id.clone(), idx);
            let todos = vec![orig_id.clone()];
            aplicar_sancoes_no_grafo(&conn, &mut graph, &todos)?;
            return Ok(calcular_caminho_minimo_ungraph(
                &graph, &node_map, &orig_id, &dest_id,
            ));
        }

        // Constrói o UnGraph a partir de orig_id e mescla a expansão reversa a partir de dest_id
        // além de pontes diretas (1 e 2 saltos), prevenindo poda prematura em mega-holdings (>1.000 sócios).
        let (mut graph, mut node_map, _c, _a) =
            self.construir_ungraph(&orig_id, camadas, K_LIMITE_CAMADA)?;

        if caps.main_ligacao {
            mesclar_pontes_e_expansao_reversa(
                &conn,
                caps,
                &mut graph,
                &mut node_map,
                &orig_id,
                &dest_id,
                camadas,
            )?;

            let todos_ids: Vec<String> = node_map.keys().cloned().collect();
            aplicar_sancoes_no_grafo(&conn, &mut graph, &todos_ids)?;
        }

        Ok(calcular_caminho_minimo_ungraph(
            &graph, &node_map, &orig_id, &dest_id,
        ))
    }
}

fn mesclar_pontes_e_expansao_reversa(
    conn: &Connection,
    caps: SchemaCapabilities,
    graph: &mut RedeUnGraph,
    node_map: &mut HashMap<String, NodeIndex>,
    orig_id: &str,
    dest_id: &str,
    max_camadas: usize,
) -> Result<()> {
    let mut seen_edges: HashSet<(String, String, String)> = HashSet::new();
    for edge_ref in graph.edge_references() {
        let w = edge_ref.weight();
        seen_edges.insert(chave_aresta_canonica(&w.source, &w.target, &w.tipo_vinculo));
    }

    // 1. Verifica ligação direta (1 salto) entre orig_id e dest_id mesmo que haja >1000 vizinhos
    if max_camadas >= 1 {
        let mut stmt_direto = conn.prepare_cached(
            "SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
             FROM ligacao
             WHERE (id1 = ?1 AND id2 = ?2) OR (id1 = ?2 AND id2 = ?1)
             LIMIT 4",
        )?;
        let rows = stmt_direto.query_map([orig_id, dest_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (id1, id2, desc, com) = row?;
            inserir_aresta_no_grafo(
                conn,
                caps,
                graph,
                node_map,
                &mut seen_edges,
                id1.trim(),
                id2.trim(),
                &desc,
                &com,
            )?;
        }
    }

    // 2. Verifica pontes de 2 saltos (sócio/empresa em comum) entre orig_id e dest_id
    if max_camadas >= 2 {
        let mut stmt_ponte = conn.prepare_cached(
            "SELECT COALESCE(l1.id1, ''), COALESCE(l1.id2, ''), COALESCE(l1.descricao, ''), COALESCE(l1.comentario, ''),
                    COALESCE(l2.id1, ''), COALESCE(l2.id2, ''), COALESCE(l2.descricao, ''), COALESCE(l2.comentario, '')
             FROM ligacao l1
             JOIN ligacao l2
               ON (CASE WHEN l1.id1 = ?1 THEN l1.id2 ELSE l1.id1 END)
                = (CASE WHEN l2.id1 = ?2 THEN l2.id2 ELSE l2.id1 END)
             WHERE (l1.id1 = ?1 OR l1.id2 = ?1)
               AND (l2.id1 = ?2 OR l2.id2 = ?2)
             LIMIT 16",
        )?;
        let rows = stmt_ponte.query_map([orig_id, dest_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?;
        for row in rows {
            let (a1, a2, d1, c1, b1, b2, d2, c2) = row?;
            inserir_aresta_no_grafo(
                conn,
                caps,
                graph,
                node_map,
                &mut seen_edges,
                a1.trim(),
                a2.trim(),
                &d1,
                &c1,
            )?;
            inserir_aresta_no_grafo(
                conn,
                caps,
                graph,
                node_map,
                &mut seen_edges,
                b1.trim(),
                b2.trim(),
                &d2,
                &c2,
            )?;
        }
    }

    // 3. Expansão BFS reversa a partir de dest_id para conectar ao subgrafo de orig_id
    let camadas_reversas = max_camadas.div_ceil(2).max(1);
    let mut visitados_reverso: HashSet<String> = HashSet::new();
    visitados_reverso.insert(dest_id.to_string());
    let mut fronteira_reversa = vec![dest_id.to_string()];

    let mut stmt_ligacao = conn.prepare_cached(
        "SELECT COALESCE(id1, ''), COALESCE(id2, ''), COALESCE(descricao, ''), COALESCE(comentario, '')
         FROM ligacao
         WHERE id1 = ?1 OR id2 = ?1",
    )?;

    for _ in 1..=camadas_reversas {
        if fronteira_reversa.is_empty() {
            break;
        }
        let mut proxima_reversa = Vec::new();
        let mut itens = 0usize;

        'rev_loop: for atual_id in &fronteira_reversa {
            if itens >= K_LIMITE_CAMADA {
                break 'rev_loop;
            }
            let rows = stmt_ligacao.query_map([atual_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;

            for row in rows {
                if itens >= K_LIMITE_CAMADA {
                    break 'rev_loop;
                }
                let (id1, id2, desc, com) = row?;
                let id1_trim = id1.trim();
                let id2_trim = id2.trim();
                let vizinho_id = if id1_trim == atual_id {
                    id2_trim.to_string()
                } else {
                    id1_trim.to_string()
                };
                if vizinho_id.is_empty() || vizinho_id == *atual_id {
                    continue;
                }

                let inseriu = inserir_aresta_no_grafo(
                    conn,
                    caps,
                    graph,
                    node_map,
                    &mut seen_edges,
                    id1_trim,
                    id2_trim,
                    &desc,
                    &com,
                )?;
                if visitados_reverso.insert(vizinho_id.clone()) {
                    proxima_reversa.push(vizinho_id);
                }
                if inseriu {
                    itens += 1;
                }
            }
        }
        fronteira_reversa = proxima_reversa;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn inserir_aresta_no_grafo(
    conn: &Connection,
    caps: SchemaCapabilities,
    graph: &mut RedeUnGraph,
    node_map: &mut HashMap<String, NodeIndex>,
    seen_edges: &mut HashSet<(String, String, String)>,
    id1: &str,
    id2: &str,
    descricao: &str,
    comentario: &str,
) -> Result<bool> {
    if id1.is_empty() || id2.is_empty() || id1 == id2 {
        return Ok(false);
    }

    let tipo_vinculo = classificar_tipo_vinculo(descricao, comentario);
    let edge_key = chave_aresta_canonica(id1, id2, &tipo_vinculo);
    if seen_edges.contains(&edge_key) {
        return Ok(false);
    }

    let idx1 = if let Some(&idx) = node_map.get(id1) {
        idx
    } else {
        let n = carregar_no(conn, id1, caps)?;
        let idx = graph.add_node(n);
        node_map.insert(id1.to_string(), idx);
        idx
    };

    let idx2 = if let Some(&idx) = node_map.get(id2) {
        idx
    } else {
        let n = carregar_no(conn, id2, caps)?;
        let idx = graph.add_node(n);
        node_map.insert(id2.to_string(), idx);
        idx
    };

    let label = if descricao.trim().is_empty() {
        tipo_vinculo.clone()
    } else {
        descricao.trim().to_string()
    };

    graph.add_edge(
        idx1,
        idx2,
        ArestaGrafo {
            source: id1.to_string(),
            target: id2.to_string(),
            label,
            tipo_vinculo,
        },
    );
    seen_edges.insert(edge_key);
    Ok(true)
}

fn resolver_indice_no_mapa(
    node_map: &HashMap<String, NodeIndex>,
    raw_id: &str,
) -> Option<(String, NodeIndex)> {
    for key in chaves_candidatas_entidade(raw_id) {
        if let Some(&idx) = node_map.get(&key) {
            return Some((key, idx));
        }
    }
    None
}

/// Executa `petgraph::algo::dijkstra` sobre um [`RedeUnGraph`] já carregado em memória
/// e reconstrói a sequência ordenada de nós e arestas do caminho mais curto.
pub fn calcular_caminho_minimo_ungraph(
    graph: &RedeUnGraph,
    node_map: &HashMap<String, NodeIndex>,
    origem: &str,
    destino: &str,
) -> Option<CaminhoMinimoResult> {
    let (orig, start_idx) = resolver_indice_no_mapa(node_map, origem)?;
    let (dest, end_idx) = resolver_indice_no_mapa(node_map, destino)?;

    if start_idx == end_idx {
        let root_node = graph.node_weight(start_idx)?.clone();
        return Some(CaminhoMinimoResult {
            origem: orig,
            destino: dest,
            distancia: 0,
            nodes: vec![root_node],
            links: Vec::new(),
        });
    }

    // Calcula distâncias mínimas a partir de start_idx usando Dijkstra com peso unitário por vínculo
    let distancias = dijkstra(graph, start_idx, Some(end_idx), |_| 1usize);
    let &distancia_total = distancias.get(&end_idx)?;

    // Reconstrói o caminho mínimo de end_idx até start_idx usando o mapa de distâncias de Dijkstra
    let mut path_nodes_rev = Vec::with_capacity(distancia_total + 1);
    let mut path_edges_rev = Vec::with_capacity(distancia_total);

    let mut curr_idx = end_idx;
    let mut curr_dist = distancia_total;
    path_nodes_rev.push(graph.node_weight(curr_idx)?.clone());

    while curr_idx != start_idx && curr_dist > 0 {
        let mut next_step: Option<(NodeIndex, ArestaGrafo)> = None;

        for edge_ref in graph.edges(curr_idx) {
            let neighbor_idx = if edge_ref.source() == curr_idx {
                edge_ref.target()
            } else {
                edge_ref.source()
            };

            if let Some(&d) = distancias.get(&neighbor_idx) {
                if d + 1 == curr_dist {
                    next_step = Some((neighbor_idx, edge_ref.weight().clone()));
                    break;
                }
            }
        }

        let (prev_idx, edge_weight) = next_step?;
        path_edges_rev.push(edge_weight);
        path_nodes_rev.push(graph.node_weight(prev_idx)?.clone());

        curr_idx = prev_idx;
        curr_dist -= 1;
    }

    path_nodes_rev.reverse();
    path_edges_rev.reverse();

    Some(CaminhoMinimoResult {
        origem: orig,
        destino: dest,
        distancia: distancia_total,
        nodes: path_nodes_rev,
        links: path_edges_rev,
    })
}
