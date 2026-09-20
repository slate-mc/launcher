use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntimeSummary {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightSummary {
    pub database_ready: bool,
    pub storage_ready: bool,
    pub account_configured: bool,
    pub java: JavaRuntimeSummary,
    pub launch_implemented: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageCategoryDto {
    Instances,
    TemporaryFiles,
    RemovedContent,
    Logs,
    SharedGameFiles,
    ManagedJava,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCategorySummary {
    pub category: StorageCategoryDto,
    pub size_bytes: u64,
    pub file_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashedInstanceSummary {
    pub id: Uuid,
    pub name: String,
    pub revision: u64,
    pub minecraft_version: String,
    pub loader_kind: crate::LoaderKindDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    pub size_bytes: u64,
    pub file_count: u64,
    pub files_present: bool,
    pub trashed_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageOverview {
    pub categories: Vec<StorageCategorySummary>,
    pub total_size_bytes: u64,
    pub reclaimable_size_bytes: u64,
    pub trashed_instances: Vec<TrashedInstanceSummary>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearStorageCategoryRequest {
    pub category: StorageCategoryDto,
    #[serde(default)]
    pub confirm_managed_data: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCleanupResult {
    pub reclaimed_bytes: u64,
    pub removed_files: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreTrashedInstanceRequest {
    pub id: Uuid,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteTrashedInstanceRequest {
    pub id: Uuid,
    pub expected_revision: u64,
    pub confirmation_name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyInstanceTrashRequest {
    pub expected_count: u32,
}
