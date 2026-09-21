use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherFeatureConfig {
    pub installs_enabled: bool,
    pub launch_enabled: bool,
    pub authentication_enabled: bool,
    pub support_reports_enabled: bool,
    pub discover_preview_rollout: u8,
    pub cache_seconds: u32,
    pub remote_available: bool,
}

impl Default for LauncherFeatureConfig {
    fn default() -> Self {
        Self {
            installs_enabled: true,
            launch_enabled: true,
            authentication_enabled: true,
            support_reports_enabled: true,
            discover_preview_rollout: 0,
            cache_seconds: 300,
            remote_available: false,
        }
    }
}
