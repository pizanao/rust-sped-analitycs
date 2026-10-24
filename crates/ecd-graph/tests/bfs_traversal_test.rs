use ecd_graph::{SanctionType, SqliteGraphStore, K_LIMITE_CAMADA};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temp_db_path(prefix: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("ecd_graph_bfs_{prefix}_{pid}_{seq}.db"))
}

/// Fixture oficial com 3 empresas (PJ_E1, PJ_E2, PJ_E3) e 4 sócios (PF_S1, PF_S2, PF_S3, PF_S4),
/// contendo ciclos societários (triângulo E1 - S2 - E2 - S1 - E1) e sanções variadas.
fn criar_fixture_3_empresas_4_socios(db_path: &PathBuf) -> SqliteGraphStore {
    let _ = std::fs::remove_file(db_path);
    let conn = Connection::open(db_path).expect("Falha ao criar SQLite de fixture");
    SqliteGraphStore::init_schema(&conn).expect("Falha ao criar tabelas da fixture");

    // 3 Empresas
    let entidades = [
        ("PJ_11111111000191", "EMPRESA ALPHA HOLDING S.A.", "PJ"),
        ("PJ_22222222000191", "EMPRESA BETA LOGISTICA LTDA", "PJ"),
        ("PJ_33333333000191", "EMPRESA GAMMA SERVICOS S.A.", "PJ"),
        // 4 Sócios
        ("PF_10000000101", "CARLOS EDUARDO SILVA", "PF"),
        ("PF_20000000202", "MARIA FERNANDA COSTA", "PF"),
        ("PF_30000000303", "ROBERTO ALMEIDA SANTOS", "PF"),
        ("PF_40000000404", "PATRICIA OLIVEIRA LIMA", "PF"),
    ];
    for (id, nome, tipo) in entidades {
        conn.execute(
            "INSERT INTO entidades (id, nome, tipo) VALUES (?1, ?2, ?3)",
            [id, nome, tipo],
        )
        .unwrap();
    }

    // Vínculos societários formando grafo conexo com ciclos e auto-loop:
    // Camada 1 a partir de PJ_11111111000191: PF_10000000101, PF_20000000202, PJ_22222222000191 (participação PJ)
    // Camada 2: PF_30000000303 (sócio de E2), PJ_33333333000191 (controlada por PF_20000000202)
    // Camada 3: PF_40000000404 (sócio de E3)
    let ligacoes = [
        (
            "PJ_11111111000191",
            "PF_10000000101",
            "Sócio-Administrador",
            "SOCIO",
        ),
        (
            "PF_10000000101",
            "PJ_11111111000191",
            "Sócio-Administrador",
            "SOCIO",
        ), // duplicata reversa para testar deduplicação
        (
            "PJ_11111111000191",
            "PJ_11111111000191",
            "Auto-vinculo invalido",
            "SOCIO",
        ), // auto-loop para testar filtro
        ("PJ_11111111000191", "PF_20000000202", "Sócio", "SOCIO"),
        (
            "PJ_11111111000191",
            "PJ_22222222000191",
            "Matriz-Filial",
            "MATRIZ_FILIAL",
        ),
        ("PF_20000000202", "PJ_22222222000191", "Sócio", "SOCIO"), // fecha ciclo triangular E1 - PF_2 - E2 - E1
        (
            "PJ_22222222000191",
            "PF_30000000303",
            "Procurador",
            "PROCURADOR",
        ),
        (
            "PF_20000000202",
            "PJ_33333333000191",
            "Sócio-Administrador",
            "SOCIO",
        ),
        ("PJ_33333333000191", "PF_40000000404", "Sócio", "SOCIO"),
    ];
    for (id1, id2, desc, com) in ligacoes {
        conn.execute(
            "INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES (?1, ?2, ?3, ?4)",
            [id1, id2, desc, com],
        )
        .unwrap();
    }

    // Sanções na tabela links cobrindo PEP, CEIS, CEPIM, CNEP e PGFN
    let sancoes = [
        (
            "PF_20000000202",
            "PEP_2025",
            "PEP",
            "Pessoa Politicamente Exposta",
        ),
        ("PJ_22222222000191", "CEIS_01", "CEIS", "Empresa Inidônea"),
        (
            "PJ_22222222000191",
            "PGFN_99",
            "Dívida PGFN",
            "Inscrita na DAU",
        ),
        (
            "PJ_33333333000191",
            "CNEP_77",
            "CNEP",
            "Acordo de Leniência",
        ),
        ("PF_40000000404", "CEPIM_12", "CEPIM", "Entidade Impedida"),
    ];
    for (id1, id2, desc, com) in sancoes {
        conn.execute(
            "INSERT INTO links (id1, id2, descricao, comentario) VALUES (?1, ?2, ?3, ?4)",
            [id1, id2, desc, com],
        )
        .unwrap();
    }

    drop(conn);
    SqliteGraphStore::new(db_path).expect("Falha ao abrir SqliteGraphStore")
}

#[test]
fn test_bfs_3_empresas_4_socios_camadas_e_prevencao_ciclos() {
    let db_path = temp_db_path("fixture_3e_4s");
    let store = criar_fixture_3_empresas_4_socios(&db_path);

    // 1 Camada a partir de PJ_11111111000191: deve trazer raiz + 3 vizinhos diretos (4 nós, 3 arestas únicas)
    let c1 = store
        .buscar_camadas("PJ_11111111000191", 1, K_LIMITE_CAMADA)
        .expect("Falha no BFS de 1 camada");
    assert_eq!(c1.total_camadas, 1);
    assert_eq!(c1.nodes.len(), 4);
    assert_eq!(c1.links.len(), 3);

    // 3 Camadas a partir de PJ_11111111000191: deve alcançar todas as 3 empresas e 4 sócios (7 nós) sem entrar em loop infinito
    let c3 = store
        .buscar_camadas("PJ_11111111000191", 5, K_LIMITE_CAMADA)
        .expect("Falha no BFS completo");
    assert_eq!(
        c3.nodes.len(),
        7,
        "Grafo completo deve conter exatamente 3 empresas + 4 sócios sem duplicatas"
    );
    assert_eq!(
        c3.links.len(),
        7,
        "Arestas duplicadas reversas e auto-loops devem ser ignorados"
    );
    assert_eq!(c3.total_camadas, 3);

    // Verifica cruzamento atômico de sanções (PEP, CEIS, PGFN, CNEP, CEPIM = 5 alertas no total)
    assert_eq!(c3.alertas_encontrados, 5);

    let e2 = c3
        .nodes
        .iter()
        .find(|n| n.id == "PJ_22222222000191")
        .unwrap();
    assert_eq!(
        e2.flags_sancionatorias,
        vec!["CEIS".to_string(), "PGFN".to_string()]
    );
    assert_eq!(e2.cor.as_deref(), Some("#ef4444"));

    // Consulta individual detalhada de sanções (com e sem prefixo/máscara)
    let sancoes_e2 = store.consultar_sancoes("22.222.222/0001-91").unwrap();
    assert_eq!(sancoes_e2.len(), 2);
    assert!(sancoes_e2
        .iter()
        .any(|s| s.tipo_sancao == SanctionType::Ceis));
    assert!(sancoes_e2
        .iter()
        .any(|s| s.tipo_sancao == SanctionType::Pgfn));

    // Busca em camadas usando CNPJ puro ou formatado sem prefixo PJ_
    let c_sem_prefixo = store
        .buscar_camadas("11.111.111/0001-91", 5, K_LIMITE_CAMADA)
        .unwrap();
    assert_eq!(c_sem_prefixo.nodes.len(), 7);
    assert_eq!(c_sem_prefixo.links.len(), 7);

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_bfs_poda_limite_camada_holding_1200_socios() {
    let db_path = temp_db_path("poda_1200");
    let _ = std::fs::remove_file(&db_path);

    {
        let conn = Connection::open(&db_path).unwrap();
        SqliteGraphStore::init_schema(&conn).unwrap();

        let tx = conn.unchecked_transaction().unwrap();
        tx.execute(
            "INSERT INTO entidades (id, nome, tipo) VALUES ('PJ_HOLDING', 'MEGA HOLDING S.A.', 'PJ')",
            [],
        )
        .unwrap();

        for i in 1..=1200 {
            let socio_id = format!("PF_SOCIO_{i:04}");
            tx.execute(
                "INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES ('PJ_HOLDING', ?1, 'Sócio', 'SOCIO')",
                [&socio_id],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }

    let store = SqliteGraphStore::new(&db_path).unwrap();

    // Testa poda customizada menor (ex: limite_itens = 25)
    let res_25 = store.buscar_camadas("PJ_HOLDING", 1, 25).unwrap();
    assert_eq!(res_25.links.len(), 25);
    assert_eq!(res_25.nodes.len(), 26); // 1 raiz + 25 sócios

    // Testa teto oficial K_LIMITE_CAMADA = 1000 mesmo quando solicitado limite superior (ex: 5000)
    let res_1000 = store.buscar_camadas("PJ_HOLDING", 1, 5000).unwrap();
    assert_eq!(
        res_1000.links.len(),
        K_LIMITE_CAMADA,
        "Número de arestas expandidas na camada não pode ultrapassar K_LIMITE_CAMADA (1000)"
    );
    assert_eq!(res_1000.nodes.len(), K_LIMITE_CAMADA + 1);

    // Garante que buscar_caminho_minimo encontra o sócio #1150 mesmo estando além do corte de 1.000 da camada 1
    let caminho_1150 = store
        .buscar_caminho_minimo("PJ_HOLDING", "PF_SOCIO_1150", 3)
        .unwrap()
        .expect("Deveria encontrar caminho direto para o sócio #1150 da mega-holding");
    assert_eq!(caminho_1150.distancia, 1);
    assert_eq!(caminho_1150.nodes.len(), 2);
    assert_eq!(caminho_1150.links.len(), 1);

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_shortest_path_dijkstra_entre_entidades() {
    let db_path = temp_db_path("dijkstra");
    let store = criar_fixture_3_empresas_4_socios(&db_path);

    // Menor caminho entre PJ_11111111000191 e PF_40000000404:
    // PJ_11111111000191 -> PF_20000000202 -> PJ_33333333000191 -> PF_40000000404 (distância = 3)
    // (Existe caminho mais longo via PJ_22222222000191 de distância 4, que o Dijkstra deve descartar)
    let caminho = store
        .buscar_caminho_minimo("11111111000191", "40000000404", 6)
        .expect("Falha ao calcular menor caminho")
        .expect("Caminho mínimo deveria existir");

    assert_eq!(caminho.distancia, 3);
    assert_eq!(caminho.nodes.len(), 4);
    assert_eq!(caminho.links.len(), 3);
    assert_eq!(caminho.nodes.first().unwrap().id, "PJ_11111111000191");
    assert_eq!(caminho.nodes[1].id, "PF_20000000202");
    assert_eq!(caminho.nodes[2].id, "PJ_33333333000191");
    assert_eq!(caminho.nodes.last().unwrap().id, "PF_40000000404");

    // Caminho de uma entidade para ela mesma tem distância 0
    let mesmo_no = store
        .buscar_caminho_minimo("PJ_11111111000191", "PJ_11111111000191", 3)
        .unwrap()
        .unwrap();
    assert_eq!(mesmo_no.distancia, 0);
    assert_eq!(mesmo_no.nodes.len(), 1);
    assert!(mesmo_no.links.is_empty());

    // Entidade inexistente ou desconectada retorna Ok(None)
    let inexistente = store
        .buscar_caminho_minimo("PJ_11111111000191", "PJ_99999999000199", 5)
        .unwrap();
    assert!(inexistente.is_none());

    // Entradas vazias retornam erro de validação
    assert!(store.buscar_camadas("   ", 2, 100).is_err());
    assert!(store
        .buscar_caminho_minimo("", "PJ_11111111000191", 2)
        .is_err());

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_bfs_resiliencia_duplicatas_e_profundidade_triangulo() {
    let store = SqliteGraphStore::new_in_memory_with_setup(|conn| {
        // 5 auto-loops e duplicatas antes do 2º vizinho real (PF_S2)
        for _ in 0..5 {
            conn.execute(
                "INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES ('PJ_E1', 'PJ_E1', 'Auto', 'SOCIO')",
                [],
            )?;
            conn.execute(
                "INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES ('PJ_E1', 'PF_S1', 'Sócio', 'SOCIO')",
                [],
            )?;
        }
        conn.execute(
            "INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES ('PJ_E1', 'PF_S2', 'Sócio', 'SOCIO')",
            [],
        )?;
        // Aresta cruzada entre S1 e S2 (ambos na camada 1)
        conn.execute(
            "INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES ('PF_S1', 'PF_S2', 'Sócio', 'SOCIO')",
            [],
        )?;
        Ok(())
    })
    .unwrap();

    // Com limite_itens = 2 na camada 1, não deve truncar antes de alcançar PF_S2
    let c1 = store.buscar_camadas("PJ_E1", 1, 2).unwrap();
    assert_eq!(c1.nodes.len(), 3);
    assert_eq!(c1.links.len(), 2);
    assert_eq!(c1.total_camadas, 1);

    // Ao expandir até 5 camadas no triângulo E1-S1-S2, a profundidade real de nós continua sendo 1 camada
    let c_full = store.buscar_camadas("PJ_E1", 5, 100).unwrap();
    assert_eq!(c_full.nodes.len(), 3);
    assert_eq!(c_full.links.len(), 3);
    assert_eq!(c_full.total_camadas, 1);
}
