//! SQLite persistence for slate's local source of truth.

mod accounts;
mod database;
mod installations;
mod instances;
mod servers;
mod settings;

pub use accounts::{AccountRecord, AccountStatus, AuthenticatedAccount, LaunchAccount};
pub use database::{Database, StorageError};
pub use installations::{
    InstallJobRecord, InstalledRevisionRecord, InstalledRuntime, JobState, PendingInstall,
};
pub use instances::{InstanceRecord, NewInstance};
pub use servers::{NewSavedServer, SavedServerRecord};
pub use settings::{AppPreferences, ReduceMotionPreference, ThemePreference};
