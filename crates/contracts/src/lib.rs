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
    CreateInstanceRequest, InstanceModeDto, InstanceSetupStateDto, InstanceSummary, LoaderKindDto,
    ManagementModeDto, ModpackSourceSummary, RenameInstanceRequest, SetFavoriteRequest,
    TrashInstanceRequest, UpdateInstanceConfigurationRequest,
};
pub use launch::{
    GameSessionStateDto, GameSessionSummary, LaunchInstanceRequest, RedactedLaunchPlan,
    SessionLogEvent, SessionLogEventKindDto, SessionLogSubscription, StopGameSessionRequest,
    SubscribeSessionLogRequest, UnsubscribeSessionLogRequest,
};
pub use modpack::{
    InstallModRequest, InstallModpackRequest, InstanceModOriginDto, InstanceModResolution,
    InstanceModSummary, InstanceModsRequest, ModSearchRequest, ModpackProjectRequest,
    ModpackSearchRequest, ModpackSortDto, ModpackVersionRequest, ModpackVersionsRequest,
    RemoveInstanceModRequest, SetInstanceModEnabledRequest,
};
pub use schema::schema_documents;
pub use settings::{
    AppPreferencesDto, ReduceMotionPreferenceDto, ThemePreferenceDto, UpdateAppPreferencesRequest,
};
pub use system::{JavaRuntimeSummary, PreflightSummary};

pub const IPC_SCHEMA_VERSION: u32 = 1;
pub const DATABASE_SCHEMA_VERSION: u32 = 5;
pub use auth::{
    AccountIdRequest, AuthCancelRequest, AuthFlowStateDto, AuthFlowStatus, AuthStartResponse,
    MinecraftAccountStatusDto, MinecraftAccountSummary, SetDefaultAccountRequest,
};
