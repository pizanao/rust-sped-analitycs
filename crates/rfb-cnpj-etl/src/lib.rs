//! `rfb_cnpj_etl` - High-performance native Rust ETL and SQLite database generator for RFB CNPJ open data.

pub mod cli;
pub mod config;
pub mod downloader;

pub use cli::{Cli, Commands};
pub use config::IngestionPragmas;

/// High-performance native Rust streaming ingestion engine for Milestone 2.
pub mod ingestion;

/// Relational network graph and FTS5 search module for Milestone 3.
pub mod graph;

/// Inspection & status reporter module for Milestone 4.
pub mod status;
