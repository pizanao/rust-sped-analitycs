//! Command-line interface definitions for `rfb-cnpj-etl` using `clap` derive.

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Root command-line parser for `rfb-cnpj-etl`.
#[derive(Parser, Debug)]
#[command(
    name = "rfb-cnpj-etl",
    author = "Rust Sped Analytics Team",
    version = env!("CARGO_PKG_VERSION"),
    about = "High-performance native Rust ETL and SQLite database generator for RFB CNPJ open data",
    long_about = "High-performance native Rust ETL utility for downloading, ingesting, and building \
                  relational graph databases (cnpj.db, rede.db, rede_search.db) from official \
                  Receita Federal do Brasil (RFB) CNPJ open data archives."
)]
pub struct Cli {
    /// Global verbosity level (-v, -vv, -vvv)
    #[arg(
        short = 'v',
        long = "verbose",
        global = true,
        action = clap::ArgAction::Count,
        help = "Increase logging verbosity (-v: info, -vv: debug, -vvv: trace)"
    )]
    pub verbose: u8,

    /// Suppress all non-error output
    #[arg(
        short = 'q',
        long = "quiet",
        global = true,
        help = "Suppress non-error output and progress bars"
    )]
    pub quiet: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands for `rfb-cnpj-etl`.
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Download the 37 official RFB CNPJ open data zip archives with HTTP 206 resume
    #[command(name = "download", alias = "down")]
    Download(DownloadArgs),

    /// Ingest the 37 zip archives into SQLite database (cnpj.db) with post-load indexing
    #[command(name = "build-cnpj", alias = "cnpj")]
    BuildCnpj(BuildCnpjArgs),

    /// Derive societary relationship graph (rede.db) and full-text search index (rede_search.db)
    #[command(name = "build-rede", alias = "rede")]
    BuildRede(BuildRedeArgs),

    /// Inspect database sizes, table row counts, and indexing status
    #[command(name = "status", alias = "info")]
    Status(StatusArgs),
}

/// Arguments for the `download` subcommand.
#[derive(Args, Debug, Clone)]
pub struct DownloadArgs {
    /// Directory where the 37 downloaded zip archives are stored
    #[arg(
        short = 'o',
        long = "output-dir",
        default_value = "dados-publicos-zip",
        value_name = "DIR",
        env = "RFB_OUTPUT_DIR"
    )]
    pub output_dir: PathBuf,

    /// Nextcloud WebDAV public share token for the RFB files repository
    #[arg(
        long = "share-token",
        default_value = "YggdBLfdninEJX9",
        value_name = "TOKEN",
        env = "RFB_SHARE_TOKEN"
    )]
    pub share_token: String,

    /// Target reference period in YYYY-MM format (e.g. '2026-10').
    /// If omitted, the latest directory will be discovered automatically via WebDAV PROPFIND.
    #[arg(
        short = 'y',
        long = "year-month",
        value_name = "YYYY-MM",
        env = "RFB_YEAR_MONTH"
    )]
    pub year_month: Option<String>,

    /// Base URL for the RFB WebDAV repository
    #[arg(
        long = "base-url",
        default_value = "https://arquivos.receitafederal.gov.br/public.php/webdav",
        value_name = "URL",
        env = "RFB_BASE_URL"
    )]
    pub base_url: String,

    /// Number of concurrent file downloads (1 to 10)
    #[arg(
        short = 'c',
        long = "concurrency",
        default_value_t = 3,
        value_parser = clap::value_parser!(u32).range(1..=10),
        value_name = "NUM"
    )]
    pub concurrency: u32,

    /// Custom User-Agent header string to bypass F5 BIG-IP WAF challenge cookies
    #[arg(
        long = "user-agent",
        default_value = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        value_name = "STRING"
    )]
    pub user_agent: String,

    /// Connection and read timeout in seconds per HTTP request
    #[arg(long = "timeout-secs", default_value_t = 60, value_name = "SECONDS")]
    pub timeout_secs: u64,

    /// Maximum number of retry attempts for failed downloads or interrupted byte chunks
    #[arg(long = "max-retries", default_value_t = 5, value_name = "COUNT")]
    pub max_retries: u32,

    /// Discover and list target files and URLs without downloading
    #[arg(
        long = "dry-run",
        help = "Discover and display target files and URLs without downloading"
    )]
    pub dry_run: bool,

    /// Overwrite existing files instead of resuming partial downloads
    #[arg(
        short = 'f',
        long = "force",
        help = "Force fresh download of all files, ignoring partial resume files"
    )]
    pub force: bool,
}

/// Arguments for the `build-cnpj` subcommand.
#[derive(Args, Debug, Clone)]
pub struct BuildCnpjArgs {
    /// Directory containing the 37 downloaded zip archives
    #[arg(
        short = 'i',
        long = "input-dir",
        default_value = "dados-publicos-zip",
        value_name = "DIR",
        env = "RFB_INPUT_DIR"
    )]
    pub input_dir: PathBuf,

    /// Output path for the SQLite database file
    #[arg(
        short = 'o',
        long = "output-db",
        default_value = "dados-publicos/cnpj.db",
        value_name = "PATH",
        env = "RFB_CNPJ_DB"
    )]
    pub output_db: PathBuf,

    /// Resident memory budget in Megabytes (adjusts SQLite cache, queues, chunk buffers)
    #[arg(long = "memory-limit-mb", default_value_t = 1536, value_name = "MB")]
    pub memory_limit_mb: usize,

    /// Number of rows per SQLite transactional insert batch
    #[arg(
        short = 'b',
        long = "batch-size",
        default_value_t = 5000,
        value_name = "ROWS"
    )]
    pub batch_size: usize,

    /// Number of worker threads for parallel decompression and parsing
    #[arg(short = 't', long = "threads", default_value_t = 4, value_name = "NUM")]
    pub threads: usize,

    /// Delete/truncate existing output database if it already exists
    #[arg(
        long = "drop-existing",
        help = "Delete existing output database file before starting ingestion"
    )]
    pub drop_existing: bool,

    /// Skip secondary index creation after inserting table rows (useful for benchmarking)
    #[arg(
        long = "skip-indices",
        help = "Skip secondary index creation after data load"
    )]
    pub skip_indices: bool,
}

/// Arguments for the `build-rede` subcommand.
#[derive(Args, Debug, Clone)]
pub struct BuildRedeArgs {
    /// Path to the source cnpj.db SQLite database
    #[arg(
        long = "cnpj-db",
        default_value = "dados-publicos/cnpj.db",
        value_name = "PATH",
        env = "RFB_CNPJ_DB"
    )]
    pub cnpj_db: PathBuf,

    /// Path to the destination rede.db SQLite database
    #[arg(
        short = 'o',
        long = "output-rede",
        default_value = "dados-publicos/rede.db",
        value_name = "PATH",
        env = "RFB_REDE_DB"
    )]
    pub output_rede: PathBuf,

    /// Path to the destination rede_search.db SQLite FTS5 database
    #[arg(
        long = "output-search",
        default_value = "dados-publicos/rede_search.db",
        value_name = "PATH",
        env = "RFB_SEARCH_DB"
    )]
    pub output_search: PathBuf,

    /// Delete existing rede.db and rede_search.db before starting
    #[arg(
        long = "drop-existing",
        help = "Delete existing destination databases if they already exist"
    )]
    pub drop_existing: bool,

    /// Resident memory budget in Megabytes for attached database execution
    #[arg(long = "memory-limit-mb", default_value_t = 1536, value_name = "MB")]
    pub memory_limit_mb: usize,
}

/// Arguments for the `status` subcommand.
#[derive(Args, Debug, Clone)]
pub struct StatusArgs {
    /// Path to the cnpj.db SQLite database
    #[arg(
        long = "cnpj-db",
        default_value = "dados-publicos/cnpj.db",
        value_name = "PATH",
        env = "RFB_CNPJ_DB"
    )]
    pub cnpj_db: PathBuf,

    /// Path to the rede.db SQLite database
    #[arg(
        long = "rede-db",
        default_value = "dados-publicos/rede.db",
        value_name = "PATH",
        env = "RFB_REDE_DB"
    )]
    pub rede_db: PathBuf,

    /// Path to the rede_search.db SQLite database
    #[arg(
        long = "search-db",
        default_value = "dados-publicos/rede_search.db",
        value_name = "PATH",
        env = "RFB_SEARCH_DB"
    )]
    pub search_db: PathBuf,

    /// Directory containing the 37 downloaded zip archives
    #[arg(
        long = "zip-dir",
        default_value = "dados-publicos-zip",
        value_name = "DIR",
        env = "RFB_ZIP_DIR"
    )]
    pub zip_dir: PathBuf,

    /// Output format (text or json)
    #[arg(
        long = "format",
        value_enum,
        default_value_t = OutputFormat::Text,
        value_name = "FORMAT"
    )]
    pub format: OutputFormat,

    /// Output status as JSON (shortcut for --format json)
    #[arg(long = "json", help = "Shortcut for --format json")]
    pub json: bool,
}

/// Output formatting mode for status reporting.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Text,
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_download_defaults() {
        let cli = Cli::try_parse_from(["rfb-cnpj-etl", "download"]).expect("parse defaults");
        match cli.command {
            Commands::Download(args) => {
                assert_eq!(args.output_dir, PathBuf::from("dados-publicos-zip"));
                assert_eq!(args.share_token, "YggdBLfdninEJX9");
                assert_eq!(args.concurrency, 3);
                assert!(!args.dry_run);
                assert!(!args.force);
                assert_eq!(args.timeout_secs, 60);
                assert_eq!(args.max_retries, 5);
            }
            _ => panic!("Expected Download command"),
        }
    }

    #[test]
    fn test_cli_download_custom_args() {
        let cli = Cli::try_parse_from([
            "rfb-cnpj-etl",
            "download",
            "--dry-run",
            "--concurrency",
            "5",
            "-o",
            "/tmp/custom_dir",
            "-y",
            "2026-10",
            "-f",
        ])
        .expect("parse custom args");

        match cli.command {
            Commands::Download(args) => {
                assert_eq!(args.output_dir, PathBuf::from("/tmp/custom_dir"));
                assert_eq!(args.concurrency, 5);
                assert!(args.dry_run);
                assert!(args.force);
                assert_eq!(args.year_month.as_deref(), Some("2026-10"));
            }
            _ => panic!("Expected Download command"),
        }
    }

    #[test]
    fn test_cli_concurrency_out_of_range() {
        let res = Cli::try_parse_from(["rfb-cnpj-etl", "download", "--concurrency", "20"]);
        assert!(
            res.is_err(),
            "Concurrency > 10 should be rejected by value_parser"
        );
    }

    #[test]
    fn test_cli_build_cnpj_options() {
        let cli = Cli::try_parse_from([
            "rfb-cnpj-etl",
            "build-cnpj",
            "--batch-size",
            "10000",
            "--threads",
            "8",
            "--drop-existing",
            "--skip-indices",
        ])
        .expect("parse build-cnpj");

        match cli.command {
            Commands::BuildCnpj(args) => {
                assert_eq!(args.batch_size, 10000);
                assert_eq!(args.threads, 8);
                assert!(args.drop_existing);
                assert!(args.skip_indices);
            }
            _ => panic!("Expected BuildCnpj command"),
        }
    }

    #[test]
    fn test_cli_build_rede_options() {
        let cli = Cli::try_parse_from([
            "rfb-cnpj-etl",
            "build-rede",
            "--drop-existing",
            "--memory-limit-mb",
            "2048",
        ])
        .expect("parse build-rede");

        match cli.command {
            Commands::BuildRede(args) => {
                assert!(args.drop_existing);
                assert_eq!(args.memory_limit_mb, 2048);
            }
            _ => panic!("Expected BuildRede command"),
        }
    }

    #[test]
    fn test_cli_status_json_flag() {
        let cli = Cli::try_parse_from(["rfb-cnpj-etl", "status", "--json"]).expect("parse status");
        match cli.command {
            Commands::Status(args) => {
                assert!(args.json);
            }
            _ => panic!("Expected Status command"),
        }
    }
}
