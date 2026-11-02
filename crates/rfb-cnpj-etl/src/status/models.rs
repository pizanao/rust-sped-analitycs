//! Strongly typed serializable data models for status reporting and diagnostics.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Complete status report for the RFB CNPJ open data pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReport {
    pub timestamp: String,
    pub version: String,
    pub overall_status: PipelineOverallStatus,
    pub databases: DatabaseStatuses,
    pub zip_archives: ZipArchivesStatus,
}

/// Overall pipeline health and progress classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineOverallStatus {
    /// All 3 databases exist, integrity checks pass, and all canonical tables are populated.
    Complete,
    /// At least one database or archive exists, but pipeline is partially built.
    Partial,
    /// No database files or downloaded archives found.
    Empty,
    /// A database is corrupted or an integrity check failed.
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatuses {
    pub cnpj: CnpjDbStatus,
    pub rede: RedeDbStatus,
    pub rede_search: SearchDbStatus,
}

/// Common SQLite database file metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCommonMetadata {
    pub path: PathBuf,
    pub exists: bool,
    pub size_bytes: u64,
    pub size_formatted: String,
    pub journal_mode: Option<String>,
    pub user_version: Option<i64>,
    pub page_count: Option<i64>,
    pub page_size: Option<i64>,
    pub integrity_check: String,
}

/// Status and table row counts for `cnpj.db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnpjDbStatus {
    #[serde(flatten)]
    pub common: DbCommonMetadata,
    pub reference_date: Option<String>,
    pub establishment_count_ref: Option<u64>,
    pub tables: BTreeMap<String, Option<u64>>,
    pub indices: IndexStatus,
}

/// Status and graph metrics for `rede.db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeDbStatus {
    #[serde(flatten)]
    pub common: DbCommonMetadata,
    pub tables: BTreeMap<String, Option<u64>>,
    pub edge_breakdown: BTreeMap<String, u64>,
    pub entity_breakdown: BTreeMap<String, u64>,
    pub indices: IndexStatus,
}

/// Status and FTS5 search index metrics for `rede_search.db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDbStatus {
    #[serde(flatten)]
    pub common: DbCommonMetadata,
    pub tables: BTreeMap<String, Option<u64>>,
}

/// Index verification results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub total_expected: usize,
    pub total_found: usize,
    pub all_present: bool,
    pub missing: Vec<String>,
}

/// Downloaded zip archives directory status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipArchivesStatus {
    pub path: PathBuf,
    pub exists: bool,
    pub total_expected: usize,
    pub total_found: usize,
    pub total_size_bytes: u64,
    pub total_size_formatted: String,
    pub missing_files: Vec<String>,
}
