mod curseforge;
mod ftb;
mod modpacks_ch;
mod modrinth;

pub use curseforge::CurseForgeProvider;
pub use ftb::FtbProvider;
pub use modrinth::ModrinthProvider;

use crate::domain::{SearchPage, SearchRequest, VersionQuery};
use async_trait::async_trait;
use slate_modpack_api_contracts::{
    CategorySummary, Hashes, ModVersionSummary, Modpack, ModpackVersion, ModpackVersionSummary,
    Provider,
};
use std::collections::BTreeMap;
use std::sync::Arc;

#[async_trait]
pub trait ModpackProvider: Send + Sync {
    fn provider(&self) -> Provider;

    async fn search(&self, request: SearchRequest) -> Result<SearchPage, ProviderError>;

    async fn get_project(&self, project_id: &str) -> Result<Modpack, ProviderError>;

    async fn list_versions(
        &self,
        project_id: &str,
        request: VersionQuery,
    ) -> Result<Vec<ModpackVersionSummary>, ProviderError>;

    async fn get_version(
        &self,
        project_id: &str,
        version_id: &str,
    ) -> Result<ModpackVersion, ProviderError>;

    async fn get_install_version(
        &self,
        project_id: &str,
        version_id: &str,
    ) -> Result<ModpackVersion, ProviderError>;

    async fn categories(&self) -> Result<Vec<CategorySummary>, ProviderError>;

    async fn search_mods(&self, request: SearchRequest) -> Result<SearchPage, ProviderError>;

    async fn list_mod_versions(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: slate_modpack_api_contracts::LoaderKind,
    ) -> Result<Vec<ModVersionSummary>, ProviderError>;

    async fn resolve_mods(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: slate_modpack_api_contracts::LoaderKind,
        version_id: Option<&str>,
    ) -> Result<Vec<ResolvedMod>, ProviderError>;

    async fn resolve_mod(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: slate_modpack_api_contracts::LoaderKind,
        version_id: &str,
    ) -> Result<ResolvedMod, ProviderError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMod {
    pub project_id: String,
    pub version_id: String,
    pub version_name: String,
    pub file_name: String,
    pub url: String,
    pub size: u64,
    pub hashes: Hashes,
}

#[derive(Clone)]
pub struct ProviderRegistry {
    providers: Arc<BTreeMap<Provider, Arc<dyn ModpackProvider>>>,
}

impl ProviderRegistry {
    #[must_use]
    pub fn new(providers: Vec<Arc<dyn ModpackProvider>>) -> Self {
        Self {
            providers: Arc::new(
                providers
                    .into_iter()
                    .map(|provider| (provider.provider(), provider))
                    .collect(),
            ),
        }
    }

    #[must_use]
    pub fn get(&self, provider: Provider) -> Option<Arc<dyn ModpackProvider>> {
        self.providers.get(&provider).cloned()
    }

    pub fn all(&self) -> impl Iterator<Item = (Provider, Arc<dyn ModpackProvider>)> + '_ {
        self.providers
            .iter()
            .map(|(provider, adapter)| (*provider, adapter.clone()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingResource {
    Project,
    Version,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider identifier is invalid")]
    InvalidIdentifier,
    #[error("provider resource was not found")]
    NotFound(MissingResource),
    #[error("provider is temporarily unavailable")]
    Unavailable,
    #[error("provider rate limit was reached")]
    RateLimited,
    #[error("provider returned an invalid response")]
    InvalidResponse,
    #[error("provider returned an unsafe installation path")]
    InvalidInstallPath,
    #[error("provider returned a file without a safe download")]
    DownloadUnavailable,
    #[error("modpack loader is unsupported")]
    UnsupportedLoader,
    #[error("provider does not support this content type")]
    UnsupportedContent,
}
