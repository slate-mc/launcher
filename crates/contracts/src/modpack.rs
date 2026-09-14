use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use slate_modpack_api_contracts::{LoaderKind, Provider, ReleaseType};

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
