//! Minecraft metadata and launch construction.
//!
//! The launch engine resolves Mojang-style metadata inheritance and produces a
//! typed, shell-free preparation for vanilla and loader overlays. Artifact
//! download/commit and process supervision remain separate ownership boundaries.

mod artifact;
mod launch_plan;
mod maven;
mod metadata;
mod mojang;
mod planner;
mod resolver;
mod rules;

pub use artifact::{
    ArtifactError, ArtifactKind, ArtifactRequirement, ExpectedHash, HashAlgorithm,
    NativeExtraction, VerificationError, verify_artifact,
};
pub use launch_plan::{
    EnvironmentValue, LaunchArgument, LaunchPlan, LaunchPlanError, REDACTED_VALUE,
};
pub use maven::{MavenCoordinate, MavenError};
pub use metadata::{
    ArgumentEntry, ArgumentValue, AssetIndex, ClientLogging, DownloadInfo, ExtractRules,
    JavaVersion, Library, LibraryDownloads, Logging, MAX_VERSION_MANIFEST_BYTES,
    MAX_VERSION_METADATA_BYTES, MetadataError, OsRule, Rule, RuleAction, VersionArguments,
    VersionDownloads, VersionManifest, VersionManifestEntry, VersionMetadata,
};
pub use mojang::{MetadataFetchError, MojangMetadataClient, VERSION_MANIFEST_URL};
pub use planner::{
    InstallPreparation, JavaRuntime, LaunchBuildError, LaunchIdentity, LaunchLayout, LaunchOptions,
    LaunchPlanner, LaunchPreparation, LaunchRequest, QuickPlay,
};
pub use resolver::{ResolveError, ResolvedVersion};
pub use rules::{Architecture, OperatingSystem, RuleContext, RuleError};
