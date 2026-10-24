//! Endpoint Server-Sent Events (`/api/jobs/:id/stream`) subscrito no canal Redis `ecd:events:status`.

use crate::{handlers::upload_handler::REDIS_STATUS_CHANNEL, AppState};
use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use ecd_domain::{JobStatusEvent, StatusImportacao};
use futures::{stream, Stream, StreamExt};
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc;
use uuid::Uuid;

/// `GET /api/jobs/:id/stream`
/// Retorna um stream Server-Sent Events (SSE) subscrito no canal Redis Pub/Sub `ecd:events:status`
/// e no barramento interno de eventos, filtrando pelo `job_id` solicitado.
pub async fn job_stream_sse_handler(
    State(state): State<AppState>,
    Path(raw_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let target_uuid = Uuid::parse_str(raw_id.trim()).ok();
    let (tx, rx) = mpsc::channel::<JobStatusEvent>(128);

    // 1. Se o job já estiver registrado, emite o estado atual imediatamente na abertura do stream
    if let Some(job_id) = target_uuid {
        let mut initial_event: Option<JobStatusEvent> = None;

        if let Some(ref repo) = state.fiscal_repo {
            if let Ok(Some(imp)) = repo.obter_importacao(job_id).await {
                let prog = match imp.status {
                    StatusImportacao::Sucesso => 100,
                    StatusImportacao::Erro => 100,
                    StatusImportacao::EmProcessamento => 50,
                    StatusImportacao::Substituida => 100,
                    StatusImportacao::Aguardando => 5,
                };
                initial_event = Some(JobStatusEvent {
                    job_id: imp.id,
                    hash_arquivo: imp.hash_arquivo,
                    status: imp.status,
                    progresso_percentual: prog,
                    linhas_processadas: imp.linhas_processadas.max(0) as usize,
                    mensagem: imp.mensagem_erro.unwrap_or_else(|| {
                        format!("Monitorando job {} ({})", imp.id, imp.nome_arquivo_original)
                    }),
                    timestamp: imp.atualizado_em,
                });
            }
        }

        if initial_event.is_none() {
            let jobs = state.in_memory_jobs.read().await;
            if let Some(imp) = jobs.iter().find(|j| j.id == job_id) {
                let prog = match imp.status {
                    StatusImportacao::Sucesso => 100,
                    StatusImportacao::Erro => 100,
                    StatusImportacao::EmProcessamento => 50,
                    StatusImportacao::Substituida => 100,
                    StatusImportacao::Aguardando => 5,
                };
                initial_event = Some(JobStatusEvent {
                    job_id: imp.id,
                    hash_arquivo: imp.hash_arquivo.clone(),
                    status: imp.status,
                    progresso_percentual: prog,
                    linhas_processadas: imp.linhas_processadas.max(0) as usize,
                    mensagem: imp.mensagem_erro.clone().unwrap_or_else(|| {
                        format!("Monitorando job {} ({})", imp.id, imp.nome_arquivo_original)
                    }),
                    timestamp: imp.atualizado_em,
                });
            }
        }

        if let Some(ev) = initial_event {
            let _ = tx.send(ev).await;
        }
    }

    // 2. Subscreve no barramento broadcast local (`state.event_tx`)
    let mut bcast_rx = state.event_tx.subscribe();
    let tx_bcast = tx.clone();
    tokio::spawn(async move {
        while let Ok(ev) = bcast_rx.recv().await {
            let matches = match target_uuid {
                Some(uid) => ev.job_id == uid,
                None => true,
            };
            if matches && tx_bcast.send(ev).await.is_err() {
                break;
            }
        }
    });

    // 3. Subscreve no canal Redis Pub/Sub `ecd:events:status` quando configurado
    if let Some(redis_client) = state.redis_client.clone() {
        let tx_redis = tx;
        tokio::spawn(async move {
            if let Ok(mut pubsub) = redis_client.get_async_pubsub().await {
                if pubsub.subscribe(REDIS_STATUS_CHANNEL).await.is_ok() {
                    let mut msg_stream = pubsub.on_message();
                    while let Some(msg) = msg_stream.next().await {
                        if let Ok(payload_str) = msg.get_payload::<String>() {
                            if let Ok(ev) = serde_json::from_str::<JobStatusEvent>(&payload_str) {
                                let matches = match target_uuid {
                                    Some(uid) => ev.job_id == uid,
                                    None => true,
                                };
                                if matches && tx_redis.send(ev).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    let event_stream = stream::unfold(rx, |mut receiver| async move {
        let ev = receiver.recv().await?;
        let json_data = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".to_string());
        let sse_event = Event::default().event("message").data(json_data);
        Some((Ok(sse_event), receiver))
    });

    Sse::new(event_stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
