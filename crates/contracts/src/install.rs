use crate::InstanceSummary;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallJobStateDto {
    Queued,
    Running,
    Paused,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallOperationDto {
    InstanceInstall,
    ModInstall,
    ModUpdate,
    ModpackUpdate,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInstanceRequest {
    pub id: Uuid,
    pub expected_revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelInstallJobRequest {
    pub job_id: Uuid,
    pub revision_id: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetInstallJobPausedRequest {
    pub job_id: Uuid,
    pub revision_id: Uuid,
    pub paused: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallQueueDirectionDto {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveInstallJobRequest {
    pub job_id: Uuid,
    pub direction: InstallQueueDirectionDto,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryInstallJobRequest {
    pub job_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallJobSummary {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub revision_id: Uuid,
    pub state: InstallJobStateDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<InstallOperationDto>,
    pub can_retry: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<u32>,
    pub phase: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_items: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_items: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackInstallStarted {
    pub instance: InstanceSummary,
    pub job: InstallJobSummary,
}
