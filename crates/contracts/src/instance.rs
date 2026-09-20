use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use slate_domain::{InstanceMode, InstanceSetupState, LoaderFamily, ManagementMode};
use slate_modpack_api_contracts::Provider;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceModeDto {
    Vanilla,
    Modded,
    Pvp,
}

impl From<InstanceMode> for InstanceModeDto {
    fn from(value: InstanceMode) -> Self {
        match value {
            InstanceMode::Vanilla => Self::Vanilla,
            InstanceMode::Modded => Self::Modded,
            InstanceMode::Pvp => Self::Pvp,
        }
    }
}

impl From<InstanceModeDto> for InstanceMode {
    fn from(value: InstanceModeDto) -> Self {
        match value {
            InstanceModeDto::Vanilla => Self::Vanilla,
            InstanceModeDto::Modded => Self::Modded,
            InstanceModeDto::Pvp => Self::Pvp,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ManagementModeDto {
    Local,
    Community,
}

impl From<ManagementMode> for ManagementModeDto {
    fn from(value: ManagementMode) -> Self {
        match value {
            ManagementMode::Local => Self::Local,
            ManagementMode::Community => Self::Community,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LoaderKindDto {
    Vanilla,
    Fabric,
    NeoForge,
}

impl From<LoaderFamily> for LoaderKindDto {
    fn from(value: LoaderFamily) -> Self {
        match value {
            LoaderFamily::Vanilla => Self::Vanilla,
            LoaderFamily::Fabric => Self::Fabric,
            LoaderFamily::NeoForge => Self::NeoForge,
        }
    }
}

impl From<LoaderKindDto> for LoaderFamily {
    fn from(value: LoaderKindDto) -> Self {
        match value {
            LoaderKindDto::Vanilla => Self::Vanilla,
            LoaderKindDto::Fabric => Self::Fabric,
            LoaderKindDto::NeoForge => Self::NeoForge,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceSetupStateDto {
    Configured,
    Preparing,
    Ready,
    Blocked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceWindowModeDto {
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LauncherBehaviorDto {
    KeepOpen,
    Minimize,
    Hide,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProcessPriorityDto {
    Low,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MemoryModeDto {
    Auto,
    Custom,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JavaSelectionModeDto {
    Managed,
    Detected,
    Custom,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PerformancePresetDto {
    Balanced,
    Throughput,
    LowLatency,
    Custom,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceArtworkKindDto {
    Icon,
    Banner,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSettingsSummary {
    pub description: String,
    pub notes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    pub tags: Vec<String>,
    pub has_custom_icon: bool,
    pub has_custom_banner: bool,
    pub banner_position_x: u8,
    pub banner_position_y: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_account_id: Option<Uuid>,
    pub window_mode: InstanceWindowModeDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_height: Option<u32>,
    pub launcher_behavior: LauncherBehaviorDto,
    pub game_language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_play_server: Option<String>,
    pub process_priority: ProcessPriorityDto,
    pub cpu_affinity: Vec<u16>,
    pub memory_mode: MemoryModeDto,
    pub initial_memory_mb: u32,
    pub effective_memory_mb: u32,
    pub java_mode: JavaSelectionModeDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_java_label: Option<String>,
    pub performance_preset: PerformancePresetDto,
    pub jvm_arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub backup_before_changes: bool,
    pub backup_retention: u8,
    pub log_retention_days: u16,
}

impl From<InstanceSetupState> for InstanceSetupStateDto {
    fn from(value: InstanceSetupState) -> Self {
        match value {
            InstanceSetupState::Configured => Self::Configured,
            InstanceSetupState::Preparing => Self::Preparing,
            InstanceSetupState::Ready => Self::Ready,
            InstanceSetupState::Blocked => Self::Blocked,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSummary {
    pub id: Uuid,
    pub name: String,
    pub storage_path: String,
    pub mode: InstanceModeDto,
    pub management_mode: ManagementModeDto,
    pub favorite: bool,
    pub revision: u64,
    pub minecraft_version: String,
    pub loader_kind: LoaderKindDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    pub memory_mb: u32,
    pub mod_count: u32,
    pub setup_state: InstanceSetupStateDto,
    pub settings: InstanceSettingsSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modpack_source: Option<ModpackSourceSummary>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_played: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackSourceSummary {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInstanceRequest {
    pub name: String,
    pub mode: InstanceModeDto,
    pub minecraft_version: String,
    pub loader_kind: LoaderKindDto,
    pub loader_version: Option<String>,
    pub memory_mb: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameInstanceRequest {
    pub id: Uuid,
    pub name: String,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstanceConfigurationRequest {
    pub id: Uuid,
    pub minecraft_version: String,
    pub loader_kind: LoaderKindDto,
    pub loader_version: Option<String>,
    pub memory_mb: u32,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstanceSettingsRequest {
    pub id: Uuid,
    pub description: String,
    pub notes: String,
    #[serde(default)]
    pub group_name: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub preferred_account_id: Option<Uuid>,
    pub banner_position_x: u8,
    pub banner_position_y: u8,
    pub window_mode: InstanceWindowModeDto,
    #[serde(default)]
    pub resolution_width: Option<u32>,
    #[serde(default)]
    pub resolution_height: Option<u32>,
    pub launcher_behavior: LauncherBehaviorDto,
    pub game_language: String,
    #[serde(default)]
    pub quick_play_server: Option<String>,
    pub process_priority: ProcessPriorityDto,
    #[serde(default)]
    pub cpu_affinity: Vec<u16>,
    pub memory_mode: MemoryModeDto,
    pub initial_memory_mb: u32,
    pub maximum_memory_mb: u32,
    pub java_mode: JavaSelectionModeDto,
    pub performance_preset: PerformancePresetDto,
    #[serde(default)]
    pub jvm_arguments: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    pub backup_before_changes: bool,
    pub backup_retention: u8,
    pub log_retention_days: u16,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectInstanceJavaRequest {
    pub id: Uuid,
    pub mode: JavaSelectionModeDto,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectInstanceArtworkRequest {
    pub id: Uuid,
    pub kind: InstanceArtworkKindDto,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetInstanceArtworkRequest {
    pub id: Uuid,
    pub kind: InstanceArtworkKindDto,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceArtworkAsset {
    pub mime_type: String,
    pub data_base64: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetInstanceGameOptionsRequest {
    pub id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstanceGameOptionsRequest {
    pub id: Uuid,
    pub values: BTreeMap<String, String>,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceGameOptionsSummary {
    pub file_exists: bool,
    pub values: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceDirectoryKindDto {
    Game,
    Mods,
    Logs,
    Screenshots,
    Saves,
    CrashReports,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInstanceDirectoryRequest {
    pub id: Uuid,
    pub kind: InstanceDirectoryKindDto,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateInstanceRequest {
    pub id: Uuid,
    pub name: String,
    pub include_worlds: bool,
    pub include_screenshots: bool,
    pub include_settings: bool,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveInstanceStorageRequest {
    pub id: Uuid,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportInstanceRequest {
    pub id: Uuid,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportInstanceRequest {}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSnapshotSummary {
    pub id: Uuid,
    pub size_bytes: u64,
    pub pinned: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSnapshotsRequest {
    pub id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInstanceSnapshotRequest {
    pub id: Uuid,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreInstanceSnapshotRequest {
    pub id: Uuid,
    pub snapshot_id: Uuid,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteInstanceSnapshotRequest {
    pub id: Uuid,
    pub snapshot_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetInstanceSnapshotPinnedRequest {
    pub id: Uuid,
    pub snapshot_id: Uuid,
    pub pinned: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetFavoriteRequest {
    pub id: Uuid,
    pub favorite: bool,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashInstanceRequest {
    pub id: Uuid,
    pub expected_revision: u64,
}
