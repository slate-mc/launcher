use super::{ModpackProvider, ProviderError, ResolvedMod, modpacks_ch::ModpacksChProvider};
use crate::domain::{SearchPage, SearchRequest, VersionQuery};
use async_trait::async_trait;
use slate_modpack_api_contracts::{
    CategorySummary, Modpack, ModpackVersion, ModpackVersionSummary, Provider,
};

pub struct FtbProvider(ModpacksChProvider);

impl FtbProvider {
    #[must_use]
    pub fn new(upstream: crate::upstream::UpstreamClient) -> Self {
        Self(ModpacksChProvider::new(Provider::Ftb, upstream))
    }
}

#[async_trait]
impl ModpackProvider for FtbProvider {
    fn provider(&self) -> Provider {
        Provider::Ftb
    }

    async fn search(&self, request: SearchRequest) -> Result<SearchPage, ProviderError> {
        self.0.search(request).await
    }

    async fn get_project(&self, project_id: &str) -> Result<Modpack, ProviderError> {
        self.0.get_project(project_id).await
    }

    async fn list_versions(
        &self,
        project_id: &str,
        request: VersionQuery,
    ) -> Result<Vec<ModpackVersionSummary>, ProviderError> {
        self.0.list_versions(project_id, request).await
    }

    async fn get_version(
        &self,
        project_id: &str,
        version_id: &str,
    ) -> Result<ModpackVersion, ProviderError> {
        self.0.get_version(project_id, version_id).await
    }

    async fn get_install_version(
        &self,
        project_id: &str,
        version_id: &str,
    ) -> Result<ModpackVersion, ProviderError> {
        self.0.get_install_version(project_id, version_id).await
    }

    async fn categories(&self) -> Result<Vec<CategorySummary>, ProviderError> {
        self.0.categories().await
    }

    async fn search_mods(&self, _request: SearchRequest) -> Result<SearchPage, ProviderError> {
        Err(ProviderError::UnsupportedContent)
    }

    async fn resolve_mods(
        &self,
        _project_id: &str,
        _minecraft_version: &str,
        _loader: slate_modpack_api_contracts::LoaderKind,
        _version_id: Option<&str>,
    ) -> Result<Vec<ResolvedMod>, ProviderError> {
        Err(ProviderError::UnsupportedContent)
    }
}
