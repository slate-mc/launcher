use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductEvent {
    LauncherStarted,
    TelemetryEnabled,
    OnboardingCompleted,
    AccountConnected,
    InstanceCreated,
    InstallStarted,
    InstallCompleted,
    InstallFailed,
    InstallCancelled,
    LaunchStarted,
    LaunchCompleted,
    LaunchFailed,
    ContentInstalled,
}

impl ProductEvent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LauncherStarted => "launcher_started",
            Self::TelemetryEnabled => "telemetry_enabled",
            Self::OnboardingCompleted => "onboarding_completed",
            Self::AccountConnected => "account_connected",
            Self::InstanceCreated => "instance_created",
            Self::InstallStarted => "install_started",
            Self::InstallCompleted => "install_completed",
            Self::InstallFailed => "install_failed",
            Self::InstallCancelled => "install_cancelled",
            Self::LaunchStarted => "launch_started",
            Self::LaunchCompleted => "launch_completed",
            Self::LaunchFailed => "launch_failed",
            Self::ContentInstalled => "content_installed",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductPlatform {
    Windows,
    Linux,
    Macos,
    Other,
}

impl ProductPlatform {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureProductEventRequest {
    pub installation_id: Uuid,
    pub event: ProductEvent,
    pub app_version: String,
    pub platform: ProductPlatform,
    pub architecture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureProductEventResponse {
    pub accepted: bool,
}
