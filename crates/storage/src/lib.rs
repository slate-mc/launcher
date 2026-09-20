//! SQLite persistence for slate's local source of truth.

mod accounts;
mod database;
mod installations;
mod instance_content;
mod instance_mods;
mod instance_settings;
mod instances;
mod onboarding;
mod servers;
mod settings;
mod snapshots;

pub use accounts::{AccountRecord, AccountStatus, AuthenticatedAccount, LaunchAccount};
pub use database::{Database, StorageError};
pub use installations::{
    CompletedInstall, CompletedModpackUpdate, InstallJobRecord, InstalledRevisionRecord,
    InstalledRuntime, JobState, PendingInstall,
};
pub use instance_content::{InstanceProviderContentRecord, NewInstanceProviderContent};
pub use instance_mods::{
    InstanceModDependency, InstanceModDependencyRecord, InstanceModEnabledChange,
    InstanceModHistoryRecord, InstanceModRecord, InstanceModTarget, NewInstanceMod,
    NewInstanceModDependencySet,
};
pub use instance_settings::{
    InstanceSettingsRecord, InstanceWindowMode, JavaSelectionMode, LauncherBehavior, MemoryMode,
    PerformancePreset, ProcessPriority, UpdateInstanceSettings,
};
pub use instances::{
    InstanceRecord, ModpackSourceRecord, NewInstance, NewModpackSource, TrashedInstanceRecord,
};
pub use onboarding::OnboardingStateRecord;
pub use servers::{NewSavedServer, SavedServerRecord};
pub use settings::{AppPreferences, ReduceMotionPreference, ThemePreference};
pub use snapshots::InstanceSnapshotRecord;
