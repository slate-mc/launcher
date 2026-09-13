use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemePreferenceDto {
    Dark,
    Light,
    System,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReduceMotionPreferenceDto {
    System,
    On,
    Off,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferencesDto {
    pub theme: ThemePreferenceDto,
    pub download_concurrency: u8,
    pub telemetry_enabled: bool,
    pub reduce_motion: ReduceMotionPreferenceDto,
}

pub type UpdateAppPreferencesRequest = AppPreferencesDto;
