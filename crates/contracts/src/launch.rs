use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactedLaunchPlan {
    pub executable: String,
    pub arguments: Vec<String>,
    pub working_directory: String,
    pub environment: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchInstanceRequest {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopGameSessionRequest {
    pub id: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeSessionLogRequest {
    pub session_id: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeSessionLogRequest {
    pub subscription_id: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLogSubscription {
    pub id: Uuid,
    pub session_id: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionLogEventKindDto {
    Snapshot,
    Append,
    Reset,
    Closed,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLogEvent {
    pub subscription_id: Uuid,
    pub session_id: Uuid,
    pub kind: SessionLogEventKindDto,
    /// Decimal string so file offsets remain lossless at the JavaScript boundary.
    pub offset: String,
    pub truncated: bool,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameSessionStateDto {
    Running,
    Stopping,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSessionSummary {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub state: GameSessionStateDto,
    pub mode: String,
    pub pid: u32,
    pub log_name: String,
}
