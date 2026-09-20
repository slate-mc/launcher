use crate::{Architecture, InstallPlan, Platform};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportPackFormat {
    CurseForge,
    Modrinth,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct ImportPackPlanRequest {
    pub format: ImportPackFormat,
    pub manifest: serde_json::Value,
    pub platform: Platform,
    pub arch: Architecture,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ImportedPackPlan {
    pub name: String,
    pub version_name: String,
    pub plan: InstallPlan,
    pub override_directories: Vec<String>,
}
