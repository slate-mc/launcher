use moka::sync::Cache;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ReleaseCatalog {
    manifest_urls: BTreeMap<ReleaseChannel, Url>,
    rollouts: BTreeMap<ReleaseChannel, u8>,
    client: reqwest::Client,
    cache: Cache<ReleaseChannel, Arc<StaticReleaseManifest>>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReleaseChannel {
    Stable,
    Beta,
}

impl ReleaseChannel {
    pub fn parse(value: &str) -> Result<Self, ReleaseError> {
        match value {
            "stable" => Ok(Self::Stable),
            "beta" => Ok(Self::Beta),
            _ => Err(ReleaseError::InvalidChannel),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct StaticReleaseManifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    pub_date: Option<String>,
    platforms: BTreeMap<String, StaticReleasePlatform>,
}

#[derive(Clone, Debug, Deserialize)]
struct StaticReleasePlatform {
    url: String,
    signature: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LauncherUpdate {
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub_date: Option<String>,
    url: String,
    signature: String,
}

impl ReleaseCatalog {
    pub fn new(
        stable_manifest_url: Url,
        beta_manifest_url: Option<Url>,
        stable_rollout: u8,
        beta_rollout: u8,
        user_agent: &str,
    ) -> Result<Self, ReleaseError> {
        let mut manifest_urls = BTreeMap::from([(ReleaseChannel::Stable, stable_manifest_url)]);
        if let Some(url) = beta_manifest_url {
            manifest_urls.insert(ReleaseChannel::Beta, url);
        }
        let configured_hosts = manifest_urls
            .values()
            .map(|url| {
                url.host_str()
                    .map(str::to_owned)
                    .ok_or(ReleaseError::InvalidManifest)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .user_agent(user_agent)
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 5 {
                    return attempt.error(std::io::Error::other("redirect limit exceeded"));
                }
                let host = attempt.url().host_str();
                if attempt.url().scheme() == "https"
                    && (configured_hosts
                        .iter()
                        .any(|allowed| host == Some(allowed.as_str()))
                        || matches!(
                            host,
                            Some(
                                "github.com"
                                    | "objects.githubusercontent.com"
                                    | "release-assets.githubusercontent.com"
                            )
                        ))
                {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()?;
        Ok(Self {
            manifest_urls,
            rollouts: BTreeMap::from([
                (ReleaseChannel::Stable, stable_rollout.min(100)),
                (ReleaseChannel::Beta, beta_rollout.min(100)),
            ]),
            client,
            cache: Cache::builder()
                .time_to_live(Duration::from_secs(60))
                .max_capacity(2)
                .build(),
        })
    }

    pub async fn update_for(
        &self,
        target: &str,
        architecture: &str,
        current_version: &str,
        channel: ReleaseChannel,
        cohort_id: Option<uuid::Uuid>,
    ) -> Result<Option<LauncherUpdate>, ReleaseError> {
        let Some(manifest) = self.manifest(channel).await? else {
            return Ok(None);
        };
        let update = select_update(&manifest, target, architecture, current_version, channel)?;
        let Some(update) = update else {
            return Ok(None);
        };
        let rollout = self.rollouts.get(&channel).copied().unwrap_or_default();
        if rollout_eligible(cohort_id, channel, &update.version, rollout) {
            Ok(Some(update))
        } else {
            Ok(None)
        }
    }

    async fn manifest(
        &self,
        channel: ReleaseChannel,
    ) -> Result<Option<Arc<StaticReleaseManifest>>, ReleaseError> {
        if let Some(manifest) = self.cache.get(&channel) {
            return Ok(Some(manifest));
        }
        let Some(manifest_url) = self.manifest_urls.get(&channel) else {
            return Ok(None);
        };
        let response = self.client.get(manifest_url.clone()).send().await?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|length| length > MAX_MANIFEST_BYTES as u64)
        {
            return Err(ReleaseError::Unavailable);
        }
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ReleaseError::InvalidManifest);
        }
        let manifest: Arc<StaticReleaseManifest> = Arc::new(serde_json::from_slice(&bytes)?);
        self.cache.insert(channel, manifest.clone());
        Ok(Some(manifest))
    }
}

fn select_update(
    manifest: &StaticReleaseManifest,
    target: &str,
    architecture: &str,
    current_version: &str,
    channel: ReleaseChannel,
) -> Result<Option<LauncherUpdate>, ReleaseError> {
    if !matches!(target, "windows" | "linux" | "darwin")
        || !matches!(architecture, "x86_64" | "aarch64" | "i686" | "armv7")
    {
        return Err(ReleaseError::UnsupportedTarget);
    }
    let current = parse_version(current_version)?;
    let latest = parse_version(&manifest.version)?;
    if channel == ReleaseChannel::Stable && !latest.pre.is_empty() {
        return Err(ReleaseError::InvalidManifest);
    }
    if latest <= current {
        return Ok(None);
    }
    let key = format!("{target}-{architecture}");
    let platform = manifest
        .platforms
        .get(&key)
        .ok_or(ReleaseError::UnsupportedTarget)?;
    let download_url = Url::parse(&platform.url).map_err(|_| ReleaseError::InvalidManifest)?;
    if download_url.scheme() != "https"
        || download_url.host_str().is_none()
        || !download_url.username().is_empty()
        || download_url.password().is_some()
        || platform.signature.trim().is_empty()
        || platform.signature.len() > 16 * 1024
    {
        return Err(ReleaseError::InvalidManifest);
    }
    Ok(Some(LauncherUpdate {
        version: manifest.version.clone(),
        notes: manifest.notes.clone(),
        pub_date: manifest.pub_date.clone(),
        url: download_url.to_string(),
        signature: platform.signature.clone(),
    }))
}

fn rollout_eligible(
    cohort_id: Option<uuid::Uuid>,
    channel: ReleaseChannel,
    version: &str,
    percentage: u8,
) -> bool {
    if percentage >= 100 {
        return true;
    }
    if percentage == 0 {
        return false;
    }
    let Some(cohort_id) = cohort_id else {
        return false;
    };
    let digest = Sha256::digest(format!("{}:{version}:{cohort_id}", channel.as_str()).as_bytes());
    let bucket = u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]) % 100;
    bucket < u64::from(percentage)
}

fn parse_version(value: &str) -> Result<Version, ReleaseError> {
    Version::parse(value.trim().trim_start_matches('v')).map_err(|_| ReleaseError::InvalidVersion)
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    #[error("the release channel is invalid")]
    InvalidChannel,
    #[error("the release target is unsupported")]
    UnsupportedTarget,
    #[error("the release version is invalid")]
    InvalidVersion,
    #[error("the release manifest is invalid")]
    InvalidManifest,
    #[error("the release catalog is unavailable")]
    Unavailable,
    #[error("the release request failed")]
    Request(#[from] reqwest::Error),
    #[error("the release manifest could not be decoded")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::{
        ReleaseChannel, ReleaseError, StaticReleaseManifest, parse_version, rollout_eligible,
        select_update,
    };

    fn release_manifest() -> Result<StaticReleaseManifest, serde_json::Error> {
        serde_json::from_str(
            r#"{
                "version":"0.2.0",
                "notes":"A safer update.",
                "pub_date":"2026-09-20T12:00:00Z",
                "platforms":{
                    "windows-x86_64":{
                        "url":"https://github.com/slate-mc/launcher/releases/download/slate-v0.2.0/slate.exe",
                        "signature":"signed"
                    }
                }
            }"#,
        )
    }

    #[test]
    fn release_versions_accept_a_v_prefix() {
        assert_eq!(parse_version("v1.2.3").ok(), parse_version("1.2.3").ok());
        assert!(matches!(
            parse_version("latest"),
            Err(ReleaseError::InvalidVersion)
        ));
    }

    #[test]
    fn static_manifest_requires_platform_downloads() -> Result<(), serde_json::Error> {
        let manifest = release_manifest()?;
        assert!(manifest.platforms.contains_key("windows-x86_64"));
        Ok(())
    }

    #[test]
    fn selects_only_newer_updates_for_the_requested_platform()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest = release_manifest()?;
        let update = select_update(
            &manifest,
            "windows",
            "x86_64",
            "0.1.0",
            ReleaseChannel::Stable,
        )?
        .ok_or("expected an update")?;
        assert_eq!(update.version, "0.2.0");
        assert!(update.url.starts_with("https://github.com/"));
        assert!(
            select_update(
                &manifest,
                "windows",
                "x86_64",
                "0.2.0",
                ReleaseChannel::Stable,
            )?
            .is_none()
        );
        assert!(matches!(
            select_update(
                &manifest,
                "darwin",
                "aarch64",
                "0.1.0",
                ReleaseChannel::Stable,
            ),
            Err(ReleaseError::UnsupportedTarget)
        ));
        Ok(())
    }

    #[test]
    fn staged_rollouts_are_stable_and_require_a_cohort() {
        let cohort = uuid::Uuid::parse_str("041328e2-92e4-413b-8813-64b229dadd49").ok();
        assert!(rollout_eligible(None, ReleaseChannel::Stable, "1.0.0", 100));
        assert!(!rollout_eligible(None, ReleaseChannel::Stable, "1.0.0", 20));
        assert_eq!(
            rollout_eligible(cohort, ReleaseChannel::Stable, "1.0.0", 20),
            rollout_eligible(cohort, ReleaseChannel::Stable, "1.0.0", 20)
        );
    }

    #[test]
    fn stable_channel_rejects_prerelease_manifests() -> Result<(), Box<dyn std::error::Error>> {
        let mut manifest = release_manifest()?;
        manifest.version = "0.2.0-beta.1".to_owned();
        assert!(matches!(
            select_update(
                &manifest,
                "windows",
                "x86_64",
                "0.1.0",
                ReleaseChannel::Stable,
            ),
            Err(ReleaseError::InvalidManifest)
        ));
        assert!(
            select_update(
                &manifest,
                "windows",
                "x86_64",
                "0.1.0",
                ReleaseChannel::Beta,
            )?
            .is_some()
        );
        Ok(())
    }
}
