//! SQLite persistence for slate's local source of truth.

mod accounts;
mod database;
mod installations;
mod instance_mods;
mod instance_settings;
mod instances;
mod servers;
mod settings;

pub use accounts::{AccountRecord, AccountStatus, AuthenticatedAccount, LaunchAccount};
pub use database::{Database, StorageError};
pub use installations::{
    CompletedInstall, InstallJobRecord, InstalledRevisionRecord, InstalledRuntime, JobState,
    PendingInstall,
};
pub use instance_mods::{
    InstanceModEnabledChange, InstanceModRecord, InstanceModTarget, NewInstanceMod,
};
pub use instance_settings::{
    InstanceSettingsRecord, InstanceWindowMode, JavaSelectionMode, LauncherBehavior, MemoryMode,
    PerformancePreset, ProcessPriority, UpdateInstanceSettings,
};
pub use instances::{InstanceRecord, ModpackSourceRecord, NewInstance, NewModpackSource};
pub use servers::{NewSavedServer, SavedServerRecord};
pub use settings::{AppPreferences, ReduceMotionPreference, ThemePreference};
