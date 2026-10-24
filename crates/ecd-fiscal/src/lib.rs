//! # ecd-fiscal
//!
//! Motor de cálculo fiscal, identificação de receita bruta via fuzzy matching (`partial_ratio` >= 0.80)
//! com paridade estrita ao legado (`apps/fiscal/utils.py` e `utils_recfuze.py`)
//! e exportação de planilhas contábeis multi-aba formatadas (`umya-spreadsheet`).

pub mod error;
pub mod revenue;
pub mod xlsx;

pub use error::FiscalError;
pub use revenue::*;
pub use xlsx::*;
