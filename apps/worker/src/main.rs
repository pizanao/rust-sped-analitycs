//! # ecd-worker
//!
//! Background worker dedicado consumindo fila Redis:
//! - Processamento assíncrono de lotes SPED
//! - Leitura streaming com memmap2
//! - Batch inserts atômicos no PostgreSQL
//! - Publicação de eventos de progresso via Redis Pub/Sub

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Worker de processamento SPED ECD iniciado e aguardando tarefas no Redis...");

    // Loop do worker será implementado no plano
    tokio::signal::ctrl_c().await?;
    tracing::info!("Worker encerrando graciosamente...");

    Ok(())
}
