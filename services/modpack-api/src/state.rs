use crate::providers::{
    CurseForgeProvider, FtbProvider, ModrinthContentProvider, ModrinthProvider, ProviderRegistry,
};
use crate::rate_limit::RateLimiter;
use crate::release::ReleaseCatalog;
use crate::support_reports::SupportReportStore;
use crate::telemetry::PostHogRelay;
use crate::upstream::{UpstreamClient, UpstreamError};
use moka::sync::Cache;
use slate_minecraft::{MetadataFetchError, MojangMetadataClient, VersionManifest};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub providers: ProviderRegistry,
    pub content: ModrinthContentProvider,
    pub upstream: UpstreamClient,
    pub minecraft: MinecraftCatalog,
    pub rate_limiter: RateLimiter,
    pub releases: ReleaseCatalog,
    pub telemetry: PostHogRelay,
    pub support_reports: SupportReportStore,
}

impl AppState {
    pub fn new(config: &crate::config::Config) -> Result<Self, StateError> {
        let upstream =
            UpstreamClient::new(config.upstream_url.clone(), &config.upstream_user_agent)?;
        let providers = ProviderRegistry::new(vec![
            Arc::new(CurseForgeProvider::new(upstream.clone())),
            Arc::new(ModrinthProvider::new(upstream.clone())),
            Arc::new(FtbProvider::new(upstream.clone())),
        ]);
        Ok(Self {
            providers,
            content: ModrinthContentProvider::new(upstream.clone()),
            upstream,
            minecraft: MinecraftCatalog::new()?,
            rate_limiter: RateLimiter::default(),
            releases: ReleaseCatalog::new(
                config.release_manifest_url.clone(),
                config.beta_release_manifest_url.clone(),
                config.stable_release_rollout,
                config.beta_release_rollout,
                &config.upstream_user_agent,
            )?,
            telemetry: PostHogRelay::new(
                config.posthog_host.clone(),
                config.posthog_project_token.clone(),
                &config.upstream_user_agent,
            )?,
            support_reports: SupportReportStore::new(&config.support_reports)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct MinecraftCatalog {
    client: MojangMetadataClient,
    manifest: Cache<&'static str, Arc<VersionManifest>>,
}

impl MinecraftCatalog {
    fn new() -> Result<Self, MetadataFetchError> {
        Ok(Self {
            client: MojangMetadataClient::new()?,
            manifest: Cache::builder()
                .time_to_live(Duration::from_secs(6 * 60 * 60))
                .max_capacity(1)
                .build(),
        })
    }

    pub async fn manifest(&self) -> Result<Arc<VersionManifest>, MetadataFetchError> {
        if let Some(manifest) = self.manifest.get("official") {
            return Ok(manifest);
        }
        let manifest = Arc::new(self.client.fetch_manifest().await?);
        self.manifest.insert("official", manifest.clone());
        Ok(manifest)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("could not initialize the modpack upstream")]
    Upstream(#[from] UpstreamError),
    #[error("could not initialize the Minecraft metadata client")]
    Minecraft(#[from] MetadataFetchError),
    #[error("could not initialize the launcher release catalog")]
    Release(#[from] crate::release::ReleaseError),
    #[error("could not initialize anonymous usage reporting")]
    Telemetry(#[from] crate::telemetry::TelemetryError),
    #[error("could not initialize private support report storage")]
    SupportReports(#[from] crate::support_reports::SupportReportStoreError),
}
