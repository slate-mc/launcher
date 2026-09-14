use crate::{Hashes, Loader, MemoryRecommendation, Provider};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    Linux,
    Macos,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    X86_64,
    Aarch64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct InstallPlanRequest {
    pub platform: Platform,
    pub arch: Architecture,
    #[serde(default)]
    pub include_optional: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct InstallPlanInstance {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct JavaPlan {
    pub major: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct RuntimePlan {
    pub minecraft: String,
    pub loader: Loader,
    pub java: JavaPlan,
    pub memory: MemoryRecommendation,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct InstallPlanDownload {
    pub id: String,
    pub destination: String,
    pub size: u64,
    pub hashes: Hashes,
    pub sources: Vec<crate::DownloadSource>,
    pub required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ExtractAction {
    pub download_id: String,
    pub destination: String,
    pub source_prefix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct InstallPlan {
    pub schema: u32,
    pub instance: InstallPlanInstance,
    pub runtime: RuntimePlan,
    pub downloads: Vec<InstallPlanDownload>,
    pub extract: Vec<ExtractAction>,
    pub delete: Vec<String>,
    pub total_download_size: u64,
}
