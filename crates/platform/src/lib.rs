//! Operating-system paths and filesystem policy for slate.

mod paths;
mod policy;
mod runtime;

pub use paths::{AppPaths, AppPathsError};
pub use policy::{ManagedRelativePath, PathPolicyError};
pub use runtime::{JavaArchitecture, JavaRuntimeProbe, detect_java_runtime, probe_java_executable};
