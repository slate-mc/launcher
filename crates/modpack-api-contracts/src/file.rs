use crate::Provider;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackFileType {
    Mod,
    Config,
    ResourcePack,
    ShaderPack,
    DataPack,
    Library,
    Override,
    Archive,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Both,
    Client,
    Server,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Hashes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
}

impl Hashes {
    #[must_use]
    pub const fn has_cryptographic_hash(&self) -> bool {
        self.sha512.is_some() || self.sha256.is_some() || self.sha1.is_some()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DownloadSource {
    Direct { url: String },
    Proxy { url: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ProviderReference {
    pub provider: Provider,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct FileOption {
    pub id: String,
    pub name: String,
    pub default: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ModpackFile {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: PackFileType,
    pub path: String,
    pub size: u64,
    pub hashes: Hashes,
    pub side: Side,
    pub optional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option: Option<FileOption>,
    pub download: DownloadSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ProviderReference>,
}
