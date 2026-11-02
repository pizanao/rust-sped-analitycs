//! Resumable HTTP 206 Range Download Client (`downloader/range_client.rs`)
//!
//! Provides crash-resilient HTTP downloading with:
//! - HTTP 206 Partial Content resume via `Range: bytes={offset}-`
//! - Safe `.part` staging and atomic renaming
//! - Status handling for 206, 200 (both truncate and EOF quirk), and 416
//! - F5 BIG-IP WAF bypass headers and cookie persistence
//! - Real-time progress bars with speed, ETA, and byte counters via `indicatif`
//! - Transient error retry loop with exponential backoff, jitter, and resume preservation

use std::path::Path;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, RANGE, USER_AGENT};
use reqwest::{Client, StatusCode};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, BufWriter, SeekFrom};

pub use crate::downloader::webdav::DownloadTarget;

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

/// Outcome of a download operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadResult {
    /// File was already completely downloaded on disk (matched expected size or server returned 416/200-0).
    AlreadyComplete { total_bytes: u64 },
    /// File was fresh-downloaded from byte 0.
    FreshDownload { bytes_transferred: u64 },
    /// File was resumed from a previous partial download.
    Resumed {
        bytes_appended: u64,
        total_bytes: u64,
    },
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
            HeaderValue::from_str(&config.user_agent)
                .unwrap_or(HeaderValue::from_static("Mozilla/5.0")),
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
                    return Ok(DownloadResult::AlreadyComplete {
                        total_bytes: existing_len,
                    });
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
                    return Ok(DownloadResult::AlreadyComplete {
                        total_bytes: expected,
                    });
                }
            }

            match self
                .execute_download_attempt(target, &part_file, local_len, pb)
                .await
            {
                Ok(result) => {
                    // VULN-01: Verify file length matches target.expected_size before atomic rename
                    let final_size = tokio::fs::metadata(&part_file).await?.len();
                    if let Some(expected) = target.expected_size {
                        if expected > 0 && final_size != expected {
                            anyhow::bail!(
                                "Size mismatch for {}: got {} bytes on disk, expected {} bytes",
                                target.filename,
                                final_size,
                                expected
                            );
                        }
                    }

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

                // Open in write mode (without truncating) and seek to start_byte for appending
                let file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(false)
                    .open(part_file)
                    .await?;
                let mut writer = BufWriter::with_capacity(self.config.buffer_size_kb * 1024, file);
                writer.seek(SeekFrom::Start(start_byte)).await?;

                let expected_total =
                    parse_content_range_total(resp.headers()).or(target.expected_size);

                let stream_res = stream_response_body(resp, &mut writer, pb).await;
                let flush_res = writer.flush().await;
                let transferred = stream_res?;
                flush_res?;

                let actual_total = start_byte + transferred;
                if let Some(expected) = expected_total {
                    if expected > 0 && actual_total != expected {
                        anyhow::bail!(
                            "Incomplete range download for {}: received {} bytes total, expected {}",
                            target.filename,
                            actual_total,
                            expected
                        );
                    }
                }

                Ok(DownloadResult::Resumed {
                    bytes_appended: transferred,
                    total_bytes: actual_total,
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

                let expected_total = resp.content_length().or(target.expected_size);

                let stream_res = stream_response_body(resp, &mut writer, pb).await;
                let flush_res = writer.flush().await;
                let transferred = stream_res?;
                flush_res?;

                if let Some(expected) = expected_total {
                    if expected > 0 && transferred != expected {
                        anyhow::bail!(
                            "Incomplete download for {}: received {} bytes, expected {}",
                            target.filename,
                            transferred,
                            expected
                        );
                    }
                }

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
                    Ok(DownloadResult::AlreadyComplete {
                        total_bytes: total_size,
                    })
                } else {
                    // Local file corrupted or longer than remote -> truncate and re-download
                    let _ = tokio::fs::remove_file(part_file).await;
                    anyhow::bail!(
                        "Range 416 received with invalid local size; file reset for fresh retry"
                    );
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
pub fn parse_content_range_total(headers: &HeaderMap) -> Option<u64> {
    let header_str = headers.get("content-range")?.to_str().ok()?;
    let slash_idx = header_str.rfind('/')?;
    let total_str = header_str[slash_idx + 1..].trim();
    total_str.parse::<u64>().ok()
}

/// Parse total bytes from `Content-Range: bytes */<total>` (HTTP 416).
pub fn parse_content_range_unsatisfiable_total(headers: &HeaderMap) -> Option<u64> {
    parse_content_range_total(headers)
}

/// Check if an error is non-retryable (fatal).
///
/// Identifies fatal HTTP response statuses (401, 403, 404) and filesystem storage
/// capacity exhaustion errors (`StorageFull` / `ENOSPC` / "No space left on device").
pub fn is_fatal_error(err: &anyhow::Error) -> bool {
    // 1. Check typed I/O errors in the causal chain for storage capacity exhaustion
    for cause in err.chain() {
        if let Some(io_err) = cause.downcast_ref::<std::io::Error>() {
            if io_err.kind() == std::io::ErrorKind::StorageFull
                || io_err.raw_os_error() == Some(28)   // Unix ENOSPC (No space left on device)
                || io_err.raw_os_error() == Some(112)
            // Windows ERROR_DISK_FULL
            {
                return true;
            }
        }
    }

    // 2. Check string messages for fatal HTTP codes and disk space exhaustion patterns
    let msg = err.to_string();
    if msg.contains("Fatal HTTP error")
        || msg.contains("401 Unauthorized")
        || msg.contains("403 Forbidden")
        || msg.contains("404 Not Found")
        || msg.contains("StorageFull")
        || msg.contains("ENOSPC")
    {
        return true;
    }

    let msg_lower = msg.to_lowercase();
    msg_lower.contains("no space left on device")
        || msg_lower.contains("not enough space on the disk")
        || msg_lower.contains("disk full")
}

/// Calculate exponential backoff duration with microsecond jitter.
pub fn calculate_backoff(attempt: usize, initial_ms: u64, max_ms: u64, factor: f64) -> Duration {
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

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn test_parse_content_range_total() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "content-range",
            HeaderValue::from_static("bytes 100-200/1180"),
        );
        assert_eq!(parse_content_range_total(&headers), Some(1180));

        let mut headers_416 = HeaderMap::new();
        headers_416.insert("content-range", HeaderValue::from_static("bytes */50000"));
        assert_eq!(
            parse_content_range_unsatisfiable_total(&headers_416),
            Some(50000)
        );
    }

    #[test]
    fn test_fatal_error_detection() {
        let err1 = anyhow::anyhow!("Fatal HTTP error 404: URL not found");
        assert!(is_fatal_error(&err1));

        let err2 = anyhow::anyhow!("Server transient error: HTTP 503");
        assert!(!is_fatal_error(&err2));

        // Typed std::io::ErrorKind::StorageFull
        let io_storage_full = std::io::Error::new(std::io::ErrorKind::StorageFull, "disk full");
        let err_storage = anyhow::Error::new(io_storage_full);
        assert!(is_fatal_error(&err_storage));

        // POSIX raw OS error 28 (ENOSPC)
        let io_enospc = std::io::Error::from_raw_os_error(28);
        let err_enospc = anyhow::Error::new(io_enospc);
        assert!(is_fatal_error(&err_enospc));

        // Context-wrapped I/O error
        let chained_err = anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::StorageFull,
            "insufficient storage capacity",
        ))
        .context("failed to write downloaded chunk to .part file");
        assert!(is_fatal_error(&chained_err));

        // String-formatted message containing 'No space left on device'
        let string_enospc = anyhow::anyhow!("IO error: No space left on device (os error 28)");
        assert!(is_fatal_error(&string_enospc));

        // Transient I/O error (e.g. connection reset or timeout) should remain retryable
        let io_transient = std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "connection reset by peer",
        );
        let err_transient = anyhow::Error::new(io_transient);
        assert!(!is_fatal_error(&err_transient));
    }

    #[test]
    fn test_calculate_backoff_bounds() {
        let d1 = calculate_backoff(1, 1000, 30000, 2.0);
        assert!(d1.as_millis() >= 800 && d1.as_millis() <= 1200);

        let d5 = calculate_backoff(5, 1000, 30000, 2.0);
        // 1000 * 2^4 = 16000 -> 16000 * 0.8 = 12800 to 16000 * 1.2 = 19200
        assert!(d5.as_millis() >= 12000 && d5.as_millis() <= 20000);

        let d10 = calculate_backoff(10, 1000, 30000, 2.0);
        // capped at 30000 -> 24000 to 36000
        assert!(d10.as_millis() >= 24000 && d10.as_millis() <= 36000);
    }

    #[tokio::test]
    async fn test_range_client_live_download_and_cache() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let dest = temp_dir.path().join("Qualificacoes.zip");

        let target = DownloadTarget {
            url: "https://arquivos.receitafederal.gov.br/public.php/dav/files/YggdBLfdninEJX9/2026-10/Qualificacoes.zip"
                .to_string(),
            filename: "Qualificacoes.zip".to_string(),
            destination_path: dest.clone(),
            expected_size: Some(980),
        };

        let config = RangeClientConfig {
            share_token: Some("YggdBLfdninEJX9".to_string()),
            ..Default::default()
        };
        let client = RangeClient::new(config).expect("create client");
        let pb = ProgressBar::hidden();

        // 1. Initial download
        let result = client
            .download_file(&target, &pb)
            .await
            .expect("download file");
        assert!(matches!(
            result,
            DownloadResult::FreshDownload { .. } | DownloadResult::AlreadyComplete { .. }
        ));
        assert!(dest.exists());
        assert_eq!(std::fs::metadata(&dest).expect("stat").len(), 980);

        // Verify valid zip
        let zip_file = std::fs::File::open(&dest).expect("open zip");
        let archive = zip::ZipArchive::new(zip_file).expect("read zip archive");
        assert!(!archive.is_empty());

        // 2. Second download attempt should be recognized as already complete (cached)
        let cached_result = client
            .download_file(&target, &pb)
            .await
            .expect("download cached file");
        assert_eq!(
            cached_result,
            DownloadResult::AlreadyComplete { total_bytes: 980 }
        );
    }

    #[tokio::test]
    async fn test_vuln01_prevent_premature_rename_on_truncated_stream() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut req_buf = vec![0u8; 1024];
                let _ = socket.read(&mut req_buf).await;
                // Return only 400 bytes claiming 1000 bytes
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 400\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.write_all(&vec![0xAAu8; 400]).await;
                let _ = socket.flush().await;
            }
        });

        let temp_dir = tempfile::tempdir().unwrap();
        let dest = temp_dir.path().join("truncated.zip");
        let target = DownloadTarget {
            url: format!("http://127.0.0.1:{}/truncated.zip", addr.port()),
            filename: "truncated.zip".to_string(),
            destination_path: dest.clone(),
            expected_size: Some(1000), // Expecting 1000, server gives 400
        };

        let config = RangeClientConfig {
            max_retries: 2,
            initial_delay_ms: 10,
            ..Default::default()
        };
        let client = RangeClient::new(config).unwrap();
        let pb = ProgressBar::hidden();

        let res = client.download_file(&target, &pb).await;
        // The download must fail because the file is incomplete
        assert!(res.is_err());
        // The truncated .part must NEVER have been renamed to .zip!
        assert!(!dest.exists());
    }

    #[tokio::test]
    async fn test_vuln02_bufwriter_flushes_on_stream_disconnect() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut req_buf = vec![0u8; 1024];
                let _ = socket.read(&mut req_buf).await;
                // Send headers claiming 1000 bytes, but abruptly disconnect after 300 bytes
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.write_all(&vec![0xBBu8; 300]).await;
                let _ = socket.flush().await;
                // Abruptly close socket
                drop(socket);
            }
        });

        let temp_dir = tempfile::tempdir().unwrap();
        let dest = temp_dir.path().join("disconnect.zip");
        let part_file = dest.with_extension("zip.part");
        let target = DownloadTarget {
            url: format!("http://127.0.0.1:{}/disconnect.zip", addr.port()),
            filename: "disconnect.zip".to_string(),
            destination_path: dest.clone(),
            expected_size: Some(1000),
        };

        let config = RangeClientConfig {
            max_retries: 1,
            ..Default::default()
        };
        let client = RangeClient::new(config).unwrap();
        let pb = ProgressBar::hidden();

        let res = client.download_file(&target, &pb).await;
        assert!(res.is_err());
        // Verify that the 300 bytes received before disconnect were flushed to disk in .part!
        assert!(part_file.exists());
        assert_eq!(tokio::fs::metadata(&part_file).await.unwrap().len(), 300);
    }
}
