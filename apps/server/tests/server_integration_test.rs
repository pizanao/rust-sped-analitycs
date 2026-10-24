//! Testes de integração completos da Trilha 8 (`ecd-server`):
//! - TASK-801: Autenticação Argon2id + JWT em cookies `HttpOnly; Secure; SameSite=Lax` e RBAC
//! - TASK-802: Upload multipart streaming (`media/ecd_stage/aguardando/`) e fila `ImportSpedJobPayload`
//! - TASK-803: Server-Sent Events (`/api/jobs/:id/stream`)
//! - TASK-804: Consultas contábeis/DRE, exportação XLSX, grafos GRC, indicadores com cache `moka` e proxy IA

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use chrono::Utc;
use ecd_domain::{JobStatusEvent, StatusImportacao};
use ecd_server::{
    auth::{extract_cookie_value, ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME},
    build_router, AppState,
};
use futures::StreamExt;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::path::PathBuf;
use tower::ServiceExt;
use uuid::Uuid;

fn create_test_app() -> (axum::Router, AppState, PathBuf) {
    let temp_dir = std::env::temp_dir().join(format!("ecd_stage_test_{}", Uuid::now_v7()));
    let state = AppState::new_for_tests(&temp_dir);
    let app = build_router(state.clone());
    (app, state, temp_dir)
}

#[tokio::test]
async fn test_task_801_auth_login_cookies_me_refresh_logout_and_rbac() {
    let (app, _state, temp_dir) = create_test_app();

    // 1. Login inválido deve retornar 401
    let bad_login_req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "email": "analista@ecd.gov.br",
                "senha": "SenhaIncorreta123"
            })
            .to_string(),
        ))
        .unwrap();

    let bad_resp = app.clone().oneshot(bad_login_req).await.unwrap();
    assert_eq!(bad_resp.status(), StatusCode::UNAUTHORIZED);

    // 2. Login válido como Analista
    let login_req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "email": "analista@ecd.gov.br",
                "senha": "Analista@2026!"
            })
            .to_string(),
        ))
        .unwrap();

    let login_resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);

    // Verifica que os cookies possuem HttpOnly; Secure; SameSite=Lax
    let set_cookies: Vec<String> = login_resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    assert_eq!(set_cookies.len(), 2);
    assert!(set_cookies
        .iter()
        .any(|c| c.starts_with("access_token=") && c.contains("HttpOnly; Secure; SameSite=Lax")));
    assert!(set_cookies
        .iter()
        .any(|c| c.starts_with("refresh_token=") && c.contains("HttpOnly; Secure; SameSite=Lax")));

    // Extrai access_token e refresh_token dos headers Set-Cookie
    let mut cookie_header_map = axum::http::HeaderMap::new();
    for sc in &set_cookies {
        let first_pair = sc.split(';').next().unwrap();
        cookie_header_map.append(header::COOKIE, first_pair.parse().unwrap());
    }
    let access_jwt = extract_cookie_value(&cookie_header_map, ACCESS_COOKIE_NAME).unwrap();
    let refresh_jwt = extract_cookie_value(&cookie_header_map, REFRESH_COOKIE_NAME).unwrap();

    // 3. GET /api/auth/me sem cookie -> 401
    let me_unauth_req = Request::builder()
        .method(Method::GET)
        .uri("/api/auth/me")
        .body(Body::empty())
        .unwrap();
    let me_unauth_resp = app.clone().oneshot(me_unauth_req).await.unwrap();
    assert_eq!(me_unauth_resp.status(), StatusCode::UNAUTHORIZED);

    // 4. GET /api/auth/me com cookie HttpOnly -> 200 OK
    let me_req = Request::builder()
        .method(Method::GET)
        .uri("/api/auth/me")
        .header(header::COOKIE, format!("access_token={access_jwt}"))
        .body(Body::empty())
        .unwrap();
    let me_resp = app.clone().oneshot(me_req).await.unwrap();
    assert_eq!(me_resp.status(), StatusCode::OK);
    let me_bytes = me_resp.into_body().collect().await.unwrap().to_bytes();
    let me_json: Value = serde_json::from_slice(&me_bytes).unwrap();
    assert_eq!(me_json["email"], "analista@ecd.gov.br");
    assert_eq!(me_json["role"], "Analista");

    // 5. RBAC: Analista tentando acessar rota restrita a Admin (/api/auth/usuarios) -> 403 Forbidden
    let rbac_denied_req = Request::builder()
        .method(Method::GET)
        .uri("/api/auth/usuarios")
        .header(header::COOKIE, format!("access_token={access_jwt}"))
        .body(Body::empty())
        .unwrap();
    let rbac_denied_resp = app.clone().oneshot(rbac_denied_req).await.unwrap();
    assert_eq!(rbac_denied_resp.status(), StatusCode::FORBIDDEN);

    // 6. RBAC: Login como Admin e acesso a /api/auth/usuarios -> 200 OK
    let admin_login_req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "email": "admin@ecd.gov.br",
                "senha": "Admin@2026!"
            })
            .to_string(),
        ))
        .unwrap();
    let admin_login_resp = app.clone().oneshot(admin_login_req).await.unwrap();
    assert_eq!(admin_login_resp.status(), StatusCode::OK);
    let admin_cookie = admin_login_resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .find(|s| s.starts_with("access_token="))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let rbac_ok_req = Request::builder()
        .method(Method::GET)
        .uri("/api/auth/usuarios")
        .header(header::COOKIE, admin_cookie)
        .body(Body::empty())
        .unwrap();
    let rbac_ok_resp = app.clone().oneshot(rbac_ok_req).await.unwrap();
    assert_eq!(rbac_ok_resp.status(), StatusCode::OK);

    // 7. Refresh token -> 200 OK e novo cookie access_token
    let refresh_req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/refresh")
        .header(header::COOKIE, format!("refresh_token={refresh_jwt}"))
        .body(Body::empty())
        .unwrap();
    let refresh_resp = app.clone().oneshot(refresh_req).await.unwrap();
    assert_eq!(refresh_resp.status(), StatusCode::OK);

    // 8. Logout -> limpa cookies com Max-Age=0
    let logout_req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/logout")
        .body(Body::empty())
        .unwrap();
    let logout_resp = app.oneshot(logout_req).await.unwrap();
    assert_eq!(logout_resp.status(), StatusCode::OK);
    let logout_cookies: Vec<String> = logout_resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    assert!(logout_cookies.iter().all(|c| c.contains("Max-Age=0")));

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_task_802_and_803_multipart_upload_and_sse_stream() {
    let (app, state, temp_dir) = create_test_app();

    let boundary = "------------------------BoundarySpedTest2026";
    let sped_content = "|0000|LECD|01012023|31122023|PETROLEO BRASILEIRO S.A.|33000167000101|RJ|||0|1|1|0||1|0||N|N|0|0|10|\r\n|I001|0|\r\n|9999|3|\r\n";
    let multipart_body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"arquivo\"; filename=\"sped_petrobras_2023.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         {sped_content}\r\n\
         --{boundary}--\r\n"
    );

    let upload_req = Request::builder()
        .method(Method::POST)
        .uri("/api/importacoes/upload")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(multipart_body))
        .unwrap();

    let upload_resp = app.clone().oneshot(upload_req).await.unwrap();
    assert_eq!(upload_resp.status(), StatusCode::ACCEPTED);

    let body_bytes = upload_resp.into_body().collect().await.unwrap().to_bytes();
    let json_resp: Value = serde_json::from_slice(&body_bytes).unwrap();

    let job_id_str = json_resp["job_id"].as_str().unwrap();
    let hash_str = json_resp["hash_arquivo"].as_str().unwrap();
    assert_eq!(hash_str.len(), 40);
    assert_eq!(json_resp["cnpj"], "33000167000101");
    assert_eq!(json_resp["status"], "Aguardando");

    // Verifica que o arquivo foi gravado em disco no diretório de staging
    let caminho_disco = json_resp["caminho_armazenamento"].as_str().unwrap();
    let disk_bytes = tokio::fs::read(caminho_disco).await.unwrap();
    assert_eq!(disk_bytes, sped_content.as_bytes());

    // Verifica que o ImportSpedJobPayload foi enfileirado
    {
        let queue = state.in_memory_queue.read().await;
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].job_id.to_string(), job_id_str);
        assert_eq!(queue[0].cnpj, "33000167000101");
        assert_eq!(queue[0].hash_arquivo, hash_str);
    }

    // Verifica GET /api/importacoes e GET /api/importacoes/:id
    let list_req = Request::builder()
        .method(Method::GET)
        .uri("/api/importacoes")
        .body(Body::empty())
        .unwrap();
    let list_resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_bytes = list_resp.into_body().collect().await.unwrap().to_bytes();
    let list_json: Value = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(list_json.as_array().unwrap().len(), 1);

    let get_job_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/importacoes/{job_id_str}"))
        .body(Body::empty())
        .unwrap();
    let get_job_resp = app.clone().oneshot(get_job_req).await.unwrap();
    assert_eq!(get_job_resp.status(), StatusCode::OK);

    // TASK-803: Conecta ao endpoint SSE GET /api/jobs/:id/stream e recebe eventos em tempo real
    let sse_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/jobs/{job_id_str}/stream"))
        .body(Body::empty())
        .unwrap();
    let sse_resp = app.clone().oneshot(sse_req).await.unwrap();
    assert_eq!(sse_resp.status(), StatusCode::OK);
    assert_eq!(
        sse_resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/event-stream"
    );

    // Dispara um evento de conclusão no barramento para o job
    let job_uuid = Uuid::parse_str(job_id_str).unwrap();
    let _ = state.event_tx.send(JobStatusEvent {
        job_id: job_uuid,
        hash_arquivo: hash_str.to_string(),
        status: StatusImportacao::Sucesso,
        progresso_percentual: 100,
        linhas_processadas: 3,
        mensagem: "Processamento concluído!".to_string(),
        timestamp: Utc::now(),
    });

    let mut body_stream = sse_resp.into_body().into_data_stream();
    let first_chunk = tokio::time::timeout(std::time::Duration::from_secs(2), body_stream.next())
        .await
        .expect("SSE deve emitir evento sem timeout")
        .expect("stream deve conter chunk")
        .expect("chunk válido");

    let sse_text = String::from_utf8_lossy(&first_chunk);
    assert!(sse_text.contains(job_id_str));

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_task_804_fiscal_xlsx_grc_indicadores_and_ai_proxy() {
    let (app, state, temp_dir) = create_test_app();

    // 1. Consulta Fiscal e Apuração DRE (/api/fiscal/consulta-cnpj/:cnpj)
    let fiscal_req = Request::builder()
        .method(Method::GET)
        .uri("/api/fiscal/consulta-cnpj/33000167000101")
        .body(Body::empty())
        .unwrap();
    let fiscal_resp = app.clone().oneshot(fiscal_req).await.unwrap();
    assert_eq!(fiscal_resp.status(), StatusCode::OK);
    let fiscal_bytes = fiscal_resp.into_body().collect().await.unwrap().to_bytes();
    let fiscal_json: Value = serde_json::from_slice(&fiscal_bytes).unwrap();

    assert_eq!(fiscal_json["cnpj"], "33000167000101");
    assert!(!fiscal_json["reg_0000"].as_array().unwrap().is_empty());
    assert!(!fiscal_json["reg_j100"].as_array().unwrap().is_empty());
    assert!(!fiscal_json["reg_j150"].as_array().unwrap().is_empty());
    assert!(!fiscal_json["reg_i155"].as_array().unwrap().is_empty());
    assert_eq!(
        fiscal_json["resultado_exercicio"]["receita_bruta"],
        12500000.0
    );
    assert_eq!(
        fiscal_json["resultado_exercicio"]["receita_servico"],
        10200000.0
    );

    // CNPJ inválido deve retornar 400 Bad Request
    let invalid_cnpj_req = Request::builder()
        .method(Method::GET)
        .uri("/api/fiscal/consulta-cnpj/12345")
        .body(Body::empty())
        .unwrap();
    let invalid_cnpj_resp = app.clone().oneshot(invalid_cnpj_req).await.unwrap();
    assert_eq!(invalid_cnpj_resp.status(), StatusCode::BAD_REQUEST);

    // 2. Exportação XLSX (/api/fiscal/exportar-xlsx/:cnpj)
    let xlsx_req = Request::builder()
        .method(Method::GET)
        .uri("/api/fiscal/exportar-xlsx/33000167000101")
        .body(Body::empty())
        .unwrap();
    let xlsx_resp = app.clone().oneshot(xlsx_req).await.unwrap();
    assert_eq!(xlsx_resp.status(), StatusCode::OK);
    assert_eq!(
        xlsx_resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
    let xlsx_bytes = xlsx_resp.into_body().collect().await.unwrap().to_bytes();
    // Assinatura ZIP/XLSX ("PK\x03\x04")
    assert!(xlsx_bytes.starts_with(b"PK\x03\x04"));

    // 3. Rede Societária GRC (/api/grc/rede) e Menor Caminho (/api/grc/caminho)
    let grc_rede_req = Request::builder()
        .method(Method::GET)
        .uri("/api/grc/rede?raiz=33000167000101&camadas=2&limite=1000")
        .body(Body::empty())
        .unwrap();
    let grc_rede_resp = app.clone().oneshot(grc_rede_req).await.unwrap();
    assert_eq!(grc_rede_resp.status(), StatusCode::OK);
    let grc_rede_bytes = grc_rede_resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let grc_rede_json: Value = serde_json::from_slice(&grc_rede_bytes).unwrap();
    assert!(grc_rede_json["total_nos"].as_u64().unwrap() >= 4);
    assert!(grc_rede_json["alertas_sancoes"].as_u64().unwrap() >= 2);

    let grc_caminho_req = Request::builder()
        .method(Method::GET)
        .uri("/api/grc/caminho?origem=12345678901&destino=11222333000199")
        .body(Body::empty())
        .unwrap();
    let grc_caminho_resp = app.clone().oneshot(grc_caminho_req).await.unwrap();
    assert_eq!(grc_caminho_resp.status(), StatusCode::OK);
    let grc_caminho_bytes = grc_caminho_resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let grc_caminho_json: Value = serde_json::from_slice(&grc_caminho_bytes).unwrap();
    assert_eq!(grc_caminho_json["distancia"], 3);
    assert_eq!(grc_caminho_json["caminho_nos"].as_array().unwrap().len(), 4);

    // 4. Indicadores Econômicos com Cache Moka (/api/indicadores/resumo)
    let ind_req = Request::builder()
        .method(Method::GET)
        .uri("/api/indicadores/resumo?uf=SP")
        .body(Body::empty())
        .unwrap();
    let ind_resp = app.clone().oneshot(ind_req).await.unwrap();
    assert_eq!(ind_resp.status(), StatusCode::OK);
    let ind_bytes = ind_resp.into_body().collect().await.unwrap().to_bytes();
    let ind_json: Value = serde_json::from_slice(&ind_bytes).unwrap();
    assert_eq!(ind_json["total_empresas"], 2);
    assert!(!ind_json["top_cnaes"].as_array().unwrap().is_empty());
    assert!(!ind_json["distribuicao_porte"]
        .as_array()
        .unwrap()
        .is_empty());
    state.analytics_cache.run_pending_tasks().await;
    assert!(state.analytics_cache.entry_count() >= 1);

    // 5. Proxy Reverso do Copiloto de IA (/api/ai/chat)
    let ai_req = Request::builder()
        .method(Method::POST)
        .uri("/api/ai/chat")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "mensagem": "Qual a Receita Bruta apurada na DRE de 2023?"
            })
            .to_string(),
        ))
        .unwrap();
    let ai_resp = app.oneshot(ai_req).await.unwrap();
    assert_eq!(ai_resp.status(), StatusCode::OK);
    let ai_bytes = ai_resp.into_body().collect().await.unwrap().to_bytes();
    let ai_json: Value = serde_json::from_slice(&ai_bytes).unwrap();
    assert!(ai_json["mensagem"]
        .as_str()
        .unwrap()
        .contains("Receita Bruta"));
    assert_eq!(ai_json["chart"]["chart_type"], "bar");

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_edge_cases_and_live_ai_gateway_proxy() {
    let (_app, mut state, temp_dir) = create_test_app();

    // 1. Sobe um servidor HTTP local real simulando o FastAPI AI Gateway
    let mock_ai_router = axum::Router::new().route(
        "/api/ai/chat",
        axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
            let msg = body["message"].as_str().unwrap_or("");
            axum::Json(json!({
                "mensagem": format!("Resposta Real do FastAPI para: {msg}"),
                "relatorio_markdown": format!("Resposta Real do FastAPI para: {msg}"),
                "chart": {
                    "chart_type": "line",
                    "title": "Evolução Mensal DRE",
                    "x_key": "mes",
                    "y_keys": ["receita"],
                    "data": [{"mes": "Jan", "receita": 1000.0}]
                },
                "ferramentas_executadas": ["buscar_dados_cadastrais"]
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, mock_ai_router).await;
    });

    state.ai_gateway_url = format!("http://{addr}");
    let app = build_router(state.clone());

    // Testa proxy reverso contra o servidor upstream ativo
    let proxy_req = Request::builder()
        .method(Method::POST)
        .uri("/api/ai/chat")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "mensagem": "Auditar CNPJ 33000167000101" }).to_string(),
        ))
        .unwrap();
    let proxy_resp = app.clone().oneshot(proxy_req).await.unwrap();
    assert_eq!(proxy_resp.status(), StatusCode::OK);
    let proxy_bytes = proxy_resp.into_body().collect().await.unwrap().to_bytes();
    let proxy_json: Value = serde_json::from_slice(&proxy_bytes).unwrap();
    assert_eq!(
        proxy_json["mensagem"],
        "Resposta Real do FastAPI para: Auditar CNPJ 33000167000101"
    );
    assert_eq!(proxy_json["chart"]["chart_type"], "line");

    // 2. Mensagem vazia no chat IA -> 400 Bad Request
    let empty_ai_req = Request::builder()
        .method(Method::POST)
        .uri("/api/ai/chat")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "mensagem": "   " }).to_string()))
        .unwrap();
    let empty_ai_resp = app.clone().oneshot(empty_ai_req).await.unwrap();
    assert_eq!(empty_ai_resp.status(), StatusCode::BAD_REQUEST);

    // 3. Upload de arquivo vazio (0 bytes) -> 400 Bad Request
    let boundary = "----EmptyFileBoundary";
    let empty_multipart = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"arquivo\"; filename=\"vazio.txt\"\r\n\
         Content-Type: text/plain\r\n\r\n\
         \r\n\
         --{boundary}--\r\n"
    );
    let empty_up_req = Request::builder()
        .method(Method::POST)
        .uri("/api/importacoes/upload")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(empty_multipart))
        .unwrap();
    let empty_up_resp = app.clone().oneshot(empty_up_req).await.unwrap();
    assert_eq!(empty_up_resp.status(), StatusCode::BAD_REQUEST);

    // 4. Busca de caminho mínimo entre nós desconectados -> 404 Not Found
    let disconnected_req = Request::builder()
        .method(Method::GET)
        .uri("/api/grc/caminho?origem=33000167000101&destino=99999999000199")
        .body(Body::empty())
        .unwrap();
    let disconnected_resp = app.clone().oneshot(disconnected_req).await.unwrap();
    assert_eq!(disconnected_resp.status(), StatusCode::NOT_FOUND);

    // 5. Autenticação M2M com Bearer API Key interna do AI Gateway
    let m2m_req = Request::builder()
        .method(Method::GET)
        .uri("/api/auth/me")
        .header(header::AUTHORIZATION, "Bearer internal_ai_gateway_key_test")
        .body(Body::empty())
        .unwrap();
    let m2m_resp = app.oneshot(m2m_req).await.unwrap();
    assert_eq!(m2m_resp.status(), StatusCode::OK);

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_task_904_security_hardening_production_guards() {
    // 1. Testa que em produção, JWT_SECRET ausente ou default falha fatalmente
    std::env::set_var("ENVIRONMENT", "production");
    std::env::remove_var("JWT_SECRET");
    let err = match AppState::from_env().await {
        Err(e) => e,
        Ok(_) => panic!("Deveria falhar com JWT_SECRET ausente"),
    };
    assert!(
        err.to_string().contains("JWT_SECRET é obrigatória"),
        "Deveria exigir JWT_SECRET em produção, erro: {}",
        err
    );

    // 2. Testa que em produção, JWT_SECRET com valor default ou curto falha fatalmente
    std::env::set_var("JWT_SECRET", "ecd-analytics-jwt-secret-key-2026-default");
    let err = match AppState::from_env().await {
        Err(e) => e,
        Ok(_) => panic!("Deveria falhar com JWT_SECRET default"),
    };
    assert!(
        err.to_string().contains("não pode usar o valor padrão"),
        "Deveria rejeitar segredo default em produção, erro: {}",
        err
    );

    std::env::set_var("JWT_SECRET", "curto_menos_de_32_chars");
    let err = match AppState::from_env().await {
        Err(e) => e,
        Ok(_) => panic!("Deveria falhar com JWT_SECRET curto"),
    };
    assert!(
        err.to_string().contains("ao menos 32 caracteres"),
        "Deveria rejeitar segredo menor que 32 chars, erro: {}",
        err
    );

    // 3. Testa que em produção, DATABASE_URL ausente falha fatalmente (sem fallback silencioso)
    std::env::set_var(
        "JWT_SECRET",
        "uma_chave_super_segura_de_producao_com_mais_de_32_caracteres_longa_e_forte!",
    );
    std::env::remove_var("DATABASE_URL");
    let err = match AppState::from_env().await {
        Err(e) => e,
        Ok(_) => panic!("Deveria falhar com DATABASE_URL ausente"),
    };
    assert!(
        err.to_string().contains("DATABASE_URL é obrigatória"),
        "Deveria exigir DATABASE_URL em produção, erro: {}",
        err
    );

    // Limpeza de variáveis de ambiente do teste
    std::env::remove_var("ENVIRONMENT");
    std::env::remove_var("JWT_SECRET");
}
