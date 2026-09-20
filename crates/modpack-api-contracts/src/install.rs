use crate::{Hashes, Loader, LoaderKind, MemoryRecommendation, Provider};
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
pub struct ModInstallPlanRequest {
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::ModInstallPlanRequest;

    #[test]
    fn mod_install_requests_accept_latest_or_an_exact_version()
    -> Result<(), Box<dyn std::error::Error>> {
        let latest: ModInstallPlanRequest = serde_json::from_value(serde_json::json!({
            "minecraft_version": "1.21.1",
            "loader": "fabric",
            "loader_version": "0.16.10"
        }))?;
        assert_eq!(latest.version_id, None);

        let exact: ModInstallPlanRequest = serde_json::from_value(serde_json::json!({
            "minecraft_version": "1.21.1",
            "loader": "fabric",
            "loader_version": "0.16.10",
            "version_id": "abc123"
        }))?;
        assert_eq!(exact.version_id.as_deref(), Some("abc123"));
        Ok(())
    }
}
