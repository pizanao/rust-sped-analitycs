//! # ecd-worker
//!
//! Background worker dedicado consumindo fila Redis (`ecd:queue:jobs`):
//! - Processamento assíncrono de lotes SPED
//! - Leitura streaming com `memmap2` (`ecd-parser`)
//! - Batch inserts atômicos de 5.000 linhas no PostgreSQL (`ecd-storage::sped_repository`)
//! - Apuração de Receita Bruta / DRE (`ecd-fiscal`) e exclusão lógica `cod_hash_sub`
//! - Publicação de eventos de progresso via Redis Pub/Sub (`ecd:events:status`)

pub mod consumer;

use std::sync::Arc;
use std::time::Duration;

use ecd_storage::create_postgres_pool;
use ecd_storage::sped_repository::SpedRepository;
use tokio::sync::watch;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::consumer::{RedisEventPublisher, WorkerConsumer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ecd_user:ecd_password@localhost:5432/ecd_db".to_string());
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    tracing::info!("Conectando ao pool PostgreSQL...");
    let pg_pool = loop {
        match create_postgres_pool(&database_url).await {
            Ok(pool) => break pool,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "Aguardando disponibilidade do PostgreSQL (nova tentativa em 2s)..."
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    let redis_client = redis::Client::open(redis_url.as_str())?;
    let storage = Arc::new(SpedRepository::new(pg_pool));
    let publisher = Arc::new(RedisEventPublisher::new(redis_client));

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::info!("Sinal SIGINT/Ctrl+C recebido; solicitando encerramento gracioso...");
            let _ = shutdown_tx.send(true);
        }
    });

    tracing::info!(
        queue = consumer::REDIS_QUEUE_JOBS,
        events_channel = consumer::REDIS_EVENTS_CHANNEL,
        "Worker de processamento SPED ECD iniciado e aguardando tarefas no Redis..."
    );

    let worker = WorkerConsumer::new(redis_url, storage, publisher);
    worker.run(shutdown_rx).await?;

    tracing::info!("Worker encerrado graciosamente.");
    Ok(())
}
