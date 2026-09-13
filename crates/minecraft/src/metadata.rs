use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_VERSION_METADATA_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_VERSION_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionManifestEntry>,
}

impl VersionManifest {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, MetadataError> {
        if bytes.len() > MAX_VERSION_MANIFEST_BYTES {
            return Err(MetadataError::DocumentTooLarge {
                actual: bytes.len(),
                maximum: MAX_VERSION_MANIFEST_BYTES,
            });
        }
        let manifest: Self = serde_json::from_slice(bytes)?;
        if manifest.versions.len() > 10_000 {
            return Err(MetadataError::TooManyVersions(manifest.versions.len()));
        }
        Ok(manifest)
    }

    #[must_use]
    pub fn find(&self, version_id: &str) -> Option<&VersionManifestEntry> {
        self.versions.iter().find(|entry| entry.id == version_id)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    pub sha1: String,
    #[serde(default)]
    pub compliance_level: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
    pub id: String,
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub main_class: Option<String>,
    #[serde(default, rename = "type")]
    pub version_type: Option<String>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub asset_index: Option<AssetIndex>,
    #[serde(default)]
    pub downloads: VersionDownloads,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub arguments: Option<VersionArguments>,
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub logging: Option<Logging>,
}

impl VersionMetadata {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, MetadataError> {
        if bytes.len() > MAX_VERSION_METADATA_BYTES {
            return Err(MetadataError::DocumentTooLarge {
                actual: bytes.len(),
                maximum: MAX_VERSION_METADATA_BYTES,
            });
        }
        let metadata: Self = serde_json::from_slice(bytes)?;
        metadata.validate()?;
        Ok(metadata)
    }

    fn validate(&self) -> Result<(), MetadataError> {
        validate_identifier("version id", &self.id)?;
        if let Some(parent) = &self.inherits_from {
            validate_identifier("inherited version id", parent)?;
        }
        if self.libraries.len() > 4_096 {
            return Err(MetadataError::TooManyLibraries(self.libraries.len()));
        }
        for library in &self.libraries {
            validate_identifier("library coordinate", &library.name)?;
        }
        if let Some(arguments) = &self.arguments
            && (arguments.game.len() > 4_096 || arguments.jvm.len() > 4_096)
        {
            return Err(MetadataError::TooManyArguments);
        }
        Ok(())
    }
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), MetadataError> {
    if value.is_empty() || value.len() > 1_024 || value.chars().any(char::is_control) {
        return Err(MetadataError::InvalidField(field));
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<DownloadInfo>,
    #[serde(default)]
    pub client_mappings: Option<DownloadInfo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(default)]
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInfo {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: BTreeMap<String, String>,
    #[serde(default)]
    pub extract: Option<ExtractRules>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<DownloadInfo>,
    #[serde(default)]
    pub classifiers: BTreeMap<String, DownloadInfo>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct VersionArguments {
    #[serde(default)]
    pub game: Vec<ArgumentEntry>,
    #[serde(default)]
    pub jvm: Vec<ArgumentEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ArgumentEntry {
    Literal(String),
    Conditional {
        rules: Vec<Rule>,
        value: ArgumentValue,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    Single(String),
    Multiple(Vec<String>),
}

impl ArgumentValue {
    pub(crate) fn values(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self {
            Self::Single(value) => Box::new(std::iter::once(value.as_str())),
            Self::Multiple(values) => Box::new(values.iter().map(String::as_str)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Logging {
    #[serde(default)]
    pub client: Option<ClientLogging>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientLogging {
    pub argument: String,
    pub file: DownloadInfo,
    #[serde(rename = "type")]
    pub logging_type: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("metadata document is too large: {actual} bytes exceeds {maximum}")]
    DocumentTooLarge { actual: usize, maximum: usize },
    #[error("metadata JSON is invalid")]
    Json(#[from] serde_json::Error),
    #[error("metadata contains too many version records: {0}")]
    TooManyVersions(usize),
    #[error("metadata contains too many libraries: {0}")]
    TooManyLibraries(usize),
    #[error("metadata contains too many launch arguments")]
    TooManyArguments,
    #[error("metadata field is missing, empty, oversized, or contains control characters: {0}")]
    InvalidField(&'static str),
}

#[cfg(test)]
mod tests {
    use super::{MAX_VERSION_METADATA_BYTES, MetadataError, VersionMetadata};

    #[test]
    fn rejects_oversized_metadata_before_parsing() {
        let oversized = vec![b' '; MAX_VERSION_METADATA_BYTES + 1];
        let result = VersionMetadata::from_json_slice(&oversized);

        assert!(matches!(
            result,
            Err(MetadataError::DocumentTooLarge { .. })
        ));
    }
}
