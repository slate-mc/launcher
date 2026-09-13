use crate::metadata::{
    MAX_VERSION_MANIFEST_BYTES, MAX_VERSION_METADATA_BYTES, MetadataError, VersionManifest,
    VersionMetadata,
};
use reqwest::Client;
use reqwest::redirect::Policy;
use sha1::{Digest, Sha1};
use std::time::Duration;
use url::Url;

pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const ALLOWED_METADATA_HOSTS: [&str; 2] = ["piston-meta.mojang.com", "launchermeta.mojang.com"];

#[derive(Clone, Debug)]
pub struct MojangMetadataClient {
    client: Client,
}

impl MojangMetadataClient {
    pub fn new() -> Result<Self, MetadataFetchError> {
        let client = Client::builder()
            .user_agent(format!("slate/{}", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .redirect(Policy::none())
            .build()?;
        Ok(Self { client })
    }

    pub async fn fetch_manifest(&self) -> Result<VersionManifest, MetadataFetchError> {
        let url = Url::parse(VERSION_MANIFEST_URL)?;
        let bytes = self.fetch_limited(url, MAX_VERSION_MANIFEST_BYTES).await?;
        Ok(VersionManifest::from_json_slice(&bytes)?)
    }

    pub async fn fetch_version(
        &self,
        manifest: &VersionManifest,
        version_id: &str,
    ) -> Result<VersionMetadata, MetadataFetchError> {
        let entry = manifest
            .find(version_id)
            .ok_or_else(|| MetadataFetchError::VersionNotFound(version_id.to_owned()))?;
        let url = Url::parse(&entry.url)?;
        validate_metadata_url(&url)?;
        let bytes = self.fetch_limited(url, MAX_VERSION_METADATA_BYTES).await?;
        verify_sha1(&bytes, &entry.sha1)?;
        let metadata = VersionMetadata::from_json_slice(&bytes)?;
        if metadata.id != entry.id {
            return Err(MetadataFetchError::VersionIdentityMismatch {
                expected: entry.id.clone(),
                actual: metadata.id,
            });
        }
        Ok(metadata)
    }

    async fn fetch_limited(&self, url: Url, maximum: usize) -> Result<Vec<u8>, MetadataFetchError> {
        validate_metadata_url(&url)?;
        let response = self.client.get(url).send().await?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > maximum as u64)
        {
            return Err(MetadataFetchError::ResponseTooLarge { maximum });
        }
        let bytes = response.bytes().await?;
        if bytes.len() > maximum {
            return Err(MetadataFetchError::ResponseTooLarge { maximum });
        }
        Ok(bytes.to_vec())
    }
}

fn validate_metadata_url(url: &Url) -> Result<(), MetadataFetchError> {
    let allowed_host = url
        .host_str()
        .is_some_and(|host| ALLOWED_METADATA_HOSTS.contains(&host));
    if url.scheme() != "https"
        || !allowed_host
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(MetadataFetchError::UntrustedMetadataUrl(url.clone()));
    }
    Ok(())
}

fn verify_sha1(bytes: &[u8], expected: &str) -> Result<(), MetadataFetchError> {
    if expected.len() != 40 || !expected.bytes().all(|value| value.is_ascii_hexdigit()) {
        return Err(MetadataFetchError::InvalidManifestHash);
    }
    let actual = crate::artifact::digest_hex(&Sha1::digest(bytes));
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(MetadataFetchError::ManifestHashMismatch)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataFetchError {
    #[error("failed to build or execute the Mojang metadata request")]
    Http(#[from] reqwest::Error),
    #[error("Mojang metadata URL is invalid")]
    Url(#[from] url::ParseError),
    #[error("metadata URL is outside the Mojang HTTPS allowlist: {0}")]
    UntrustedMetadataUrl(Url),
    #[error("metadata response exceeds {maximum} bytes")]
    ResponseTooLarge { maximum: usize },
    #[error("Minecraft version was not present in the manifest: {0}")]
    VersionNotFound(String),
    #[error("version manifest contains an invalid SHA-1 digest")]
    InvalidManifestHash,
    #[error("version metadata does not match the manifest SHA-1")]
    ManifestHashMismatch,
    #[error("version metadata identity mismatch: expected {expected}, got {actual}")]
    VersionIdentityMismatch { expected: String, actual: String },
    #[error(transparent)]
    Metadata(#[from] MetadataError),
}

#[cfg(test)]
mod tests {
    use super::{MetadataFetchError, validate_metadata_url, verify_sha1};
    use url::Url;

    #[test]
    fn only_allows_known_mojang_metadata_origins() -> Result<(), url::ParseError> {
        let allowed = Url::parse("https://piston-meta.mojang.com/v1/packages/hash/1.21.1.json")?;
        let private = Url::parse("https://127.0.0.1/version.json")?;
        let lookalike = Url::parse("https://piston-meta.mojang.com.example.test/version.json")?;

        assert!(validate_metadata_url(&allowed).is_ok());
        assert!(matches!(
            validate_metadata_url(&private),
            Err(MetadataFetchError::UntrustedMetadataUrl(_))
        ));
        assert!(validate_metadata_url(&lookalike).is_err());
        Ok(())
    }

    #[test]
    fn validates_manifest_sha1() {
        assert!(verify_sha1(b"slate", "4ad75af70d6bfc638e293e0072e5ffd3577dc91a").is_ok());
        assert!(matches!(
            verify_sha1(b"slate", "0000000000000000000000000000000000000000"),
            Err(MetadataFetchError::ManifestHashMismatch)
        ));
    }
}
