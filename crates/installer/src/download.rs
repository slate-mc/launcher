use futures_util::{StreamExt, stream};
use reqwest::Client;
use reqwest::redirect::Policy;
use slate_minecraft::{
    ArtifactRequirement, ExpectedHash, HashAlgorithm, VerificationError, verify_artifact,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use url::Url;
use uuid::Uuid;

const MAX_UNDECLARED_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_HASH_SIDECAR_BYTES: usize = 512;
const MAX_DOWNLOAD_ATTEMPTS: u8 = 3;
const MAX_BANDWIDTH_LIMIT_MIB: u32 = 1024;
const ARTIFACT_HOSTS: [&str; 11] = [
    "launcher.mojang.com",
    "launchermeta.mojang.com",
    "libraries.minecraft.net",
    "maven.fabricmc.net",
    "maven.neoforged.net",
    "maven.minecraftforge.net",
    "piston-data.mojang.com",
    "piston-meta.mojang.com",
    "repo.maven.apache.org",
    "repo1.maven.org",
    "resources.download.minecraft.net",
];

#[derive(Clone, Debug)]
pub struct Downloader {
    client: Client,
    concurrency: usize,
    throttle: Option<BandwidthThrottle>,
}

#[derive(Clone, Debug)]
pub(super) struct BandwidthThrottle {
    bytes_per_second: u64,
    next_available: Arc<tokio::sync::Mutex<std::time::Instant>>,
}

impl BandwidthThrottle {
    pub(super) fn from_mebibytes(limit_mib: u32) -> Result<Option<Self>, DownloadError> {
        if limit_mib > MAX_BANDWIDTH_LIMIT_MIB {
            return Err(DownloadError::InvalidBandwidthLimit(limit_mib));
        }
        if limit_mib == 0 {
            return Ok(None);
        }
        Ok(Some(Self {
            bytes_per_second: u64::from(limit_mib) * 1024 * 1024,
            next_available: Arc::new(tokio::sync::Mutex::new(std::time::Instant::now())),
        }))
    }

    pub(super) async fn acquire(&self, bytes: usize) {
        if bytes == 0 {
            return;
        }
        let wait = {
            let mut next_available = self.next_available.lock().await;
            let now = std::time::Instant::now();
            let scheduled = (*next_available).max(now);
            let wait = scheduled.saturating_duration_since(now);
            let allocation =
                std::time::Duration::from_secs_f64(bytes as f64 / self.bytes_per_second as f64);
            *next_available = scheduled + allocation;
            wait
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DownloadSummary {
    pub downloaded: usize,
    pub reused: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DownloadProgress {
    pub completed: usize,
    pub total: usize,
    pub downloaded: usize,
    pub reused: usize,
}

impl Downloader {
    pub fn new(concurrency: u8) -> Result<Self, DownloadError> {
        Self::with_bandwidth_limit(concurrency, 0)
    }

    pub fn with_bandwidth_limit(
        concurrency: u8,
        bandwidth_limit_mib: u32,
    ) -> Result<Self, DownloadError> {
        if !(1..=8).contains(&concurrency) {
            return Err(DownloadError::InvalidConcurrency(concurrency));
        }
        let client = Client::builder()
            .user_agent(format!(
                "slate/{} (artifact installer)",
                env!("CARGO_PKG_VERSION")
            ))
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(300))
            .redirect(Policy::none())
            .build()?;
        Ok(Self {
            client,
            concurrency: usize::from(concurrency),
            throttle: BandwidthThrottle::from_mebibytes(bandwidth_limit_mib)?,
        })
    }

    pub async fn download_all(
        &self,
        requirements: Vec<ArtifactRequirement>,
    ) -> Result<DownloadSummary, DownloadError> {
        self.download_all_with_progress(requirements, |_| {}).await
    }

    pub async fn download_all_with_progress<F>(
        &self,
        requirements: Vec<ArtifactRequirement>,
        on_progress: F,
    ) -> Result<DownloadSummary, DownloadError>
    where
        F: Fn(DownloadProgress) + Send + Sync,
    {
        let total = requirements.len();
        let mut results = stream::iter(requirements.into_iter().map(|requirement| {
            let downloader = self.clone();
            async move { downloader.download_one(requirement).await }
        }))
        .buffer_unordered(self.concurrency);

        let mut summary = DownloadSummary::default();
        let mut completed = 0;
        while let Some(result) = results.next().await {
            match result? {
                DownloadDisposition::Downloaded => summary.downloaded += 1,
                DownloadDisposition::Reused => summary.reused += 1,
            }
            completed += 1;
            if should_report_progress(completed, total) {
                on_progress(DownloadProgress {
                    completed,
                    total,
                    downloaded: summary.downloaded,
                    reused: summary.reused,
                });
            }
        }
        Ok(summary)
    }

    async fn download_one(
        &self,
        mut requirement: ArtifactRequirement,
    ) -> Result<DownloadDisposition, DownloadError> {
        validate_artifact_url(requirement.source_url())?;
        if requirement.expected_hashes().is_empty() {
            let sidecar = requirement
                .hash_sidecar()
                .ok_or(DownloadError::IntegrityMetadataMissing)?;
            let digest = self.fetch_sha1_sidecar(sidecar).await?;
            requirement =
                requirement.with_expected_hash(ExpectedHash::new(HashAlgorithm::Sha1, &digest)?);
        }

        if requirement.target_path().is_file() {
            let verification = {
                let requirement = requirement.clone();
                tokio::task::spawn_blocking(move || verify_artifact(&requirement)).await?
            };
            if verification.is_ok() {
                return Ok(DownloadDisposition::Reused);
            }
            quarantine(requirement.target_path()).await?;
        }

        let parent = requirement.target_path().parent().ok_or_else(|| {
            DownloadError::TargetParentMissing(requirement.target_path().to_path_buf())
        })?;
        tokio::fs::create_dir_all(parent).await?;
        let partial = partial_path(requirement.target_path())?;
        let mut attempt = 1;
        loop {
            match self.download_to_partial(&requirement, &partial).await {
                Ok(()) => break,
                Err(error) => {
                    let _ = tokio::fs::remove_file(&partial).await;
                    if attempt >= MAX_DOWNLOAD_ATTEMPTS || !is_retryable_http_error(&error) {
                        return Err(error);
                    }
                    tokio::time::sleep(retry_delay(attempt)).await;
                    attempt += 1;
                }
            }
        }
        {
            let verification_requirement = requirement_for_path(&requirement, partial.clone());
            tokio::task::spawn_blocking(move || verify_artifact(&verification_requirement))
                .await??
        }
        tokio::fs::rename(&partial, requirement.target_path()).await?;
        Ok(DownloadDisposition::Downloaded)
    }

    async fn download_to_partial(
        &self,
        requirement: &ArtifactRequirement,
        partial: &Path,
    ) -> Result<(), DownloadError> {
        let mut response = self
            .client
            .get(requirement.source_url().clone())
            .send()
            .await?
            .error_for_status()?;
        let maximum = requirement.size().unwrap_or(MAX_UNDECLARED_ARTIFACT_BYTES);
        if response.content_length().is_some_and(|size| size > maximum) {
            return Err(DownloadError::ResponseTooLarge { maximum });
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(partial)
            .await?;
        let mut downloaded = 0_u64;
        while let Some(chunk) = response.chunk().await? {
            downloaded = downloaded
                .checked_add(chunk.len() as u64)
                .ok_or(DownloadError::ResponseTooLarge { maximum })?;
            if downloaded > maximum {
                return Err(DownloadError::ResponseTooLarge { maximum });
            }
            if let Some(throttle) = &self.throttle {
                throttle.acquire(chunk.len()).await;
            }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(())
    }

    async fn fetch_sha1_sidecar(&self, url: &Url) -> Result<String, DownloadError> {
        let mut attempt = 1;
        loop {
            match self.fetch_sha1_sidecar_once(url).await {
                Ok(digest) => return Ok(digest),
                Err(error) => {
                    if attempt >= MAX_DOWNLOAD_ATTEMPTS || !is_retryable_http_error(&error) {
                        return Err(error);
                    }
                    tokio::time::sleep(retry_delay(attempt)).await;
                    attempt += 1;
                }
            }
        }
    }

    async fn fetch_sha1_sidecar_once(&self, url: &Url) -> Result<String, DownloadError> {
        validate_artifact_url(url)?;
        let response = self
            .client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_HASH_SIDECAR_BYTES as u64)
        {
            return Err(DownloadError::InvalidHashSidecar);
        }
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_HASH_SIDECAR_BYTES {
            return Err(DownloadError::InvalidHashSidecar);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| DownloadError::InvalidHashSidecar)?;
        let digest = text
            .split_ascii_whitespace()
            .next()
            .ok_or(DownloadError::InvalidHashSidecar)?;
        ExpectedHash::new(HashAlgorithm::Sha1, digest)?;
        Ok(digest.to_ascii_lowercase())
    }
}

fn should_report_progress(completed: usize, total: usize) -> bool {
    let interval = (total / 100).max(1);
    completed == 1 || completed == total || completed.is_multiple_of(interval)
}

fn is_retryable_http_error(error: &DownloadError) -> bool {
    let DownloadError::Http(error) = error else {
        return false;
    };
    error.is_timeout()
        || error.is_connect()
        || error
            .status()
            .is_some_and(|status| status.as_u16() == 429 || status.is_server_error())
}

fn retry_delay(attempt: u8) -> std::time::Duration {
    let base_millis = 250_u64.saturating_mul(1_u64 << u32::from(attempt.saturating_sub(1)));
    let jitter_millis = u64::from(Uuid::new_v4().as_bytes()[0]);
    std::time::Duration::from_millis(base_millis + jitter_millis)
}

fn requirement_for_path(requirement: &ArtifactRequirement, path: PathBuf) -> ArtifactRequirement {
    requirement.clone().with_target_path(path)
}

fn validate_artifact_url(url: &Url) -> Result<(), DownloadError> {
    let trusted = url
        .host_str()
        .is_some_and(|host| ARTIFACT_HOSTS.contains(&host));
    if url.scheme() != "https" || !trusted || !url.username().is_empty() || url.password().is_some()
    {
        return Err(DownloadError::UntrustedArtifactOrigin(url.clone()));
    }
    Ok(())
}

fn partial_path(target: &Path) -> Result<PathBuf, DownloadError> {
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| DownloadError::TargetNameInvalid(target.to_path_buf()))?;
    Ok(target.with_file_name(format!(".{name}.partial-{}", Uuid::new_v4())))
}

async fn quarantine(target: &Path) -> Result<(), std::io::Error> {
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact");
    let quarantine = target.with_file_name(format!("{name}.corrupt-{}", Uuid::new_v4()));
    tokio::fs::rename(target, quarantine).await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DownloadDisposition {
    Downloaded,
    Reused,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("download concurrency must be between 1 and 8, got {0}")]
    InvalidConcurrency(u8),
    #[error("download bandwidth limit must be between 0 and 1024 MiB/s, got {0}")]
    InvalidBandwidthLimit(u32),
    #[error("artifact URL is outside slate's HTTPS origin allowlist: {0}")]
    UntrustedArtifactOrigin(Url),
    #[error("artifact does not have trusted integrity metadata")]
    IntegrityMetadataMissing,
    #[error("artifact hash sidecar is invalid")]
    InvalidHashSidecar,
    #[error("artifact response exceeds {maximum} bytes")]
    ResponseTooLarge { maximum: u64 },
    #[error("artifact target has no parent: {0}")]
    TargetParentMissing(PathBuf),
    #[error("artifact target filename is invalid: {0}")]
    TargetNameInvalid(PathBuf),
    #[error("artifact HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("artifact filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("artifact verification failed")]
    Verification(#[from] VerificationError),
    #[error("artifact integrity declaration is invalid")]
    Artifact(#[from] slate_minecraft::ArtifactError),
    #[error("artifact verification worker failed")]
    Worker(#[from] tokio::task::JoinError),
}

#[cfg(test)]
mod tests {
    use super::{
        BandwidthThrottle, DownloadError, partial_path, retry_delay, should_report_progress,
        validate_artifact_url,
    };
    use std::path::Path;
    use url::Url;

    #[test]
    fn limits_artifacts_to_known_origins() -> Result<(), url::ParseError> {
        let allowed = Url::parse("https://libraries.minecraft.net/a/b.jar")?;
        let metadata = Url::parse("https://piston-meta.mojang.com/v1/packages/hash/version.json")?;
        let local = Url::parse("https://127.0.0.1/a.jar")?;
        assert!(validate_artifact_url(&allowed).is_ok());
        assert!(validate_artifact_url(&metadata).is_ok());
        assert!(matches!(
            validate_artifact_url(&local),
            Err(DownloadError::UntrustedArtifactOrigin(_))
        ));
        Ok(())
    }

    #[test]
    fn partial_files_stay_next_to_the_target() -> Result<(), Box<dyn std::error::Error>> {
        let target = Path::new("C:/slate/library.jar");
        let partial = partial_path(target)?;
        assert_eq!(partial.parent(), target.parent());
        assert!(
            partial
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains("partial"))
        );
        Ok(())
    }

    #[test]
    fn retries_use_bounded_exponential_backoff() {
        let first = retry_delay(1);
        let second = retry_delay(2);
        assert!(first >= std::time::Duration::from_millis(250));
        assert!(first <= std::time::Duration::from_millis(505));
        assert!(second >= std::time::Duration::from_millis(500));
        assert!(second <= std::time::Duration::from_millis(755));
    }

    #[test]
    fn download_progress_is_bounded_to_about_one_hundred_updates() {
        let updates = (1..=5_000)
            .filter(|completed| should_report_progress(*completed, 5_000))
            .count();
        assert!((100..=102).contains(&updates));
        assert!(should_report_progress(1, 5_000));
        assert!(should_report_progress(5_000, 5_000));
    }

    #[test]
    fn bandwidth_limit_is_optional_and_bounded() -> Result<(), DownloadError> {
        assert!(BandwidthThrottle::from_mebibytes(0)?.is_none());
        let Some(limited) = BandwidthThrottle::from_mebibytes(25)? else {
            return Err(DownloadError::InvalidBandwidthLimit(25));
        };
        assert_eq!(limited.bytes_per_second, 25 * 1024 * 1024);
        assert!(matches!(
            BandwidthThrottle::from_mebibytes(1025),
            Err(DownloadError::InvalidBandwidthLimit(1025))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn bandwidth_limit_is_shared_across_download_tasks() -> Result<(), DownloadError> {
        let Some(throttle) = BandwidthThrottle::from_mebibytes(8)? else {
            return Err(DownloadError::InvalidBandwidthLimit(8));
        };
        throttle.acquire(1024 * 1024).await;
        let started = std::time::Instant::now();
        throttle.clone().acquire(1024 * 1024).await;
        assert!(started.elapsed() >= std::time::Duration::from_millis(100));
        Ok(())
    }
}
