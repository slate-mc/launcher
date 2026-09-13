use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallJobStateDto {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInstanceRequest {
    pub id: Uuid,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallJobSummary {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub revision_id: Uuid,
    pub state: InstallJobStateDto,
    pub phase: String,
    pub message: String,
    pub created_at: String,
    pub updated_at: String,
}
