use crate::LoaderKindDto;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MinecraftReleaseKindDto {
    Release,
    Snapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersionOption {
    pub id: String,
    pub kind: MinecraftReleaseKindDto,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersionCatalog {
    pub latest_release: String,
    pub versions: Vec<MinecraftVersionOption>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersionsRequest {
    pub minecraft_version: String,
    pub loader_kind: LoaderKindDto,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersionCatalog {
    pub loader_kind: LoaderKindDto,
    pub minecraft_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_version: Option<String>,
    pub versions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}
