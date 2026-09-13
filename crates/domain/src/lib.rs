//! Pure domain types and policies for slate.
//!
//! This crate deliberately has no database, network, process, or Tauri dependency.

mod ids;
mod instance;
mod launch;

pub use ids::{
    AccountId, InstanceId, JobId, RequestId, RevisionId, ServerId, SessionId, StorageRootId,
};
pub use instance::{
    InstanceMode, InstanceName, InstanceNameError, InstanceSetupState, LoaderFamily, LoaderKind,
    ManagementMode,
};
pub use launch::{LaunchReadiness, LaunchState, TransitionError};

/// Public product identifier used by native components.
pub const PRODUCT_NAME: &str = "slate";

#[cfg(test)]
mod tests {
    use super::PRODUCT_NAME;

    #[test]
    fn product_name_is_canonical_lowercase() {
        assert_eq!(PRODUCT_NAME, "slate");
        assert_eq!(PRODUCT_NAME, PRODUCT_NAME.to_lowercase());
    }
}
