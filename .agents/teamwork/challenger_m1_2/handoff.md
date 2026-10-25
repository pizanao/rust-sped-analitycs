# Handoff Report: Empirical Stress-Test for Milestone 1 Downloader

**Agent**: `challenger_m1_2`  
**Milestone**: Milestone 1 (Scaffold & Resumable Downloader)  
**Role**: Empirical Challenger (critic, specialist)  
**Working Directory**: `/home/pizani/Projects/rust-sped-analitycs/.agents/teamwork/challenger_m1_2`  
**Target Code Directory**: `/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas`  
**Recipient**: `orchestrator_1` (Conversation ID: `0ebc4e6f-ccdc-4481-a769-92fd7f6528cc`)  
**Date**: 2026-10-24T20:20:30Z  
**Verdict**: **APPROVE**  

---

## 1. Observation

### 1.1 Test 1: Live WebDAV Discovery & Dry-Run Formatting
**Command executed**:
```bash
/home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/target/debug/rfb-cnpj-etl download --dry-run
```
**Verbatim Output**:
```
🔍 Discovery mode (dry-run): 37 files found for snapshot '2026-10' (fallback: false, total: 7447.01 MB)
  [ 1/37] Cnaes.zip                 -> "dados-publicos-zip/Cnaes.zip" (22078 bytes (0.02 MB))
  [ 2/37] Empresas0.zip             -> "dados-publicos-zip/Empresas0.zip" (570972127 bytes (544.52 MB))
  [ 3/37] Empresas1.zip             -> "dados-publicos-zip/Empresas1.zip" (77901368 bytes (74.29 MB))
  [ 4/37] Empresas2.zip             -> "dados-publicos-zip/Empresas2.zip" (79125712 bytes (75.46 MB))
  [ 5/37] Empresas3.zip             -> "dados-publicos-zip/Empresas3.zip" (85102880 bytes (81.16 MB))
  [ 6/37] Empresas4.zip             -> "dados-publicos-zip/Empresas4.zip" (90222463 bytes (86.04 MB))
  [ 7/37] Empresas5.zip             -> "dados-publicos-zip/Empresas5.zip" (97310075 bytes (92.80 MB))
  [ 8/37] Empresas6.zip             -> "dados-publicos-zip/Empresas6.zip" (94194819 bytes (89.83 MB))
  [ 9/37] Empresas7.zip             -> "dados-publicos-zip/Empresas7.zip" (98676055 bytes (94.10 MB))
  [10/37] Empresas8.zip             -> "dados-publicos-zip/Empresas8.zip" (98736571 bytes (94.16 MB))
  [11/37] Empresas9.zip             -> "dados-publicos-zip/Empresas9.zip" (94309858 bytes (89.94 MB))
  [12/37] Estabelecimentos0.zip     -> "dados-publicos-zip/Estabelecimentos0.zip" (2277696710 bytes (2172.18 MB))
  [13/37] Estabelecimentos1.zip     -> "dados-publicos-zip/Estabelecimentos1.zip" (341948627 bytes (326.11 MB))
  [14/37] Estabelecimentos2.zip     -> "dados-publicos-zip/Estabelecimentos2.zip" (336288966 bytes (320.71 MB))
  [15/37] Estabelecimentos3.zip     -> "dados-publicos-zip/Estabelecimentos3.zip" (368597760 bytes (351.52 MB))
  [16/37] Estabelecimentos4.zip     -> "dados-publicos-zip/Estabelecimentos4.zip" (340413465 bytes (324.64 MB))
  [17/37] Estabelecimentos5.zip     -> "dados-publicos-zip/Estabelecimentos5.zip" (335452647 bytes (319.91 MB))
  [18/37] Estabelecimentos6.zip     -> "dados-publicos-zip/Estabelecimentos6.zip" (369782592 bytes (352.65 MB))
  [19/37] Estabelecimentos7.zip     -> "dados-publicos-zip/Estabelecimentos7.zip" (338419961 bytes (322.74 MB))
  [20/37] Estabelecimentos8.zip     -> "dados-publicos-zip/Estabelecimentos8.zip" (343568209 bytes (327.65 MB))
  [21/37] Estabelecimentos9.zip     -> "dados-publicos-zip/Estabelecimentos9.zip" (364761032 bytes (347.86 MB))
  [22/37] Motivos.zip               -> "dados-publicos-zip/Motivos.zip" (1180 bytes (0.00 MB))
  [23/37] Municipios.zip            -> "dados-publicos-zip/Municipios.zip" (43443 bytes (0.04 MB))
  [24/37] Naturezas.zip             -> "dados-publicos-zip/Naturezas.zip" (1585 bytes (0.00 MB))
  [25/37] Paises.zip                -> "dados-publicos-zip/Paises.zip" (2745 bytes (0.00 MB))
  [26/37] Qualificacoes.zip         -> "dados-publicos-zip/Qualificacoes.zip" (980 bytes (0.00 MB))
  [27/37] Simples.zip               -> "dados-publicos-zip/Simples.zip" (310608217 bytes (296.22 MB))
  [28/37] Socios0.zip               -> "dados-publicos-zip/Socios0.zip" (251430070 bytes (239.78 MB))
  [29/37] Socios1.zip               -> "dados-publicos-zip/Socios1.zip" (49546399 bytes (47.25 MB))
  [30/37] Socios2.zip               -> "dados-publicos-zip/Socios2.zip" (49056863 bytes (46.78 MB))
  [31/37] Socios3.zip               -> "dados-publicos-zip/Socios3.zip" (49011621 bytes (46.74 MB))
  [32/37] Socios4.zip               -> "dados-publicos-zip/Socios4.zip" (49560198 bytes (47.26 MB))
  [33/37] Socios5.zip               -> "dados-publicos-zip/Socios5.zip" (49063747 bytes (46.79 MB))
  [34/37] Socios6.zip               -> "dados-publicos-zip/Socios6.zip" (49147194 bytes (46.87 MB))
  [35/37] Socios7.zip               -> "dados-publicos-zip/Socios7.zip" (49212792 bytes (46.93 MB))
  [36/37] Socios8.zip               -> "dados-publicos-zip/Socios8.zip" (49447177 bytes (47.16 MB))
  [37/37] Socios9.zip               -> "dados-publicos-zip/Socios9.zip" (49114756 bytes (46.84 MB))
```
- **Byte verification**: Direct summation of all 37 individual file sizes equals `7,808,752,942` bytes, which evaluates to `7808752942 / (1024 * 1024) = 7447.0097... MB`, rounding to exactly `7447.01 MB`.
- **Offline / Failure Fallback**: Tested with an unreachable endpoint (`--base-url https://127.0.0.1:9999/webdav --timeout-secs 2`). It cleanly reported:
  `⚠️ Live WebDAV discovery failed (...). Activating static fallback catalog for '2026-10'.`
  and completed with returncode 0.

### 1.2 Test 2: File Skipping Idempotency
- **Scenario A (All 37 files already complete)**:
  Pre-populated 37 files in `/tmp/rfb_test_idempotency` matching exact byte lengths reported by the RFB server.
  Command: `rfb-cnpj-etl download --output-dir /tmp/rfb_test_idempotency`
  Result:
  ```
  ✅ Finished in 0.04s (0.00 MB/s avg): 0 downloaded, 0 resumed, 37 already complete.
  ```
  Total execution time was 40ms. All files marked as `(cached)`, zero network payload downloaded.

- **Scenario B (Partial directory: 35 cached, 1 deleted, 1 corrupted size)**:
  Deleted `Qualificacoes.zip` (expected 980 bytes) and truncated `Naturezas.zip` from 1585 bytes down to 50 bytes.
  Executed `rfb-cnpj-etl download --output-dir /tmp/rfb_test_idempotency`.
  Result:
  ```
  ✅ Finished in 0.59s (0.00 MB/s avg): 2 downloaded, 0 resumed, 35 already complete.
  ```
  Verified `Qualificacoes.zip` (980 bytes) and `Naturezas.zip` (1585 bytes) were re-downloaded, while the remaining 35 files were skipped. Both downloaded zip archives were verified with `zipfile.ZipFile.testzip() == None`.

- **Scenario C (Subsequent run immediately after)**:
  Result:
  ```
  ✅ Finished in 0.06s (0.00 MB/s avg): 0 downloaded, 0 resumed, 37 already complete.
  ```

### 1.3 Test 3: SIGINT Crash Recovery, Atomicity & HTTP 206 Resume
- **Live Mid-Stream Interruption (SIGINT)**:
  Target: `Empresas1.zip` (74.29 MB). Launched download subprocess. Monitored until `Empresas1.zip.part` was created and actively streaming (observed at 129,989 bytes). Sent `signal.SIGINT` (Ctrl-C).
  Observation:
  - Subprocess terminated immediately with returncode `-2` (SIGINT).
  - Target file `Empresas1.zip` was NOT created (atomicity preserved, zero corrupt .zip files).
  - Staging file `Empresas1.zip.part` was preserved on disk with 129,989 bytes intact.

- **Subsequent Resume from Incomplete `.part`**:
  Relaunched `rfb-cnpj-etl download` on the directory containing `Empresas1.zip.part`.
  Observation:
  - Request sent with `Range: bytes=129989-`.
  - Server returned HTTP 206 Partial Content.
  - Client appended the remaining 77,771,379 bytes to `Empresas1.zip.part`.
  - Atomic rename moved `Empresas1.zip.part` to `Empresas1.zip`.
  - Output:
    ```
    ✅ Finished in 20.27s (3.66 MB/s avg): 0 downloaded, 1 resumed, 36 already complete.
    ```
  - Final file size: `77,901,368` bytes (exact expected size).
  - `zipfile.ZipFile.testzip()` returned `None` (valid archive containing `K3241.K03200Y1.D61010.EMPRECSV`).

- **Bit-for-Bit SHA-256 Resume Parity Test**:
  Pre-created `Qualificacoes.zip.part` containing exactly the first 450 bytes of the official file. Relaunched downloader.
  Observation:
  - Download completed with `0 downloaded, 1 resumed, 36 already complete`.
  - Resulting SHA-256 checksum: `6e8600e3af8543be0fc2281bd24c08499bd81aa17dd76d7b8bffa58c1cbddba3`.
  - Matches the full official file SHA-256 bit-for-bit.

### 1.4 Test 4: Adversarial Edge Cases
- **0-Byte Target File (`touch Qualificacoes.zip`)**:
  Downloader checked `existing_len == expected && expected > 0`. Because `0 != 980`, it did not falsely treat the 0-byte file as cached. Downloaded fresh to `.part` and atomically renamed over the 0-byte file, yielding a valid 980-byte archive.
- **Corrupted Oversized `.part` File (60,000 bytes vs expected 1180 bytes)**:
  Downloader checked `local_len > expected`. Recognized stale/oversized file, removed the corrupt `.part`, reset `local_len = 0`, downloaded cleanly, and yielded a valid 1180-byte `Motivos.zip`.

---

## 2. Logic Chain

1. **Discovery Integrity (Test 1 -> Obs 1.1)**:
   - WebDAV PROPFIND with `Depth: 1` successfully queries `https://arquivos.receitafederal.gov.br/public.php/webdav/`.
   - The regex directory selector picks the latest month (`2026-10`), and the XML parser extracts all 37 canonical files.
   - The computed total (7447.01 MB) matches the exact mathematical byte sum of the 37 files.
   - Fault tolerance works: when WebDAV network errors occur, the static fallback catalog automatically activates without crash.

2. **Idempotency & Caching (Test 2 -> Obs 1.2)**:
   - In `src/downloader/range_client.rs:122-134`, the client inspects `dest.exists()` and verifies `existing_len == expected && expected > 0`.
   - When all files match, all are marked `(cached)` in parallel without initiating HTTP requests, taking 40ms total.
   - If a file is missing or truncated, the check fails, initiating download only for invalidated files.

3. **Crash Recovery & Resume Safety (Test 3 & 4 -> Obs 1.3, 1.4)**:
   - In `src/downloader/range_client.rs:178`, atomic rename `tokio::fs::rename(&part_file, dest)` occurs ONLY after `stream_response_body` successfully terminates.
   - When interrupted by SIGINT, the process terminates without renaming, leaving the partial download as `.part`.
   - Upon restart, the downloader opens `.part` in append mode, sends `Range: bytes={start_byte}-`, handles HTTP 206, and completes the file without data corruption (verified by SHA-256 parity and zip CRC validation).
   - Oversized or 0-byte anomalies are detected and corrected before resuming.

---

## 3. Caveats

- **Network Bandwidth / Throttling**: Live tests downloading full partitions (74 MB to 2.1 GB) depend on RFB WebDAV server bandwidth and F5 WAF rate-limiting. Using concurrency `-c 2` or `-c 3` is optimal to avoid HTTP 429 backoff.
- **Static Fallback Size Reporting**: In static fallback mode (when WebDAV is offline), individual file sizes are marked `unknown size` because the XML was not fetched. This is expected behavior as defined in the fallback spec.

---

## 4. Conclusion

**Verdict: APPROVE**

Milestone 1 satisfies all requirements for WebDAV discovery, idempotency, and crash resilience:
1. Live WebDAV discovery accurately detects all 37 files, resolves the latest snapshot (`2026-10`), calculates total size correctly, and falls back gracefully when offline.
2. Idempotent file skipping operates in constant time (~40ms) when files are already present, and selectively downloads missing or truncated files.
3. Mid-stream SIGINT abortion leaves `.part` files intact without creating corrupted final `.zip` files. Subsequent execution resumes via HTTP 206 and produces bit-for-bit identical archives.

Milestone 1 downloader implementation is rock solid and approved for downstream ingestion pipeline integration (Milestone 2).

---

## 5. Verification Method

To independently reproduce all empirical findings:

1. **Verify WebDAV Live Discovery**:
   ```bash
   cargo run --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml -- download --dry-run
   # Expected: 37 files found for snapshot '2026-10' (fallback: false, total: 7447.01 MB)
   ```

2. **Verify Offline Fallback**:
   ```bash
   cargo run --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml -- download --dry-run --base-url https://127.0.0.1:9999/webdav --timeout-secs 2
   # Expected: "Activating static fallback catalog", 37 files (fallback: true)
   ```

3. **Verify All Crate Unit Tests**:
   ```bash
   cargo test --manifest-path /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas/Cargo.toml
   # Expected: 17 passed; 0 failed
   ```
