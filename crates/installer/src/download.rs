use futures_util::{StreamExt, stream};
use reqwest::Client;
use reqwest::redirect::Policy;
use slate_minecraft::{
    ArtifactRequirement, ExpectedHash, HashAlgorithm, VerificationError, verify_artifact,
};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use url::Url;
use uuid::Uuid;

const MAX_UNDECLARED_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_HASH_SIDECAR_BYTES: usize = 512;
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DownloadSummary {
    pub downloaded: usize,
    pub reused: usize,
}

impl Downloader {
    pub fn new(concurrency: u8) -> Result<Self, DownloadError> {
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
        })
    }

    pub async fn download_all(
        &self,
        requirements: Vec<ArtifactRequirement>,
    ) -> Result<DownloadSummary, DownloadError> {
        let results = stream::iter(requirements.into_iter().map(|requirement| {
            let downloader = self.clone();
            async move { downloader.download_one(requirement).await }
        }))
        .buffer_unordered(self.concurrency)
        .collect::<Vec<_>>()
        .await;

        let mut summary = DownloadSummary::default();
        for result in results {
            match result? {
                DownloadDisposition::Downloaded => summary.downloaded += 1,
                DownloadDisposition::Reused => summary.reused += 1,
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
        let result = self.download_to_partial(&requirement, &partial).await;
        if let Err(error) = result {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(error);
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
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(())
    }

    async fn fetch_sha1_sidecar(&self, url: &Url) -> Result<String, DownloadError> {
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
    use super::{DownloadError, partial_path, validate_artifact_url};
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
}
