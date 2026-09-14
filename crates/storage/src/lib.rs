//! SQLite persistence for slate's local source of truth.

mod accounts;
mod database;
mod installations;
mod instance_mods;
mod instances;
mod servers;
mod settings;

pub use accounts::{AccountRecord, AccountStatus, AuthenticatedAccount, LaunchAccount};
pub use database::{Database, StorageError};
pub use installations::{
    CompletedInstall, InstallJobRecord, InstalledRevisionRecord, InstalledRuntime, JobState,
    PendingInstall,
};
pub use instance_mods::{InstanceModRecord, NewInstanceMod};
pub use instances::{InstanceRecord, ModpackSourceRecord, NewInstance, NewModpackSource};
pub use servers::{NewSavedServer, SavedServerRecord};
pub use settings::{AppPreferences, ReduceMotionPreference, ThemePreference};
