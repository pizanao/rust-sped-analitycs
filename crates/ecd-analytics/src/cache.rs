//! Cache em memória assíncrono para indicadores econômicos utilizando `moka`.
//!
//! Fornece armazenamento volátil thread-safe com expiração configurável por TTL
//! (Time-To-Live) e capacidade máxima de entradas, suportando caching do resumo
//! completo (`IndicadoresResumo`) e de coleções de itens específicos (`Vec<IndicadorItem>`).

use std::future::Future;
use std::time::Duration;

use chrono::NaiveDate;
use moka::future::Cache;

use crate::{IndicadorItem, IndicadoresResumo, TipoIndicador};

/// Configuração de tempo de vida e dimensionamento do cache em memória.
#[derive(Clone, Debug)]
pub struct IndicadoresCacheConfig {
    /// Tempo de vida das entradas no cache antes da expiração automática.
    pub ttl: Duration,
    /// Capacidade máxima de entradas mantidas simultaneamente no cache.
    pub max_capacity: u64,
}

impl Default for IndicadoresCacheConfig {
    fn default() -> Self {
        Self {
            // Padrão de 24 horas (paridade com o sistema Django legado)
            ttl: Duration::from_secs(86_400),
            max_capacity: 10_000,
        }
    }
}

/// Gerenciador de cache assíncrono de alta performance para agregações e indicadores.
#[derive(Clone)]
pub struct IndicadoresCache {
    cache_resumo: Cache<String, IndicadoresResumo>,
    cache_itens: Cache<String, Vec<IndicadorItem>>,
    config: IndicadoresCacheConfig,
}

impl Default for IndicadoresCache {
    fn default() -> Self {
        Self::new(IndicadoresCacheConfig::default())
    }
}

impl IndicadoresCache {
    /// Cria uma nova instância do cache com a configuração especificada.
    pub fn new(config: IndicadoresCacheConfig) -> Self {
        let cache_resumo = Cache::builder()
            .time_to_live(config.ttl)
            .max_capacity(config.max_capacity)
            .build();

        let cache_itens = Cache::builder()
            .time_to_live(config.ttl)
            .max_capacity(config.max_capacity)
            .build();

        Self {
            cache_resumo,
            cache_itens,
            config,
        }
    }

    /// Construtor conveniente para instanciar com um TTL customizado.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self::new(IndicadoresCacheConfig {
            ttl,
            max_capacity: 10_000,
        })
    }

    /// Retorna a configuração ativa do cache.
    pub fn config(&self) -> &IndicadoresCacheConfig {
        &self.config
    }

    /// Obtém o resumo de indicadores do cache se presente e não expirado.
    pub async fn get_resumo(&self, chave: &str) -> Option<IndicadoresResumo> {
        self.cache_resumo.get(chave).await
    }

    /// Insere ou atualiza um resumo de indicadores no cache.
    pub async fn insert_resumo(&self, chave: String, resumo: IndicadoresResumo) {
        self.cache_resumo.insert(chave, resumo).await;
    }

    /// Obtém uma lista de itens específicos do cache.
    pub async fn get_itens(&self, chave: &str) -> Option<Vec<IndicadorItem>> {
        self.cache_itens.get(chave).await
    }

    /// Insere uma lista de itens específicos no cache.
    pub async fn insert_itens(&self, chave: String, itens: Vec<IndicadorItem>) {
        self.cache_itens.insert(chave, itens).await;
    }

    /// Obtém o resumo do cache ou executa a função de inicialização se ausente.
    pub async fn get_or_compute_resumo<F, Fut, E>(
        &self,
        chave: &str,
        init: F,
    ) -> Result<IndicadoresResumo, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<IndicadoresResumo, E>>,
    {
        if let Some(cached) = self.get_resumo(chave).await {
            return Ok(cached);
        }

        let computed = init().await?;
        self.insert_resumo(chave.to_string(), computed.clone())
            .await;
        Ok(computed)
    }

    /// Obtém uma lista de itens específicos do cache ou executa a função de inicialização se ausente.
    pub async fn get_or_compute_itens<F, Fut, E>(
        &self,
        chave: &str,
        init: F,
    ) -> Result<Vec<IndicadorItem>, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Vec<IndicadorItem>, E>>,
    {
        if let Some(cached) = self.get_itens(chave).await {
            return Ok(cached);
        }

        let computed = init().await?;
        self.insert_itens(chave.to_string(), computed.clone()).await;
        Ok(computed)
    }

    /// Invalida manualmente uma entrada de chave específica em todos os caches internos.
    pub async fn invalidate(&self, chave: &str) {
        self.cache_resumo.invalidate(chave).await;
        self.cache_itens.invalidate(chave).await;
    }

    /// Invalida todas as entradas armazenadas no cache.
    pub fn invalidate_all(&self) {
        self.cache_resumo.invalidate_all();
        self.cache_itens.invalidate_all();
    }

    /// Retorna o total aproximado de entradas armazenadas no cache de resumos.
    pub fn entry_count(&self) -> u64 {
        self.cache_resumo.entry_count() + self.cache_itens.entry_count()
    }

    /// Executa tarefas pendentes internas de manutenção e expiração do `moka`.
    pub async fn run_pending_tasks(&self) {
        self.cache_resumo.run_pending_tasks().await;
        self.cache_itens.run_pending_tasks().await;
    }

    /// Gera uma chave de cache padronizada baseada nas opções do filtro.
    pub fn gerar_chave_filtro(filtro: &crate::IndicadoresFiltro) -> String {
        let data_str = filtro
            .data_referencia
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "atual".to_string());
        let suffix = filtro.cache_key_suffix();
        if suffix.is_empty() {
            format!("indicadores_resumo_{}", data_str)
        } else {
            format!("indicadores_resumo_{}_{}", data_str, suffix)
        }
    }

    /// Gera uma chave de cache padronizada para o resumo consolidado.
    pub fn gerar_chave_resumo(data_ref: Option<NaiveDate>, tag: Option<&str>) -> String {
        let data_str = data_ref
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "atual".to_string());
        match tag {
            Some(t) => format!("indicadores_resumo_{}_{}", data_str, t),
            None => format!("indicadores_resumo_{}", data_str),
        }
    }

    /// Gera uma chave de cache padronizada para um tipo de indicador específico.
    pub fn gerar_chave_tipo(tipo: TipoIndicador, data_ref: Option<NaiveDate>) -> String {
        Self::gerar_chave(tipo.as_str(), data_ref)
    }

    /// Gera chave de cache no formato legado: `indicadores_{tipo}_{data_referencia}`.
    pub fn gerar_chave(tipo: &str, data_ref: Option<NaiveDate>) -> String {
        let data_str = data_ref
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "atual".to_string());
        format!("indicadores_{}_{}", tipo, data_str)
    }
}
