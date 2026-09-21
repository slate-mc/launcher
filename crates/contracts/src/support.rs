use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportReportPreview {
    pub diagnostic_file_count: u32,
    pub diagnostic_bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSupportReportRequest {
    pub include_launcher_logs: bool,
    pub include_install_activity: bool,
    pub include_instance_summary: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportReportExport {
    pub report_id: Uuid,
    pub file_name: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportReportSubmission {
    pub report_id: Uuid,
    pub bytes: u64,
}
