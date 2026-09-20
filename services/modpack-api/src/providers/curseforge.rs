use super::{ModpackProvider, ProviderError, ResolvedMod, modpacks_ch::ModpacksChProvider};
use crate::domain::{SearchPage, SearchRequest, VersionQuery};
use async_trait::async_trait;
use slate_modpack_api_contracts::{
    CategorySummary, ModVersionSummary, Modpack, ModpackVersion, ModpackVersionSummary, Provider,
};

pub struct CurseForgeProvider(ModpacksChProvider);

impl CurseForgeProvider {
    #[must_use]
    pub fn new(upstream: crate::upstream::UpstreamClient) -> Self {
        Self(ModpacksChProvider::new(Provider::CurseForge, upstream))
    }
}

#[async_trait]
impl ModpackProvider for CurseForgeProvider {
    fn provider(&self) -> Provider {
        Provider::CurseForge
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

    async fn search_mods(&self, request: SearchRequest) -> Result<SearchPage, ProviderError> {
        self.0.search_mods(request).await
    }

    async fn list_mod_versions(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: slate_modpack_api_contracts::LoaderKind,
    ) -> Result<Vec<ModVersionSummary>, ProviderError> {
        self.0
            .list_mod_versions(project_id, minecraft_version, loader)
            .await
    }

    async fn resolve_mods(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: slate_modpack_api_contracts::LoaderKind,
        version_id: Option<&str>,
    ) -> Result<Vec<ResolvedMod>, ProviderError> {
        self.0
            .resolve_mods(project_id, minecraft_version, loader, version_id)
            .await
    }

    async fn resolve_mod(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: slate_modpack_api_contracts::LoaderKind,
        version_id: &str,
    ) -> Result<ResolvedMod, ProviderError> {
        self.0
            .resolve_mod(project_id, minecraft_version, loader, version_id)
            .await
    }
}
