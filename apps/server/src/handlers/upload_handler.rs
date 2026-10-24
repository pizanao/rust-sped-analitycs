//! Handlers HTTP de upload multipart streaming (`/api/importacoes/upload`) e consulta de jobs (`/api/importacoes`).
//!
//! Nunca executa o parsing completo do arquivo SPED na thread da requisição HTTP;
//! grava os chunks diretamente em `media/ecd_stage/aguardando/` e despacha `ImportSpedJobPayload`
//! para a fila Redis `ecd:queue:jobs`.

use crate::AppState;
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use ecd_domain::{ImportSpedJobPayload, ImportacaoEcd, JobStatusEvent, StatusImportacao};
use redis::AsyncCommands;
use serde_json::json;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

/// Nome da fila Redis consumida pelo `apps/worker`.
pub const REDIS_JOBS_QUEUE: &str = "ecd:queue:jobs";

/// Nome do canal Redis Pub/Sub para eventos de progresso.
pub const REDIS_STATUS_CHANNEL: &str = "ecd:events:status";

/// Extrai o CNPJ (14 dígitos) do registro de abertura `|0000|` a partir dos primeiros bytes do arquivo.
pub fn extrair_cnpj_cabecalho_sped(header_bytes: &[u8]) -> Option<String> {
    let preview = String::from_utf8_lossy(header_bytes);
    for line in preview.lines().take(5) {
        let trimmed = line.trim();
        if trimmed.starts_with("|0000|") {
            let fields: Vec<&str> = trimmed.split('|').collect();
            // Layout oficial SPED ECD: |0000|LECD|DT_INI|DT_FIM|NOME|CNPJ|UF|...
            // indices após split('|'): 0:"", 1:"0000", 2:"LECD", 3:DT_INI, 4:DT_FIM, 5:NOME, 6:CNPJ
            if let Some(raw_cnpj) = fields.get(6) {
                let digits: String = raw_cnpj.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() == 14 {
                    return Some(digits);
                }
            }
            // Fallback: procura qualquer campo com 14 dígitos numéricos na linha 0000
            for field in fields.iter().skip(2) {
                let digits: String = field.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() == 14 {
                    return Some(digits);
                }
            }
        }
    }
    None
}

/// Sanitiza o nome do arquivo para armazenamento seguro em disco.
pub fn sanitizar_nome_arquivo(nome: &str) -> String {
    let base = std::path::Path::new(nome)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sped_ecd.txt");

    let limpo: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if limpo.is_empty() {
        "sped_ecd.txt".to_string()
    } else {
        limpo
    }
}

/// `POST /api/importacoes/upload`
/// Recebe o arquivo SPED via streaming multipart, grava em `media/ecd_stage/aguardando/`
/// e publica `ImportSpedJobPayload` na fila Redis `ecd:queue:jobs`.
pub async fn upload_sped_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    tokio::fs::create_dir_all(&state.stage_dir)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "erro": format!("Falha ao criar diretório de staging '{}': {e}", state.stage_dir.display()),
                })),
            )
        })?;

    let job_id = Uuid::now_v7();
    let mut cnpj_informado: Option<String> = None;
    let mut arquivo_processado = false;
    let mut nome_original = String::from("sped_ecd.txt");
    let mut caminho_final_str = String::new();
    let mut tamanho_bytes: i64 = 0;
    let mut hash_arquivo = String::new();
    let mut header_sample: Vec<u8> = Vec::with_capacity(2048);

    while let Some(mut field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "erro": format!("Falha ao ler stream multipart: {e}"),
            })),
        )
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "cnpj" {
            if let Ok(text) = field.text().await {
                let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() == 14 {
                    cnpj_informado = Some(digits);
                }
            }
            continue;
        }

        if field_name == "arquivo" || field_name == "file" || field.file_name().is_some() {
            if let Some(fname) = field.file_name() {
                nome_original = sanitizar_nome_arquivo(fname);
            }

            let file_path = state.stage_dir.join(format!("{job_id}_{nome_original}"));
            let mut file = tokio::fs::File::create(&file_path).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "erro": format!("Não foi possível criar arquivo em disco: {e}"),
                    })),
                )
            })?;

            let mut hasher = Sha1::new();

            while let Some(chunk) = field.chunk().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "erro": format!("Erro durante streaming de bytes do arquivo: {e}"),
                    })),
                )
            })? {
                if header_sample.len() < 2048 {
                    let restante = 2048 - header_sample.len();
                    let take_len = restante.min(chunk.len());
                    header_sample.extend_from_slice(&chunk[..take_len]);
                }

                hasher.update(&chunk);
                tamanho_bytes += chunk.len() as i64;

                file.write_all(&chunk).await.map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "erro": format!("Erro de I/O ao gravar chunk em disco: {e}"),
                        })),
                    )
                })?;
            }

            file.flush().await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "erro": format!("Erro ao finalizar gravação em disco: {e}"),
                    })),
                )
            })?;

            if tamanho_bytes == 0 {
                let _ = tokio::fs::remove_file(&file_path).await;
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "erro": "O arquivo SPED enviado está vazio (0 bytes).",
                    })),
                ));
            }

            hash_arquivo = format!("{:x}", hasher.finalize());
            caminho_final_str = file_path.to_string_lossy().to_string();
            arquivo_processado = true;
            break;
        }
    }

    if !arquivo_processado {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "erro": "Nenhum campo 'arquivo' encontrado no upload multipart.",
            })),
        ));
    }

    let cnpj = cnpj_informado
        .or_else(|| extrair_cnpj_cabecalho_sped(&header_sample))
        .unwrap_or_else(|| "00000000000191".to_string());

    let agora = Utc::now();
    let importacao = ImportacaoEcd {
        id: job_id,
        hash_arquivo: hash_arquivo.clone(),
        cnpj: cnpj.clone(),
        nome_arquivo_original: nome_original.clone(),
        caminho_armazenamento: caminho_final_str.clone(),
        tamanho_bytes,
        status: StatusImportacao::Aguardando,
        linhas_processadas: 0,
        tempo_processamento_ms: 0,
        mensagem_erro: None,
        criado_em: agora,
        atualizado_em: agora,
    };

    // 1. Persiste no PostgreSQL se disponível e sincroniza store em memória
    if let Some(ref repo) = state.fiscal_repo {
        let _ = repo.registrar_importacao(&importacao).await;
    }
    {
        let mut jobs = state.in_memory_jobs.write().await;
        jobs.retain(|j| j.id != importacao.id && j.hash_arquivo != importacao.hash_arquivo);
        jobs.insert(0, importacao.clone());
    }

    // 2. Constrói e publica `ImportSpedJobPayload` na fila Redis (`ecd:queue:jobs`)
    let job_payload = ImportSpedJobPayload {
        job_id,
        hash_arquivo: hash_arquivo.clone(),
        cnpj: cnpj.clone(),
        caminho_arquivo: caminho_final_str.clone(),
        nome_original: nome_original.clone(),
        tamanho_bytes,
        enfileirado_em: agora,
    };

    let payload_json = serde_json::to_string(&job_payload).unwrap_or_default();

    let evento_inicial = JobStatusEvent {
        job_id,
        hash_arquivo: hash_arquivo.clone(),
        status: StatusImportacao::Aguardando,
        progresso_percentual: 5,
        linhas_processadas: 0,
        mensagem: format!(
            "Arquivo '{}' ({} bytes) gravado em staging e enfileirado para processamento.",
            nome_original, tamanho_bytes
        ),
        timestamp: agora,
    };
    let evento_json = serde_json::to_string(&evento_inicial).unwrap_or_default();

    if let Some(ref redis_client) = state.redis_client {
        if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
            let _: Result<i64, _> = conn.lpush(REDIS_JOBS_QUEUE, &payload_json).await;
            let _: Result<i64, _> = conn.publish(REDIS_STATUS_CHANNEL, &evento_json).await;
        }
    }

    {
        let mut queue = state.in_memory_queue.write().await;
        queue.push(job_payload);
    }
    let _ = state.event_tx.send(evento_inicial);

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "job_id": job_id,
            "hash_arquivo": hash_arquivo,
            "cnpj": cnpj,
            "caminho_armazenamento": caminho_final_str,
            "tamanho_bytes": tamanho_bytes,
            "status": "Aguardando",
            "mensagem": "Arquivo SPED recebido via streaming e enfileirado com sucesso.",
        })),
    )
        .into_response())
}

/// `GET /api/importacoes`
/// Lista todas as importações registradas.
pub async fn listar_importacoes_handler(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(ref repo) = state.fiscal_repo {
        if let Ok(lista) = repo.listar_importacoes(100).await {
            if !lista.is_empty() {
                return (StatusCode::OK, Json(lista));
            }
        }
    }

    let jobs = state.in_memory_jobs.read().await;
    (StatusCode::OK, Json(jobs.clone()))
}

/// `GET /api/importacoes/:id`
/// Obtém os detalhes de um job de importação específico pelo UUID.
pub async fn obter_importacao_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ImportacaoEcd>, (StatusCode, Json<serde_json::Value>)> {
    if let Some(ref repo) = state.fiscal_repo {
        if let Ok(Some(imp)) = repo.obter_importacao(id).await {
            return Ok(Json(imp));
        }
    }

    let jobs = state.in_memory_jobs.read().await;
    if let Some(found) = jobs.iter().find(|j| j.id == id) {
        return Ok(Json(found.clone()));
    }

    Err((
        StatusCode::NOT_FOUND,
        Json(json!({
            "erro": format!("Importação {id} não encontrada"),
        })),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extrair_cnpj_cabecalho_sped() {
        let sample = b"|0000|LECD|01012023|31122023|EMPRESA TESTE S.A.|33000167000101|RJ|||0|1|1|0||1|0||N|N|0|0|10|\r\n|I001|0|\r\n";
        assert_eq!(
            extrair_cnpj_cabecalho_sped(sample),
            Some("33000167000101".to_string())
        );
    }

    #[test]
    fn test_sanitizar_nome_arquivo() {
        assert_eq!(
            sanitizar_nome_arquivo("../../etc/passwd/SPED 2023!.txt"),
            "SPED_2023_.txt"
        );
    }
}
