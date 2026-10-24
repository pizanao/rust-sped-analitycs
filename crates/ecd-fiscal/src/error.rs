use thiserror::Error;

#[derive(Debug, Error)]
pub enum FiscalError {
    #[error("Erro ao gerar ou manipular planilha XLSX: {0}")]
    Xlsx(#[from] umya_spreadsheet::XlsxError),

    #[error("Erro de E/S na operação fiscal: {0}")]
    Io(#[from] std::io::Error),

    #[error("Erro de validação ou consistência contábil/fiscal: {0}")]
    Validacao(String),
}
