//! Versioned, serializable contracts at slate process boundaries.
//!
//! Types in this crate are safe to expose over IPC. Secrets and arbitrary local
//! filesystem paths do not belong here.

mod auth;
mod bootstrap;
mod catalog;
mod error;
mod events;
mod install;
mod instance;
mod launch;
mod modpack;
mod schema;
mod settings;
mod system;

pub use bootstrap::{BootstrapResponse, CapabilitySummary};
pub use catalog::{
    LoaderVersionCatalog, LoaderVersionsRequest, MinecraftReleaseKindDto, MinecraftVersionCatalog,
    MinecraftVersionOption,
};
pub use error::{AppError, FieldError};
pub use events::EventEnvelope;
pub use install::{
    InstallInstanceRequest, InstallJobStateDto, InstallJobSummary, ModpackInstallStarted,
};
pub use instance::{
    CreateInstanceRequest, CreateInstanceSnapshotRequest, DeleteInstanceSnapshotRequest,
    DuplicateInstanceRequest, ExportInstanceRequest, GetInstanceArtworkRequest,
    GetInstanceGameOptionsRequest, ImportInstanceRequest, InstanceArtworkAsset,
    InstanceArtworkKindDto, InstanceDirectoryKindDto, InstanceGameOptionsSummary, InstanceModeDto,
    InstanceSettingsSummary, InstanceSetupStateDto, InstanceSnapshotSummary,
    InstanceSnapshotsRequest, InstanceSummary, InstanceWindowModeDto, JavaSelectionModeDto,
    LauncherBehaviorDto, LoaderKindDto, ManagementModeDto, MemoryModeDto, ModpackSourceSummary,
    MoveInstanceStorageRequest, OpenInstanceDirectoryRequest, PerformancePresetDto,
    ProcessPriorityDto, RenameInstanceRequest, RestoreInstanceSnapshotRequest,
    SelectInstanceArtworkRequest, SelectInstanceJavaRequest, SetFavoriteRequest,
    SetInstanceSnapshotPinnedRequest, TrashInstanceRequest, UpdateInstanceConfigurationRequest,
    UpdateInstanceGameOptionsRequest, UpdateInstanceSettingsRequest,
};
pub use launch::{
    GameSessionStateDto, GameSessionSummary, LaunchInstanceRequest, RedactedLaunchPlan,
    SessionLogEvent, SessionLogEventKindDto, SessionLogSubscription, StopGameSessionRequest,
    SubscribeSessionLogRequest, UnsubscribeSessionLogRequest,
};
pub use modpack::{
    InstallModRequest, InstallModSelection, InstallModpackRequest, InstanceContentFileSummary,
    InstanceContentFilesRequest, InstanceContentKindDto, InstanceModOriginDto,
    InstanceModResolution, InstanceModSummary, InstanceModsRequest, ModSearchRequest,
    ModpackProjectRequest, ModpackSearchRequest, ModpackSortDto, ModpackVersionRequest,
    ModpackVersionsRequest, RemoveInstanceContentFileRequest, RemoveInstanceModRequest,
    SetInstanceContentFileEnabledRequest, SetInstanceModEnabledRequest,
    SetInstanceModPinnedRequest,
};
pub use schema::schema_documents;
pub use settings::{
    AppPreferencesDto, ReduceMotionPreferenceDto, ThemePreferenceDto, UpdateAppPreferencesRequest,
};
pub use system::{
    ClearStorageCategoryRequest, DeleteTrashedInstanceRequest, EmptyInstanceTrashRequest,
    JavaRuntimeSummary, PreflightSummary, RestoreTrashedInstanceRequest, StorageCategoryDto,
    StorageCategorySummary, StorageCleanupResult, StorageOverview, TrashedInstanceSummary,
};

pub const IPC_SCHEMA_VERSION: u32 = 1;
pub const DATABASE_SCHEMA_VERSION: u32 = 9;
pub use auth::{
    AccountIdRequest, AuthCancelRequest, AuthFlowStateDto, AuthFlowStatus, AuthStartResponse,
    MinecraftAccountStatusDto, MinecraftAccountSummary, SetDefaultAccountRequest,
};
