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
mod onboarding;
mod schema;
mod server;
mod settings;
mod support;
mod system;

pub use bootstrap::{BootstrapResponse, CapabilitySummary};
pub use catalog::{
    LoaderVersionCatalog, LoaderVersionsRequest, MinecraftReleaseKindDto, MinecraftVersionCatalog,
    MinecraftVersionOption,
};
pub use error::{AppError, FieldError};
pub use events::EventEnvelope;
pub use install::{
    CancelInstallJobRequest, InstallInstanceRequest, InstallJobStateDto, InstallJobSummary,
    InstallOperationDto, InstallQueueDirectionDto, ModpackInstallStarted, MoveInstallJobRequest,
    RetryInstallJobRequest, SetInstallJobPausedRequest,
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
    GameSessionStateDto, GameSessionSummary, InstanceSessionsRequest, LaunchInstanceRequest,
    ReadSessionLogRequest, RedactedLaunchPlan, SessionHistoryStateDto, SessionHistorySummary,
    SessionLogEvent, SessionLogEventKindDto, SessionLogSnapshot, SessionLogSubscription,
    StopGameSessionRequest, SubscribeSessionLogRequest, UnsubscribeSessionLogRequest,
};
pub use modpack::{
    ApplyModpackUpdateRequest, CheckModpackUpdateRequest, ContentSearchRequest,
    ImportLocalContentFileRequest, ImportLocalModRequest, InstallContentRequest,
    InstallContentSelection, InstallModRequest, InstallModSelection, InstallModpackRequest,
    InstanceContentFileSummary, InstanceContentFilesRequest, InstanceContentHistoryRequest,
    InstanceContentHistorySummary, InstanceContentKindDto, InstanceContentVersionsRequest,
    InstanceModHistoryRequest, InstanceModHistorySummary, InstanceModOriginDto,
    InstanceModReferenceSummary, InstanceModResolution, InstanceModSummary,
    InstanceModVersionsRequest, InstanceModsRequest, InstanceWorldsRequest, ModSearchRequest,
    ModpackProjectRequest, ModpackSearchRequest, ModpackSortDto, ModpackUpdateSummary,
    ModpackVersionRequest, ModpackVersionsRequest, MoveInstanceResourcePackRequest,
    RemoveInstanceContentFileRequest, RemoveInstanceModRequest,
    ResolveInstanceModRelationshipsRequest, ResourcePackOrderDirectionDto,
    SetInstanceContentFileEnabledRequest, SetInstanceContentPinnedRequest,
    SetInstanceModEnabledRequest, SetInstanceModPinnedRequest,
    SetInstanceResourcePackActiveRequest, UpdateInstanceContentRequest, UpdateInstanceModRequest,
};
pub use onboarding::OnboardingStateSummary;
pub use schema::schema_documents;
pub use server::{
    CreateSavedServerRequest, PingServerRequest, RemoveSavedServerRequest, SavedServerSummary,
    ServerStatusSummary, ServerTextSegment, UpdateSavedServerRequest,
};
pub use settings::{
    AppPreferencesDto, ReduceMotionPreferenceDto, ThemePreferenceDto, UpdateAppPreferencesRequest,
};
pub use support::{CreateSupportReportRequest, SupportReportExport, SupportReportPreview};
pub use system::{
    ClearStorageCategoryRequest, DeleteTrashedInstanceRequest, EmptyInstanceTrashRequest,
    JavaRuntimeSummary, PreflightSummary, RestoreTrashedInstanceRequest, StorageCategoryDto,
    StorageCategorySummary, StorageCleanupResult, StorageOverview, TrashedInstanceSummary,
};

pub const IPC_SCHEMA_VERSION: u32 = 1;
pub const DATABASE_SCHEMA_VERSION: u32 = 14;
pub use auth::{
    AccountIdRequest, AuthCancelRequest, AuthFlowStateDto, AuthFlowStatus, AuthStartResponse,
    MinecraftAccountStatusDto, MinecraftAccountSummary, SetDefaultAccountRequest,
};
