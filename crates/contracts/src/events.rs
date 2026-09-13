use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub event_id: Uuid,
    pub sequence: u64,
    pub source_id: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub entity_revision: u64,
    /// UTC RFC3339 timestamp.
    pub occurred_at: String,
    pub kind: String,
    pub payload: Value,
}
