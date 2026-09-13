use crate::{DATABASE_SCHEMA_VERSION, IPC_SCHEMA_VERSION};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use slate_domain::PRODUCT_NAME;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySummary {
    pub id: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

impl CapabilitySummary {
    #[must_use]
    pub fn available(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            available: true,
            unavailable_reason: None,
        }
    }

    #[must_use]
    pub fn unavailable(id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            available: false,
            unavailable_reason: Some(reason.into()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResponse {
    pub product_name: String,
    pub ipc_schema_version: u32,
    pub database_schema_version: u32,
    pub capabilities: Vec<CapabilitySummary>,
}

impl BootstrapResponse {
    #[must_use]
    pub fn new(capabilities: Vec<CapabilitySummary>) -> Self {
        Self {
            product_name: PRODUCT_NAME.to_owned(),
            ipc_schema_version: IPC_SCHEMA_VERSION,
            database_schema_version: DATABASE_SCHEMA_VERSION,
            capabilities,
        }
    }
}
