use crate::{ModpackFile, Provider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Vanilla,
    Forge,
    NeoForge,
    Fabric,
    Quilt,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Loader {
    #[serde(rename = "type")]
    pub kind: LoaderKind,
    pub version: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseType {
    Release,
    Beta,
    Alpha,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct MemoryRecommendation {
    pub minimum_mb: u32,
    pub recommended_mb: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct MinecraftTarget {
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ModpackVersionSummary {
    pub id: String,
    pub name: String,
    pub release_type: ReleaseType,
    pub minecraft_version: String,
    pub loader: Loader,
    pub published_at: String,
    pub changelog: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct VersionPage {
    pub items: Vec<ModpackVersionSummary>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ModpackVersion {
    pub provider: Provider,
    pub project_id: String,
    pub id: String,
    pub name: String,
    pub release_type: ReleaseType,
    pub minecraft: MinecraftTarget,
    pub loader: Loader,
    pub memory: MemoryRecommendation,
    pub files: Vec<ModpackFile>,
    pub total_download_size: u64,
    pub changelog: Option<String>,
    pub published_at: String,
}
