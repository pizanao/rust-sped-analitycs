//! WebDAV Discovery & URL Catalog Module (`downloader/webdav.rs`)
//!
//! Provides automated discovery and fallback catalog generation for Receita Federal do Brasil (RFB)
//! Open Data (Dados Abertos do CNPJ) hosted on Nextcloud WebDAV.
//!
//! # Responsibilities
//! 1. Query WebDAV endpoint `https://arquivos.receitafederal.gov.br/public.php/webdav/` using HTTP `PROPFIND`
//!    with share token `YggdBLfdninEJX9` (Basic Auth: `{token}:""`, header `Depth: 1`).
//! 2. Parse XML Multi-Status response (HTTP 207) to detect available `YYYY-MM` directories and select the latest.
//! 3. Query the chosen `YYYY-MM` directory to extract metadata (filename, byte size, last modified, ETag)
//!    for all 37 official `.zip` files.
//! 4. Provide a 100% deterministic, offline-capable static fallback catalogue of the canonical 37 files
//!    if WebDAV directory listing is unreachable, rate-limited, blocked, or altered.
//! 5. Convert discovered files into `DownloadTarget` structs for consumption by `RangeClient`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, Method, StatusCode};
use thiserror::Error;

/// Default public Nextcloud WebDAV base endpoint for RFB Dados Abertos do CNPJ.
pub const DEFAULT_WEBDAV_BASE_URL: &str = "https://arquivos.receitafederal.gov.br/public.php/webdav";

/// Canonical active public share token for RFB CNPJ open data repository.
pub const DEFAULT_SHARE_TOKEN: &str = "YggdBLfdninEJX9";

/// Direct file download URL template using Nextcloud dav/files public endpoint.
pub const DEFAULT_DAV_FILES_URL_TEMPLATE: &str =
    "https://arquivos.receitafederal.gov.br/public.php/dav/files/{share_token}/{year_month}/{filename}";

/// Standard browser User-Agent to bypass F5 BIG-IP WAF challenge drops and cookies.
pub const DEFAULT_BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Fallback snapshot year-month used when network is offline and no explicit year-month is supplied.
pub const DEFAULT_FALLBACK_YEAR_MONTH: &str = "2026-10";

/// Canonical list of exactly 37 zip files published monthly by Receita Federal.
///
/// Organized into 5 distinct groups:
/// - 6 Domain lookup tables (cnae, motivo, municipio, natureza_juridica, pais, qualificacao_socio)
/// - 1 Simples / MEI registration table
/// - 10 Core Empresas partition files (0 to 9)
/// - 10 Core Estabelecimentos partition files (0 to 9)
/// - 10 Core Socios partition files (0 to 9)
pub const CANONICAL_37_FILES: [&str; 37] = [
    // 6 Domain lookup tables
    "Cnaes.zip",
    "Motivos.zip",
    "Municipios.zip",
    "Naturezas.zip",
    "Paises.zip",
    "Qualificacoes.zip",
    // 1 Simples / MEI table
    "Simples.zip",
    // 10 Empresas partitions
    "Empresas0.zip",
    "Empresas1.zip",
    "Empresas2.zip",
    "Empresas3.zip",
    "Empresas4.zip",
    "Empresas5.zip",
    "Empresas6.zip",
    "Empresas7.zip",
    "Empresas8.zip",
    "Empresas9.zip",
    // 10 Estabelecimentos partitions
    "Estabelecimentos0.zip",
    "Estabelecimentos1.zip",
    "Estabelecimentos2.zip",
    "Estabelecimentos3.zip",
    "Estabelecimentos4.zip",
    "Estabelecimentos5.zip",
    "Estabelecimentos6.zip",
    "Estabelecimentos7.zip",
    "Estabelecimentos8.zip",
    "Estabelecimentos9.zip",
    // 10 Socios partitions
    "Socios0.zip",
    "Socios1.zip",
    "Socios2.zip",
    "Socios3.zip",
    "Socios4.zip",
    "Socios5.zip",
    "Socios6.zip",
    "Socios7.zip",
    "Socios8.zip",
    "Socios9.zip",
];

/// Errors occurring during WebDAV discovery or XML parsing.
#[derive(Debug, Error)]
pub enum WebdavError {
    #[error("HTTP network error during WebDAV operation: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Unexpected HTTP status code {status} from WebDAV endpoint: {url}")]
    UnexpectedStatus { status: StatusCode, url: String },

    #[error("Failed to parse WebDAV XML response: {0}")]
    XmlParse(String),

    #[error("No monthly directory matching YYYY-MM found in WebDAV root {0}")]
    NoDirectoriesFound(String),

    #[error("Invalid WebDAV URL: {0}")]
    InvalidUrl(String),

    #[error("Discovery returned incomplete file catalog: missing {missing:?} from canonical 37 files")]
    IncompleteCatalog { missing: Vec<String> },
}

/// Metadata for a single remote file discovered via WebDAV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    /// Canonical file name (e.g. `Empresas0.zip`).
    pub name: String,
    /// Raw WebDAV href attribute (e.g. `/public.php/webdav/2026-10/Empresas0.zip`).
    pub href: String,
    /// Absolute URL for downloading the file.
    pub download_url: String,
    /// Expected file size in bytes (from `<d:getcontentlength>`), if reported.
    pub size_bytes: Option<u64>,
    /// HTTP last-modified timestamp string (from `<d:getlastmodified>`), if reported.
    pub last_modified: Option<String>,
    /// HTTP entity tag (from `<d:getetag>`), if reported.
    pub etag: Option<String>,
}

/// Target file information compatible with `RangeClient` and `download_all`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadTarget {
    pub url: String,
    pub filename: String,
    pub destination_path: PathBuf,
    pub expected_size: Option<u64>,
}

/// Catalog of files discovered for a specific monthly snapshot.
#[derive(Debug, Clone)]
pub struct DiscoveryCatalog {
    /// The reference year and month (e.g. `2026-10`).
    pub year_month: String,
    /// Discovered or synthesized file metadata entries.
    pub files: Vec<RemoteFile>,
    /// True if this catalog was generated from static fallback rather than live WebDAV.
    pub is_fallback: bool,
    /// Base URL or source used for discovery.
    pub source_endpoint: String,
}

impl DiscoveryCatalog {
    /// Total expected download size across all files with known sizes.
    pub fn total_expected_bytes(&self) -> u64 {
        self.files.iter().filter_map(|f| f.size_bytes).sum()
    }

    /// Check if all 37 canonical files are present in this catalog.
    pub fn is_complete(&self) -> bool {
        self.missing_canonical_files().is_empty()
    }

    /// Return list of canonical filenames missing from this catalog.
    pub fn missing_canonical_files(&self) -> Vec<&'static str> {
        let present: HashSet<&str> = self.files.iter().map(|f| f.name.as_str()).collect();
        CANONICAL_37_FILES
            .iter()
            .copied()
            .filter(|&c| !present.contains(c))
            .collect()
    }

    /// Convert catalog entries into concrete `DownloadTarget` structs for the download manager.
    pub fn to_download_targets(&self, destination_dir: &Path) -> Vec<DownloadTarget> {
        self.files
            .iter()
            .map(|f| DownloadTarget {
                url: f.download_url.clone(),
                filename: f.name.clone(),
                destination_path: destination_dir.join(&f.name),
                expected_size: f.size_bytes,
            })
            .collect()
    }

    /// Generate human-readable summary of the catalog.
    pub fn summary(&self) -> String {
        let total_mb = self.total_expected_bytes() as f64 / (1024.0 * 1024.0);
        format!(
            "Catalog {} ({} files, {:.2} MB total, fallback: {})",
            self.year_month,
            self.files.len(),
            total_mb,
            self.is_fallback
        )
    }
}

/// Configuration for the WebDAV discovery client.
#[derive(Debug, Clone)]
pub struct WebdavConfig {
    pub base_url: String,
    pub share_token: String,
    pub user_agent: String,
    pub timeout: Duration,
}

impl Default for WebdavConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_WEBDAV_BASE_URL.to_string(),
            share_token: DEFAULT_SHARE_TOKEN.to_string(),
            user_agent: DEFAULT_BROWSER_USER_AGENT.to_string(),
            timeout: Duration::from_secs(30),
        }
    }
}

/// Client for discovering RFB open data archives via WebDAV.
pub struct WebdavDiscovery {
    client: Client,
    config: WebdavConfig,
}

impl WebdavDiscovery {
    /// Create a new WebDAV discovery client with the specified configuration.
    pub fn new(config: WebdavConfig) -> Result<Self, WebdavError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&config.user_agent)
                .map_err(|e| WebdavError::InvalidUrl(format!("Invalid User-Agent header: {e}")))?,
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/xml, text/xml, */*"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(config.timeout)
            .danger_accept_invalid_certs(false)
            .build()?;

        Ok(Self { client, config })
    }

    /// Create a discovery client using default parameters.
    pub fn with_defaults() -> Result<Self, WebdavError> {
        Self::new(WebdavConfig::default())
    }

    /// Discover available archives for either an explicitly requested year-month or the latest available.
    ///
    /// If live WebDAV discovery encounters network or parsing failures, automatically falls back
    /// to the deterministic static catalog.
    pub async fn discover_catalog(
        &self,
        year_month_override: Option<&str>,
    ) -> Result<DiscoveryCatalog, WebdavError> {
        match self.try_discover_live(year_month_override).await {
            Ok(catalog) => {
                let missing = catalog.missing_canonical_files();
                if !missing.is_empty() {
                    eprintln!(
                        "⚠️ Live WebDAV returned incomplete catalog ({}/37 files). Missing: {:?}",
                        catalog.files.len(),
                        missing
                    );
                    // Supplement missing files from static catalog
                    let supplemented = self.supplement_catalog(catalog);
                    Ok(supplemented)
                } else {
                    Ok(catalog)
                }
            }
            Err(err) => {
                let target_ym = year_month_override.unwrap_or(DEFAULT_FALLBACK_YEAR_MONTH);
                eprintln!(
                    "⚠️ Live WebDAV discovery failed ({err}). Activating static fallback catalog for '{target_ym}'."
                );
                Ok(self.build_static_fallback_catalog(target_ym))
            }
        }
    }

    /// Perform live discovery against WebDAV without fallback.
    async fn try_discover_live(
        &self,
        year_month_override: Option<&str>,
    ) -> Result<DiscoveryCatalog, WebdavError> {
        let year_month = match year_month_override {
            Some(ym) => ym.trim().to_string(),
            None => self.query_latest_year_month().await?,
        };

        let files = self.query_monthly_files(&year_month).await?;

        Ok(DiscoveryCatalog {
            year_month,
            files,
            is_fallback: false,
            source_endpoint: self.config.base_url.clone(),
        })
    }

    /// Query the WebDAV root endpoint (`/public.php/webdav/`) with `Depth: 1`
    /// to find all available monthly directories (`YYYY-MM`) and return the latest.
    pub async fn query_latest_year_month(&self) -> Result<String, WebdavError> {
        let root_url = format!("{}/", self.config.base_url.trim_end_matches('/'));
        let propfind_method = Method::from_bytes(b"PROPFIND")
            .map_err(|e| WebdavError::InvalidUrl(format!("Invalid HTTP method: {e}")))?;

        let response = self
            .client
            .request(propfind_method, &root_url)
            .header("Depth", "1")
            .basic_auth(&self.config.share_token, Some(""))
            .send()
            .await?;

        let status = response.status();
        if status != StatusCode::MULTI_STATUS && status != StatusCode::OK {
            return Err(WebdavError::UnexpectedStatus {
                status,
                url: root_url,
            });
        }

        let body = response.text().await?;
        let mut months = parse_year_months_from_multistatus(&body);

        if months.is_empty() {
            return Err(WebdavError::NoDirectoriesFound(root_url));
        }

        months.sort();
        let latest = months.pop().unwrap();
        Ok(latest)
    }

    /// Query the specific monthly folder (`/public.php/webdav/{YYYY-MM}/`) with `Depth: 1`
    /// to extract metadata for all `.zip` files.
    pub async fn query_monthly_files(&self, year_month: &str) -> Result<Vec<RemoteFile>, WebdavError> {
        let month_url = format!(
            "{}/{}/",
            self.config.base_url.trim_end_matches('/'),
            year_month.trim_matches('/')
        );

        let propfind_method = Method::from_bytes(b"PROPFIND")
            .map_err(|e| WebdavError::InvalidUrl(format!("Invalid HTTP method: {e}")))?;

        let response = self
            .client
            .request(propfind_method, &month_url)
            .header("Depth", "1")
            .basic_auth(&self.config.share_token, Some(""))
            .send()
            .await?;

        let status = response.status();
        if status != StatusCode::MULTI_STATUS && status != StatusCode::OK {
            return Err(WebdavError::UnexpectedStatus {
                status,
                url: month_url,
            });
        }

        let body = response.text().await?;
        let base_host = extract_base_host(&self.config.base_url);
        let files = parse_files_from_multistatus(&body, &base_host, &self.config.share_token, year_month);

        Ok(files)
    }

    /// Build deterministic static fallback catalog containing all 37 canonical files.
    pub fn build_static_fallback_catalog(&self, year_month: &str) -> DiscoveryCatalog {
        let clean_ym = year_month.trim().trim_matches('/');
        let files = CANONICAL_37_FILES
            .iter()
            .map(|&filename| {
                let download_url = format!(
                    "https://arquivos.receitafederal.gov.br/public.php/dav/files/{}/{}/{}",
                    self.config.share_token, clean_ym, filename
                );
                let href = format!("/public.php/webdav/{clean_ym}/{filename}");
                RemoteFile {
                    name: filename.to_string(),
                    href,
                    download_url,
                    size_bytes: None,
                    last_modified: None,
                    etag: None,
                }
            })
            .collect();

        DiscoveryCatalog {
            year_month: clean_ym.to_string(),
            files,
            is_fallback: true,
            source_endpoint: "static_canonical_fallback".to_string(),
        }
    }

    /// Supplement any missing canonical files into a partial live catalog.
    fn supplement_catalog(&self, mut catalog: DiscoveryCatalog) -> DiscoveryCatalog {
        let present: HashSet<String> = catalog.files.iter().map(|f| f.name.clone()).collect();
        for &canon in &CANONICAL_37_FILES {
            if !present.contains(canon) {
                let download_url = format!(
                    "https://arquivos.receitafederal.gov.br/public.php/dav/files/{}/{}/{}",
                    self.config.share_token, catalog.year_month, canon
                );
                let href = format!("/public.php/webdav/{}/{}", catalog.year_month, canon);
                catalog.files.push(RemoteFile {
                    name: canon.to_string(),
                    href,
                    download_url,
                    size_bytes: None,
                    last_modified: None,
                    etag: None,
                });
            }
        }
        catalog
    }
}

/// Extract base scheme and authority (e.g. `https://arquivos.receitafederal.gov.br`) from base URL.
fn extract_base_host(url: &str) -> String {
    if let Some(idx) = url.find("://") {
        let rest = &url[idx + 3..];
        if let Some(slash_idx) = rest.find('/') {
            return url[..idx + 3 + slash_idx].to_string();
        }
    }
    url.trim_end_matches('/').to_string()
}

/// Parse all `YYYY-MM` directory names from WebDAV root Multi-Status XML.
///
/// Looks for `<d:href>` elements containing paths like `/public.php/webdav/2026-10/`.
pub fn parse_year_months_from_multistatus(xml: &str) -> Vec<String> {
    use regex::Regex;

    // Pattern matches /YYYY-MM/ or /YYYY-MM inside <href>
    let re = Regex::new(r"(?i)<(?:[a-zA-Z0-9]+:)?href>[^<]*/(20\d{2}-(?:0[1-9]|1[0-2]))/?</(?:[a-zA-Z0-9]+:)?href>")
        .expect("Failed to compile year-month regex");

    let mut set = HashSet::new();
    for cap in re.captures_iter(xml) {
        if let Some(m) = cap.get(1) {
            set.insert(m.as_str().to_string());
        }
    }

    let mut list: Vec<String> = set.into_iter().collect();
    list.sort();
    list
}

/// Parse individual file metadata entries from a monthly folder's Multi-Status XML.
pub fn parse_files_from_multistatus(
    xml: &str,
    base_host: &str,
    share_token: &str,
    year_month: &str,
) -> Vec<RemoteFile> {
    use regex::Regex;

    let re_response = Regex::new(r"(?s)<(?:[a-zA-Z0-9]+:)?response>(.*?)</(?:[a-zA-Z0-9]+:)?response>")
        .expect("Failed to compile response regex");
    let re_href = Regex::new(r"<(?:[a-zA-Z0-9]+:)?href>([^<]+)</(?:[a-zA-Z0-9]+:)?href>")
        .expect("Failed to compile href regex");
    let re_len = Regex::new(r"<(?:[a-zA-Z0-9]+:)?getcontentlength>(\d+)</(?:[a-zA-Z0-9]+:)?getcontentlength>")
        .expect("Failed to compile content-length regex");
    let re_modified = Regex::new(r"<(?:[a-zA-Z0-9]+:)?getlastmodified>([^<]+)</(?:[a-zA-Z0-9]+:)?getlastmodified>")
        .expect("Failed to compile last-modified regex");
    let re_etag = Regex::new(r"<(?:[a-zA-Z0-9]+:)?getetag>([^<]+)</(?:[a-zA-Z0-9]+:)?getetag>")
        .expect("Failed to compile etag regex");
    let re_zip = Regex::new(r"(?i)/?([^/]+\.zip)$").expect("Failed to compile zip filename regex");

    let clean_host = base_host.trim_end_matches('/');
    let mut files = Vec::new();

    for cap in re_response.captures_iter(xml) {
        let block = &cap[1];

        // Extract href
        let href = match re_href.captures(block).and_then(|c| c.get(1)) {
            Some(h) => h.as_str().trim(),
            None => continue,
        };

        // Extract zip filename
        let filename = match re_zip.captures(href).and_then(|c| c.get(1)) {
            Some(f) => f.as_str().trim(),
            None => continue,
        };

        // Parse content length
        let size_bytes = re_len
            .captures(block)
            .and_then(|c| c.get(1))
            .and_then(|s| s.as_str().parse::<u64>().ok());

        // Parse last modified
        let last_modified = re_modified
            .captures(block)
            .and_then(|c| c.get(1))
            .map(|s| s.as_str().trim().to_string());

        // Parse etag (stripping quotes if present)
        let etag = re_etag
            .captures(block)
            .and_then(|c| c.get(1))
            .map(|s| s.as_str().trim().trim_matches('"').to_string());

        // Standardize download URL:
        // Use direct Nextcloud public dav/files URL template for reliable Range resume
        let download_url = format!(
            "{clean_host}/public.php/dav/files/{share_token}/{year_month}/{filename}"
        );

        files.push(RemoteFile {
            name: filename.to_string(),
            href: href.to_string(),
            download_url,
            size_bytes,
            last_modified,
            etag,
        });
    }

    files
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ROOT_XML: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns">
  <d:response><d:href>/public.php/webdav/</d:href></d:response>
  <d:response><d:href>/public.php/webdav/2023-05/</d:href></d:response>
  <d:response><d:href>/public.php/webdav/2026-09/</d:href></d:response>
  <d:response><d:href>/public.php/webdav/2026-10/</d:href></d:response>
  <d:response><d:href>/public.php/webdav/cnpj.tar.gz</d:href></d:response>
</d:multistatus>"#;

    const SAMPLE_MONTH_XML: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/public.php/webdav/2026-10/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/public.php/webdav/2026-10/Cnaes.zip</d:href>
    <d:propstat>
      <d:prop>
        <d:getlastmodified>Tue, 13 Oct 2026 22:00:16 GMT</d:getlastmodified>
        <d:getcontentlength>22078</d:getcontentlength>
        <d:getetag>&quot;17e2f9b2f51164f0e35a0e6011551c63&quot;</d:getetag>
      </d:prop>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/public.php/webdav/2026-10/Empresas0.zip</d:href>
    <d:propstat>
      <d:prop>
        <d:getlastmodified>Tue, 13 Oct 2026 22:00:54 GMT</d:getlastmodified>
        <d:getcontentlength>570972127</d:getcontentlength>
        <d:getetag>&quot;51ffd6930af710720929322b11eb1027&quot;</d:getetag>
      </d:prop>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

    #[test]
    fn test_parse_year_months() {
        let months = parse_year_months_from_multistatus(SAMPLE_ROOT_XML);
        assert_eq!(months, vec!["2023-05", "2026-09", "2026-10"]);
        assert_eq!(months.last().unwrap(), "2026-10");
    }

    #[test]
    fn test_parse_monthly_files() {
        let files = parse_files_from_multistatus(
            SAMPLE_MONTH_XML,
            "https://arquivos.receitafederal.gov.br",
            DEFAULT_SHARE_TOKEN,
            "2026-10",
        );
        assert_eq!(files.len(), 2);

        let cnaes = &files[0];
        assert_eq!(cnaes.name, "Cnaes.zip");
        assert_eq!(cnaes.size_bytes, Some(22078));
        assert_eq!(cnaes.etag.as_deref(), Some("17e2f9b2f51164f0e35a0e6011551c63"));
        assert_eq!(
            cnaes.download_url,
            format!(
                "https://arquivos.receitafederal.gov.br/public.php/dav/files/{}/2026-10/Cnaes.zip",
                DEFAULT_SHARE_TOKEN
            )
        );

        let empresas0 = &files[1];
        assert_eq!(empresas0.name, "Empresas0.zip");
        assert_eq!(empresas0.size_bytes, Some(570972127));
    }

    #[test]
    fn test_canonical_37_files_count_and_completeness() {
        assert_eq!(CANONICAL_37_FILES.len(), 37);

        // Check uniqueness
        let unique: HashSet<&str> = CANONICAL_37_FILES.iter().copied().collect();
        assert_eq!(unique.len(), 37);

        // Check prefixes
        assert_eq!(CANONICAL_37_FILES.iter().filter(|f| f.starts_with("Empresas")).count(), 10);
        assert_eq!(CANONICAL_37_FILES.iter().filter(|f| f.starts_with("Estabelecimentos")).count(), 10);
        assert_eq!(CANONICAL_37_FILES.iter().filter(|f| f.starts_with("Socios")).count(), 10);
    }

    #[test]
    fn test_static_fallback_catalog() {
        let discovery = WebdavDiscovery::with_defaults().unwrap();
        let catalog = discovery.build_static_fallback_catalog("2026-10");

        assert!(catalog.is_fallback);
        assert_eq!(catalog.year_month, "2026-10");
        assert_eq!(catalog.files.len(), 37);
        assert!(catalog.is_complete());
        assert!(catalog.missing_canonical_files().is_empty());

        let targets = catalog.to_download_targets(Path::new("/tmp/test_download"));
        assert_eq!(targets.len(), 37);
        assert_eq!(targets[0].destination_path, PathBuf::from("/tmp/test_download/Cnaes.zip"));
    }
}
