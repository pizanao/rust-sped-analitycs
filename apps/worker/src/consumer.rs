//! Consumer assíncrono da fila Redis (`ecd:queue:jobs`), pipeline de ingestão SPED ECD
//! (`memmap2` -> `sped_repository` -> `ecd-fiscal`) e publicação de progresso via Pub/Sub (`ecd:events:status`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use ecd_domain::{ImportSpedJobPayload, JobStatusEvent, ResultadoExercicio, StatusImportacao};
use ecd_fiscal::{ReceitaApurada, ReceitaExtractor, TipoReceita};
use ecd_parser::{enrich_i355, enrich_plano_contas, SpedDocument};
use ecd_storage::sped_repository::{BatchInsertSummary, SpedBatchRecords, SpedRepository};
use redis::AsyncCommands;
use rust_decimal::Decimal;
use serde_json::json;
use tokio::sync::watch;
use uuid::Uuid;

/// Nome canônico da fila Redis onde o servidor HTTP enfileira jobs de importação SPED.
pub const REDIS_QUEUE_JOBS: &str = "ecd:queue:jobs";

/// Canal Redis Pub/Sub monitorado pelo endpoint SSE do servidor HTTP.
pub const REDIS_EVENTS_CHANNEL: &str = "ecd:events:status";

/// Timeout padrão (em segundos) do comando bloqueante `BRPOP`.
pub const DEFAULT_BRPOP_TIMEOUT_SECS: f64 = 5.0;

/// Subpasta de destino para arquivos processados com sucesso.
pub const SUBDIR_PROCESSADOS: &str = "processados";

/// Subpasta de destino para arquivos com falha de parsing ou validação.
pub const SUBDIR_ERRO: &str = "erro";

/// Resultado detalhado do processamento de um job pelo worker.
#[derive(Clone, Debug)]
pub struct JobExecutionOutcome {
    pub job_id: Uuid,
    pub hash_arquivo: String,
    pub cnpj: String,
    pub status_final: StatusImportacao,
    pub linhas_processadas: usize,
    pub registros_inseridos: usize,
    pub receita_apurada: ReceitaApurada,
    pub escrituracao_substituida_hash: Option<String>,
    pub caminho_final: PathBuf,
    pub tempo_processamento_ms: i32,
}

/// Abstração para publicação de eventos `JobStatusEvent` (permite testes determinísticos sem Redis).
#[async_trait]
pub trait JobEventPublisher: Send + Sync {
    async fn publish_status(&self, event: &JobStatusEvent) -> anyhow::Result<()>;
}

/// Publicador de eventos real sobre Redis Pub/Sub (`PUBLISH ecd:events:status`).
#[derive(Clone)]
pub struct RedisEventPublisher {
    client: redis::Client,
    channel: String,
}

impl RedisEventPublisher {
    pub fn new(client: redis::Client) -> Self {
        Self {
            client,
            channel: REDIS_EVENTS_CHANNEL.to_string(),
        }
    }

    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = channel.into();
        self
    }
}

#[async_trait]
impl JobEventPublisher for RedisEventPublisher {
    async fn publish_status(&self, event: &JobStatusEvent) -> anyhow::Result<()> {
        let payload = serde_json::to_string(event)?;
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: i64 = conn.publish(&self.channel, payload).await?;
        Ok(())
    }
}

/// Abstração da camada de persistência para o pipeline do Worker.
#[async_trait]
pub trait WorkerStorage: Send + Sync {
    async fn carregar_plano_contas_base(&self) -> anyhow::Result<HashMap<String, String>>;
    async fn obter_hashes_substituidos(&self, cnpj: &str) -> anyhow::Result<HashSet<String>>;
    async fn registrar_inicio_importacao(&self, job: &ImportSpedJobPayload) -> anyhow::Result<()>;
    async fn persistir_ingestao_completa(
        &self,
        batch: &SpedBatchRecords,
        resultado: &ResultadoExercicio,
    ) -> anyhow::Result<BatchInsertSummary>;
    #[allow(clippy::too_many_arguments)]
    async fn finalizar_importacao_sucesso(
        &self,
        job_id: Uuid,
        hash_arquivo: &str,
        cnpj: &str,
        caminho_final: &str,
        linhas_processadas: i32,
        tempo_ms: i32,
        ja_substituida: bool,
    ) -> anyhow::Result<()>;
    async fn finalizar_importacao_erro(
        &self,
        job_id: Uuid,
        hash_arquivo: &str,
        caminho_final: &str,
        tempo_ms: i32,
        mensagem_erro: &str,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl WorkerStorage for SpedRepository {
    async fn carregar_plano_contas_base(&self) -> anyhow::Result<HashMap<String, String>> {
        Ok(SpedRepository::carregar_plano_contas_base(self).await?)
    }

    async fn obter_hashes_substituidos(&self, cnpj: &str) -> anyhow::Result<HashSet<String>> {
        Ok(SpedRepository::obter_hashes_substituidos(self, cnpj).await?)
    }

    async fn registrar_inicio_importacao(&self, job: &ImportSpedJobPayload) -> anyhow::Result<()> {
        SpedRepository::registrar_inicio_importacao(self, job).await?;
        Ok(())
    }

    async fn persistir_ingestao_completa(
        &self,
        batch: &SpedBatchRecords,
        resultado: &ResultadoExercicio,
    ) -> anyhow::Result<BatchInsertSummary> {
        Ok(SpedRepository::persistir_ingestao_completa(self, batch, resultado).await?)
    }

    async fn finalizar_importacao_sucesso(
        &self,
        job_id: Uuid,
        hash_arquivo: &str,
        cnpj: &str,
        caminho_final: &str,
        linhas_processadas: i32,
        tempo_ms: i32,
        ja_substituida: bool,
    ) -> anyhow::Result<()> {
        SpedRepository::finalizar_importacao_sucesso(
            self,
            job_id,
            hash_arquivo,
            cnpj,
            caminho_final,
            linhas_processadas,
            tempo_ms,
            ja_substituida,
        )
        .await?;
        Ok(())
    }

    async fn finalizar_importacao_erro(
        &self,
        job_id: Uuid,
        hash_arquivo: &str,
        caminho_final: &str,
        tempo_ms: i32,
        mensagem_erro: &str,
    ) -> anyhow::Result<()> {
        SpedRepository::finalizar_importacao_erro(
            self,
            job_id,
            hash_arquivo,
            caminho_final,
            tempo_ms,
            mensagem_erro,
        )
        .await?;
        Ok(())
    }
}

/// Converte um `SpedDocument` lido pelo `ecd-parser` na estrutura de lote `SpedBatchRecords`.
pub fn document_to_batch_records(doc: SpedDocument) -> SpedBatchRecords {
    SpedBatchRecords {
        reg_0000: doc.reg_0000,
        reg_i010: doc.reg_i010,
        reg_i030: doc.reg_i030,
        reg_i050: doc.reg_i050,
        reg_i051: doc.reg_i051,
        reg_i052: doc.reg_i052,
        reg_i053: doc.reg_i053,
        plano_contas: doc.plano_contas,
        reg_i150: doc.reg_i150,
        reg_i155: doc.reg_i155,
        reg_i355: doc.reg_i355,
        reg_j005: doc.reg_j005,
        reg_j100: doc.reg_j100,
        reg_j150: doc.reg_j150,
        reg_j210: doc.reg_j210,
        reg_j215: doc.reg_j215,
    }
}

/// Constrói o `ResultadoExercicio` e a `ReceitaApurada` usando o motor `ecd-fiscal` (`ReceitaExtractor`).
pub fn apurar_resultado_exercicio(
    doc: &SpedDocument,
    hashes_substituidos: Option<&HashSet<String>>,
) -> (ResultadoExercicio, ReceitaApurada) {
    let extractor = ReceitaExtractor::new();

    // Filtra hashes substituídos de outras escriturações sem zerar as rubricas do próprio documento atual
    let outros_hashes_substituidos: Option<HashSet<String>> = hashes_substituidos.map(|subs| {
        subs.iter()
            .filter(|h| *h != &doc.reg_0000.hash_arquivo_id)
            .cloned()
            .collect()
    });

    let j150_ativos: Vec<_> = doc
        .reg_j150
        .iter()
        .filter(|r| {
            outros_hashes_substituidos
                .as_ref()
                .map(|subs| !subs.contains(&r.hash_arquivo))
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    let i355_ativos: Vec<_> = doc
        .reg_i355
        .iter()
        .filter(|r| {
            outros_hashes_substituidos
                .as_ref()
                .map(|subs| !subs.contains(&r.hash_arquivo))
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    let apuracao = extractor.apurar_consolidado(&j150_ativos, &i355_ativos);
    let extraidas_j150 = extractor.filtra_receitas_j150(
        &doc.reg_0000.cnpj,
        &doc.reg_j150,
        outros_hashes_substituidos.as_ref(),
    );

    let apuracao_i355 = extractor.apurar_i355(&i355_ativos);

    let detalhes_j150 = Some(json!({
        "receita_bruta": apuracao.receita_bruta,
        "receita_servico": apuracao.receita_servico,
        "outras_receitas": apuracao.outras_receitas,
        "total_rubricas_dre": doc.reg_j150.len(),
        "rubricas_elegiveis": extraidas_j150,
        "linhas_identificadas": apuracao
            .linhas_identificadas
            .iter()
            .filter(|l| matches!(l.tipo, TipoReceita::Bruta | TipoReceita::Servico | TipoReceita::Outras))
            .collect::<Vec<_>>(),
    }));

    let detalhes_i355 = Some(json!({
        "receita_bruta_i355": apuracao_i355.receita_bruta,
        "receita_servico_i355": apuracao_i355.receita_servico,
        "outras_receitas_i355": apuracao_i355.outras_receitas,
        "total_contas_i355": doc.reg_i355.len(),
        "linhas_identificadas": apuracao_i355.linhas_identificadas,
    }));

    let situacao = if hashes_substituidos
        .map(|subs| subs.contains(&doc.reg_0000.hash_arquivo_id))
        .unwrap_or(false)
    {
        "X".to_string()
    } else {
        "S".to_string()
    };

    let resultado = ResultadoExercicio {
        id: Uuid::now_v7(),
        cnpj: doc.reg_0000.cnpj.clone(),
        razao_social: doc.reg_0000.nome_razao.clone(),
        periodo_inicial: doc.reg_0000.dt_ini,
        periodo_final: doc.reg_0000.dt_fim,
        cnpj_scp: doc.reg_0000.cod_scp.clone(),
        receita_bruta: apuracao.receita_bruta,
        receita_servico: apuracao.receita_servico,
        outras_receitas: apuracao.outras_receitas,
        receita_acumulada: apuracao.receita_acumulada,
        total_nota: Decimal::ZERO,
        hash_arquivo: doc.reg_0000.hash_arquivo_id.clone(),
        cod_hash_sub: doc.reg_0000.cod_hash_sub.clone(),
        situacao,
        detalhes_j150,
        detalhes_i355,
        criado_em: Utc::now(),
    };

    (resultado, apuracao)
}

/// Calcula o caminho de destino (`processados/` ou `erro/`) a partir do caminho de estágio original.
pub fn calcular_caminho_destino(caminho_origem: &Path, subpasta_destino: &str) -> PathBuf {
    let nome_arquivo = caminho_origem
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("sped_ecd.txt"));

    let parent = caminho_origem.parent().unwrap_or_else(|| Path::new("."));
    let base_stage = if parent
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("aguardando"))
    {
        parent.parent().unwrap_or(parent)
    } else {
        parent
    };

    base_stage.join(subpasta_destino).join(nome_arquivo)
}

/// Move o arquivo SPED processado para `processados/` ou `erro/`, criando o diretório se necessário.
pub async fn mover_arquivo_estagio(
    caminho_origem: impl AsRef<Path>,
    subpasta_destino: &str,
) -> std::io::Result<PathBuf> {
    let origem = caminho_origem.as_ref();
    let destino = calcular_caminho_destino(origem, subpasta_destino);

    if let Some(dir_pai) = destino.parent() {
        tokio::fs::create_dir_all(dir_pai).await?;
    }

    if tokio::fs::try_exists(origem).await.unwrap_or(false) {
        match tokio::fs::rename(origem, &destino).await {
            Ok(()) => {}
            Err(_) => {
                // Fallback seguro caso origem e destino estejam em partições/mounts distintos (EXDEV)
                tokio::fs::copy(origem, &destino).await?;
                let _ = tokio::fs::remove_file(origem).await;
            }
        }
    }

    Ok(destino)
}

/// Emite um `JobStatusEvent` sem interromper o pipeline caso o canal Pub/Sub falhe momentaneamente.
async fn emitir_evento(
    publisher: &dyn JobEventPublisher,
    job: &ImportSpedJobPayload,
    status: StatusImportacao,
    progresso_percentual: u8,
    linhas_processadas: usize,
    mensagem: impl Into<String>,
) {
    let evento = JobStatusEvent {
        job_id: job.job_id,
        hash_arquivo: job.hash_arquivo.clone(),
        status,
        progresso_percentual,
        linhas_processadas,
        mensagem: mensagem.into(),
        timestamp: Utc::now(),
    };

    if let Err(err) = publisher.publish_status(&evento).await {
        tracing::warn!(
            job_id = %job.job_id,
            hash_arquivo = %job.hash_arquivo,
            error = %err,
            "Falha ao publicar JobStatusEvent no Redis Pub/Sub"
        );
    }
}

/// Executa o pipeline completo de ingestão para um `ImportSpedJobPayload`:
/// 1. Atualiza status para `EmProcessamento` e publica evento SSE (10%)
/// 2. Abre e decodifica o arquivo SPED via `memmap2` (`ecd-parser`)
/// 3. Enriquece o plano de contas (`plano_contas_base`) e apura receitas DRE/I355 (`ecd-fiscal`)
/// 4. Persiste atomicamente em lotes de 5.000 linhas no PostgreSQL (`sped_repository`) e aplica exclusão lógica `cod_hash_sub`
/// 5. Move o arquivo para `processados/` (ou `erro/` em caso de falha) e publica evento final (100%)
pub async fn processar_job(
    job: &ImportSpedJobPayload,
    storage: Arc<dyn WorkerStorage>,
    publisher: Arc<dyn JobEventPublisher>,
) -> anyhow::Result<JobExecutionOutcome> {
    let inicio = Instant::now();

    if let Err(err) = storage.registrar_inicio_importacao(job).await {
        tracing::warn!(
            job_id = %job.job_id,
            error = %err,
            "Aviso ao registrar início de importação no banco"
        );
    }

    emitir_evento(
        publisher.as_ref(),
        job,
        StatusImportacao::EmProcessamento,
        10,
        0,
        format!(
            "Iniciando leitura zero-copy (memmap2) do arquivo '{}'...",
            job.nome_original
        ),
    )
    .await;

    let resultado_pipeline: anyhow::Result<(
        String,
        usize,
        BatchInsertSummary,
        ReceitaApurada,
    )> = async {
        let caminho_clone = PathBuf::from(&job.caminho_arquivo);
        let hash_clone = job.hash_arquivo.clone();
        let data_entrega = job.enfileirado_em;

        // 1. Leitura e parsing com memmap2 em thread de bloqueio dedicada
        let mut doc = tokio::task::spawn_blocking(move || {
            SpedDocument::parse_file(&caminho_clone, &hash_clone, data_entrega)
        })
        .await
        .map_err(|e| anyhow::anyhow!("Erro de join na thread do parser SPED: {e}"))??;

        emitir_evento(
            publisher.as_ref(),
            job,
            StatusImportacao::EmProcessamento,
            45,
            doc.total_linhas,
            format!(
                "Arquivo decodificado ({} linhas). Enriquecendo plano de contas e apurando DRE/I355...",
                doc.total_linhas
            ),
        )
        .await;

        // 2. Enriquecimento opcional com catálogo plano_contas_base do banco
        if let Ok(plano_ref) = storage.carregar_plano_contas_base().await {
            if !plano_ref.is_empty() {
                enrich_plano_contas(&mut doc.plano_contas, &plano_ref);
                for item in &mut doc.reg_i355 {
                    enrich_i355(item, &doc.plano_contas, &doc.reg_j150);
                }
            }
        }

        // 3. Consulta hashes já substituídos para o CNPJ e executa apuração fiscal
        let hashes_sub = storage
            .obter_hashes_substituidos(&doc.reg_0000.cnpj)
            .await
            .unwrap_or_default();

        let (resultado_exercicio, receita_apurada) =
            apurar_resultado_exercicio(&doc, Some(&hashes_sub));

        let cnpj_doc = doc.reg_0000.cnpj.clone();
        let total_linhas = doc.total_linhas;

        emitir_evento(
            publisher.as_ref(),
            job,
            StatusImportacao::EmProcessamento,
            75,
            total_linhas,
            format!(
                "Executando batch inserts atômicos (lotes de 5.000 linhas) para CNPJ {}...",
                cnpj_doc
            ),
        )
        .await;

        // 4. Persistência transacional atômica de todas as tabelas core_*, reg* e resultados_exercicio (sem clonar doc)
        let batch_records = document_to_batch_records(doc);
        let summary = storage
            .persistir_ingestao_completa(&batch_records, &resultado_exercicio)
            .await?;

        Ok((cnpj_doc, total_linhas, summary, receita_apurada))
    }
    .await;

    let tempo_ms = inicio.elapsed().as_millis().min(i32::MAX as u128) as i32;

    match resultado_pipeline {
        Ok((cnpj_doc, total_linhas, summary, receita_apurada)) => {
            let caminho_final = mover_arquivo_estagio(&job.caminho_arquivo, SUBDIR_PROCESSADOS)
                .await
                .unwrap_or_else(|_| PathBuf::from(&job.caminho_arquivo));
            let caminho_final_str = caminho_final.to_string_lossy().to_string();

            storage
                .finalizar_importacao_sucesso(
                    job.job_id,
                    &job.hash_arquivo,
                    &cnpj_doc,
                    &caminho_final_str,
                    total_linhas.min(i32::MAX as usize) as i32,
                    tempo_ms,
                    summary.arquivo_atual_ja_substituido,
                )
                .await?;

            let status_final = if summary.arquivo_atual_ja_substituido {
                StatusImportacao::Substituida
            } else {
                StatusImportacao::Sucesso
            };

            let msg_sub = match &summary.escrituracao_substituida_hash {
                Some(h) => format!(" (substituiu escrituração {h})"),
                None => String::new(),
            };

            emitir_evento(
                publisher.as_ref(),
                job,
                status_final,
                100,
                total_linhas,
                format!(
                    "Processamento concluído em {}ms! {} registros gravados, Receita Bruta: R$ {}{}.",
                    tempo_ms,
                    summary.total_registros_inseridos,
                    receita_apurada.receita_bruta,
                    msg_sub
                ),
            )
            .await;

            Ok(JobExecutionOutcome {
                job_id: job.job_id,
                hash_arquivo: job.hash_arquivo.clone(),
                cnpj: cnpj_doc,
                status_final,
                linhas_processadas: total_linhas,
                registros_inseridos: summary.total_registros_inseridos,
                receita_apurada,
                escrituracao_substituida_hash: summary.escrituracao_substituida_hash,
                caminho_final,
                tempo_processamento_ms: tempo_ms,
            })
        }
        Err(err) => {
            let erro_msg = err.to_string();
            tracing::error!(
                job_id = %job.job_id,
                hash_arquivo = %job.hash_arquivo,
                error = %erro_msg,
                "Falha no processamento do arquivo SPED; movendo para pasta de erro"
            );

            let caminho_erro = mover_arquivo_estagio(&job.caminho_arquivo, SUBDIR_ERRO)
                .await
                .unwrap_or_else(|_| PathBuf::from(&job.caminho_arquivo));
            let caminho_erro_str = caminho_erro.to_string_lossy().to_string();

            let _ = storage
                .finalizar_importacao_erro(
                    job.job_id,
                    &job.hash_arquivo,
                    &caminho_erro_str,
                    tempo_ms,
                    &erro_msg,
                )
                .await;

            emitir_evento(
                publisher.as_ref(),
                job,
                StatusImportacao::Erro,
                100,
                0,
                format!("Erro ao processar arquivo SPED: {erro_msg}"),
            )
            .await;

            Err(err)
        }
    }
}

/// Calcula o tempo de espera de reconexão exponencial (limitado a 5 segundos).
pub fn calcular_backoff_reconexao(tentativa: u32) -> Duration {
    let shift = tentativa.min(4);
    let ms = (500u64.saturating_mul(1u64 << shift)).min(5_000);
    Duration::from_millis(ms)
}

/// Consumer assíncrono Redis (`BRPOP ecd:queue:jobs`) com reconexão automática e graceful shutdown.
pub struct WorkerConsumer {
    redis_url: String,
    queue_name: String,
    brpop_timeout_secs: f64,
    storage: Arc<dyn WorkerStorage>,
    publisher: Arc<dyn JobEventPublisher>,
}

impl WorkerConsumer {
    pub fn new(
        redis_url: impl Into<String>,
        storage: Arc<dyn WorkerStorage>,
        publisher: Arc<dyn JobEventPublisher>,
    ) -> Self {
        Self {
            redis_url: redis_url.into(),
            queue_name: REDIS_QUEUE_JOBS.to_string(),
            brpop_timeout_secs: DEFAULT_BRPOP_TIMEOUT_SECS,
            storage,
            publisher,
        }
    }

    pub fn with_queue_name(mut self, queue_name: impl Into<String>) -> Self {
        self.queue_name = queue_name.into();
        self
    }

    pub fn with_brpop_timeout(mut self, timeout_secs: f64) -> Self {
        self.brpop_timeout_secs = timeout_secs.max(0.05);
        self
    }

    /// Executa o loop contínuo de consumo da fila Redis até receber sinal de encerramento (`shutdown_rx`).
    pub async fn run(&self, mut shutdown_rx: watch::Receiver<bool>) -> anyhow::Result<()> {
        let mut tentativa_reconexao = 0u32;

        loop {
            if *shutdown_rx.borrow() {
                tracing::info!("Sinal de encerramento recebido antes de conectar ao Redis.");
                break;
            }

            let client = match redis::Client::open(self.redis_url.as_str()) {
                Ok(c) => c,
                Err(err) => {
                    tentativa_reconexao = tentativa_reconexao.saturating_add(1);
                    let delay = calcular_backoff_reconexao(tentativa_reconexao);
                    tracing::error!(
                        error = %err,
                        delay_ms = delay.as_millis(),
                        "URL Redis inválida ou erro ao criar cliente; aguardando para tentar novamente"
                    );
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => continue,
                        res = shutdown_rx.changed() => {
                            if res.is_err() || *shutdown_rx.borrow() {
                                break;
                            }
                            continue;
                        }
                    }
                }
            };

            let mut conn = match client.get_multiplexed_async_connection().await {
                Ok(c) => {
                    if tentativa_reconexao > 0 {
                        tracing::info!(
                            tentativas = tentativa_reconexao,
                            "Conexão com o Redis restabelecida com sucesso!"
                        );
                    }
                    tentativa_reconexao = 0;
                    c
                }
                Err(err) => {
                    tentativa_reconexao = tentativa_reconexao.saturating_add(1);
                    let delay = calcular_backoff_reconexao(tentativa_reconexao);
                    tracing::warn!(
                        error = %err,
                        tentativa = tentativa_reconexao,
                        delay_ms = delay.as_millis(),
                        "Falha ao conectar ao Redis; reconectando automaticamente..."
                    );
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => continue,
                        res = shutdown_rx.changed() => {
                            if res.is_err() || *shutdown_rx.borrow() {
                                break;
                            }
                            continue;
                        }
                    }
                }
            };

            // Loop interno de BRPOP com conexão ativa
            loop {
                if *shutdown_rx.borrow() {
                    return Ok(());
                }

                let brpop_fut = conn.brpop::<_, Option<(String, String)>>(
                    &self.queue_name,
                    self.brpop_timeout_secs,
                );

                let item = tokio::select! {
                    res = brpop_fut => res,
                    res = shutdown_rx.changed() => {
                        if res.is_err() || *shutdown_rx.borrow() {
                            tracing::info!("Encerrando loop BRPOP graciosamente...");
                            return Ok(());
                        }
                        continue;
                    }
                };

                match item {
                    Ok(Some((_fila, payload_json))) => {
                        match serde_json::from_str::<ImportSpedJobPayload>(&payload_json) {
                            Ok(job_payload) => {
                                tracing::info!(
                                    job_id = %job_payload.job_id,
                                    cnpj = %job_payload.cnpj,
                                    arquivo = %job_payload.nome_original,
                                    "Job SPED recebido via BRPOP; iniciando pipeline..."
                                );
                                if let Err(err) = processar_job(
                                    &job_payload,
                                    Arc::clone(&self.storage),
                                    Arc::clone(&self.publisher),
                                )
                                .await
                                {
                                    tracing::error!(
                                        job_id = %job_payload.job_id,
                                        error = %err,
                                        "Job SPED finalizado com erro tratado"
                                    );
                                }
                            }
                            Err(err) => {
                                tracing::error!(
                                    error = %err,
                                    payload = %payload_json,
                                    "Payload JSON inválido na fila ecd:queue:jobs; descartando mensagem corrompida"
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        // Timeout normal do BRPOP (fila vazia), continua aguardando
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            "Conexão Redis interrompida durante BRPOP; acionando reconexão automática..."
                        );
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockEventPublisher {
        eventos: Mutex<Vec<JobStatusEvent>>,
    }

    impl MockEventPublisher {
        fn listar(&self) -> Vec<JobStatusEvent> {
            self.eventos.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl JobEventPublisher for MockEventPublisher {
        async fn publish_status(&self, event: &JobStatusEvent) -> anyhow::Result<()> {
            self.eventos.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    type SucessoRecord = (Uuid, String, String, String, i32, bool);
    type ErroRecord = (Uuid, String, String, String);

    #[derive(Default)]
    struct MockWorkerStorage {
        plano_base: Mutex<HashMap<String, String>>,
        hashes_substituidos: Mutex<HashSet<String>>,
        inicios: Mutex<Vec<ImportSpedJobPayload>>,
        lotes_salvos: Mutex<Vec<SpedBatchRecords>>,
        resultados_salvos: Mutex<Vec<ResultadoExercicio>>,
        sucessos: Mutex<Vec<SucessoRecord>>,
        erros: Mutex<Vec<ErroRecord>>,
        forcar_erro_banco: Mutex<bool>,
    }

    #[async_trait]
    impl WorkerStorage for MockWorkerStorage {
        async fn carregar_plano_contas_base(&self) -> anyhow::Result<HashMap<String, String>> {
            Ok(self.plano_base.lock().unwrap().clone())
        }

        async fn obter_hashes_substituidos(&self, _cnpj: &str) -> anyhow::Result<HashSet<String>> {
            Ok(self.hashes_substituidos.lock().unwrap().clone())
        }

        async fn registrar_inicio_importacao(
            &self,
            job: &ImportSpedJobPayload,
        ) -> anyhow::Result<()> {
            self.inicios.lock().unwrap().push(job.clone());
            Ok(())
        }

        async fn persistir_ingestao_completa(
            &self,
            batch: &SpedBatchRecords,
            resultado: &ResultadoExercicio,
        ) -> anyhow::Result<BatchInsertSummary> {
            if *self.forcar_erro_banco.lock().unwrap() {
                anyhow::bail!("Falha simulada de conexão/transação no PostgreSQL");
            }

            let total_registros = 1
                + batch.reg_i010.len()
                + batch.reg_i030.len()
                + batch.reg_i050.len()
                + batch.reg_i051.len()
                + batch.reg_i052.len()
                + batch.reg_i053.len()
                + batch.plano_contas.len()
                + batch.reg_i150.len()
                + batch.reg_i155.len()
                + batch.reg_i355.len()
                + batch.reg_j005.len()
                + batch.reg_j100.len()
                + batch.reg_j150.len()
                + batch.reg_j210.len()
                + batch.reg_j215.len();

            let escrituracao_substituida_hash = batch
                .reg_0000
                .cod_hash_sub
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);

            if let Some(ref sub_hash) = escrituracao_substituida_hash {
                self.hashes_substituidos
                    .lock()
                    .unwrap()
                    .insert(sub_hash.clone());
                for r in self.resultados_salvos.lock().unwrap().iter_mut() {
                    if &r.hash_arquivo == sub_hash {
                        r.situacao = "X".to_string();
                    }
                }
            }

            let ja_substituido = self
                .hashes_substituidos
                .lock()
                .unwrap()
                .contains(&batch.reg_0000.hash_arquivo_id);

            let mut res_final = resultado.clone();
            if ja_substituido {
                res_final.situacao = "X".to_string();
            }

            self.lotes_salvos.lock().unwrap().push(batch.clone());
            self.resultados_salvos.lock().unwrap().push(res_final);

            Ok(BatchInsertSummary {
                reg_0000_id: 1,
                total_registros_inseridos: total_registros,
                escrituracao_substituida_hash,
                arquivo_atual_ja_substituido: ja_substituido,
            })
        }

        async fn finalizar_importacao_sucesso(
            &self,
            job_id: Uuid,
            hash_arquivo: &str,
            cnpj: &str,
            caminho_final: &str,
            linhas_processadas: i32,
            _tempo_ms: i32,
            ja_substituida: bool,
        ) -> anyhow::Result<()> {
            self.sucessos.lock().unwrap().push((
                job_id,
                hash_arquivo.to_string(),
                cnpj.to_string(),
                caminho_final.to_string(),
                linhas_processadas,
                ja_substituida,
            ));
            Ok(())
        }

        async fn finalizar_importacao_erro(
            &self,
            job_id: Uuid,
            hash_arquivo: &str,
            caminho_final: &str,
            _tempo_ms: i32,
            mensagem_erro: &str,
        ) -> anyhow::Result<()> {
            self.erros.lock().unwrap().push((
                job_id,
                hash_arquivo.to_string(),
                caminho_final.to_string(),
                mensagem_erro.to_string(),
            ));
            Ok(())
        }
    }

    fn criar_conteudo_sped_completo(cnpj: &str, cod_hash_sub: Option<&str>) -> String {
        let sub = cod_hash_sub.unwrap_or("");
        [
            format!("|0000|LECD|01012023|31122023|INDUSTRIA E COMERCIO ALFA S.A.|{cnpj}|SP||3550308||0|0|1|1|{sub}|0|0||N|N|0|0|1|"),
            "|I010|G|9.00|".to_string(),
            format!("|I030|TERMO DE ABERTURA|1|DIARIO GERAL|25|INDUSTRIA E COMERCIO ALFA S.A.|35212345678|{cnpj}|01012023||SAO PAULO|31122023|"),
            "|I050|01012023|04|S|1|3||RECEITAS OPERACIONAIS|".to_string(),
            "|I050|01012023|04|A|2|3.01|3|RECEITA BRUTA DE VENDAS E SERVICOS|".to_string(),
            "|I051||3.01.01.01.01.06|".to_string(),
            "|I052||AGL_REC_01|".to_string(),
            "|I053|001|3.01.99|01|".to_string(),
            "|I150|01012023|31122023|".to_string(),
            "|I155|3.01||0,00|C|50000,00|850000,00|800000,00|C|".to_string(),
            "|I350|31122023|".to_string(),
            "|I355|3.01||800000,00|C|".to_string(),
            "|I990|11|".to_string(),
            "|J005|01012023|31122023|1|DEMONSTRACOES CONTABEIS|".to_string(),
            "|J100|1|T|1||A|ATIVO TOTAL|500000,00|D|1200000,00|D||".to_string(),
            "|J150|1|AGL_REC_01|T|1||RECEITA BRUTA OPERACIONAL|0,00|C|1000000,00|C|R||".to_string(),
            "|J150|2|AGL_SERV|D|2|AGL_REC_01|RECEITA BRUTA DE SERVICOS PRESTADOS|0,00|C|600000,00|C|R||".to_string(),
            "|J150|3|AGL_LOC|D|2|AGL_REC_01|RECEITA BRUTA DE LOCACAO DE BENS|0,00|C|150000,00|C|R||".to_string(),
            "|J150|4|AGL_DED|T|1||DEDUCOES DA RECEITA BRUTA E IMPOSTOS|0,00|D|180000,00|D|D||".to_string(),
            "|J210|0|PL01|LUCROS OU PREJUIZOS ACUMULADOS|100000,00|C|450000,00|C||".to_string(),
            "|J215|H001|LUCRO LIQUIDO DO EXERCICIO|350000,00|C|".to_string(),
            "|J990|9|".to_string(),
        ]
        .join("\n")
    }

    #[tokio::test]
    async fn test_pipeline_completo_sucesso_move_para_processados_e_publica_eventos() {
        let base_dir = std::env::temp_dir().join(format!("ecd_worker_test_{}", Uuid::now_v7()));
        let dir_aguardando = base_dir.join("aguardando");
        tokio::fs::create_dir_all(&dir_aguardando).await.unwrap();

        let arquivo_path = dir_aguardando.join("sped_alfa_2023.txt");
        let conteudo = criar_conteudo_sped_completo("12345678000195", None);
        tokio::fs::write(&arquivo_path, conteudo.as_bytes())
            .await
            .unwrap();

        let storage = Arc::new(MockWorkerStorage::default());
        storage.plano_base.lock().unwrap().insert(
            "3.01.01.01.01.06".to_string(),
            "RECEITA DE PRESTACAO DE SERVICOS NO MERCADO INTERNO".to_string(),
        );
        let publisher = Arc::new(MockEventPublisher::default());

        let job = ImportSpedJobPayload {
            job_id: Uuid::now_v7(),
            hash_arquivo: "HASH_ALFA_2023".to_string(),
            cnpj: "12345678000195".to_string(),
            caminho_arquivo: arquivo_path.to_string_lossy().to_string(),
            nome_original: "sped_alfa_2023.txt".to_string(),
            tamanho_bytes: conteudo.len() as i64,
            enfileirado_em: Utc::now(),
        };

        let outcome = processar_job(&job, storage.clone(), publisher.clone())
            .await
            .expect("Pipeline deve concluir com sucesso");

        assert_eq!(outcome.status_final, StatusImportacao::Sucesso);
        assert_eq!(outcome.cnpj, "12345678000195");
        assert_eq!(outcome.linhas_processadas, 22);
        assert_eq!(
            outcome.receita_apurada.receita_bruta,
            Decimal::from(1000000)
        );
        assert_eq!(
            outcome.receita_apurada.receita_servico,
            Decimal::from(600000)
        );
        assert_eq!(
            outcome.receita_apurada.outras_receitas,
            Decimal::from(150000)
        );

        // Arquivo deve ter sido movido de aguardando/ para processados/
        assert!(!arquivo_path.exists());
        let esperado_processado = base_dir.join("processados").join("sped_alfa_2023.txt");
        assert_eq!(outcome.caminho_final, esperado_processado);
        assert!(esperado_processado.exists());

        // Verifica eventos Pub/Sub publicados (10% -> 45% -> 75% -> 100% Sucesso)
        let eventos = publisher.listar();
        assert_eq!(eventos.len(), 4);
        assert_eq!(eventos[0].progresso_percentual, 10);
        assert_eq!(eventos[1].progresso_percentual, 45);
        assert_eq!(eventos[2].progresso_percentual, 75);
        assert_eq!(eventos[3].progresso_percentual, 100);
        assert_eq!(eventos[3].status, StatusImportacao::Sucesso);

        // Verifica que o enriquecimento com plano_contas_base foi aplicado no lote salvo
        {
            let lotes = storage.lotes_salvos.lock().unwrap();
            assert_eq!(lotes.len(), 1);
            let conta_analitica = lotes[0]
                .plano_contas
                .iter()
                .find(|c| c.cod_cta == "3.01")
                .unwrap();
            assert_eq!(
                conta_analitica.nome_cta_referencia.as_deref(),
                Some("RECEITA DE PRESTACAO DE SERVICOS NO MERCADO INTERNO")
            );
        }

        let _ = tokio::fs::remove_dir_all(&base_dir).await;
    }

    #[tokio::test]
    async fn test_pipeline_substituicao_cod_hash_sub_e_arquivo_ja_substituido() {
        let base_dir = std::env::temp_dir().join(format!("ecd_worker_sub_{}", Uuid::now_v7()));
        let dir_aguardando = base_dir.join("aguardando");
        tokio::fs::create_dir_all(&dir_aguardando).await.unwrap();

        let storage = Arc::new(MockWorkerStorage::default());
        let publisher = Arc::new(MockEventPublisher::default());

        // 1. Importa escrituração original
        let arq_orig = dir_aguardando.join("sped_original.txt");
        let txt_orig = criar_conteudo_sped_completo("12345678000195", None);
        tokio::fs::write(&arq_orig, txt_orig.as_bytes())
            .await
            .unwrap();

        let job_orig = ImportSpedJobPayload {
            job_id: Uuid::now_v7(),
            hash_arquivo: "HASH_ORIGINAL_001".to_string(),
            cnpj: "12345678000195".to_string(),
            caminho_arquivo: arq_orig.to_string_lossy().to_string(),
            nome_original: "sped_original.txt".to_string(),
            tamanho_bytes: txt_orig.len() as i64,
            enfileirado_em: Utc::now(),
        };

        let out_orig = processar_job(&job_orig, storage.clone(), publisher.clone())
            .await
            .unwrap();
        assert_eq!(out_orig.status_final, StatusImportacao::Sucesso);

        // 2. Importa escrituração retificadora apontando cod_hash_sub = HASH_ORIGINAL_001
        let arq_ret = dir_aguardando.join("sped_retificadora.txt");
        let txt_ret = criar_conteudo_sped_completo("12345678000195", Some("HASH_ORIGINAL_001"));
        tokio::fs::write(&arq_ret, txt_ret.as_bytes())
            .await
            .unwrap();

        let job_ret = ImportSpedJobPayload {
            job_id: Uuid::now_v7(),
            hash_arquivo: "HASH_RETIFICADORA_002".to_string(),
            cnpj: "12345678000195".to_string(),
            caminho_arquivo: arq_ret.to_string_lossy().to_string(),
            nome_original: "sped_retificadora.txt".to_string(),
            tamanho_bytes: txt_ret.len() as i64,
            enfileirado_em: Utc::now(),
        };

        let out_ret = processar_job(&job_ret, storage.clone(), publisher.clone())
            .await
            .unwrap();
        assert_eq!(out_ret.status_final, StatusImportacao::Sucesso);
        assert_eq!(
            out_ret.escrituracao_substituida_hash.as_deref(),
            Some("HASH_ORIGINAL_001")
        );

        // Verifica que o resultado_exercicio do HASH_ORIGINAL_001 teve situacao alterada para 'X'
        {
            let resultados = storage.resultados_salvos.lock().unwrap();
            assert_eq!(resultados.len(), 2);
            assert_eq!(resultados[0].hash_arquivo, "HASH_ORIGINAL_001");
            assert_eq!(resultados[0].situacao, "X");
            assert_eq!(resultados[1].hash_arquivo, "HASH_RETIFICADORA_002");
            assert_eq!(resultados[1].situacao, "S");
        }

        // 3. Reimportar um arquivo cujo hash já consta como substituído marca status como Substituida
        //    mas mantém a apuração histórica das receitas do próprio documento (não zera a receita)
        let arq_antigo = dir_aguardando.join("sped_antigo_atrasado.txt");
        tokio::fs::write(&arq_antigo, txt_orig.as_bytes())
            .await
            .unwrap();
        let job_antigo = ImportSpedJobPayload {
            job_id: Uuid::now_v7(),
            hash_arquivo: "HASH_ORIGINAL_001".to_string(),
            cnpj: "12345678000195".to_string(),
            caminho_arquivo: arq_antigo.to_string_lossy().to_string(),
            nome_original: "sped_antigo_atrasado.txt".to_string(),
            tamanho_bytes: txt_orig.len() as i64,
            enfileirado_em: Utc::now(),
        };
        let out_antigo = processar_job(&job_antigo, storage.clone(), publisher.clone())
            .await
            .unwrap();
        assert_eq!(out_antigo.status_final, StatusImportacao::Substituida);
        assert_eq!(
            out_antigo.receita_apurada.receita_bruta,
            Decimal::from(1000000)
        );
        assert_eq!(
            out_antigo.receita_apurada.receita_servico,
            Decimal::from(600000)
        );

        let _ = tokio::fs::remove_dir_all(&base_dir).await;
    }

    #[tokio::test]
    async fn test_pipeline_erro_parsing_move_para_pasta_erro_sem_registros_orfaos() {
        let base_dir = std::env::temp_dir().join(format!("ecd_worker_err_{}", Uuid::now_v7()));
        let dir_aguardando = base_dir.join("aguardando");
        tokio::fs::create_dir_all(&dir_aguardando).await.unwrap();

        let arq_invalido = dir_aguardando.join("sped_corrompido.txt");
        tokio::fs::write(&arq_invalido, b"|I010|SEM_REGISTRO_0000_NO_INICIO|\n")
            .await
            .unwrap();

        let storage = Arc::new(MockWorkerStorage::default());
        let publisher = Arc::new(MockEventPublisher::default());

        let job = ImportSpedJobPayload {
            job_id: Uuid::now_v7(),
            hash_arquivo: "HASH_CORROMPIDO".to_string(),
            cnpj: "12345678000195".to_string(),
            caminho_arquivo: arq_invalido.to_string_lossy().to_string(),
            nome_original: "sped_corrompido.txt".to_string(),
            tamanho_bytes: 35,
            enfileirado_em: Utc::now(),
        };

        let res = processar_job(&job, storage.clone(), publisher.clone()).await;
        assert!(res.is_err());

        // Nenhum lote ou resultado foi salvo no banco
        assert!(storage.lotes_salvos.lock().unwrap().is_empty());
        assert!(storage.resultados_salvos.lock().unwrap().is_empty());
        assert_eq!(storage.erros.lock().unwrap().len(), 1);

        // Arquivo foi movido de aguardando/ para erro/
        assert!(!arq_invalido.exists());
        let esperado_erro = base_dir.join("erro").join("sped_corrompido.txt");
        assert!(esperado_erro.exists());

        // Último evento Pub/Sub publicado deve ter status Erro e progresso 100%
        let eventos = publisher.listar();
        let ultimo = eventos.last().unwrap();
        assert_eq!(ultimo.status, StatusImportacao::Erro);
        assert_eq!(ultimo.progresso_percentual, 100);

        let _ = tokio::fs::remove_dir_all(&base_dir).await;
    }

    #[tokio::test]
    async fn test_pipeline_falha_banco_move_para_erro_e_notifica_pubsub() {
        let base_dir = std::env::temp_dir().join(format!("ecd_worker_dberr_{}", Uuid::now_v7()));
        let dir_aguardando = base_dir.join("aguardando");
        tokio::fs::create_dir_all(&dir_aguardando).await.unwrap();

        let arq_path = dir_aguardando.join("sped_valido_mas_db_caiu.txt");
        let txt = criar_conteudo_sped_completo("12345678000195", None);
        tokio::fs::write(&arq_path, txt.as_bytes()).await.unwrap();

        let storage = Arc::new(MockWorkerStorage::default());
        *storage.forcar_erro_banco.lock().unwrap() = true;
        let publisher = Arc::new(MockEventPublisher::default());

        let job = ImportSpedJobPayload {
            job_id: Uuid::now_v7(),
            hash_arquivo: "HASH_DB_ERR".to_string(),
            cnpj: "12345678000195".to_string(),
            caminho_arquivo: arq_path.to_string_lossy().to_string(),
            nome_original: "sped_valido_mas_db_caiu.txt".to_string(),
            tamanho_bytes: txt.len() as i64,
            enfileirado_em: Utc::now(),
        };

        let res = processar_job(&job, storage.clone(), publisher.clone()).await;
        assert!(res.is_err());

        // Transação revertida: zero registros persistidos
        assert!(storage.lotes_salvos.lock().unwrap().is_empty());
        assert_eq!(storage.erros.lock().unwrap().len(), 1);
        assert!(base_dir
            .join("erro")
            .join("sped_valido_mas_db_caiu.txt")
            .exists());

        let eventos = publisher.listar();
        assert_eq!(eventos.last().unwrap().status, StatusImportacao::Erro);

        let _ = tokio::fs::remove_dir_all(&base_dir).await;
    }

    #[tokio::test]
    async fn test_backoff_reconexao_e_shutdown_gracioso_consumer() {
        assert_eq!(calcular_backoff_reconexao(0), Duration::from_millis(500));
        assert_eq!(calcular_backoff_reconexao(1), Duration::from_millis(1000));
        assert_eq!(calcular_backoff_reconexao(2), Duration::from_millis(2000));
        assert_eq!(calcular_backoff_reconexao(3), Duration::from_millis(4000));
        assert_eq!(calcular_backoff_reconexao(4), Duration::from_millis(5000));
        assert_eq!(calcular_backoff_reconexao(99), Duration::from_millis(5000));

        let storage = Arc::new(MockWorkerStorage::default());
        let publisher = Arc::new(MockEventPublisher::default());
        let consumer = WorkerConsumer::new("redis://127.0.0.1:1", storage, publisher);

        let (tx, rx) = watch::channel(false);
        let handle = tokio::spawn(async move { consumer.run(rx).await });

        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.send(true).unwrap();

        let res = tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("Consumer deve encerrar imediatamente ao receber sinal de shutdown")
            .unwrap();
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_redis_real_brpop_e_pubsub_pipeline_completo() {
        let redis_url = "redis://127.0.0.1:6379";
        let client = match redis::Client::open(redis_url) {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut conn = match client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(_) => return,
        };

        let test_id = Uuid::now_v7();
        let queue_key = format!("ecd:test:queue:{test_id}");
        let channel_key = format!("ecd:test:events:{test_id}");

        let base_dir = std::env::temp_dir().join(format!("ecd_worker_redis_{test_id}"));
        let dir_aguardando = base_dir.join("aguardando");
        tokio::fs::create_dir_all(&dir_aguardando).await.unwrap();

        let arq_path = dir_aguardando.join("sped_redis_real.txt");
        let txt = criar_conteudo_sped_completo("12345678000195", None);
        tokio::fs::write(&arq_path, txt.as_bytes()).await.unwrap();

        // Assina canal Pub/Sub antes de enfileirar o job
        let mut pubsub = client.get_async_pubsub().await.unwrap();
        pubsub.subscribe(&channel_key).await.unwrap();

        let storage = Arc::new(MockWorkerStorage::default());
        let publisher =
            Arc::new(RedisEventPublisher::new(client.clone()).with_channel(&channel_key));
        let consumer = WorkerConsumer::new(redis_url, storage.clone(), publisher)
            .with_queue_name(&queue_key)
            .with_brpop_timeout(0.1);

        let (tx, rx) = watch::channel(false);
        let handle = tokio::spawn(async move { consumer.run(rx).await });

        let job = ImportSpedJobPayload {
            job_id: Uuid::now_v7(),
            hash_arquivo: "HASH_REDIS_REAL_001".to_string(),
            cnpj: "12345678000195".to_string(),
            caminho_arquivo: arq_path.to_string_lossy().to_string(),
            nome_original: "sped_redis_real.txt".to_string(),
            tamanho_bytes: txt.len() as i64,
            enfileirado_em: Utc::now(),
        };
        let payload_json = serde_json::to_string(&job).unwrap();

        // Enfileira via LPUSH real no Redis
        let _: i64 = conn.lpush(&queue_key, &payload_json).await.unwrap();

        // Coleta os 4 eventos publicados via Redis Pub/Sub real (10%, 45%, 75%, 100%)
        use futures::StreamExt;
        let mut stream = pubsub.on_message();
        let mut eventos_recebidos = Vec::new();
        for _ in 0..4 {
            let msg = tokio::time::timeout(Duration::from_secs(3), stream.next())
                .await
                .expect("Timeout aguardando evento Pub/Sub do Redis")
                .expect("Stream Pub/Sub encerrado prematuramente");
            let payload: String = msg.get_payload().unwrap();
            let ev: JobStatusEvent = serde_json::from_str(&payload).unwrap();
            eventos_recebidos.push(ev);
        }

        assert_eq!(eventos_recebidos[0].progresso_percentual, 10);
        assert_eq!(eventos_recebidos[1].progresso_percentual, 45);
        assert_eq!(eventos_recebidos[2].progresso_percentual, 75);
        assert_eq!(eventos_recebidos[3].progresso_percentual, 100);
        assert_eq!(eventos_recebidos[3].status, StatusImportacao::Sucesso);

        // Encerra o consumer graciosamente
        tx.send(true).unwrap();
        let res = tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("Consumer deve encerrar após BRPOP timeout")
            .unwrap();
        assert!(res.is_ok());

        assert_eq!(storage.sucessos.lock().unwrap().len(), 1);
        assert!(base_dir
            .join("processados")
            .join("sped_redis_real.txt")
            .exists());

        let _: i64 = conn.del(&queue_key).await.unwrap_or(0);
        let _ = tokio::fs::remove_dir_all(&base_dir).await;
    }
}
