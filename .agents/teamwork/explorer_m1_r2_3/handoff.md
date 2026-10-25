# Handoff Report: Quiet Flag Propagation and ENOSPC / StorageFull Error Handling

**Agent**: `explorer_m1_r2_3`  
**Milestone**: Milestone 1 Iteration 2 (Scaffold & Resumable Downloader Remediation)  
**Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/explorer_m1_r2_3`  
**Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`  
**Parent / Caller**: `orchestrator_1` (Conversation ID: `0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)  
**Date**: 2026-10-24T20:30:00Z  

---

## 1. Observation

Direct examination of `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas` identified the following code locations and behaviors:

### 1.1 Quiet Flag Disconnect
1. In `src/cli.rs:29-36`, `Cli` declares the global quiet flag:
   ```rust
   #[arg(
       short = 'q',
       long = "quiet",
       global = true,
       help = "Suppress non-error output and progress bars"
   )]
   pub quiet: bool,
   ```
2. In `src/main.rs:27-30`, the `download` subcommand dispatch discards `cli.quiet`:
   ```rust
   match cli.command {
       Commands::Download(args) => {
           rfb_cnpj_etl::downloader::run(args).await?;
       }
   ```
   `cli.quiet` adjusts the `tracing` filter in `main.rs:14-19`, but is never forwarded to `downloader::run(args)`.
3. In `src/downloader/mod.rs:22-50`, `DownloaderOptions` lacks a `quiet` field, and `impl From<DownloadArgs> for DownloaderOptions` has no quiet awareness.
4. In `src/downloader/mod.rs:70-94`, discovery dry-run unconditionally prints file details via `println!`:
   ```rust
   if options.dry_run {
       let total_bytes = catalog.total_expected_bytes();
       let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
       println!(
           "🔍 Discovery mode (dry-run): {} files found for snapshot '{}' (fallback: {}, total: {:.2} MB)",
           ...
       );
       for (i, t) in targets.iter().enumerate() { ... println!(...); }
       return Ok(());
   }
   ```
5. In `src/downloader/mod.rs:127-137, 155-160, 200-207`, `download_all` unconditionally instantiates `indicatif::MultiProgress`, adds child progress bars for every file, and prints the completion summary banner with `println!`.
6. Running `cargo run -- download -q --dry-run` verified that all 37 file targets are printed to stdout, confirming that `-q` is currently ignored.

### 1.2 Disk Space Exhaustion (`ENOSPC` / `StorageFull`) Retry Loop
1. In `src/downloader/range_client.rs:374-381`, `is_fatal_error` inspects only string representations of HTTP status codes:
   ```rust
   pub fn is_fatal_error(err: &anyhow::Error) -> bool {
       let msg = err.to_string();
       msg.contains("Fatal HTTP error")
           || msg.contains("401 Unauthorized")
           || msg.contains("403 Forbidden")
           || msg.contains("404 Not Found")
   }
   ```
2. In `src/downloader/range_client.rs:182-205`, `RangeClient::download_file` handles errors from `execute_download_attempt`:
   ```rust
   Err(err) => {
       if is_fatal_error(&err) {
           return Err(err);
       }
       last_error = Some(err);

       // Exponential backoff with jitter calculation
       let delay = calculate_backoff(attempt, ...);
       pb.set_message(format!("⚠ Retry {}/{} in {:.1}s ({})", attempt, ...));
       tokio::time::sleep(delay).await;
   }
   ```
   When `is_fatal_error` returns `false`, the client treats the error as transient and performs up to `max_retries` attempts (default 5 to 8 attempts) with exponential backoff delays.
3. In `src/downloader/range_client.rs:346-359`, `stream_response_body` writes chunks into `tokio::io::BufWriter`:
   ```rust
   while let Some(chunk) = resp.chunk().await? {
       writer.write_all(&chunk).await?;
       let len = chunk.len() as u64;
       transferred += len;
       pb.inc(len);
   }
   ```
   When the filesystem runs out of disk space, `writer.write_all` fails with `std::io::Error` (`io::ErrorKind::StorageFull`, POSIX errno 28 `ENOSPC`).
4. Because `is_fatal_error` does not recognize `StorageFull` or `ENOSPC`, the downloader retries 5+ times, sleeping between retries, instead of aborting immediately.

---

## 2. Logic Chain

1. **Quiet Flag Propagation**:
   - `clap` parses `-q` / `--quiet` globally into `cli.quiet: bool` on `src/cli.rs:Cli` (Observation 1.1.1).
   - Passing `cli.quiet` from `src/main.rs:29` into `downloader::run(args, cli.quiet)` propagates the command-line intent to the execution layer without duplicating clap argument definitions.
   - Adding `pub quiet: bool` to `DownloaderOptions` and a constructor `DownloaderOptions::from_args(args: DownloadArgs, quiet: bool)` ensures that all downloader routines have access to the setting while keeping `From<DownloadArgs>` backward-compatible (Observation 1.1.3).
   - In `indicatif`, `ProgressBar::hidden()` is a no-op progress bar that satisfies all `ProgressBar` API calls (`set_length`, `inc`, `set_position`, `finish_with_message`, `abandon_with_message`) without emitting terminal control codes or characters.
   - When `options.quiet` is `true`, setting `overall_pb` and task child `pb` instances to `ProgressBar::hidden()` and skipping `MultiProgress::new()` completely eliminates progress bar rendering.
   - Wrapping dry-run discovery `println!` and completion summary `println!` under `if !options.quiet` eliminates non-error stdout messages (Observations 1.1.4, 1.1.5).

2. **Fatal Disk Space Error Detection**:
   - In Rust, I/O writes on a full volume return `std::io::Error` with `io::ErrorKind::StorageFull` (standardized in Rust stdlib) and raw OS error 28 (`ENOSPC` on POSIX Unix/Linux) or 112 (`ERROR_DISK_FULL` on Windows).
   - In `anyhow::Error`, underlying causes may be wrapped in context layers. Calling `err.chain()` iterates through the causal chain, enabling `cause.downcast_ref::<std::io::Error>()` to inspect the typed I/O error directly.
   - Adding checks for `ErrorKind::StorageFull`, raw OS error 28/112, and message fallback strings (`"StorageFull"`, `"ENOSPC"`, `"No space left on device"`, `"disk full"`) to `is_fatal_error` guarantees that both typed and string-formatted errors match.
   - In `src/downloader/range_client.rs:182-185`, returning `true` from `is_fatal_error` causes `download_file` to bypass the retry loop and immediately return `Err(err)` on the first attempt, preventing redundant write retries on an exhausted disk.

---

## 3. Caveats

1. **Stderr Fallback Warnings in `webdav.rs`**:
   In `src/downloader/webdav.rs:271` and `285`, `eprintln!` is used to notify when live WebDAV fails and static fallback catalog activates:
   ```rust
   eprintln!("⚠️ Live WebDAV discovery failed ({err}). Activating static fallback catalog for '{target_ym}'.");
   ```
   Because these messages write directly to `stderr` rather than `tracing`, they are not muted by `main.rs`'s `Level::ERROR` tracing filter. While standard CLI conventions allow diagnostic error/warning messages on `stderr` even in quiet mode, changing them to `tracing::warn!` in a subsequent cleanup would make `webdav.rs` fully silent under `-q`.
2. **Platform-Specific OS Error Codes**:
   Linux/Unix uses errno 28 (`ENOSPC`); Windows uses error 112 (`ERROR_DISK_FULL`) and error 39 (`ERROR_HANDLE_DISK_FULL`). Rust's `std::io::ErrorKind::StorageFull` abstracts both. The proposed implementation checks both `ErrorKind::StorageFull` and the raw OS error numbers for maximum cross-platform defense-in-depth.
3. **Decoupled Concurrency Implementation**:
   Another remediation task refactors task joining in `src/downloader/mod.rs` to use `tokio::task::JoinSet`. The diffs proposed here for quiet mode and `is_fatal_error` are completely orthogonal: `ProgressBar::hidden()` and `is_fatal_error` function identically whether tasks are awaited in a `Vec<JoinHandle>` or a `JoinSet`.

---

## 4. Conclusion & Concrete Code Diffs

The design resolves both issues cleanly with zero architectural changes to existing structs and without introducing new external dependencies.

Below are the exact unified code diffs to be applied to the target project (`/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`).

### Diff 1: `src/main.rs`
Propagate `cli.quiet` to `rfb_cnpj_etl::downloader::run`:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -26,7 +26,7 @@ async fn main() -> Result<()> {
 
     match cli.command {
         Commands::Download(args) => {
-            rfb_cnpj_etl::downloader::run(args).await?;
+            rfb_cnpj_etl::downloader::run(args, cli.quiet).await?;
         }
         Commands::BuildCnpj(args) => {
             rfb_cnpj_etl::ingestion::run(args)?;
```

---

### Diff 2: `src/downloader/mod.rs`
Add `quiet` to `DownloaderOptions`, support `from_args`, update `run`, silence `MultiProgress` and non-error prints, and add a unit test:

```diff
--- a/src/downloader/mod.rs
+++ b/src/downloader/mod.rs
@@ -31,26 +31,44 @@ pub struct DownloaderOptions {
     pub max_retries: u32,
     pub dry_run: bool,
     pub force: bool,
+    pub quiet: bool,
 }
 
+impl DownloaderOptions {
+    /// Construct downloader options from CLI `DownloadArgs` and explicit `quiet` setting.
+    pub fn from_args(args: DownloadArgs, quiet: bool) -> Self {
+        Self {
+            output_dir: args.output_dir,
+            share_token: args.share_token,
+            year_month: args.year_month,
+            base_url: args.base_url,
+            concurrency: args.concurrency as usize,
+            user_agent: args.user_agent,
+            timeout_secs: args.timeout_secs,
+            max_retries: args.max_retries,
+            dry_run: args.dry_run,
+            force: args.force,
+            quiet,
+        }
+    }
+}
+
 impl From<DownloadArgs> for DownloaderOptions {
     fn from(args: DownloadArgs) -> Self {
-        Self {
-            output_dir: args.output_dir,
-            share_token: args.share_token,
-            year_month: args.year_month,
-            base_url: args.base_url,
-            concurrency: args.concurrency as usize,
-            user_agent: args.user_agent,
-            timeout_secs: args.timeout_secs,
-            max_retries: args.max_retries,
-            dry_run: args.dry_run,
-            force: args.force,
-        }
+        Self::from_args(args, false)
     }
 }
 
 /// Main entry point for the `download` subcommand.
-pub async fn run(args: DownloadArgs) -> Result<(), anyhow::Error> {
-    let options = DownloaderOptions::from(args);
+pub async fn run(args: DownloadArgs, quiet: bool) -> Result<(), anyhow::Error> {
+    let options = DownloaderOptions::from_args(args, quiet);
 
     let webdav_config = WebdavConfig {
         base_url: options.base_url.clone(),
@@ -70,25 +88,27 @@ pub async fn run(args: DownloadArgs) -> Result<(), anyhow::Error> {
     let targets = catalog.to_download_targets(&options.output_dir);
 
     if options.dry_run {
-        let total_bytes = catalog.total_expected_bytes();
-        let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
-        println!(
-            "🔍 Discovery mode (dry-run): {} files found for snapshot '{}' (fallback: {}, total: {:.2} MB)",
-            targets.len(),
-            catalog.year_month,
-            catalog.is_fallback,
-            total_mb
-        );
-        for (i, t) in targets.iter().enumerate() {
-            let size_str = match t.expected_size {
-                Some(sz) => format!("{sz} bytes ({:.2} MB)", sz as f64 / (1024.0 * 1024.0)),
-                None => "unknown size".to_string(),
-            };
+        if !options.quiet {
+            let total_bytes = catalog.total_expected_bytes();
+            let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
             println!(
-                "  [{:>2}/37] {:<25} -> {:?} ({})",
-                i + 1,
-                t.filename,
-                t.destination_path,
-                size_str
+                "🔍 Discovery mode (dry-run): {} files found for snapshot '{}' (fallback: {}, total: {:.2} MB)",
+                targets.len(),
+                catalog.year_month,
+                catalog.is_fallback,
+                total_mb
             );
+            for (i, t) in targets.iter().enumerate() {
+                let size_str = match t.expected_size {
+                    Some(sz) => format!("{sz} bytes ({:.2} MB)", sz as f64 / (1024.0 * 1024.0)),
+                    None => "unknown size".to_string(),
+                };
+                println!(
+                    "  [{:>2}/37] {:<25} -> {:?} ({})",
+                    i + 1,
+                    t.filename,
+                    t.destination_path,
+                    size_str
+                );
+            }
         }
         return Ok(());
     }
@@ -124,16 +144,20 @@ pub async fn download_all(
     };
     let client = Arc::new(RangeClient::new(config)?);
 
-    let mp = Arc::new(MultiProgress::new());
-
-    // Overall progress bar (Total Files)
-    let total_files = targets.len() as u64;
-    let overall_style = ProgressStyle::default_bar()
-        .template("{prefix:<18} [{elapsed_precise}] [{bar:30.green/black}] {pos}/{len} files ({percent}%) [ETA: {eta}]")?
-        .progress_chars("=>-");
-    let overall_pb = mp.add(ProgressBar::new(total_files));
-    overall_pb.set_style(overall_style);
-    overall_pb.set_prefix("Overall Progress");
+    let (mp, overall_pb) = if options.quiet {
+        (None, ProgressBar::hidden())
+    } else {
+        let mp = Arc::new(MultiProgress::new());
+        let total_files = targets.len() as u64;
+        let overall_style = ProgressStyle::default_bar()
+            .template("{prefix:<18} [{elapsed_precise}] [{bar:30.green/black}] {pos}/{len} files ({percent}%) [ETA: {eta}]")?
+            .progress_chars("=>-");
+        let pb = mp.add(ProgressBar::new(total_files));
+        pb.set_style(overall_style);
+        pb.set_prefix("Overall Progress");
+        (Some(mp), pb)
+    };
 
     // Bounded concurrency semaphore (e.g. 3 or 4 concurrent connections)
     let semaphore = Arc::new(Semaphore::new(options.concurrency.max(1)));
@@ -153,9 +177,15 @@ pub async fn download_all(
         let task = tokio::spawn(async move {
             let _permit = sem.acquire().await.expect("Semaphore acquire failed");
 
-            // Allocate a child progress bar in MultiProgress for this active file
-            let pb = mp.add(ProgressBar::new(target.expected_size.unwrap_or(0)));
-            pb.set_style(file_style);
-            pb.set_message(target.filename.clone());
+            let pb = match &mp {
+                Some(mp) => {
+                    let pb = mp.add(ProgressBar::new(target.expected_size.unwrap_or(0)));
+                    pb.set_style(file_style);
+                    pb.set_message(target.filename.clone());
+                    pb
+                }
+                None => ProgressBar::hidden(),
+            };
 
             let res = client.download_file(&target, &pb).await;
 
@@ -185,24 +215,59 @@ pub async fn download_all(
             }
             Err(e) => {
-                overall_pb.abandon_with_message("Download failed");
+                if !options.quiet {
+                    overall_pb.abandon_with_message("Download failed");
+                }
                 return Err(e);
             }
         }
     }
 
-    overall_pb.finish_with_message("Download complete!");
+    if !options.quiet {
+        overall_pb.finish_with_message("Download complete!");
 
-    let elapsed = start_time.elapsed();
-    let speed_mb_s =
-        (total_transferred_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64().max(0.001);
+        let elapsed = start_time.elapsed();
+        let speed_mb_s =
+            (total_transferred_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64().max(0.001);
 
-    println!(
-        "\n✅ Finished in {:.2}s ({:.2} MB/s avg): {} downloaded, {} resumed, {} already complete.",
-        elapsed.as_secs_f64(),
-        speed_mb_s,
-        fresh_count,
-        resumed_count,
-        cached_count,
-    );
+        println!(
+            "\n✅ Finished in {:.2}s ({:.2} MB/s avg): {} downloaded, {} resumed, {} already complete.",
+            elapsed.as_secs_f64(),
+            speed_mb_s,
+            fresh_count,
+            resumed_count,
+            cached_count,
+        );
+    }
 
     Ok(())
 }
+
+#[cfg(test)]
+mod tests {
+    use super::*;
+
+    #[test]
+    fn test_downloader_options_quiet_propagation() {
+        let args = DownloadArgs {
+            output_dir: PathBuf::from("dados-publicos-zip"),
+            share_token: "token123".to_string(),
+            year_month: Some("2026-10".to_string()),
+            base_url: "https://example.com".to_string(),
+            concurrency: 4,
+            user_agent: "test-agent".to_string(),
+            timeout_secs: 30,
+            max_retries: 3,
+            dry_run: true,
+            force: false,
+        };
+
+        let opts_default = DownloaderOptions::from(args.clone());
+        assert!(!opts_default.quiet);
+
+        let opts_quiet = DownloaderOptions::from_args(args, true);
+        assert!(opts_quiet.quiet);
+        assert!(opts_quiet.dry_run);
+        assert_eq!(opts_quiet.concurrency, 4);
+    }
+}
```

---

### Diff 3: `src/downloader/range_client.rs`
Enhance `is_fatal_error` to detect `ErrorKind::StorageFull`, raw OS error 28 (`ENOSPC`), 112 (`ERROR_DISK_FULL`), and message substrings; add comprehensive unit tests:

```diff
--- a/src/downloader/range_client.rs
+++ b/src/downloader/range_client.rs
@@ -372,10 +372,30 @@ pub fn parse_content_range_unsatisfiable_total(headers: &HeaderMap) -> Option<u6
 }
 
 /// Check if an error is non-retryable (fatal).
+///
+/// Identifies fatal HTTP response statuses (401, 403, 404) and filesystem storage
+/// capacity exhaustion errors (`StorageFull` / `ENOSPC` / "No space left on device").
 pub fn is_fatal_error(err: &anyhow::Error) -> bool {
+    // 1. Check typed I/O errors in the causal chain for storage capacity exhaustion
+    for cause in err.chain() {
+        if let Some(io_err) = cause.downcast_ref::<std::io::Error>() {
+            if io_err.kind() == std::io::ErrorKind::StorageFull
+                || io_err.raw_os_error() == Some(28)   // Unix ENOSPC (No space left on device)
+                || io_err.raw_os_error() == Some(112)  // Windows ERROR_DISK_FULL
+            {
+                return true;
+            }
+        }
+    }
+
+    // 2. Check string messages for fatal HTTP codes and disk space exhaustion patterns
     let msg = err.to_string();
-    msg.contains("Fatal HTTP error")
+    if msg.contains("Fatal HTTP error")
         || msg.contains("401 Unauthorized")
         || msg.contains("403 Forbidden")
         || msg.contains("404 Not Found")
+        || msg.contains("StorageFull")
+        || msg.contains("ENOSPC")
+    {
+        return true;
+    }
+
+    let msg_lower = msg.to_lowercase();
+    msg_lower.contains("no space left on device")
+        || msg_lower.contains("not enough space on the disk")
+        || msg_lower.contains("disk full")
 }
 
 /// Calculate exponential backoff duration with microsecond jitter.
@@ -434,6 +454,34 @@ mod tests {
         let err2 = anyhow::anyhow!("Server transient error: HTTP 503");
         assert!(!is_fatal_error(&err2));
+
+        // Typed std::io::ErrorKind::StorageFull
+        let io_storage_full = std::io::Error::new(std::io::ErrorKind::StorageFull, "disk full");
+        let err_storage = anyhow::Error::new(io_storage_full);
+        assert!(is_fatal_error(&err_storage));
+
+        // POSIX raw OS error 28 (ENOSPC)
+        let io_enospc = std::io::Error::from_raw_os_error(28);
+        let err_enospc = anyhow::Error::new(io_enospc);
+        assert!(is_fatal_error(&err_enospc));
+
+        // Context-wrapped I/O error
+        let chained_err = anyhow::Error::new(std::io::Error::new(
+            std::io::ErrorKind::StorageFull,
+            "insufficient storage capacity",
+        ))
+        .context("failed to write downloaded chunk to .part file");
+        assert!(is_fatal_error(&chained_err));
+
+        // String-formatted message containing 'No space left on device'
+        let string_enospc = anyhow::anyhow!("IO error: No space left on device (os error 28)");
+        assert!(is_fatal_error(&string_enospc));
+
+        // Transient I/O error (e.g. connection reset or timeout) should remain retryable
+        let io_transient =
+            std::io::Error::new(std::io::ErrorKind::ConnectionReset, "connection reset by peer");
+        let err_transient = anyhow::Error::new(io_transient);
+        assert!(!is_fatal_error(&err_transient));
     }
 
     #[test]
```

---

### Diff 4: `src/cli.rs`
Add unit tests for `-q` and `--quiet` parsing:

```diff
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -415,4 +415,19 @@ mod tests {
             _ => panic!("Expected Status command"),
         }
     }
+
+    #[test]
+    fn test_cli_quiet_flag() {
+        let cli_short =
+            Cli::try_parse_from(["rfb-cnpj-etl", "-q", "download"]).expect("parse -q before");
+        assert!(cli_short.quiet);
+
+        let cli_long =
+            Cli::try_parse_from(["rfb-cnpj-etl", "download", "--quiet"]).expect("parse --quiet after");
+        assert!(cli_long.quiet);
+
+        let cli_default =
+            Cli::try_parse_from(["rfb-cnpj-etl", "download"]).expect("parse default quiet");
+        assert!(!cli_default.quiet);
+    }
 }
```

---

## 5. Verification Method

Once the implementer applies the diffs above, verify the fixes using the following procedures:

1. **Verify Quiet Flag Suppresses Output**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   cargo run -- -q download --dry-run
   # Expected output: (silent, zero stdout output; exit code 0)

   cargo run -- download --dry-run -q
   # Expected output: (silent, zero stdout output; exit code 0)
   ```

2. **Verify Normal Output Remains Functional Without `-q`**:
   ```bash
   cargo run -- download --dry-run
   # Expected output:
   # 🔍 Discovery mode (dry-run): 37 files found for snapshot '2026-10' (fallback: false, total: 7447.01 MB)
   #   [ 1/37] Cnaes.zip ...
   ```

3. **Verify Fatal Error & StorageFull Unit Tests**:
   ```bash
   cargo test downloader::range_client::tests::test_fatal_error_detection
   # Expected output: test downloader::range_client::tests::test_fatal_error_detection ... ok
   ```

4. **Verify Quiet Option and CLI Tests**:
   ```bash
   cargo test downloader::tests::test_downloader_options_quiet_propagation
   cargo test cli::tests::test_cli_quiet_flag
   # Expected output: both tests pass ok
   ```

5. **Verify Full Workspace Test Suite**:
   ```bash
   cargo test --all-targets
   # Expected output: all unit and integration tests pass ok
   ```

6. **Invalidation Conditions**:
   - If `cargo run -- -q download --dry-run` produces any stdout text, quiet suppression is broken.
   - If an error with `std::io::ErrorKind::StorageFull` enters exponential backoff retry sleeping instead of returning immediately, `is_fatal_error` detection is invalidated.
