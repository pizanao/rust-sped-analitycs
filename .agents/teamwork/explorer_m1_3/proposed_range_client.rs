//! Proposed Range Resume Downloading Client (`range_client.rs`)
//!
//! Provides crash-resilient HTTP downloading with:
//! - HTTP 206 Partial Content resume via `Range: bytes={offset}-`
//! - Safe `.part` staging and atomic renaming
//! - Status handling for 206, 200 (both truncate and EOF quirk), and 416
//! - F5 BIG-IP WAF bypass headers and cookie persistence
//! - Real-time progress bars with speed, ETA, and byte counters via `indicatif`
//! - Transient error retry loop with exponential backoff, jitter, and resume preservation

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, RANGE, USER_AGENT};
use reqwest::{Client, StatusCode};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, BufWriter, SeekFrom};

/// Default User-Agent simulating a modern Google Chrome browser to bypass F5 BIG-IP WAF challenges.
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Configuration parameters for the HTTP 206 Range Download Client.
#[derive(Debug, Clone)]
pub struct RangeClientConfig {
    pub user_agent: String,
    pub share_token: Option<String>,
    pub max_retries: usize,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_factor: f64,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub buffer_size_kb: usize,
}

impl Default for RangeClientConfig {
    fn default() -> Self {
        Self {
            user_agent: DEFAULT_USER_AGENT.to_string(),
            share_token: None,
            max_retries: 8,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_factor: 2.0,
            connect_timeout: Duration::from_secs(15),
            request_timeout: Duration::from_secs(600),
            buffer_size_kb: 128,
        }
    }
}

/// Description of a target file to download.
#[derive(Debug, Clone)]
pub struct DownloadTarget {
    pub url: String,
    pub filename: String,
    pub destination_path: PathBuf,
    pub expected_size: Option<u64>,
}

/// Outcome of a download operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadResult {
    /// File was already completely downloaded on disk (matched expected size or server returned 416/200-0).
    AlreadyComplete { total_bytes: u64 },
    /// File was fresh-downloaded from byte 0.
    FreshDownload { bytes_transferred: u64 },
    /// File was resumed from a previous partial download.
    Resumed { bytes_appended: u64, total_bytes: u64 },
}

/// Resumable HTTP download client built on `reqwest` and `tokio`.
pub struct RangeClient {
    client: Client,
    config: RangeClientConfig,
}

impl RangeClient {
    /// Construct a new client instance configured with browser headers and cookie store.
    pub fn new(config: RangeClientConfig) -> Result<Self, reqwest::Error> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&config.user_agent).unwrap_or(HeaderValue::from_static("Mozilla/5.0")),
        );
        default_headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"),
        );
        default_headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7"),
        );
        default_headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("document"));
        default_headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
        default_headers.insert("Sec-Fetch-Site", HeaderValue::from_static("none"));
        default_headers.insert("Sec-Fetch-User", HeaderValue::from_static("?1"));
        default_headers.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));

        let client = Client::builder()
            .default_headers(default_headers)
            .cookie_store(true) // Crucial for F5 BIG-IP TS* cookies and Nextcloud session affinity
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .tcp_keepalive(Duration::from_secs(30))
            .build()?;

        Ok(Self { client, config })
    }

    /// Download or resume a target file with real-time progress reporting.
    pub async fn download_file(
        &self,
        target: &DownloadTarget,
        pb: &ProgressBar,
    ) -> Result<DownloadResult, anyhow::Error> {
        let dest = &target.destination_path;
        let part_file = dest.with_extension("zip.part");

        // 1. If final destination file already exists and matches expected size, skip!
        if dest.exists() {
            let existing_len = tokio::fs::metadata(dest).await?.len();
            if let Some(expected) = target.expected_size {
                if existing_len == expected && expected > 0 {
                    pb.set_length(existing_len);
                    pb.set_position(existing_len);
                    pb.finish_with_message(format!("✔ {} (cached)", target.filename));
                    return Ok(DownloadResult::AlreadyComplete { total_bytes: existing_len });
                }
            }
        }

        // Ensure parent directory exists
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut attempt = 0;
        let mut last_error: Option<anyhow::Error> = None;

        while attempt < self.config.max_retries {
            attempt += 1;

            // Check current size of .part file on disk
            let mut local_len = if part_file.exists() {
                tokio::fs::metadata(&part_file).await?.len()
            } else {
                0
            };

            // If local part is already >= expected_size, verify
            if let Some(expected) = target.expected_size {
                if local_len > expected {
                    // Part file larger than expected (corrupt/stale) -> truncate to 0
                    let _ = tokio::fs::remove_file(&part_file).await;
                    local_len = 0;
                } else if local_len == expected && expected > 0 {
                    // Already completely downloaded into part file -> promote to final
                    tokio::fs::rename(&part_file, dest).await?;
                    pb.set_length(expected);
                    pb.set_position(expected);
                    pb.finish_with_message(format!("✔ {} (completed)", target.filename));
                    return Ok(DownloadResult::AlreadyComplete { total_bytes: expected });
                }
            }

            match self.execute_download_attempt(target, &part_file, local_len, pb).await {
                Ok(result) => {
                    // Atomic rename from .part to final destination
                    tokio::fs::rename(&part_file, dest).await?;
                    pb.finish_with_message(format!("✔ {}", target.filename));
                    return Ok(result);
                }
                Err(err) => {
                    if is_fatal_error(&err) {
                        return Err(err);
                    }
                    last_error = Some(err);

                    // Exponential backoff with jitter calculation
                    let delay = calculate_backoff(
                        attempt,
                        self.config.initial_delay_ms,
                        self.config.max_delay_ms,
                        self.config.backoff_factor,
                    );

                    pb.set_message(format!(
                        "⚠ Retry {}/{} in {:.1}s ({})",
                        attempt,
                        self.config.max_retries,
                        delay.as_secs_f32(),
                        target.filename
                    ));

                    tokio::time::sleep(delay).await;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Download failed after max retries")))
    }

    /// Single download/resume HTTP attempt.
    async fn execute_download_attempt(
        &self,
        target: &DownloadTarget,
        part_file: &Path,
        start_byte: u64,
        pb: &ProgressBar,
    ) -> Result<DownloadResult, anyhow::Error> {
        let mut req = self.client.get(&target.url);

        // Add Basic Auth if share_token is set
        if let Some(ref token) = self.config.share_token {
            req = req.basic_auth(token, Some(""));
        }

        // Add Range header if start_byte > 0
        if start_byte > 0 {
            req = req.header(RANGE, format!("bytes={}-", start_byte));
        }

        let resp = req.send().await?;
        let status = resp.status();

        match status {
            // Server honored Range request
            StatusCode::PARTIAL_CONTENT => {
                let total_size = parse_content_range_total(resp.headers())
                    .or(target.expected_size)
                    .unwrap_or(start_byte + resp.content_length().unwrap_or(0));

                pb.set_length(total_size);
                pb.set_position(start_byte);

                // Open in write mode and seek to start_byte for appending
                let file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(part_file)
                    .await?;
                let mut writer = BufWriter::with_capacity(self.config.buffer_size_kb * 1024, file);
                writer.seek(SeekFrom::Start(start_byte)).await?;

                let transferred = stream_response_body(resp, &mut writer, pb).await?;
                writer.flush().await?;

                Ok(DownloadResult::Resumed {
                    bytes_appended: transferred,
                    total_bytes: start_byte + transferred,
                })
            }

            // Server returned 200 OK
            StatusCode::OK => {
                let content_len = resp.content_length().unwrap_or(0);

                // RFB / Nextcloud quirk: If start_byte >= total_size, returns 200 OK with Content-Length: 0
                if start_byte > 0 && content_len == 0 {
                    return Ok(DownloadResult::AlreadyComplete {
                        total_bytes: start_byte,
                    });
                }

                // If server ignored Range header (or start_byte == 0), truncate to 0 and stream full file
                let total_size = if content_len > 0 {
                    content_len
                } else {
                    target.expected_size.unwrap_or(0)
                };

                pb.set_length(total_size);
                pb.set_position(0);

                let file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(part_file)
                    .await?;
                let mut writer = BufWriter::with_capacity(self.config.buffer_size_kb * 1024, file);

                let transferred = stream_response_body(resp, &mut writer, pb).await?;
                writer.flush().await?;

                Ok(DownloadResult::FreshDownload {
                    bytes_transferred: transferred,
                })
            }

            // RFC 9110: Range Not Satisfiable (file already complete on server)
            StatusCode::RANGE_NOT_SATISFIABLE => {
                let total_size = parse_content_range_unsatisfiable_total(resp.headers())
                    .or(target.expected_size)
                    .unwrap_or(start_byte);

                if start_byte >= total_size && total_size > 0 {
                    pb.set_length(total_size);
                    pb.set_position(total_size);
                    Ok(DownloadResult::AlreadyComplete { total_bytes: total_size })
                } else {
                    // Local file corrupted or longer than remote -> truncate and re-download
                    let _ = tokio::fs::remove_file(part_file).await;
                    anyhow::bail!("Range 416 received with invalid local size; file reset for fresh retry");
                }
            }

            // Transient error status codes
            StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT => {
                anyhow::bail!("Server transient error: HTTP {}", status);
            }

            // Fatal client errors
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {
                let err_msg = format!("Fatal HTTP error {}: URL {}", status, target.url);
                anyhow::bail!(err_msg);
            }

            // Any other status code
            _ => {
                anyhow::bail!("Unexpected HTTP status code: {}", status);
            }
        }
    }
}

/// Stream response chunks into BufWriter while updating the ProgressBar.
async fn stream_response_body(
    mut resp: reqwest::Response,
    writer: &mut BufWriter<File>,
    pb: &ProgressBar,
) -> Result<u64, anyhow::Error> {
    let mut transferred: u64 = 0;
    while let Some(chunk) = resp.chunk().await? {
        writer.write_all(&chunk).await?;
        let len = chunk.len() as u64;
        transferred += len;
        pb.inc(len);
    }
    Ok(transferred)
}

/// Parse total bytes from `Content-Range: bytes <start>-<end>/<total>`.
fn parse_content_range_total(headers: &HeaderMap) -> Option<u64> {
    let header_str = headers.get("content-range")?.to_str().ok()?;
    let slash_idx = header_str.rfind('/')?;
    let total_str = &header_str[slash_idx + 1..].trim();
    total_str.parse::<u64>().ok()
}

/// Parse total bytes from `Content-Range: bytes */<total>` (HTTP 416).
fn parse_content_range_unsatisfiable_total(headers: &HeaderMap) -> Option<u64> {
    parse_content_range_total(headers)
}

/// Check if an error is non-retryable (fatal).
fn is_fatal_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.contains("Fatal HTTP error")
        || msg.contains("401 Unauthorized")
        || msg.contains("403 Forbidden")
        || msg.contains("404 Not Found")
}

/// Calculate exponential backoff duration with microsecond jitter.
fn calculate_backoff(attempt: usize, initial_ms: u64, max_ms: u64, factor: f64) -> Duration {
    let exponent = (attempt.saturating_sub(1)) as i32;
    let base_delay = (initial_ms as f64) * factor.powi(exponent);
    let capped_delay = base_delay.min(max_ms as f64);

    // Microsecond jitter: pseudo-random offset within ±20%
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let jitter_ratio = 0.8 + ((now_nanos % 400) as f64 / 1000.0); // 0.80 to 1.20
    let final_ms = (capped_delay * jitter_ratio) as u64;

    Duration::from_millis(final_ms)
}

/// Helper to configure a standard indicatif ProgressBar style.
pub fn create_download_progress_style() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("{msg:<24} [{elapsed_precise}] [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA: {eta})")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-")
}
