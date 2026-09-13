use crate::http::{BoundedHttpClient, HttpError};
use serde::Deserialize;
use slate_minecraft::{MetadataError, VersionMetadata};
use url::Url;

pub const FABRIC_META_ORIGIN: &str = "https://meta.fabricmc.net/";
const FABRIC_META_HOST: &str = "meta.fabricmc.net";

#[derive(Clone, Debug)]
pub struct FabricAdapter {
    http: BoundedHttpClient,
}

impl FabricAdapter {
    pub fn new() -> Result<Self, FabricError> {
        Ok(Self {
            http: BoundedHttpClient::new("slate/0.1 (launcher metadata)")?,
        })
    }

    pub async fn fetch_profile(
        &self,
        minecraft_version: &str,
        loader_version: &str,
    ) -> Result<VersionMetadata, FabricError> {
        let url = profile_url(minecraft_version, loader_version)?;
        let bytes = self
            .http
            .get(
                url,
                FABRIC_META_HOST,
                slate_minecraft::MAX_VERSION_METADATA_BYTES,
            )
            .await?;
        Self::parse_profile(&bytes, minecraft_version, loader_version)
    }

    pub async fn fetch_loader_versions(
        &self,
        minecraft_version: &str,
    ) -> Result<Vec<String>, FabricError> {
        validate_version_segment(minecraft_version)?;
        let mut url = Url::parse(FABRIC_META_ORIGIN)?;
        url.path_segments_mut()
            .map_err(|()| FabricError::InvalidOrigin)?
            .extend(["v2", "versions", "loader", minecraft_version]);
        let bytes = self
            .http
            .get(url, FABRIC_META_HOST, 4 * 1024 * 1024)
            .await?;
        Self::parse_loader_versions(&bytes)
    }

    pub fn parse_loader_versions(bytes: &[u8]) -> Result<Vec<String>, FabricError> {
        let entries: Vec<FabricLoaderEntry> = serde_json::from_slice(bytes)?;
        if entries.len() > 1_000 {
            return Err(FabricError::TooManyLoaderVersions(entries.len()));
        }
        for entry in &entries {
            validate_version_segment(&entry.loader.version)?;
        }
        let recommended = entries
            .iter()
            .find(|entry| entry.loader.stable)
            .map(|entry| entry.loader.version.clone());
        let mut versions: Vec<String> = entries
            .into_iter()
            .map(|entry| entry.loader.version)
            .collect();
        versions.sort_by_key(|version| std::cmp::Reverse(version_numbers(version)));
        versions.dedup();
        if let Some(recommended) = recommended
            && let Some(index) = versions.iter().position(|version| version == &recommended)
        {
            versions.remove(index);
            versions.insert(0, recommended);
        }
        Ok(versions)
    }

    pub fn parse_profile(
        bytes: &[u8],
        minecraft_version: &str,
        loader_version: &str,
    ) -> Result<VersionMetadata, FabricError> {
        validate_version_segment(minecraft_version)?;
        validate_version_segment(loader_version)?;
        let profile = VersionMetadata::from_json_slice(bytes)?;
        let expected_id = format!("fabric-loader-{loader_version}-{minecraft_version}");
        if profile.id != expected_id {
            return Err(FabricError::UnexpectedProfileId {
                expected: expected_id,
                actual: profile.id,
            });
        }
        if profile.inherits_from.as_deref() != Some(minecraft_version) {
            return Err(FabricError::UnexpectedParent {
                expected: minecraft_version.to_owned(),
                actual: profile.inherits_from,
            });
        }
        if profile.main_class.as_deref() != Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
        {
            return Err(FabricError::UnexpectedMainClass(profile.main_class));
        }
        Ok(profile)
    }
}

fn version_numbers(version: &str) -> Vec<u32> {
    version
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}

#[derive(Debug, Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoaderVersion,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderVersion {
    version: String,
    #[serde(default)]
    stable: bool,
}

fn profile_url(minecraft_version: &str, loader_version: &str) -> Result<Url, FabricError> {
    validate_version_segment(minecraft_version)?;
    validate_version_segment(loader_version)?;
    let mut url = Url::parse(FABRIC_META_ORIGIN)?;
    url.path_segments_mut()
        .map_err(|()| FabricError::InvalidOrigin)?
        .extend([
            "v2",
            "versions",
            "loader",
            minecraft_version,
            loader_version,
            "profile",
            "json",
        ]);
    Ok(url)
}

fn validate_version_segment(value: &str) -> Result<(), FabricError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
    {
        return Err(FabricError::InvalidVersion(value.to_owned()));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum FabricError {
    #[error("Fabric version is not a safe metadata path segment: {0}")]
    InvalidVersion(String),
    #[error("the configured Fabric metadata origin cannot accept path segments")]
    InvalidOrigin,
    #[error("Fabric returned profile id {actual}; expected {expected}")]
    UnexpectedProfileId { expected: String, actual: String },
    #[error("Fabric profile inherits from {actual:?}; expected {expected}")]
    UnexpectedParent {
        expected: String,
        actual: Option<String>,
    },
    #[error("Fabric profile uses an unexpected client main class: {0:?}")]
    UnexpectedMainClass(Option<String>),
    #[error("Fabric profile metadata is invalid")]
    Metadata(#[from] MetadataError),
    #[error("Fabric metadata URL is invalid")]
    Url(#[from] url::ParseError),
    #[error("Fabric metadata request failed")]
    Http(#[from] HttpError),
    #[error("Fabric loader version response is invalid")]
    Json(#[from] serde_json::Error),
    #[error("Fabric returned too many loader versions: {0}")]
    TooManyLoaderVersions(usize),
}

#[cfg(test)]
mod tests {
    use super::{FabricAdapter, FabricError, profile_url};

    const PROFILE: &[u8] = include_bytes!("../tests/fixtures/fabric-1.21.1.json");

    #[test]
    fn parses_current_profile_shape() {
        let profile = FabricAdapter::parse_profile(PROFILE, "1.21.1", "0.19.5");

        assert!(profile.is_ok());
        let profile = match profile {
            Ok(profile) => profile,
            Err(error) => panic!("profile should parse: {error}"),
        };
        assert_eq!(
            profile.main_class.as_deref(),
            Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
        );
    }

    #[test]
    fn encodes_only_valid_version_segments() {
        let url = profile_url("1.21.1", "0.19.5");
        let url = match url {
            Ok(url) => url,
            Err(error) => panic!("profile URL should be valid: {error}"),
        };
        assert_eq!(
            url.as_str(),
            "https://meta.fabricmc.net/v2/versions/loader/1.21.1/0.19.5/profile/json"
        );
        assert!(matches!(
            profile_url("../latest", "0.19.5"),
            Err(FabricError::InvalidVersion(_))
        ));
    }

    #[test]
    fn lists_stable_compatible_loaders_first() {
        let document = br#"[
            {"loader":{"version":"0.20.0-beta.1","stable":false}},
            {"loader":{"version":"0.19.5","stable":true}}
        ]"#;

        let versions = FabricAdapter::parse_loader_versions(document);
        assert!(versions.is_ok());
        assert_eq!(
            versions.ok(),
            Some(vec!["0.19.5".to_owned(), "0.20.0-beta.1".to_owned()])
        );
    }

    #[test]
    fn oldest_loader_never_becomes_the_default() {
        let document = br#"[
            {"loader":{"version":"0.1.0.48","stable":false}},
            {"loader":{"version":"0.19.4","stable":false}},
            {"loader":{"version":"0.19.5","stable":true}}
        ]"#;

        let versions = FabricAdapter::parse_loader_versions(document);
        assert_eq!(
            versions.ok().and_then(|values| values.first().cloned()),
            Some("0.19.5".to_owned())
        );
    }
}
