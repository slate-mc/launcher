use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use slate_modpack_api_contracts::{LoaderKind, Provider, ReleaseType};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModpackSortDto {
    #[default]
    Relevance,
    Downloads,
    Updated,
    Newest,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackSearchRequest {
    pub query: Option<String>,
    pub provider: Option<Provider>,
    pub minecraft_version: Option<String>,
    pub loader: Option<LoaderKind>,
    pub category: Option<String>,
    pub sort: ModpackSortDto,
    pub cursor: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackProjectRequest {
    pub provider: Provider,
    pub project_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackVersionsRequest {
    pub provider: Provider,
    pub project_id: String,
    pub minecraft_version: Option<String>,
    pub loader: Option<LoaderKind>,
    pub release_type: Option<ReleaseType>,
    pub cursor: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackVersionRequest {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallModpackRequest {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub instance_name: String,
    #[serde(default)]
    pub include_optional: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckModpackUpdateRequest {
    pub instance_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyModpackUpdateRequest {
    pub instance_id: Uuid,
    pub expected_revision: u64,
    pub target_version_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackUpdateSummary {
    pub update_available: bool,
    pub current_version_id: String,
    pub current_version_name: Option<String>,
    pub latest_version_id: Option<String>,
    pub latest_version_name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSearchRequest {
    pub instance_id: uuid::Uuid,
    pub query: Option<String>,
    pub provider: Option<Provider>,
    pub sort: ModpackSortDto,
    pub cursor: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallModSelection {
    pub provider: Provider,
    pub project_id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallModRequest {
    pub instance_id: uuid::Uuid,
    pub expected_revision: u64,
    pub mods: Vec<InstallModSelection>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceModsRequest {
    pub instance_id: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLocalModRequest {
    pub instance_id: uuid::Uuid,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetInstanceModEnabledRequest {
    pub instance_id: uuid::Uuid,
    pub expected_revision: u64,
    pub file_path: String,
    pub provider: Option<Provider>,
    pub project_id: Option<String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetInstanceModPinnedRequest {
    pub instance_id: uuid::Uuid,
    pub expected_revision: u64,
    pub file_path: String,
    pub provider: Option<Provider>,
    pub project_id: Option<String>,
    pub pinned: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveInstanceModRequest {
    pub instance_id: uuid::Uuid,
    pub expected_revision: u64,
    pub file_path: String,
    pub provider: Option<Provider>,
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceModOriginDto {
    Added,
    Modpack,
    Local,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceModSummary {
    pub provider: Option<Provider>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub display_name: String,
    pub file_path: String,
    pub enabled: bool,
    pub pinned: bool,
    pub installed_at: Option<String>,
    pub origin: InstanceModOriginDto,
    pub file_size: u64,
    pub icon_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceModResolution {
    pub file_path: String,
    pub provider: Provider,
    pub project_id: String,
    pub version_id: Option<String>,
    pub display_name: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceContentKindDto {
    ResourcePack,
    ShaderPack,
    DataPack,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceContentFilesRequest {
    pub instance_id: Uuid,
    pub kind: InstanceContentKindDto,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceWorldsRequest {
    pub instance_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLocalContentFileRequest {
    pub instance_id: Uuid,
    pub kind: InstanceContentKindDto,
    #[serde(default)]
    pub world_name: Option<String>,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceContentFileSummary {
    pub display_name: String,
    pub file_path: String,
    pub kind: InstanceContentKindDto,
    pub enabled: bool,
    pub can_toggle: bool,
    pub origin: InstanceModOriginDto,
    pub file_size: u64,
    pub modified_at: Option<String>,
    pub world_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetInstanceContentFileEnabledRequest {
    pub instance_id: Uuid,
    pub kind: InstanceContentKindDto,
    pub file_path: String,
    pub enabled: bool,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveInstanceContentFileRequest {
    pub instance_id: Uuid,
    pub kind: InstanceContentKindDto,
    pub file_path: String,
    pub expected_revision: u64,
}
