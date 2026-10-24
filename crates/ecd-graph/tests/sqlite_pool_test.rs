use ecd_graph::{SqliteGraphStore, DEFAULT_CACHE_SIZE, DEFAULT_MMAP_SIZE};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::thread;

fn temp_db_path(prefix: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("ecd_graph_{prefix}_{pid}_{seq}.db"))
}

#[test]
fn test_sqlite_pool_pragmas_and_query_only_enforcement() {
    let db_path = temp_db_path("pragmas");
    let _ = std::fs::remove_file(&db_path);

    // Inicializa banco em modo de escrita antes de abrir o pool read-only
    {
        let conn = Connection::open(&db_path).expect("Falha ao criar SQLite temporário");
        SqliteGraphStore::init_schema(&conn).expect("Falha ao inicializar schema");
        conn.execute(
            "INSERT INTO entidades (id, nome, tipo) VALUES (?1, ?2, ?3)",
            ["PJ_11111111000191", "EMPRESA ALFA S.A.", "PJ"],
        )
        .expect("Falha ao inserir entidade inicial");
    }

    let store = SqliteGraphStore::new(&db_path).expect("Falha ao criar SqliteGraphStore");
    let pragmas = store
        .verificar_pragmas()
        .expect("Falha ao consultar PRAGMAs");

    assert!(pragmas.query_only, "PRAGMA query_only deve estar ativo");
    assert_eq!(
        pragmas.mmap_size, DEFAULT_MMAP_SIZE,
        "PRAGMA mmap_size deve ser 268435456 (256MB)"
    );
    assert_eq!(
        pragmas.cache_size, DEFAULT_CACHE_SIZE,
        "PRAGMA cache_size deve ser -64000 (64MB)"
    );

    // Garante que qualquer tentativa de escrita em conexão do pool falhe por query_only = ON
    let conn = store.get_conn().expect("Falha ao obter conexão do pool");
    let write_attempt = conn.execute(
        "INSERT INTO entidades (id, nome, tipo) VALUES ('PJ_999', 'INVASAO', 'PJ')",
        [],
    );
    assert!(
        write_attempt.is_err(),
        "Conexão com query_only = ON deve rejeitar comandos INSERT"
    );

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_sqlite_pool_thread_safe_concurrent_reads() {
    let db_path = temp_db_path("concurrent");
    let _ = std::fs::remove_file(&db_path);

    {
        let conn = Connection::open(&db_path).expect("Falha ao criar SQLite temporário");
        SqliteGraphStore::init_schema(&conn).expect("Falha ao inicializar schema");
        conn.execute(
            "INSERT INTO entidades (id, nome, tipo) VALUES ('PJ_01', 'HOLDING CENTRAL', 'PJ')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES ('PJ_01', 'PF_SOCIO_1', 'Sócio-Administrador', 'SOCIO')",
            [],
        )
        .unwrap();
    }

    let store = Arc::new(
        SqliteGraphStore::new_with_pool_size(&db_path, 8).expect("Falha ao criar pool SQLite"),
    );

    let mut handles = Vec::new();
    for _ in 0..8 {
        let store_clone = Arc::clone(&store);
        handles.push(thread::spawn(move || {
            let pragmas = store_clone.verificar_pragmas().unwrap();
            assert!(pragmas.query_only);
            let res = store_clone.buscar_camadas("PJ_01", 1, 100).unwrap();
            assert_eq!(res.nodes.len(), 2);
            assert_eq!(res.links.len(), 1);
        }));
    }

    for h in handles {
        h.join().expect("Thread de leitura concorrente falhou");
    }

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_sqlite_pool_with_attached_links_db() {
    let rede_path = temp_db_path("rede_attached");
    let cnpj_path = temp_db_path("cnpj_attached");
    let links_path = temp_db_path("links_attached");
    let _ = std::fs::remove_file(&rede_path);
    let _ = std::fs::remove_file(&cnpj_path);
    let _ = std::fs::remove_file(&links_path);

    {
        // Inicializa rede.db com schema completo (incluindo tabela main.links vazia)
        let conn_rede = Connection::open(&rede_path).unwrap();
        SqliteGraphStore::init_schema(&conn_rede).unwrap();
        conn_rede
            .execute(
                "INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES ('PJ_10000000000191', 'PF_20000000202', 'Sócio', 'SOCIO')",
                [],
            )
            .unwrap();

        // Cria cnpj.db com tabela empresas (cnpj_basico -> razao_social)
        let conn_cnpj = Connection::open(&cnpj_path).unwrap();
        conn_cnpj
            .execute_batch(
                "CREATE TABLE empresas (
                    cnpj_basico TEXT PRIMARY KEY,
                    razao_social TEXT NOT NULL
                );
                INSERT INTO empresas VALUES ('10000000', 'INDUSTRIA NACIONAL S.A.');",
            )
            .unwrap();

        // Cria links.db separado com sanções em links_db.links
        let conn_links = Connection::open(&links_path).unwrap();
        conn_links
            .execute_batch(
                "CREATE TABLE links (
                    id1 TEXT NOT NULL,
                    id2 TEXT NOT NULL,
                    descricao TEXT NOT NULL,
                    comentario TEXT NOT NULL
                );
                INSERT INTO links VALUES ('PF_20000000202', 'PEP_FEDERAL', 'PEP', 'Ex-Diretor');",
            )
            .unwrap();
    }

    let store =
        SqliteGraphStore::with_attached_dbs(&rede_path, Some(&cnpj_path), Some(&links_path))
            .unwrap();
    let res = store.buscar_camadas("10000000000191", 1, 100).unwrap();
    assert_eq!(res.alertas_encontrados, 1);

    let empresa = res
        .nodes
        .iter()
        .find(|n| n.id == "PJ_10000000000191")
        .unwrap();
    assert_eq!(empresa.label, "INDUSTRIA NACIONAL S.A.");

    let socio = res.nodes.iter().find(|n| n.id == "PF_20000000202").unwrap();
    assert_eq!(socio.flags_sancionatorias, vec!["PEP".to_string()]);

    let _ = std::fs::remove_file(&rede_path);
    let _ = std::fs::remove_file(&cnpj_path);
    let _ = std::fs::remove_file(&links_path);
}

#[test]
fn test_sqlite_in_memory_store() {
    let store = SqliteGraphStore::new_in_memory_with_setup(|conn| {
        conn.execute(
            "INSERT INTO entidades (id, nome, tipo) VALUES ('PJ_MEM_1', 'MEMORIA LTDA', 'PJ')",
            [],
        )?;
        conn.execute(
            "INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES ('PJ_MEM_1', 'PF_MEM_2', 'Sócio', 'SOCIO')",
            [],
        )?;
        Ok(())
    })
    .unwrap();

    let pragmas = store.verificar_pragmas().unwrap();
    assert!(pragmas.query_only);

    let res = store.buscar_camadas("PJ_MEM_1", 1, 10).unwrap();
    assert_eq!(res.nodes.len(), 2);
    assert_eq!(res.links.len(), 1);
}
