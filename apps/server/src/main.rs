//! # ecd-server
//!
//! Servidor HTTP principal em Axum:
//! - Endpoints RESTful para a SPA
//! - Autenticação JWT com cookies HttpOnly
//! - Upload multipart streaming de arquivos SPED
//! - Server-Sent Events (SSE) para logs em tempo real
//! - Proxy reverso para o AI Gateway

use axum::{routing::get, Json, Router};
use serde_json::json;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok", "service": "ecd-server"})) }));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("Servidor Axum iniciado em http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
