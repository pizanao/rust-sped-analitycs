//! Downloader module coordinating WebDAV discovery and HTTP 206 range downloading.

pub mod range_client;
pub mod webdav;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

pub use range_client::{
    create_download_progress_style, DownloadResult, DownloadTarget, RangeClient, RangeClientConfig,
};
pub use webdav::{DiscoveryCatalog, RemoteFile, WebdavConfig, WebdavDiscovery};

use crate::cli::DownloadArgs;

/// Downloader execution options.
#[derive(Debug, Clone)]
pub struct DownloaderOptions {
    pub output_dir: PathBuf,
    pub share_token: String,
    pub year_month: Option<String>,
    pub base_url: String,
    pub concurrency: usize,
    pub user_agent: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub dry_run: bool,
    pub force: bool,
    pub quiet: bool,
}

impl DownloaderOptions {
    /// Construct downloader options from CLI `DownloadArgs` and explicit `quiet` setting.
    pub fn from_args(args: DownloadArgs, quiet: bool) -> Self {
        Self {
            output_dir: args.output_dir,
            share_token: args.share_token,
            year_month: args.year_month,
            base_url: args.base_url,
            concurrency: args.concurrency as usize,
            user_agent: args.user_agent,
            timeout_secs: args.timeout_secs,
            max_retries: args.max_retries,
            dry_run: args.dry_run,
            force: args.force,
            quiet,
        }
    }
}

impl From<DownloadArgs> for DownloaderOptions {
    fn from(args: DownloadArgs) -> Self {
        Self::from_args(args, false)
    }
}

/// Main entry point for the `download` subcommand.
pub async fn run(args: DownloadArgs, quiet: bool) -> Result<(), anyhow::Error> {
    let options = DownloaderOptions::from_args(args, quiet);

    let webdav_config = WebdavConfig {
        base_url: options.base_url.clone(),
        share_token: options.share_token.clone(),
        user_agent: options.user_agent.clone(),
        timeout: Duration::from_secs(options.timeout_secs),
    };

    let discovery = WebdavDiscovery::new(webdav_config)?;
    let catalog = discovery
        .discover_catalog(options.year_month.as_deref())
        .await?;

    let targets = catalog.to_download_targets(&options.output_dir);

    if options.dry_run {
        if !options.quiet {
            let total_bytes = catalog.total_expected_bytes();
            let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
            println!(
                "🔍 Discovery mode (dry-run): {} files found for snapshot '{}' (fallback: {}, total: {:.2} MB)",
                targets.len(),
                catalog.year_month,
                catalog.is_fallback,
                total_mb
            );
            for (i, t) in targets.iter().enumerate() {
                let size_str = match t.expected_size {
                    Some(sz) => format!("{sz} bytes ({:.2} MB)", sz as f64 / (1024.0 * 1024.0)),
                    None => "unknown size".to_string(),
                };
                println!(
                    "  [{:>2}/37] {:<25} -> {:?} ({})",
                    i + 1,
                    t.filename,
                    t.destination_path,
                    size_str
                );
            }
        }
        return Ok(());
    }

    download_all(targets, options).await
}

/// Download all targets concurrently with bounded concurrency and multi-progress tracking.
pub async fn download_all(
    targets: Vec<DownloadTarget>,
    options: DownloaderOptions,
) -> Result<(), anyhow::Error> {
    tokio::fs::create_dir_all(&options.output_dir).await?;

    if options.force {
        for t in &targets {
            if t.destination_path.exists() {
                let _ = tokio::fs::remove_file(&t.destination_path).await;
            }
            let part_file = t.destination_path.with_extension("zip.part");
            if part_file.exists() {
                let _ = tokio::fs::remove_file(&part_file).await;
            }
        }
    }

    let config = RangeClientConfig {
        user_agent: options.user_agent,
        share_token: Some(options.share_token),
        max_retries: options.max_retries as usize,
        request_timeout: Duration::from_secs(options.timeout_secs),
        ..Default::default()
    };
    let client = Arc::new(RangeClient::new(config)?);

    let (mp, overall_pb) = if options.quiet {
        (None, ProgressBar::hidden())
    } else {
        let mp = Arc::new(MultiProgress::new());
        let total_files = targets.len() as u64;
        let overall_style = ProgressStyle::default_bar()
            .template("{prefix:<18} [{elapsed_precise}] [{bar:30.green/black}] {pos}/{len} files ({percent}%) [ETA: {eta}]")?
            .progress_chars("=>-");
        let pb = mp.add(ProgressBar::new(total_files));
        pb.set_style(overall_style);
        pb.set_prefix("Overall Progress");
        (Some(mp), pb)
    };

    // Bounded concurrency semaphore (e.g. 3 or 4 concurrent connections)
    let semaphore = Arc::new(Semaphore::new(options.concurrency.max(1)));
    let file_style = create_download_progress_style();

    let start_time = Instant::now();
    let mut join_set = JoinSet::new();

    for target in targets {
        let client = Arc::clone(&client);
        let sem = Arc::clone(&semaphore);
        let mp = mp.clone();
        let overall_pb = overall_pb.clone();
        let file_style = file_style.clone();

        join_set.spawn(async move {
            let _permit = sem.acquire().await.expect("Semaphore acquire failed");

            // Allocate a child progress bar in MultiProgress for this active file if not quiet
            let pb = match &mp {
                Some(mp) => {
                    let pb = mp.add(ProgressBar::new(target.expected_size.unwrap_or(0)));
                    pb.set_style(file_style);
                    pb.set_message(target.filename.clone());
                    pb
                }
                None => ProgressBar::hidden(),
            };

            let res = client.download_file(&target, &pb).await;

            overall_pb.inc(1);
            res
        });
    }

    let mut fresh_count = 0;
    let mut resumed_count = 0;
    let mut cached_count = 0;
    let mut total_transferred_bytes = 0u64;

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(DownloadResult::FreshDownload { bytes_transferred })) => {
                fresh_count += 1;
                total_transferred_bytes += bytes_transferred;
            }
            Ok(Ok(DownloadResult::Resumed { bytes_appended, .. })) => {
                resumed_count += 1;
                total_transferred_bytes += bytes_appended;
            }
            Ok(Ok(DownloadResult::AlreadyComplete { .. })) => {
                cached_count += 1;
            }
            Ok(Err(e)) => {
                join_set.abort_all();
                if !options.quiet {
                    overall_pb.abandon_with_message("Download failed");
                }
                return Err(e);
            }
            Err(join_err) => {
                join_set.abort_all();
                if !options.quiet {
                    overall_pb.abandon_with_message("Download failed");
                }
                return Err(anyhow::anyhow!("Download task failed: {join_err}"));
            }
        }
    }

    if !options.quiet {
        overall_pb.finish_with_message("Download complete!");

        let elapsed = start_time.elapsed();
        let speed_mb_s =
            (total_transferred_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64().max(0.001);

        println!(
            "\n✅ Finished in {:.2}s ({:.2} MB/s avg): {} downloaded, {} resumed, {} already complete.",
            elapsed.as_secs_f64(),
            speed_mb_s,
            fresh_count,
            resumed_count,
            cached_count,
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downloader_options_quiet_propagation() {
        let args = DownloadArgs {
            output_dir: PathBuf::from("dados-publicos-zip"),
            share_token: "token123".to_string(),
            year_month: Some("2026-10".to_string()),
            base_url: "https://example.com".to_string(),
            concurrency: 4,
            user_agent: "test-agent".to_string(),
            timeout_secs: 30,
            max_retries: 3,
            dry_run: true,
            force: false,
        };

        let opts_default = DownloaderOptions::from(args.clone());
        assert!(!opts_default.quiet);

        let opts_quiet = DownloaderOptions::from_args(args, true);
        assert!(opts_quiet.quiet);
        assert!(opts_quiet.dry_run);
        assert_eq!(opts_quiet.concurrency, 4);
    }
}
