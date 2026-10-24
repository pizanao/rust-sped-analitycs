use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusImportacao {
    Aguardando,
    EmProcessamento,
    Sucesso,
    Erro,
    Substituida,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportacaoEcd {
    pub id: Uuid,
    pub hash_arquivo: String,
    pub cnpj: String,
    pub nome_arquivo_original: String,
    pub caminho_armazenamento: String,
    pub tamanho_bytes: i64,
    pub status: StatusImportacao,
    pub linhas_processadas: i32,
    pub tempo_processamento_ms: i32,
    pub mensagem_erro: Option<String>,
    pub criado_em: NaiveDateTime,
    pub atualizado_em: NaiveDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultadoExercicio {
    pub id: Uuid,
    pub cnpj: String,
    pub razao_social: String,
    pub periodo_inicial: NaiveDate,
    pub periodo_final: NaiveDate,
    pub receita_bruta: Decimal,
    pub receita_servico: Decimal,
    pub outras_receitas: Decimal,
    pub receita_acumulada: Decimal,
    pub total_nota: Decimal,
    pub hash_arquivo: String,
    pub situacao: String,
    pub detalhes_j150: Option<serde_json::Value>,
    pub detalhes_i355: Option<serde_json::Value>,
    pub criado_em: NaiveDateTime,
}
