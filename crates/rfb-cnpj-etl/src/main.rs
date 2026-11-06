//! Binary entry point for `rfb-cnpj-etl`.

use anyhow::Result;
use clap::Parser;
use rfb_cnpj_etl::cli::{Cli, Commands, OutputFormat};
use tracing::Level;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Configure logging based on verbosity flags
    let log_level = match (cli.quiet, cli.verbose) {
        (true, _) => Level::ERROR,
        (false, 0) => Level::INFO,
        (false, 1) => Level::DEBUG,
        (false, _) => Level::TRACE,
    };

    let filter = EnvFilter::default()
        .add_directive(format!("rfb_cnpj_etl={}", log_level).parse()?)
        .add_directive(format!("rfb_cnpj_etl::main={}", log_level).parse()?);

    fmt().with_env_filter(filter).with_target(false).init();

    match cli.command {
        Commands::Download(args) => {
            rfb_cnpj_etl::downloader::run(args, cli.quiet).await?;
        }
        Commands::BuildCnpj(args) => {
            rfb_cnpj_etl::ingestion::run(args)?;
        }
        Commands::BuildRede(args) => {
            rfb_cnpj_etl::graph::run(args)?;
        }
        Commands::Status(mut args) => {
            if args.json {
                args.format = OutputFormat::Json;
            }
            rfb_cnpj_etl::status::run(args)?;
        }
    }

    Ok(())
}
