use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MinecraftAccountStatusDto {
    Ready,
    ReauthenticationRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftAccountSummary {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin_url: Option<String>,
    pub status: MinecraftAccountStatusDto,
    pub is_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_validated_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthFlowStateDto {
    WaitingForBrowser,
    Verifying,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStartResponse {
    pub flow_id: Uuid,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthFlowStatus {
    pub flow_id: Uuid,
    pub state: AuthFlowStateDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<MinecraftAccountSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_message: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthCancelRequest {
    pub flow_id: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountIdRequest {
    pub id: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDefaultAccountRequest {
    pub id: Uuid,
}
