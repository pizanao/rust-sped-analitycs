# Empirical Challenge Report: Milestone 1 (Scaffold & Resumable Downloader)

**Agent**: `challenger_m1_1`  
**Milestone**: Milestone 1 (Scaffold & Resumable Downloader)  
**Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m1_1`  
**Target Project Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`  
**Recipient**: `orchestrator_1` (Conversation ID: `0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)  
**Date**: 2026-10-24T20:23:00Z  
**Verdict**: **REJECT** (Blocking on VULN-01; full remediation instructions provided)

---

## 1. Observation

### 1.1 CLI Boundary Testing
Empirical execution of CLI boundary conditions yielded the following verbatim results:
- **Zero & Negative Concurrency**:
  ```bash
  $ ./target/debug/rfb-cnpj-etl download --dry-run --concurrency 0
  error: invalid value '0' for '--concurrency <NUM>': 0 is not in 1..=10
  Exit code: 2
  ```
  ```bash
  $ ./target/debug/rfb-cnpj-etl download --dry-run --concurrency -1
  error: unexpected argument '-1' found
  Exit code: 2
  ```
- **Over-range Concurrency**:
  ```bash
  $ ./target/debug/rfb-cnpj-etl download --dry-run --concurrency 11
  error: invalid value '11' for '--concurrency <NUM>': 11 is not in 1..=10
  Exit code: 2
  ```
- **Valid Boundary Concurrency (`--concurrency 1` and `--concurrency 10`)**:
  Accepted and executed successfully with exit code 0.
- **Unrecognized Commands & Flags**:
  `--nonexistent-flag` and invalid subcommands correctly rejected with exit code 2.
- **Directory Permissions**:
  When targeting a non-writable directory (`chmod 555 /tmp/test_readonly_rfb`), the process returned `Error: Permission denied (os error 13)` and exited with code 1.
- **Invalid Snapshot Year-Month (`-y 2026-99`, `-y invalid-format`)**:
  Live WebDAV returned 404 Not Found, automatically activating the deterministic static fallback catalog without crashing.

### 1.2 Resumable Downloading (HTTP 206) & File Parity
In a live test against the official RFB endpoint (`Qualificacoes.zip`, 980 bytes):
1. **Full Ground Truth**: Fresh download fetched exactly 980 bytes (`Qualificacoes.zip`).
2. **Partial Resume**: Pre-populated `.part` file with the first 500 bytes of ground truth. Execution with `RangeClient` issued `Range: bytes=500-`, received HTTP 206, appended exactly 480 bytes, and cleanly promoted `.part` to `.zip`.
3. **Byte-for-Byte Exact Parity**: Comparing `ground_truth` and `resumed_bytes` showed identical length (980 bytes) and 100% byte equality (`assert_eq!(ground_truth, resumed_bytes)` passed). `zip::ZipArchive` validated the archive structure without warnings.
4. **Oversized Part Recovery**: Pre-creating `.part` with 2,000 bytes (> 980 bytes) caused `RangeClient` to truncate to 0 and re-download cleanly from byte 0.
5. **Cached Bypass**: Completed `.zip` on disk correctly bypassed network requests (`DownloadResult::AlreadyComplete`).

### 1.3 Bad Share Token & Unreachable Host
- **Bad Share Token (`--share-token INVALID_SHARE_TOKEN_12345`)**:
  Live WebDAV returned 401 Unauthorized; fallback catalog activated. `RangeClient` immediately caught `Fatal HTTP error 401 Unauthorized` in `is_fatal_error`, aborted without retry loops, and exited cleanly with code 1 in 891 ms.
- **Unreachable Host (`--base-url http://127.0.0.1:54321`)**:
  Failed WebDAV discovery with connection refused, activated static fallback, attempted retries with exponential backoff and jitter, and exited with code 1 in 3,003 ms with `operation timed out`.

---

## 2. Logic Chain & Vulnerabilities Discovered

### Finding 1 [CRITICAL/HIGH] — VULN-01: Premature Promotion of Truncated Files without Expected Size Verification
- **Observation**: In `src/downloader/range_client.rs` lines 176–181:
  ```rust
  match self.execute_download_attempt(target, &part_file, local_len, pb).await {
      Ok(result) => {
          // Atomic rename from .part to final destination
          tokio::fs::rename(&part_file, dest).await?;
          pb.finish_with_message(format!("✔ {}", target.filename));
          return Ok(result);
      }
  ```
  Neither `execute_download_attempt` nor `download_file` verifies that the size on disk matches `target.expected_size` before renaming `.part` to `.zip`.
- **Empirical Proof**:
  We ran a mock HTTP server test where `target.expected_size = Some(1000)`.
  1. *Scenario A (Premature EOF on HTTP 200)*: Server sends `Content-Length: 500` and closes socket cleanly. `RangeClient` returns `Ok(FreshDownload { bytes_transferred: 500 })` and executes `rename(&part_file, dest)`. The corrupted 500-byte file was promoted to `incomplete.zip`!
  2. *Scenario B (Truncated HTTP 206 range)*: Server returns `Content-Range: bytes 200-599/1000` (400 bytes). `RangeClient` appends 400 bytes to 200 existing bytes and returns `Ok(Resumed { bytes_appended: 400, total_bytes: 600 })`. The corrupted 600-byte file was promoted to `partial_trunc.zip`!
- **Blast Radius**:
  If a network connection drops or a CDN/proxy terminates early without a TCP error, or returns an HTTP truncation, the downloader marks the file complete with exit code 0. Downstream Milestone 2 (ingestion) will fail with `ZipError::InvalidArchive` or `UnexpectedEof`, or silently miss rows. Furthermore, lines 122–134 skip checking expected size if `dest.exists()` unless `existing_len == expected`, causing corrupted files to trigger re-download only on subsequent runs, while reporting false success on the initial run.
- **Remediation**:
  Before calling `tokio::fs::rename(&part_file, dest).await?`:
  ```rust
  let final_size = tokio::fs::metadata(&part_file).await?.len();
  if let Some(expected) = target.expected_size {
      if final_size != expected {
          anyhow::bail!(
              "Size mismatch for {}: got {} bytes, expected {} bytes",
              target.filename,
              final_size,
              expected
          );
      }
  }
  ```

### Finding 2 [MEDIUM] — VULN-02: `BufWriter` Drop on Connection Drop Discards In-Transit Buffered Bytes
- **Observation**: In `src/downloader/range_client.rs`:
  ```rust
  let mut writer = BufWriter::with_capacity(self.config.buffer_size_kb * 1024, file);
  let transferred = stream_response_body(resp, &mut writer, pb).await?;
  writer.flush().await?;
  ```
  `BufWriter` capacity is 128 KB. When a network connection drops mid-stream, `stream_response_body` returns `Err` via `?`, bypassing `writer.flush().await?`. In Tokio, `AsyncWriteExt` / `BufWriter` does not flush synchronously in `drop()`.
- **Empirical Proof**:
  In a mid-stream disconnect test (sending 300 bytes before abrupt TCP close), the 300 bytes remained in memory and were never flushed to disk. On the subsequent retry, the file on disk had 0 bytes instead of 300 bytes, forcing byte 0 to be re-downloaded.
- **Blast Radius**:
  While data is not corrupted (the retry re-downloads from the disk offset), up to 128 KB of data per disconnect is discarded rather than preserved.
- **Remediation**:
  In `execute_download_attempt`, use a pattern that flushes even on error:
  ```rust
  let stream_res = stream_response_body(resp, &mut writer, pb).await;
  let _ = writer.flush().await; // Flush whatever arrived before propagating error
  let transferred = stream_res?;
  ```

### Finding 3 [LOW] — VULN-03: Static Fallback Catalog Ignores Custom `--base-url`
- **Observation**: In `src/downloader/webdav.rs` lines 393–396 and 423–426:
  `download_url` hardcodes `"https://arquivos.receitafederal.gov.br/public.php/dav/files/{}/{}/{}"`.
- **Blast Radius**:
  If `--base-url` points to an internal mirror (e.g. `http://mirror.corp/public.php/webdav`) and WebDAV PROPFIND fails or is offline, the fallback download attempts to contact the public internet RFB server rather than the configured mirror.
- **Remediation**:
  Use `format!("{}/public.php/dav/files/{}/{}/{}", extract_base_host(&self.config.base_url), ...)` in `build_static_fallback_catalog` and `supplement_catalog`.

---

## 3. Caveats

1. **Full 7.45 GB Download Test**:
   The full download of all 37 files was not executed across all 7.45 GB to preserve bandwidth and test run time; testing focused empirically on individual live files (`Qualificacoes.zip`), mock servers, boundary conditions, and resume mechanics.
2. **RFB Nextcloud Dynamic ETag Rotation**:
   Nextcloud ETags may vary across monthly snapshots; RangeClient relies on byte offsets and content lengths rather than `If-Match: ETag`, which was verified to be the correct design for RFB public shares.

---

## 4. Conclusion & Verdict

**VERDICT**: **REJECT**

Milestone 1 is exceptionally well-engineered: CLI argument parsing, clap boundary validations, WebDAV PROPFIND XML extraction, exponential backoff with jitter, F5 WAF header bypass, and live HTTP 206 partial resume all work cleanly.

However, **VULN-01 represents a critical data integrity risk**: premature promotion of truncated `.part` files to final `.zip` files without verifying `actual_size == expected_size` creates corrupted archives that will break Milestone 2. 

**Conditions to unblock and approve**:
1. Add expected size validation in `range_client.rs` before atomic rename.
2. Add `writer.flush().await` on error in `stream_response_body` (VULN-02).
3. Update `build_static_fallback_catalog` to respect `--base-url` host (VULN-03).
4. Re-run `cargo test` and submit for challenger re-verification.

---

## 5. Verification Method

To independently verify the empirical vulnerabilities identified in this report:

1. **Verify VULN-01 (Premature Promotion of Truncated File)**:
   ```bash
   cd /tmp/stress_harness && cargo run
   ```
   Observe console confirmation:
   `TRUNCATED FILE PROMOTED TO DESTINATION! Actual size: 500, Expected: 1000`
2. **Verify CLI Boundary Rejection**:
   ```bash
   cd /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
   ./target/debug/rfb-cnpj-etl download --concurrency 0
   # Returns exit code 2: '0 is not in 1..=10'
   ```
3. **Verify Live Resume Byte-for-Byte Exact Parity**:
   Run the test harness in `/tmp/stress_harness` which downloads `Qualificacoes.zip`, saves 500 bytes to `.part`, resumes, and executes `assert_eq!(ground_truth, resumed_bytes)`.
