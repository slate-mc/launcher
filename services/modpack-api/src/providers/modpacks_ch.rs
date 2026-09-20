use super::{MissingResource, ProviderError, ResolvedMod};
use crate::domain::{SearchPage, SearchRequest, VersionQuery};
use crate::upstream::{CachePolicy, UpstreamClient};
use slate_modpack_api_contracts::{
    CategorySummary, Hashes, LoaderKind, ModVersionSummary, Modpack, ModpackVersion,
    ModpackVersionSummary, Provider,
};
use std::collections::{BTreeMap, VecDeque};

mod mapping;
mod wire;

use mapping::*;
use wire::*;

const INSTALL_METADATA_CONCURRENCY: usize = 4;
const MAX_MOD_DEPENDENCIES: usize = 256;
const MAX_DEPENDENCY_VERSION_PAGES: u32 = 10;
const MAX_LISTED_MOD_VERSIONS: usize = 200;

#[derive(Clone, Debug)]
pub struct ModpacksChProvider {
    provider: Provider,
    upstream: UpstreamClient,
}

impl ModpacksChProvider {
    #[must_use]
    pub const fn new(provider: Provider, upstream: UpstreamClient) -> Self {
        Self { provider, upstream }
    }

    pub async fn search(&self, request: SearchRequest) -> Result<SearchPage, ProviderError> {
        if self.provider == Provider::Ftb {
            return self.search_ftb(request).await;
        }

        let action = if request.query.is_some() {
            "search"
        } else {
            "browse"
        };
        let mut filters = Vec::new();
        if let Some(version) = &request.minecraft_version {
            filters.push(version.clone());
        }
        if let Some(loader) = request.loader {
            filters.push(loader_name(loader).to_owned());
        }
        if let Some(category) = &request.category {
            filters.push(category.clone());
        }
        let sort = upstream_sort(request.sort);
        let page = request.page.to_string();
        let mut path = vec![
            "public".to_owned(),
            self.provider.as_str().to_owned(),
            action.to_owned(),
        ];
        path.append(&mut filters);
        path.push(sort.to_owned());
        path.push(page);
        let path_refs = path.iter().map(String::as_str).collect::<Vec<_>>();
        let query = request
            .query
            .as_deref()
            .map_or_else(Vec::new, |term| vec![("term", term)]);
        let response: UpstreamBrowse = self
            .upstream
            .get_json(&path_refs, &query, CachePolicy::Search)
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
        ensure_success(response.status.as_deref())?;
        let page = response
            .page
            .as_ref()
            .and_then(FlexibleId::as_u32)
            .unwrap_or(request.page);
        let pages = response
            .pages
            .as_ref()
            .and_then(FlexibleId::as_u32)
            .unwrap_or(page);
        let mut items = response
            .packs
            .into_iter()
            .filter(|card| provider_matches(card, self.provider))
            .map(|card| map_search_card(card, self.provider, request.loader))
            .filter(|item| search_item_matches(item, &request))
            .collect::<Vec<_>>();
        items.truncate(request.limit);
        Ok(SearchPage { items, page, pages })
    }

    pub async fn search_mods(&self, request: SearchRequest) -> Result<SearchPage, ProviderError> {
        if self.provider == Provider::Ftb {
            return Err(ProviderError::UnsupportedContent);
        }
        let minecraft_version = request
            .minecraft_version
            .as_deref()
            .ok_or(ProviderError::InvalidResponse)?;
        let loader = request.loader.ok_or(ProviderError::UnsupportedLoader)?;
        if loader == LoaderKind::Vanilla {
            return Err(ProviderError::UnsupportedLoader);
        }

        let action = if request.query.is_some() {
            "search"
        } else {
            "browse"
        };
        let page = request.page.to_string();
        let path = [
            "public",
            self.provider.as_str(),
            "mods",
            action,
            minecraft_version,
            loader_name(loader),
            upstream_sort(request.sort),
            page.as_str(),
        ];
        let query = request
            .query
            .as_deref()
            .map_or_else(Vec::new, |term| vec![("term", term)]);
        let response: UpstreamBrowse = self
            .upstream
            .get_json(&path, &query, CachePolicy::Search)
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
        ensure_success(response.status.as_deref())?;
        let page = response
            .page
            .as_ref()
            .and_then(FlexibleId::as_u32)
            .unwrap_or(request.page);
        let pages = response
            .pages
            .as_ref()
            .and_then(FlexibleId::as_u32)
            .unwrap_or(page);
        let mut items = response
            .mods
            .into_iter()
            .filter(|card| provider_matches(card, self.provider))
            .map(|card| {
                let mut item = map_search_card(card, self.provider, Some(loader));
                // The upstream route itself applies these exact filters. Its card tags
                // can describe only the newest release, so preserve the queried target.
                item.minecraft_versions = vec![minecraft_version.to_owned()];
                item.loaders = vec![loader];
                item
            })
            .collect::<Vec<_>>();
        items.truncate(request.limit);
        Ok(SearchPage { items, page, pages })
    }

    pub async fn resolve_mods(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
        version_id: Option<&str>,
    ) -> Result<Vec<ResolvedMod>, ProviderError> {
        validate_identifier(project_id)?;
        if version_id.is_some_and(|value| validate_identifier(value).is_err()) {
            return Err(ProviderError::InvalidIdentifier);
        }
        if minecraft_version.is_empty() || minecraft_version.len() > 32 {
            return Err(ProviderError::InvalidIdentifier);
        }
        if loader == LoaderKind::Vanilla {
            return Err(ProviderError::UnsupportedLoader);
        }
        match self.provider {
            Provider::CurseForge => {
                self.resolve_curseforge_mod_graph(project_id, minecraft_version, loader, version_id)
                    .await
            }
            Provider::Modrinth => {
                self.resolve_modrinth_mod_graph(project_id, minecraft_version, loader, version_id)
                    .await
            }
            Provider::Ftb => Err(ProviderError::UnsupportedContent),
        }
    }

    pub async fn resolve_mod(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
        version_id: &str,
    ) -> Result<ResolvedMod, ProviderError> {
        validate_identifier(project_id)?;
        validate_identifier(version_id)?;
        if minecraft_version.is_empty() || minecraft_version.len() > 32 {
            return Err(ProviderError::InvalidIdentifier);
        }
        if loader == LoaderKind::Vanilla {
            return Err(ProviderError::UnsupportedLoader);
        }
        match self.provider {
            Provider::CurseForge => {
                let version = self
                    .select_curseforge_mod_version(
                        project_id,
                        Some(version_id),
                        minecraft_version,
                        loader,
                    )
                    .await?;
                self.map_curseforge_mod_version(version, project_id).await
            }
            Provider::Modrinth => {
                let version = self
                    .select_modrinth_version(
                        project_id,
                        Some(version_id),
                        minecraft_version,
                        loader,
                    )
                    .await?;
                self.map_modrinth_version(version, project_id).await
            }
            Provider::Ftb => Err(ProviderError::UnsupportedContent),
        }
    }

    pub async fn list_mod_versions(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<Vec<ModVersionSummary>, ProviderError> {
        validate_identifier(project_id)?;
        if minecraft_version.is_empty() || minecraft_version.len() > 32 {
            return Err(ProviderError::InvalidIdentifier);
        }
        if loader == LoaderKind::Vanilla {
            return Err(ProviderError::UnsupportedLoader);
        }
        match self.provider {
            Provider::CurseForge => {
                self.list_curseforge_mod_versions(project_id, minecraft_version, loader)
                    .await
            }
            Provider::Modrinth => {
                self.list_modrinth_mod_versions(project_id, minecraft_version, loader)
                    .await
            }
            Provider::Ftb => Err(ProviderError::UnsupportedContent),
        }
    }

    async fn list_curseforge_mod_versions(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<Vec<ModVersionSummary>, ProviderError> {
        let query = VersionQuery {
            minecraft_version: Some(minecraft_version.to_owned()),
            loader: Some(loader),
            release_type: None,
            page: 1,
            limit: MAX_LISTED_MOD_VERSIONS,
        };
        let mut page = 1_u32;
        let mut versions = Vec::new();
        loop {
            let page_value = page.to_string();
            let response: UpstreamVersionHistory = self
                .upstream
                .get_json(
                    &[
                        "public",
                        "mod",
                        project_id,
                        "versions",
                        minecraft_version,
                        loader_name(loader),
                        page_value.as_str(),
                    ],
                    &[],
                    CachePolicy::Versions,
                )
                .await
                .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
            ensure_success(response.status.as_deref())?;
            let pages = response
                .pages
                .as_ref()
                .and_then(FlexibleId::as_u32)
                .unwrap_or(1);
            versions.extend(
                response
                    .versions
                    .into_iter()
                    .filter(|version| !version.private.unwrap_or(false))
                    .filter(|version| summary_matches(version, &query)),
            );
            if versions.len() >= MAX_LISTED_MOD_VERSIONS
                || page >= pages
                || page >= MAX_DEPENDENCY_VERSION_PAGES
            {
                break;
            }
            page = page.saturating_add(1);
        }
        versions.sort_by_key(|version| std::cmp::Reverse(version.updated));
        Ok(versions
            .into_iter()
            .take(MAX_LISTED_MOD_VERSIONS)
            .map(|version| ModVersionSummary {
                id: version.id.to_string_value(),
                name: bounded_text(&version.name, 160),
            })
            .collect())
    }

    async fn list_modrinth_mod_versions(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<Vec<ModVersionSummary>, ProviderError> {
        let game_versions = serde_json::to_string(&[minecraft_version])
            .map_err(|_| ProviderError::InvalidResponse)?;
        let loaders = serde_json::to_string(&[loader_name(loader)])
            .map_err(|_| ProviderError::InvalidResponse)?;
        let mut versions: Vec<ModrinthVersion> = self
            .upstream
            .get_modrinth_json(
                &["project", project_id, "version"],
                &[
                    ("game_versions", game_versions.as_str()),
                    ("loaders", loaders.as_str()),
                ],
                CachePolicy::Versions,
            )
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
        versions.retain(|version| modrinth_version_matches(version, minecraft_version, loader));
        versions.sort_by(|left, right| right.date_published.cmp(&left.date_published));
        Ok(versions
            .into_iter()
            .take(MAX_LISTED_MOD_VERSIONS)
            .map(|version| ModVersionSummary {
                id: version.id,
                name: bounded_text(&version.name, 160),
            })
            .collect())
    }

    async fn resolve_curseforge_mod_graph(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
        version_id: Option<&str>,
    ) -> Result<Vec<ResolvedMod>, ProviderError> {
        let mut queue =
            VecDeque::from([(project_id.to_owned(), version_id.map(str::to_owned), true)]);
        let mut resolved_versions = BTreeMap::new();
        let mut resolved = Vec::new();
        while let Some((dependency_project_id, version_id, root)) = queue.pop_front() {
            let version = self
                .select_curseforge_mod_version(
                    &dependency_project_id,
                    version_id.as_deref(),
                    minecraft_version,
                    loader,
                )
                .await?;
            let selected_version_id = version.id.to_string_value();
            if let Some(existing) = resolved_versions.get(&dependency_project_id) {
                if existing != &selected_version_id {
                    return Err(ProviderError::InvalidResponse);
                }
                continue;
            }
            resolved_versions.insert(dependency_project_id.clone(), selected_version_id);
            if resolved_versions.len() > MAX_MOD_DEPENDENCIES {
                return Err(ProviderError::InvalidResponse);
            }
            for dependency in &version.dependencies {
                if dependency.required {
                    queue.push_back((
                        dependency.id.to_string_value(),
                        dependency.file.as_ref().map(FlexibleId::to_string_value),
                        false,
                    ));
                }
            }
            let identity = if root {
                project_id
            } else {
                dependency_project_id.as_str()
            };
            resolved.push(self.map_curseforge_mod_version(version, identity).await?);
        }
        Ok(resolved)
    }

    async fn select_curseforge_mod_version(
        &self,
        project_id: &str,
        version_id: Option<&str>,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<UpstreamVersionSummary, ProviderError> {
        let query = VersionQuery {
            minecraft_version: Some(minecraft_version.to_owned()),
            loader: Some(loader),
            release_type: None,
            page: 1,
            limit: 1,
        };
        let mut page = 1_u32;
        loop {
            let page_value = page.to_string();
            let response: UpstreamVersionHistory = self
                .upstream
                .get_json(
                    &[
                        "public",
                        "mod",
                        project_id,
                        "versions",
                        minecraft_version,
                        loader_name(loader),
                        page_value.as_str(),
                    ],
                    &[],
                    CachePolicy::Versions,
                )
                .await
                .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
            ensure_success(response.status.as_deref())?;
            let pages = response
                .pages
                .as_ref()
                .and_then(FlexibleId::as_u32)
                .unwrap_or(1);
            let mut matches = response
                .versions
                .into_iter()
                .filter(|version| !version.private.unwrap_or(false))
                .filter(|version| summary_matches(version, &query));
            if let Some(version_id) = version_id {
                if let Some(version) =
                    matches.find(|version| version.id.to_string_value() == version_id)
                {
                    return Ok(version);
                }
            } else if let Some(version) = matches.max_by_key(|version| version.updated) {
                return Ok(version);
            }
            if page >= pages || page >= MAX_DEPENDENCY_VERSION_PAGES {
                break;
            }
            page = page.saturating_add(1);
        }
        Err(ProviderError::NotFound(MissingResource::Version))
    }

    async fn map_curseforge_mod_version(
        &self,
        version: UpstreamVersionSummary,
        project_id: &str,
    ) -> Result<ResolvedMod, ProviderError> {
        let version_id = version.id.to_string_value();
        let url = version
            .url
            .as_deref()
            .into_iter()
            .chain(version.mirrors.iter().flatten().map(String::as_str))
            .find_map(safe_mod_artifact_url)
            .ok_or(ProviderError::DownloadUnavailable)?;
        let mut hashes = map_hashes(version.sha1.as_deref(), version.hashes.as_ref());
        let mut size = nonnegative_u64(version.size);
        if !hashes.has_cryptographic_hash() || size == 0 {
            let metadata = self
                .upstream
                .artifact_metadata(&url)
                .await
                .map_err(|error| map_upstream_error(error, MissingResource::Version))?;
            size = metadata.size;
            hashes.sha256 = Some(metadata.sha256.clone());
            hashes.sha512 = Some(metadata.sha512.clone());
        }
        let file_name = safe_jar_file_name(&version.name)
            .or_else(|| file_name_from_url(&url))
            .unwrap_or_else(|| format!("{}-{version_id}.jar", slugify(&version.name)));
        Ok(ResolvedMod {
            project_id: project_id.to_owned(),
            version_id,
            version_name: bounded_text(&version.name, 160),
            file_name,
            url,
            size,
            hashes,
        })
    }

    async fn resolve_modrinth_mod_graph(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
        version_id: Option<&str>,
    ) -> Result<Vec<ResolvedMod>, ProviderError> {
        let mut queue = VecDeque::from([(
            Some(project_id.to_owned()),
            version_id.map(str::to_owned),
            true,
        )]);
        let mut resolved_versions = BTreeMap::new();
        let mut resolved = Vec::new();
        while let Some((requested_project_id, version_id, root)) = queue.pop_front() {
            let version = self
                .select_modrinth_version(
                    requested_project_id.as_deref().unwrap_or_default(),
                    version_id.as_deref(),
                    minecraft_version,
                    loader,
                )
                .await?;
            if let Some(existing) = resolved_versions.get(&version.project_id) {
                if existing != &version.id {
                    return Err(ProviderError::InvalidResponse);
                }
                continue;
            }
            resolved_versions.insert(version.project_id.clone(), version.id.clone());
            if resolved_versions.len() > MAX_MOD_DEPENDENCIES {
                return Err(ProviderError::InvalidResponse);
            }
            for dependency in &version.dependencies {
                if dependency.dependency_type == "required" {
                    queue.push_back((
                        dependency.project_id.clone(),
                        dependency.version_id.clone(),
                        false,
                    ));
                }
            }
            let identity = if root {
                project_id.to_owned()
            } else {
                version.project_id.clone()
            };
            resolved.push(self.map_modrinth_version(version, &identity).await?);
        }
        Ok(resolved)
    }

    async fn select_modrinth_version(
        &self,
        project_id: &str,
        version_id: Option<&str>,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<ModrinthVersion, ProviderError> {
        if let Some(version_id) = version_id {
            let version: ModrinthVersion = self
                .upstream
                .get_modrinth_json(&["version", version_id], &[], CachePolicy::Version)
                .await
                .map_err(|error| map_upstream_error(error, MissingResource::Version))?;
            if (!project_id.is_empty() && version.project_id != project_id)
                || !modrinth_version_matches(&version, minecraft_version, loader)
            {
                return Err(ProviderError::InvalidResponse);
            }
            return Ok(version);
        }
        if project_id.is_empty() {
            return Err(ProviderError::InvalidResponse);
        }
        let game_versions = serde_json::to_string(&[minecraft_version])
            .map_err(|_| ProviderError::InvalidResponse)?;
        let loaders = serde_json::to_string(&[loader_name(loader)])
            .map_err(|_| ProviderError::InvalidResponse)?;
        let versions: Vec<ModrinthVersion> = self
            .upstream
            .get_modrinth_json(
                &["project", project_id, "version"],
                &[
                    ("game_versions", game_versions.as_str()),
                    ("loaders", loaders.as_str()),
                ],
                CachePolicy::Versions,
            )
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
        versions
            .into_iter()
            .filter(|version| modrinth_version_matches(version, minecraft_version, loader))
            .max_by(|left, right| left.date_published.cmp(&right.date_published))
            .ok_or(ProviderError::NotFound(MissingResource::Version))
    }

    async fn map_modrinth_version(
        &self,
        version: ModrinthVersion,
        project_id: &str,
    ) -> Result<ResolvedMod, ProviderError> {
        let file = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| version.files.first())
            .ok_or(ProviderError::DownloadUnavailable)?;
        let url = safe_mod_artifact_url(&file.url).ok_or(ProviderError::DownloadUnavailable)?;
        let mut hashes = Hashes {
            sha512: valid_hash(file.hashes.sha512.as_deref(), 128),
            sha256: valid_hash(file.hashes.sha256.as_deref(), 64),
            sha1: valid_hash(file.hashes.sha1.as_deref(), 40),
        };
        let mut size = file.size;
        if !hashes.has_cryptographic_hash() || size == 0 {
            let metadata = self
                .upstream
                .artifact_metadata(&url)
                .await
                .map_err(|error| map_upstream_error(error, MissingResource::Version))?;
            size = metadata.size;
            hashes.sha256 = Some(metadata.sha256.clone());
            hashes.sha512 = Some(metadata.sha512.clone());
        }
        let file_name = safe_jar_file_name(&file.filename)
            .unwrap_or_else(|| format!("{}-{}.jar", slugify(&version.version_number), version.id));
        Ok(ResolvedMod {
            project_id: project_id.to_owned(),
            version_id: version.id,
            version_name: bounded_text(&version.name, 160),
            file_name,
            url,
            size,
            hashes,
        })
    }

    async fn search_ftb(&self, request: SearchRequest) -> Result<SearchPage, ProviderError> {
        if request.page > 1 {
            return Ok(SearchPage {
                items: Vec::new(),
                page: request.page,
                pages: 1,
            });
        }
        let limit = request.limit.clamp(1, 50).to_string();
        let term = request.query.as_deref().unwrap_or("ftb");
        let response: UpstreamBrowse = self
            .upstream
            .get_json(
                &["public", "modpack", "search", &limit, "detailed"],
                &[("term", term)],
                CachePolicy::Search,
            )
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
        let mut items = response
            .packs
            .into_iter()
            .filter(|card| provider_matches(card, Provider::Ftb))
            .map(|card| map_search_card(card, Provider::Ftb, None))
            .filter(|item| search_item_matches(item, &request))
            .collect::<Vec<_>>();
        items.truncate(request.limit);
        Ok(SearchPage {
            items,
            page: 1,
            pages: 1,
        })
    }

    pub async fn get_project(&self, project_id: &str) -> Result<Modpack, ProviderError> {
        validate_identifier(project_id)?;
        let provider = self.provider.as_str();
        let response: UpstreamProject = self
            .upstream
            .get_json(&["public", provider, project_id], &[], CachePolicy::Project)
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
        ensure_success(response.status.as_deref())?;
        map_project(response, self.provider)
    }

    pub async fn list_versions(
        &self,
        project_id: &str,
        request: VersionQuery,
    ) -> Result<Vec<ModpackVersionSummary>, ProviderError> {
        validate_identifier(project_id)?;
        let mut filters = Vec::new();
        if let Some(version) = &request.minecraft_version {
            filters.push(version.clone());
        }
        if let Some(loader) = request.loader {
            filters.push(loader_name(loader).to_owned());
        }
        if filters.is_empty() {
            filters.push("all".to_owned());
        }
        let page = request.page.to_string();
        let mut path = vec![
            "public".to_owned(),
            self.provider.as_str().to_owned(),
            project_id.to_owned(),
            "versions".to_owned(),
            filters[0].clone(),
        ];
        if let Some(second) = filters.get(1) {
            path.push(second.clone());
        }
        path.push(page);
        let path_refs = path.iter().map(String::as_str).collect::<Vec<_>>();
        let response: UpstreamVersionHistory = self
            .upstream
            .get_json(&path_refs, &[], CachePolicy::Versions)
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
        ensure_success(response.status.as_deref())?;

        response
            .versions
            .into_iter()
            .filter(|version| summary_matches(version, &request))
            .take(request.limit)
            .map(map_version_summary)
            .collect()
    }

    pub async fn get_version(
        &self,
        project_id: &str,
        version_id: &str,
    ) -> Result<ModpackVersion, ProviderError> {
        validate_identifier(project_id)?;
        validate_identifier(version_id)?;
        let response: UpstreamVersion = self
            .upstream
            .get_json(
                &["public", self.provider.as_str(), project_id, version_id],
                &[],
                CachePolicy::Version,
            )
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Version))?;
        ensure_success(response.status.as_deref())?;
        map_version(response, self.provider, project_id)
    }

    pub async fn get_install_version(
        &self,
        project_id: &str,
        version_id: &str,
    ) -> Result<ModpackVersion, ProviderError> {
        let version = self.get_version(project_id, version_id).await?;
        resolve_install_metadata(version, &self.upstream).await
    }

    pub async fn categories(&self) -> Result<Vec<CategorySummary>, ProviderError> {
        let path = if self.provider == Provider::Ftb {
            vec!["public", "modpack", "tags"]
        } else {
            vec!["public", self.provider.as_str(), "tags"]
        };
        let response: UpstreamTags = self
            .upstream
            .get_json(&path, &[], CachePolicy::Categories)
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
        ensure_success(response.status.as_deref())?;
        Ok(response
            .tags
            .into_iter()
            .filter(|tag| !looks_like_minecraft_version(&tag.name))
            .map(|tag| CategorySummary {
                id: slugify(&tag.name),
                name: bounded_text(&tag.name, 80),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FlexibleId, ProviderError, UpstreamFile, UpstreamTarget, UpstreamVersion,
        UpstreamVersionHistory, UpstreamVersionSummary, map_file, map_version, map_version_summary,
    };
    use slate_modpack_api_contracts::{LoaderKind, Provider, ProviderReference, ReleaseType};

    #[test]
    fn version_lists_are_mapped_without_resolving_full_manifests() -> Result<(), ProviderError> {
        let summary = map_version_summary(UpstreamVersionSummary {
            id: FlexibleId::Unsigned(8_764_211),
            name: "All the Mods 10-8.1".to_owned(),
            release_type: "release".to_owned(),
            updated: 1_788_034_505,
            targets: vec![
                UpstreamTarget {
                    name: "minecraft".to_owned(),
                    version: "1.21.1".to_owned(),
                    kind: "game".to_owned(),
                },
                UpstreamTarget {
                    name: "neoforge".to_owned(),
                    version: "neoforge".to_owned(),
                    kind: "modloader".to_owned(),
                },
            ],
            private: Some(false),
            url: None,
            mirrors: None,
            sha1: None,
            hashes: None,
            size: None,
            dependencies: Vec::new(),
        })?;

        assert_eq!(summary.id, "8764211");
        assert_eq!(summary.minecraft_version, "1.21.1");
        assert_eq!(summary.loader.kind, LoaderKind::NeoForge);
        assert_eq!(summary.loader.version, None);
        assert_eq!(summary.release_type, ReleaseType::Release);
        assert_eq!(summary.changelog, None);
        Ok(())
    }

    #[test]
    fn curseforge_mod_versions_preserve_required_dependencies()
    -> Result<(), Box<dyn std::error::Error>> {
        let history: UpstreamVersionHistory = serde_json::from_value(serde_json::json!({
            "status": "success",
            "page": 1,
            "pages": 2,
            "versions": [{
                "id": 8929207,
                "name": "jei-1.21.1-neoforge.jar",
                "type": "release",
                "updated": 1789911066,
                "url": "https://edge.forgecdn.net/files/8929/207/jei.jar",
                "sha1": "0df1c73ad0f8fd8e62b0e7b8a2f6b826bbbecc8f",
                "size": 2202704,
                "targets": [
                    { "name": "minecraft", "type": "game", "version": "1.21.1" },
                    { "name": "neoforge", "type": "modloader", "version": "neoforge" }
                ],
                "dependencies": [
                    { "id": 1689768, "required": true },
                    { "id": 1700987, "required": false }
                ]
            }]
        }))?;

        assert_eq!(history.pages.and_then(|pages| pages.as_u32()), Some(2));
        assert_eq!(history.versions.len(), 1);
        assert_eq!(history.versions[0].dependencies.len(), 2);
        assert!(history.versions[0].dependencies[0].required);
        assert!(!history.versions[0].dependencies[1].required);
        assert_eq!(
            history.versions[0].dependencies[0].id.to_string_value(),
            "1689768"
        );
        Ok(())
    }

    #[test]
    fn modrinth_pack_files_preserve_the_mod_project_identity() -> Result<(), ProviderError> {
        let file = map_file(
            UpstreamFile {
                id: FlexibleId::String("AANobbMI".to_owned()),
                name: "example.jar".to_owned(),
                kind: Some("mod".to_owned()),
                version: Some(FlexibleId::String("version-id".to_owned())),
                path: Some("mods".to_owned()),
                url: Some("https://cdn.modrinth.com/data/example.jar".to_owned()),
                mirrors: None,
                sha1: Some("1111111111111111111111111111111111111111".to_owned()),
                hashes: None,
                size: Some(42),
                client_only: None,
                server_only: None,
                optional: None,
                curseforge: None,
            },
            Provider::Modrinth,
            "parent-pack",
        )?;

        assert_eq!(
            file.source,
            Some(ProviderReference {
                provider: Provider::Modrinth,
                project_id: "AANobbMI".to_owned(),
                version_id: Some("version-id".to_owned()),
            })
        );
        Ok(())
    }

    #[test]
    fn version_details_defer_missing_hash_resolution_until_install() -> Result<(), ProviderError> {
        let version = map_version(
            UpstreamVersion {
                status: Some("success".to_owned()),
                id: FlexibleId::Unsigned(8_846_424),
                name: "FTB StoneBlock 4 1.21.0".to_owned(),
                release_type: "release".to_owned(),
                updated: 1_788_982_062,
                released: 1_788_982_062,
                targets: vec![
                    UpstreamTarget {
                        name: "minecraft".to_owned(),
                        version: "1.21.1".to_owned(),
                        kind: "game".to_owned(),
                    },
                    UpstreamTarget {
                        name: "neoforge".to_owned(),
                        version: "21.1.248".to_owned(),
                        kind: "modloader".to_owned(),
                    },
                ],
                specs: serde_json::json!({ "recommended": 4096 }),
                files: vec![UpstreamFile {
                    id: FlexibleId::Unsigned(8_846_424),
                    name: "overrides.zip".to_owned(),
                    kind: Some("archive".to_owned()),
                    version: Some(FlexibleId::Unsigned(8_846_424)),
                    path: Some(".".to_owned()),
                    url: Some(
                        "https://edge.forgecdn.net/files/8846/424/ftb-stoneblock-4.zip".to_owned(),
                    ),
                    mirrors: None,
                    sha1: None,
                    hashes: None,
                    size: Some(-1),
                    client_only: None,
                    server_only: None,
                    optional: None,
                    curseforge: None,
                }],
                changelog: None,
            },
            Provider::CurseForge,
            "1373378",
        )?;

        assert_eq!(version.files.len(), 1);
        assert!(!version.files[0].hashes.has_cryptographic_hash());
        assert_eq!(version.files[0].size, 0);
        assert_eq!(version.total_download_size, 0);
        Ok(())
    }
}
