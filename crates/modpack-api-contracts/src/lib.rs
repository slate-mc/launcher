//! Stable public contracts shared by the slate modpack API and its clients.

mod content;
mod error;
mod file;
mod import_plan;
mod install;
mod metadata;
mod modpack;
mod provider;
mod telemetry;
mod version;

pub use content::{ContentInstallPlanRequest, ContentKind};
pub use error::{ApiEnvelope, ApiErrorCode, ApiErrorDetail, ApiFieldError, ApiMeta};
pub use file::{
    DownloadSource, FileOption, Hashes, ModpackFile, PackFileType, ProviderReference, Side,
};
pub use import_plan::{ImportPackFormat, ImportPackPlanRequest, ImportedPackPlan};
pub use install::{
    Architecture, ExtractAction, InstallPlan, InstallPlanDownload, InstallPlanInstance,
    InstallPlanRequest, JavaPlan, ModInstallPlanRequest, Platform, RuntimePlan,
};
pub use metadata::{
    CategoriesResponse, CategorySummary, HealthResponse, LoaderTypesResponse, MinecraftReleaseKind,
    MinecraftVersionSummary, MinecraftVersionsResponse, ReadinessResponse, ReadinessStatus,
    UpdateResponse,
};
pub use modpack::{
    Author, ModProjectReference, ModVersionList, ModVersionSummary, Modpack, ModpackLinks,
    ModpackSummary, ResolveModsRequest, ResolveModsResponse, ResolvedModProject, SearchResponse,
    VersionReference,
};
pub use provider::{Provider, ProviderMetadata, ProviderStatus, ProvidersResponse};
pub use telemetry::{
    CaptureProductEventRequest, CaptureProductEventResponse, ProductEvent, ProductPlatform,
};
pub use version::{
    Loader, LoaderKind, MemoryRecommendation, MinecraftTarget, ModpackVersion,
    ModpackVersionSummary, ReleaseType, VersionPage,
};
