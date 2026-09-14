use crate::{LoaderKind, Provider, ProviderStatus, VersionReference};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadinessStatus {
    Ok,
    Degraded,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ReadinessResponse {
    pub status: ReadinessStatus,
    pub providers: BTreeMap<Provider, ProviderStatus>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct CategorySummary {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct CategoriesResponse {
    pub items: Vec<CategorySummary>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MinecraftReleaseKind {
    Release,
    Snapshot,
    Old,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct MinecraftVersionSummary {
    pub version: String,
    #[serde(rename = "type")]
    pub kind: MinecraftReleaseKind,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct MinecraftVersionsResponse {
    pub items: Vec<MinecraftVersionSummary>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct LoaderTypesResponse {
    pub items: Vec<LoaderKind>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct UpdateResponse {
    pub update_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<VersionReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<VersionReference>,
}
