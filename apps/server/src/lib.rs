//! # ecd-server
//!
//! Biblioteca central e roteador Axum do Servidor HTTP:
//! - Autenticação Argon2id + JWT em cookies `HttpOnly; Secure; SameSite=Lax` e RBAC (`/api/auth/*`)
//! - Upload multipart streaming (`/api/importacoes/upload`) e listagem de jobs (`/api/importacoes`)
//! - Server-Sent Events (`/api/jobs/:id/stream`) subscrito no canal Redis `ecd:events:status`
//! - Consultas contábeis/DRE (`/api/fiscal/consulta-cnpj/:cnpj`) e exportação XLSX (`/api/fiscal/exportar-xlsx/:cnpj`)
//! - Grafos societários GRC (`/api/grc/rede`, `/api/grc/caminho`)
//! - Indicadores econômicos com cache `moka` (`/api/indicadores/resumo`)
//! - Proxy reverso para o AI Gateway (`/api/ai/chat`)

pub mod auth;
pub mod handlers;

use auth::hash_password;
use axum::{
    extract::DefaultBodyLimit,
    http::{header, Method},
    routing::{get, post},
    Json, Router,
};
use chrono::{NaiveDate, Utc};
use ecd_analytics::{EmpresaRegistro, IndicadoresCache};
use ecd_domain::{
    ImportSpedJobPayload, ImportacaoEcd, JobStatusEvent, RegI355, RegJ150, Registro0000,
    RegistroI155, RegistroJ100,
};
use ecd_graph::SqliteGraphStore;
use ecd_storage::{
    auth_repository::{AuthRepository, UserRole, Usuario},
    create_postgres_pool,
    fiscal_repository::{ConsultaCnpjConsolidada, FiscalRepository},
};
use handlers::{
    auth_handler::{
        listar_usuarios_handler, login_handler, logout_handler, me_handler, refresh_handler,
    },
    fiscal_handler::{consultar_cnpj_handler, exportar_xlsx_handler},
    grc_handler::{buscar_caminho_handler, buscar_rede_handler},
    indicadores_handler::{
        ai_chat_proxy_handler, ai_chat_stream_proxy_handler, obter_resumo_indicadores_handler,
    },
    sse_handler::job_stream_sse_handler,
    upload_handler::{listar_importacoes_handler, obter_importacao_handler, upload_sped_handler},
};
use rust_decimal::Decimal;
use serde_json::json;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{broadcast, RwLock};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use uuid::Uuid;

/// Limite máximo para upload de arquivos SPED via streaming multipart (2 GB).
pub const MAX_UPLOAD_SIZE_BYTES: usize = 2 * 1024 * 1024 * 1024;

/// Estado compartilhado thread-safe injetado em todos os handlers do Axum.
#[derive(Clone)]
pub struct AppState {
    pub auth_repo: Option<AuthRepository>,
    pub fiscal_repo: Option<FiscalRepository>,
    pub redis_client: Option<redis::Client>,
    pub graph_store: Arc<SqliteGraphStore>,
    pub analytics_cache: IndicadoresCache,
    pub http_client: reqwest::Client,
    pub jwt_secret: String,
    pub internal_api_key: Option<String>,
    pub stage_dir: PathBuf,
    pub ai_gateway_url: String,
    pub event_tx: broadcast::Sender<JobStatusEvent>,
    pub in_memory_users: Arc<RwLock<Vec<Usuario>>>,
    pub in_memory_jobs: Arc<RwLock<Vec<ImportacaoEcd>>>,
    pub in_memory_queue: Arc<RwLock<Vec<ImportSpedJobPayload>>>,
    pub in_memory_empresas: Arc<RwLock<Vec<EmpresaRegistro>>>,
    pub in_memory_sped: Arc<RwLock<HashMap<String, ConsultaCnpjConsolidada>>>,
}

impl AppState {
    /// Inicializa o [`AppState`] a partir de variáveis de ambiente (`.env`),
    /// conectando-se ao PostgreSQL, Redis e bases SQLite locais quando disponíveis.
    /// Em modo release ou quando `ENVIRONMENT=production`, executa hardening estrito (TASK-904):
    /// - Exige `JWT_SECRET` com no mínimo 32 caracteres e rejeita segredos padrão de desenvolvimento;
    /// - Exige conexão válida com o PostgreSQL via `DATABASE_URL` e desativa fallback silencioso em memória;
    /// - Não injeta credenciais ou usuários hardcoded no sistema.
    pub async fn from_env() -> anyhow::Result<Self> {
        let is_production = std::env::var("ENVIRONMENT")
            .map(|e| e.eq_ignore_ascii_case("production") || e.eq_ignore_ascii_case("prod"))
            .unwrap_or(!cfg!(debug_assertions));

        let jwt_secret = match std::env::var("JWT_SECRET") {
            Ok(secret) if !secret.trim().is_empty() => {
                if is_production
                    && (secret == "ecd-analytics-jwt-secret-key-2026-default" || secret.len() < 32)
                {
                    anyhow::bail!(
                        "Segurança (TASK-904): Em ambiente de produção, JWT_SECRET não pode usar o valor padrão de desenvolvimento e deve conter ao menos 32 caracteres seguros."
                    );
                }
                secret
            }
            _ => {
                if is_production {
                    anyhow::bail!(
                        "Segurança (TASK-904): A variável de ambiente JWT_SECRET é obrigatória em ambiente de produção."
                    );
                }
                tracing::warn!(
                    "Desenvolvimento: Usando JWT_SECRET padrão. Defina uma chave forte para produção."
                );
                "ecd-analytics-jwt-secret-key-2026-default".to_string()
            }
        };

        let internal_api_key = std::env::var("ECD_BACKEND_API_KEY").ok();
        let stage_dir = std::env::var("ECD_STAGE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("media/ecd_stage/aguardando"));
        let ai_gateway_url =
            std::env::var("AI_GATEWAY_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());

        let (auth_repo, fiscal_repo) = match std::env::var("DATABASE_URL") {
            Ok(db_url) if !db_url.trim().is_empty() => match create_postgres_pool(&db_url).await {
                Ok(pool) => (
                    Some(AuthRepository::new(pool.clone())),
                    Some(FiscalRepository::new(pool)),
                ),
                Err(e) => {
                    if is_production {
                        anyhow::bail!(
                            "Segurança (TASK-904): Falha fatal ao conectar ao PostgreSQL em '{}': {}. Fallback em memória desativado em produção.",
                            db_url,
                            e
                        );
                    }
                    tracing::warn!(
                        "Não foi possível conectar ao PostgreSQL em '{}': {}. Usando store em memória (apenas desenvolvimento).",
                        db_url,
                        e
                    );
                    (None, None)
                }
            },
            _ => {
                if is_production {
                    anyhow::bail!(
                        "Segurança (TASK-904): A variável DATABASE_URL é obrigatória em ambiente de produção. Fallback em memória desativado."
                    );
                }
                tracing::warn!(
                    "DATABASE_URL não configurada. Usando store em memória para desenvolvimento local."
                );
                (None, None)
            }
        };

        let redis_client = match std::env::var("REDIS_URL") {
            Ok(redis_url) if !redis_url.trim().is_empty() => redis::Client::open(redis_url).ok(),
            _ => redis::Client::open("redis://127.0.0.1:6379").ok(),
        };

        let graph_store = Self::inicializar_graph_store_padrao();
        let analytics_cache = IndicadoresCache::default();
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()?;
        let (event_tx, _) = broadcast::channel(512);

        let initial_users = if is_production {
            tracing::info!(
                "Segurança (TASK-904): Modo de produção ativo. Nenhuma credencial padrão injetada em memória."
            );
            Vec::new()
        } else {
            Self::seed_usuarios_padrao()
        };

        Ok(Self {
            auth_repo,
            fiscal_repo,
            redis_client,
            graph_store: Arc::new(graph_store),
            analytics_cache,
            http_client,
            jwt_secret,
            internal_api_key,
            stage_dir,
            ai_gateway_url,
            event_tx,
            in_memory_users: Arc::new(RwLock::new(initial_users)),
            in_memory_jobs: Arc::new(RwLock::new(Vec::new())),
            in_memory_queue: Arc::new(RwLock::new(Vec::new())),
            in_memory_empresas: Arc::new(RwLock::new(Self::seed_empresas_padrao())),
            in_memory_sped: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Cria uma instância isolada do [`AppState`] para testes unitários e de integração.
    pub fn new_for_tests(stage_dir: impl AsRef<Path>) -> Self {
        let graph_store = Self::criar_graph_store_em_memoria();
        let (event_tx, _) = broadcast::channel(256);
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("deve criar reqwest::Client");

        Self {
            auth_repo: None,
            fiscal_repo: None,
            redis_client: None,
            graph_store: Arc::new(graph_store),
            analytics_cache: IndicadoresCache::with_ttl(Duration::from_secs(300)),
            http_client,
            jwt_secret: "segredo_teste_axum_server_2026".to_string(),
            internal_api_key: Some("internal_ai_gateway_key_test".to_string()),
            stage_dir: stage_dir.as_ref().to_path_buf(),
            ai_gateway_url: "http://127.0.0.1:59999".to_string(),
            event_tx,
            in_memory_users: Arc::new(RwLock::new(Self::seed_usuarios_padrao())),
            in_memory_jobs: Arc::new(RwLock::new(Vec::new())),
            in_memory_queue: Arc::new(RwLock::new(Vec::new())),
            in_memory_empresas: Arc::new(RwLock::new(Self::seed_empresas_padrao())),
            in_memory_sped: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn inicializar_graph_store_padrao() -> SqliteGraphStore {
        if let Ok(bases_dir) = std::env::var("TURMALINA_BASES_PATH") {
            let base = PathBuf::from(bases_dir);
            let rede = base.join("rede.db");
            if rede.exists() {
                let cnpj = base.join("cnpj.db");
                let links = base.join("links.db");
                if let Ok(store) = SqliteGraphStore::with_attached_dbs(
                    &rede,
                    cnpj.exists().then_some(&cnpj),
                    links.exists().then_some(&links),
                ) {
                    return store;
                }
            }
        }
        Self::criar_graph_store_em_memoria()
    }

    fn criar_graph_store_em_memoria() -> SqliteGraphStore {
        let db_path = std::env::temp_dir().join(format!("ecd_server_graph_{}.db", Uuid::now_v7()));
        {
            let conn = rusqlite::Connection::open(&db_path)
                .expect("deve criar banco SQLite temporário para grafo");
            SqliteGraphStore::init_schema(&conn).expect("deve criar schema SQLite do grafo");
            conn.execute_batch(
                r#"
                INSERT INTO entidades (id, nome, tipo) VALUES
                    ('33000167000101', 'PETROLEO BRASILEIRO S.A. PETROBRAS', 'PJ'),
                    ('02709449000159', 'PETROBRAS TRANSPORTE S.A. - TRANSPETRO', 'PJ'),
                    ('11222333000199', 'DELTA ENGENHARIA E PARTICIPACOES S.A.', 'PJ'),
                    ('12345678901', 'CARLOS ALBERTO PEREIRA', 'PF'),
                    ('98765432100', 'MARCOS VINICIUS DA SILVA', 'PF');

                INSERT INTO ligacao (id1, id2, descricao, comentario) VALUES
                    ('12345678901', '33000167000101', 'SOCIO-ADMINISTRADOR', 'Diretor Executivo'),
                    ('33000167000101', '02709449000159', 'SOCIO', 'Subsidiaria Integral'),
                    ('02709449000159', '11222333000199', 'SOCIO', 'Participacao Coligada'),
                    ('98765432100', '11222333000199', 'SOCIO', 'Socio Administrador');

                INSERT INTO links (id1, id2, descricao, comentario) VALUES
                    ('12345678901', 'PEP_001', 'PEP - Pessoa Politicamente Exposta', 'Ex-Dirigente'),
                    ('11222333000199', 'CEIS_002', 'CEIS - Cadastro de Empresas Inidoneas', 'Sancao CGU');
                "#,
            )
            .expect("deve popular dados iniciais do grafo SQLite");
        }
        SqliteGraphStore::new(&db_path).expect("deve inicializar SqliteGraphStore")
    }

    fn seed_usuarios_padrao() -> Vec<Usuario> {
        let agora = Utc::now();
        let hash_admin = hash_password("Admin@2026!").unwrap_or_default();
        let hash_analista = hash_password("Analista@2026!").unwrap_or_default();
        let hash_viewer = hash_password("Viewer@2026!").unwrap_or_default();

        vec![
            Usuario {
                id: Uuid::now_v7(),
                email: "admin@ecd.gov.br".to_string(),
                nome: "Administrador Geral".to_string(),
                senha_hash: hash_admin,
                role: UserRole::Admin.as_str().to_string(),
                ativo: true,
                criado_em: agora,
                ultimo_login: None,
            },
            Usuario {
                id: Uuid::now_v7(),
                email: "analista@ecd.gov.br".to_string(),
                nome: "Auditor Fiscal Senior".to_string(),
                senha_hash: hash_analista,
                role: UserRole::Analista.as_str().to_string(),
                ativo: true,
                criado_em: agora,
                ultimo_login: None,
            },
            Usuario {
                id: Uuid::now_v7(),
                email: "visualizador@ecd.gov.br".to_string(),
                nome: "Consultor Externo".to_string(),
                senha_hash: hash_viewer,
                role: UserRole::Visualizador.as_str().to_string(),
                ativo: true,
                criado_em: agora,
                ultimo_login: None,
            },
        ]
    }

    fn seed_empresas_padrao() -> Vec<EmpresaRegistro> {
        vec![
            EmpresaRegistro::new()
                .with_porte("01")
                .with_situacao("02")
                .with_cnae("6201501")
                .with_natureza("2062")
                .with_capital(Decimal::from(50_000))
                .with_simples("S")
                .with_uf("SP")
                .with_municipio("SAO PAULO"),
            EmpresaRegistro::new()
                .with_porte("03")
                .with_situacao("02")
                .with_cnae("6204000")
                .with_natureza("2062")
                .with_capital(Decimal::from(250_000))
                .with_regime("Lucro Presumido")
                .with_uf("SP")
                .with_municipio("CAMPINAS"),
            EmpresaRegistro::new()
                .with_porte("05")
                .with_situacao("02")
                .with_cnae("4711302")
                .with_natureza("2054")
                .with_capital(Decimal::from(5_000_000))
                .with_regime("Lucro Real")
                .with_uf("RJ")
                .with_municipio("RIO DE JANEIRO"),
            EmpresaRegistro::new()
                .with_porte("01")
                .with_situacao("02")
                .with_cnae("6920601")
                .with_natureza("2135")
                .with_capital(Decimal::from(5_000))
                .with_mei("S")
                .with_uf("MS")
                .with_municipio("CAMPO GRANDE"),
        ]
    }

    /// Retorna dados contábeis do store em memória ou gera um conjunto determinístico do CNPJ para consulta/exportação.
    pub async fn obter_dados_sped_fallback(
        &self,
        clean_cnpj: &str,
        _ano: Option<i32>,
    ) -> ConsultaCnpjConsolidada {
        {
            let map = self.in_memory_sped.read().await;
            if let Some(existing) = map.get(clean_cnpj) {
                return existing.clone();
            }
        }

        let dt_ini = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let dt_fim = NaiveDate::from_ymd_opt(2023, 12, 31).unwrap();
        let hash_arquivo = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".to_string();
        let razao = format!("EMPRESA AUDITADA {clean_cnpj} S.A.");

        let reg_0000 = vec![Registro0000 {
            id: Some(1),
            dt_ini,
            dt_fim,
            nome_razao: razao.clone(),
            cnpj: clean_cnpj.to_string(),
            uf: "SP".to_string(),
            ie: Some("110042490114".to_string()),
            cod_mun: Some(3550308),
            im: None,
            ind_sit_esp: None,
            ind_sit_ini_per: 0,
            ind_nire: 1,
            ind_fin_esc: 0,
            cod_hash_sub: None,
            ind_grande_porte: 1,
            tip_ecd: 0,
            cod_scp: None,
            ident_mf: "N".to_string(),
            ind_esc_cons: "N".to_string(),
            ind_centralizada: 0,
            ind_mudanc_pc: 0,
            cod_plan_ref: Some("10".to_string()),
            i990: 150,
            j990: 45,
            hash_arquivo_id: hash_arquivo.clone(),
        }];

        let reg_j100 = vec![
            RegistroJ100 {
                id: Some(1),
                cod_agl: "1".to_string(),
                ind_cod_agl: "T".to_string(),
                nivel_agl: 1,
                cod_agl_sup: None,
                ind_grp_bal: "A".to_string(),
                descr_cod_agl: "ATIVO TOTAL".to_string(),
                vl_cta_ini: Decimal::from(15_000_000),
                ind_dc_cta_ini: "D".to_string(),
                vl_cta_fin: Decimal::from(18_500_000),
                ind_dc_cta_fin: "D".to_string(),
                nota_exp_ref: None,
                cnpj: clean_cnpj.to_string(),
                hash_arquivo_id: Some(hash_arquivo.clone()),
                reg_0000_id: 1,
                reg_j005_id: 1,
            },
            RegistroJ100 {
                id: Some(2),
                cod_agl: "2".to_string(),
                ind_cod_agl: "T".to_string(),
                nivel_agl: 1,
                cod_agl_sup: None,
                ind_grp_bal: "P".to_string(),
                descr_cod_agl: "PASSIVO E PATRIMONIO LIQUIDO".to_string(),
                vl_cta_ini: Decimal::from(15_000_000),
                ind_dc_cta_ini: "C".to_string(),
                vl_cta_fin: Decimal::from(18_500_000),
                ind_dc_cta_fin: "C".to_string(),
                nota_exp_ref: None,
                cnpj: clean_cnpj.to_string(),
                hash_arquivo_id: Some(hash_arquivo.clone()),
                reg_0000_id: 1,
                reg_j005_id: 1,
            },
        ];

        let reg_j150 = vec![
            RegJ150 {
                id: Some(1),
                nu_ordem: 1,
                cod_agl: Some("3.01".to_string()),
                ind_cod_agl: "T".to_string(),
                nivel_agl: 1,
                cod_agl_sup: None,
                descr_cod_agl: "RECEITA BRUTA OPERACIONAL".to_string(),
                vl_cta_ini: Decimal::from(10_000_000),
                ind_dc_cta_ini: Some("C".to_string()),
                vl_cta_fin: Decimal::from(12_500_000),
                ind_dc_cta_fin: "C".to_string(),
                ind_grp_dre: "R".to_string(),
                nota_exp_ref: None,
                cnpj: clean_cnpj.to_string(),
                dt_ini: Some(dt_ini),
                dt_fim: Some(dt_fim),
                hash_arquivo: hash_arquivo.clone(),
            },
            RegJ150 {
                id: Some(2),
                nu_ordem: 2,
                cod_agl: Some("3.01.01".to_string()),
                ind_cod_agl: "D".to_string(),
                nivel_agl: 2,
                cod_agl_sup: Some("3.01".to_string()),
                descr_cod_agl: "RECEITA BRUTA DE PRESTACAO DE SERVICOS".to_string(),
                vl_cta_ini: Decimal::from(8_000_000),
                ind_dc_cta_ini: Some("C".to_string()),
                vl_cta_fin: Decimal::from(10_200_000),
                ind_dc_cta_fin: "C".to_string(),
                ind_grp_dre: "R".to_string(),
                nota_exp_ref: None,
                cnpj: clean_cnpj.to_string(),
                dt_ini: Some(dt_ini),
                dt_fim: Some(dt_fim),
                hash_arquivo: hash_arquivo.clone(),
            },
            RegJ150 {
                id: Some(3),
                nu_ordem: 3,
                cod_agl: Some("3.01.02".to_string()),
                ind_cod_agl: "D".to_string(),
                nivel_agl: 2,
                cod_agl_sup: Some("3.01".to_string()),
                descr_cod_agl: "RECEITA BRUTA DE LOCACAO DE BENS".to_string(),
                vl_cta_ini: Decimal::from(2_000_000),
                ind_dc_cta_ini: Some("C".to_string()),
                vl_cta_fin: Decimal::from(2_300_000),
                ind_dc_cta_fin: "C".to_string(),
                ind_grp_dre: "R".to_string(),
                nota_exp_ref: None,
                cnpj: clean_cnpj.to_string(),
                dt_ini: Some(dt_ini),
                dt_fim: Some(dt_fim),
                hash_arquivo: hash_arquivo.clone(),
            },
        ];

        let reg_i155 = vec![RegistroI155 {
            id: Some(1),
            cod_cta: "3.01.01.01.01".to_string(),
            cod_ccus: None,
            vl_sld_ini: Decimal::ZERO,
            ind_dc_ini: "C".to_string(),
            vl_deb: Decimal::ZERO,
            vl_cred: Decimal::from(12_500_000),
            vl_sld_fim: Decimal::from(12_500_000),
            ind_dc_fim: "C".to_string(),
            reg_0000_id: 1,
            reg_i150_id: 1,
        }];

        let reg_i355 = vec![RegI355 {
            id: Uuid::now_v7(),
            cnpj: clean_cnpj.to_string(),
            cod_cta: "3.01.01.01.01".to_string(),
            nome_cta_referencia: Some("RECEITA BRUTA DE SERVICOS".to_string()),
            cod_ccus: None,
            vl_cta: Decimal::from(10_200_000),
            ind_dc: "C".to_string(),
            vl_cta_mf: Some(Decimal::from(10_200_000)),
            ind_dc_mf: Some("C".to_string()),
            dt_res: dt_fim,
            cod_agl: Some("3.01.01".to_string()),
            cod_cta_ref: Some("3.01.01.01.01.06".to_string()),
            nome_aglutinador: Some("RECEITA BRUTA DE PRESTACAO DE SERVICOS".to_string()),
            hash_arquivo,
        }];

        ConsultaCnpjConsolidada {
            cnpj: clean_cnpj.to_string(),
            razao_social: razao,
            uf: Some("SP".to_string()),
            reg_0000,
            reg_j100,
            reg_j150,
            reg_i155,
            reg_i355,
            resultado_exercicio: None,
        }
    }
}

/// Constrói o roteador principal do Axum com todas as rotas da Trilha 8.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::AUTHORIZATION,
            header::COOKIE,
        ])
        .allow_credentials(true);

    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"status": "ok", "service": "ecd-server"})) }),
        )
        .route(
            "/api/health",
            get(|| async { Json(json!({"status": "ok", "service": "ecd-server"})) }),
        )
        // Rotas de Autenticação e RBAC (TASK-801)
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/me", get(me_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/auth/refresh", post(refresh_handler))
        .route("/api/auth/usuarios", get(listar_usuarios_handler))
        // Rotas de Ingestão e Upload Multipart Streaming (TASK-802)
        .route(
            "/api/importacoes/upload",
            post(upload_sped_handler).layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE_BYTES)),
        )
        .route("/api/importacoes", get(listar_importacoes_handler))
        .route("/api/importacoes/:id", get(obter_importacao_handler))
        // Rota de Server-Sent Events (SSE) em Tempo Real (TASK-803)
        .route("/api/jobs/:id/stream", get(job_stream_sse_handler))
        // Rotas Fiscais, DRE e Exportação XLSX (TASK-804)
        .route(
            "/api/fiscal/consulta-cnpj/:cnpj",
            get(consultar_cnpj_handler),
        )
        .route(
            "/api/fiscal/exportar-xlsx/:cnpj",
            get(exportar_xlsx_handler),
        )
        // Rotas de Inteligência Societária GRC (TASK-804)
        .route("/api/grc/rede", get(buscar_rede_handler))
        .route("/api/grc/caminho", get(buscar_caminho_handler))
        // Rotas de Indicadores Econômicos e Proxy de IA (TASK-804)
        .route(
            "/api/indicadores/resumo",
            get(obter_resumo_indicadores_handler),
        )
        .route("/api/ai/chat", post(ai_chat_proxy_handler))
        .route(
            "/api/ai/chat/stream",
            get(ai_chat_stream_proxy_handler).post(ai_chat_proxy_handler),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
