//! Proposed Download Orchestrator / Multi-Progress Manager (`downloader/mod.rs`)
//!
//! Coordinates concurrent file downloads across 37 RFB CNPJ zip archives using:
//! - Tokio asynchronous task pool bounded by `tokio::sync::Semaphore`
//! - `indicatif::MultiProgress` with an overall progress bar and per-worker file progress bars
//! - Resumable HTTP 206 transfers via `RangeClient`

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::Semaphore;

use crate::downloader::range_client::{
    create_download_progress_style, DownloadResult, DownloadTarget, RangeClient, RangeClientConfig,
};

/// Downloader execution options.
#[derive(Debug, Clone)]
pub struct DownloaderOptions {
    pub output_dir: PathBuf,
    pub share_token: String,
    pub year_month: String,
    pub concurrency: usize,
    pub dry_run: bool,
}

/// Download all targets concurrently with bounded concurrency and multi-progress tracking.
pub async fn download_all(
    targets: Vec<DownloadTarget>,
    options: DownloaderOptions,
) -> Result<(), anyhow::Error> {
    if options.dry_run {
        println!("Dry run mode: {} files to download to {:?}", targets.len(), options.output_dir);
        for t in &targets {
            println!(" - {} (expected size: {:?})", t.filename, t.expected_size);
        }
        return Ok(());
    }

    tokio::fs::create_dir_all(&options.output_dir).await?;

    let config = RangeClientConfig {
        share_token: Some(options.share_token),
        ..Default::default()
    };
    let client = Arc::new(RangeClient::new(config)?);

    let mp = Arc::new(MultiProgress::new());

    // Overall progress bar (Total Files)
    let total_files = targets.len() as u64;
    let overall_style = ProgressStyle::default_bar()
        .template("{prefix:<18} [{elapsed_precise}] [{bar:30.green/black}] {pos}/{len} files ({percent}%) [ETA: {eta}]")?
        .progress_chars("=>-");
    let overall_pb = mp.add(ProgressBar::new(total_files));
    overall_pb.set_style(overall_style);
    overall_pb.set_prefix("Overall Progress");

    // Bounded concurrency semaphore (e.g. 4 concurrent connections)
    let semaphore = Arc::new(Semaphore::new(options.concurrency.max(1)));
    let file_style = create_download_progress_style();

    let start_time = Instant::now();
    let mut tasks = Vec::with_capacity(targets.len());

    for target in targets {
        let client = Arc::clone(&client);
        let sem = Arc::clone(&semaphore);
        let mp = Arc::clone(&mp);
        let overall_pb = overall_pb.clone();
        let file_style = file_style.clone();

        let task = tokio::spawn(async move {
            let _permit = sem.acquire().await.expect("Semaphore acquire failed");

            // Allocate a child progress bar in MultiProgress for this active file
            let pb = mp.add(ProgressBar::new(target.expected_size.unwrap_or(0)));
            pb.set_style(file_style);
            pb.set_message(target.filename.clone());

            let res = client.download_file(&target, &pb).await;

            overall_pb.inc(1);
            res
        });

        tasks.push(task);
    }

    let mut fresh_count = 0;
    let mut resumed_count = 0;
    let mut cached_count = 0;
    let mut total_transferred_bytes = 0u64;

    for task in tasks {
        match task.await? {
            Ok(DownloadResult::FreshDownload { bytes_transferred }) => {
                fresh_count += 1;
                total_transferred_bytes += bytes_transferred;
            }
            Ok(DownloadResult::Resumed { bytes_appended, .. }) => {
                resumed_count += 1;
                total_transferred_bytes += bytes_appended;
            }
            Ok(DownloadResult::AlreadyComplete { .. }) => {
                cached_count += 1;
            }
            Err(e) => {
                overall_pb.abandon_with_message("Download failed");
                return Err(e);
            }
        }
    }

    overall_pb.finish_with_message("Download complete!");

    let elapsed = start_time.elapsed();
    let speed_mb_s = (total_transferred_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64().max(0.001);

    println!(
        "\n✅ Finished in {:.2}s ({:.2} MB/s avg): {} downloaded, {} resumed, {} already complete.",
        elapsed.as_secs_f64(),
        speed_mb_s,
        fresh_count,
        resumed_count,
        cached_count,
    );

    Ok(())
}
