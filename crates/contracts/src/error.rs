use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub user_message: Box<String>,
    pub retryable: bool,
    pub correlation_id: Uuid,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<FieldError>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacted_details: Option<Box<Value>>,
}

impl AppError {
    #[must_use]
    pub fn new(code: impl Into<String>, user_message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            user_message: Box::new(user_message.into()),
            retryable: false,
            correlation_id: Uuid::new_v4(),
            field_errors: Vec::new(),
            action_ids: Vec::new(),
            redacted_details: None,
        }
    }

    #[must_use]
    pub const fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    #[must_use]
    pub fn with_field_error(
        mut self,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.field_errors.push(FieldError {
            field: field.into(),
            message: message.into(),
        });
        self
    }
}
