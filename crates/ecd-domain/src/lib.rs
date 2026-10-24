//! # ecd-domain
//!
//! Entidades de domínio, modelos contábeis oficiais do SPED ECD (Blocos 0, I e J)
//! e estruturas de validação/contratos da aplicação.

pub mod cnpj;
pub mod fiscal;
pub mod sped;

pub use cnpj::Cnpj;
pub use fiscal::*;
pub use sped::*;
