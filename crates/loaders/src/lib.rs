//! Loader-specific metadata acquisition and validation.
//!
//! Fabric and NeoForge publish Mojang-compatible version metadata, but through
//! different distribution formats. This crate translates both formats into
//! slate_minecraft::VersionMetadata without coupling the core launch planner
//! to provider-specific HTTP or archive concerns.

mod fabric;
mod http;
mod neoforge;

pub use fabric::{FABRIC_META_ORIGIN, FabricAdapter, FabricError};
pub use http::HttpError;
pub use neoforge::{
    MAX_NEOFORGE_INSTALLER_BYTES, NEOFORGE_MAVEN_ORIGIN, NeoForgeAdapter, NeoForgeError,
    NeoForgeInstallProfile, NeoForgeInstallerBundle, NeoForgeProcessor, SidedDataValue,
};
