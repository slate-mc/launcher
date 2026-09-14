use super::{MissingResource, ProviderError, ResolvedMod};
use crate::domain::{SearchPage, SearchRequest, SearchSort, VersionQuery, normalize_install_path};
use crate::upstream::{CachePolicy, UpstreamClient, UpstreamError};
use futures_util::stream::{self, StreamExt, TryStreamExt};
use serde::Deserialize;
use serde_json::Value;
use slate_modpack_api_contracts::{
    Author, CategorySummary, DownloadSource, FileOption, Hashes, Loader, LoaderKind, Modpack,
    ModpackFile, ModpackLinks, ModpackSummary, ModpackVersion, ModpackVersionSummary, PackFileType,
    Provider, ProviderReference, ReleaseType, Side, VersionReference,
};
use std::collections::BTreeSet;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use url::{Host, Url};

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

    pub async fn resolve_mod(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<ResolvedMod, ProviderError> {
        validate_identifier(project_id)?;
        if minecraft_version.is_empty() || minecraft_version.len() > 32 {
            return Err(ProviderError::InvalidIdentifier);
        }
        if loader == LoaderKind::Vanilla {
            return Err(ProviderError::UnsupportedLoader);
        }
        match self.provider {
            Provider::CurseForge => {
                self.resolve_curseforge_mod(project_id, minecraft_version, loader)
                    .await
            }
            Provider::Modrinth => {
                self.resolve_modrinth_mod(project_id, minecraft_version, loader)
                    .await
            }
            Provider::Ftb => Err(ProviderError::UnsupportedContent),
        }
    }

    async fn resolve_curseforge_mod(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<ResolvedMod, ProviderError> {
        let response: UpstreamVersionHistory = self
            .upstream
            .get_json(
                &[
                    "public",
                    "curseforge",
                    project_id,
                    "versions",
                    minecraft_version,
                    loader_name(loader),
                    "1",
                ],
                &[],
                CachePolicy::Versions,
            )
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Project))?;
        ensure_success(response.status.as_deref())?;
        let query = VersionQuery {
            minecraft_version: Some(minecraft_version.to_owned()),
            loader: Some(loader),
            release_type: None,
            page: 1,
            limit: 1,
        };
        let version = response
            .versions
            .into_iter()
            .filter(|version| !version.private.unwrap_or(false))
            .filter(|version| summary_matches(version, &query))
            .max_by_key(|version| version.updated)
            .ok_or(ProviderError::NotFound(MissingResource::Version))?;
        let version_id = version.id.to_string_value();
        let artifact: UpstreamModArtifact = self
            .upstream
            .get_json(
                &["public", "curseforge", project_id, &version_id],
                &[],
                CachePolicy::Version,
            )
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Version))?;
        // modpacks.ch identifies a standalone jar as a non-pack error while still
        // returning its authoritative manifestUrl. That URL is the useful payload.
        let url = artifact
            .manifest_url
            .as_deref()
            .and_then(safe_mod_artifact_url)
            .ok_or(ProviderError::DownloadUnavailable)?;
        let metadata = self
            .upstream
            .artifact_metadata(&url)
            .await
            .map_err(|error| map_upstream_error(error, MissingResource::Version))?;
        let file_name = file_name_from_url(&url)
            .unwrap_or_else(|| format!("{}-{version_id}.jar", slugify(&version.name)));
        Ok(ResolvedMod {
            project_id: project_id.to_owned(),
            version_id,
            version_name: bounded_text(&version.name, 160),
            file_name,
            url,
            size: metadata.size,
            hashes: Hashes {
                sha512: Some(metadata.sha512.clone()),
                sha256: Some(metadata.sha256.clone()),
                sha1: None,
            },
        })
    }

    async fn resolve_modrinth_mod(
        &self,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<ResolvedMod, ProviderError> {
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
        let version = versions
            .into_iter()
            .filter(|version| {
                version
                    .game_versions
                    .iter()
                    .any(|value| value == minecraft_version)
            })
            .filter(|version| {
                version
                    .loaders
                    .iter()
                    .any(|value| parse_loader(value) == Some(loader))
            })
            .max_by(|left, right| left.date_published.cmp(&right.date_published))
            .ok_or(ProviderError::NotFound(MissingResource::Version))?;
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

        let candidates = response
            .versions
            .into_iter()
            .filter(|version| summary_matches(version, &request))
            .take(request.limit)
            .collect::<Vec<_>>();
        stream::iter(candidates)
            .map(|candidate| {
                let adapter = self.clone();
                let project_id = project_id.to_owned();
                async move {
                    adapter
                        .get_version(&project_id, &candidate.id.to_string_value())
                        .await
                        .map(|version| ModpackVersionSummary {
                            id: version.id,
                            name: version.name,
                            release_type: version.release_type,
                            minecraft_version: version.minecraft.version,
                            loader: version.loader,
                            published_at: version.published_at,
                            changelog: version.changelog,
                        })
                }
            })
            .buffer_unordered(8)
            .try_collect()
            .await
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
        map_version(response, self.provider, project_id, &self.upstream).await
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

fn map_search_card(
    card: UpstreamBrowseCard,
    provider: Provider,
    requested_loader: Option<LoaderKind>,
) -> ModpackSummary {
    let tags = card.tags.unwrap_or_default();
    let minecraft_versions = unique_sorted(
        tags.iter()
            .filter(|tag| looks_like_minecraft_version(&tag.name))
            .map(|tag| tag.name.clone()),
    );
    let mut loaders = unique_sorted(
        tags.iter()
            .filter_map(|tag| parse_loader(&tag.name))
            .collect::<Vec<_>>(),
    );
    if loaders.is_empty()
        && let Some(loader) = requested_loader
    {
        loaders.push(loader);
    }
    let categories = unique_sorted(
        tags.iter()
            .filter(|tag| {
                !looks_like_minecraft_version(&tag.name) && parse_loader(&tag.name).is_none()
            })
            .map(|tag| tag.name.to_ascii_lowercase()),
    );
    let name = bounded_text(&card.name, 160);
    ModpackSummary {
        provider,
        id: card.id.to_string_value(),
        slug: slugify(&name),
        name,
        summary: bounded_text(card.synopsis.as_deref().unwrap_or_default(), 500),
        authors: map_authors(card.authors.unwrap_or_default()),
        icon_url: select_art(&card.art.unwrap_or_default(), &["square", "icon"]),
        downloads: nonnegative_u64(card.installs),
        updated_at: timestamp(card.updated),
        minecraft_versions,
        loaders,
        categories,
    }
}

fn map_project(project: UpstreamProject, provider: Provider) -> Result<Modpack, ProviderError> {
    let versions = project.versions.unwrap_or_default();
    let minecraft_versions = unique_sorted(versions.iter().filter_map(|version| {
        version
            .targets
            .iter()
            .find(|target| target.kind == "game" || target.name.eq_ignore_ascii_case("minecraft"))
            .map(|target| target.version.clone())
    }));
    let loaders = unique_sorted(
        versions
            .iter()
            .filter_map(|version| {
                loader_from_targets(&version.targets)
                    .ok()
                    .map(|value| value.kind)
            })
            .filter(|loader| *loader != LoaderKind::Vanilla),
    );
    let latest_version = versions
        .iter()
        .filter(|version| !version.private.unwrap_or(false))
        .max_by_key(|version| version.updated)
        .map(|version| VersionReference {
            id: version.id.to_string_value(),
            name: bounded_text(&version.name, 160),
        });
    let tags = project.tags.unwrap_or_default();
    let categories = unique_sorted(
        tags.into_iter()
            .filter(|tag| {
                !looks_like_minecraft_version(&tag.name) && parse_loader(&tag.name).is_none()
            })
            .map(|tag| tag.name.to_ascii_lowercase()),
    );
    let links = map_links(project.links.unwrap_or_default());
    let name = bounded_text(&project.name, 160);
    Ok(Modpack {
        provider,
        id: project.id.to_string_value(),
        slug: slug_from_links(&links).unwrap_or_else(|| slugify(&name)),
        name,
        summary: bounded_text(project.synopsis.as_deref().unwrap_or_default(), 500),
        description: bounded_text(project.description.as_deref().unwrap_or_default(), 100_000),
        authors: map_authors(project.authors.unwrap_or_default()),
        icon_url: select_art(
            &project.art.clone().unwrap_or_default(),
            &["square", "icon"],
        ),
        banner_url: select_art(
            &project.art.unwrap_or_default(),
            &["splash", "banner", "background"],
        ),
        downloads: nonnegative_u64(project.installs),
        categories,
        minecraft_versions,
        loaders,
        links,
        updated_at: timestamp(project.updated),
        latest_version,
    })
}

async fn map_version(
    version: UpstreamVersion,
    provider: Provider,
    project_id: &str,
    upstream: &UpstreamClient,
) -> Result<ModpackVersion, ProviderError> {
    let minecraft_version = version
        .targets
        .iter()
        .find(|target| target.kind == "game" || target.name.eq_ignore_ascii_case("minecraft"))
        .map(|target| target.version.clone())
        .ok_or(ProviderError::InvalidResponse)?;
    let loader = loader_from_targets(&version.targets)?;
    let memory = memory_from_value(&version.specs);
    let mut total_download_size = 0_u64;
    let mut files = Vec::with_capacity(version.files.len());
    for file in version.files {
        let mut mapped = map_file(file, provider, project_id)?;
        if !mapped.hashes.has_cryptographic_hash() {
            let DownloadSource::Direct { url } = &mapped.download else {
                return Err(ProviderError::DownloadUnavailable);
            };
            let metadata = upstream
                .artifact_metadata(url)
                .await
                .map_err(|_| ProviderError::DownloadUnavailable)?;
            mapped.size = metadata.size;
            mapped.hashes.sha256 = Some(metadata.sha256.clone());
            mapped.hashes.sha512 = Some(metadata.sha512.clone());
        }
        total_download_size = total_download_size
            .checked_add(mapped.size)
            .ok_or(ProviderError::InvalidResponse)?;
        files.push(mapped);
    }
    let published = if version.released > 0 {
        version.released
    } else {
        version.updated
    };
    Ok(ModpackVersion {
        provider,
        project_id: project_id.to_owned(),
        id: version.id.to_string_value(),
        name: bounded_text(&version.name, 160),
        release_type: release_type(&version.release_type),
        minecraft: slate_modpack_api_contracts::MinecraftTarget {
            version: minecraft_version,
        },
        loader,
        memory,
        files,
        total_download_size,
        changelog: version
            .changelog
            .filter(|value| !value.trim().is_empty() && !value.trim().starts_with("http"))
            .map(|value| bounded_text(&value, 100_000)),
        published_at: timestamp(published),
    })
}

fn map_file(
    file: UpstreamFile,
    provider: Provider,
    project_id: &str,
) -> Result<ModpackFile, ProviderError> {
    let path = normalize_install_path(&format!(
        "{}/{}",
        file.path.as_deref().unwrap_or_default(),
        file.name
    ))
    .map_err(|_| ProviderError::InvalidInstallPath)?;
    let hashes = map_hashes(file.sha1.as_deref(), file.hashes.as_ref());
    let url = std::iter::once(file.url.as_deref())
        .chain(file.mirrors.iter().flatten().map(String::as_str).map(Some))
        .flatten()
        .find_map(safe_download_url)
        .ok_or(ProviderError::DownloadUnavailable)?;
    let side = if file.server_only.unwrap_or(false) {
        Side::Server
    } else if file.client_only.unwrap_or(false) {
        Side::Client
    } else {
        Side::Both
    };
    let kind = file_type(file.kind.as_deref().unwrap_or_default());
    let size = file.size.unwrap_or(0).max(0).unsigned_abs();
    let raw_id = file.id.to_string_value();
    let id = if raw_id == "0" || raw_id.is_empty() {
        hashes
            .sha1
            .clone()
            .unwrap_or_else(|| format!("path-{}", slugify(&path)))
    } else {
        raw_id
    };
    let option = file.optional.unwrap_or(false).then(|| FileOption {
        id: format!("optional-{}", slugify(&id)),
        name: bounded_text(&file.name, 160),
        default: false,
    });
    let source = file.curseforge.map_or_else(
        || {
            Some(ProviderReference {
                provider,
                project_id: project_id.to_owned(),
                version_id: file.version.map(|value| value.to_string_value()),
            })
        },
        |reference| {
            Some(ProviderReference {
                provider: Provider::CurseForge,
                project_id: reference.project.to_string_value(),
                version_id: Some(reference.file.to_string_value()),
            })
        },
    );
    Ok(ModpackFile {
        id,
        kind,
        path,
        size,
        hashes,
        side,
        optional: file.optional.unwrap_or(false),
        option,
        download: DownloadSource::Direct { url },
        source,
    })
}

fn loader_from_targets(targets: &[UpstreamTarget]) -> Result<Loader, ProviderError> {
    let mut candidates = targets.iter().filter(|target| {
        target.kind == "modloader"
            || parse_loader(&target.name).is_some()
            || parse_loader(&target.version).is_some()
    });
    if let Some(target) = candidates.next() {
        let kind = parse_loader(&target.name)
            .or_else(|| parse_loader(&target.version))
            .ok_or(ProviderError::UnsupportedLoader)?;
        let version = (!target.version.eq_ignore_ascii_case(loader_name(kind)))
            .then(|| target.version.clone());
        return Ok(Loader { kind, version });
    }
    Ok(Loader {
        kind: LoaderKind::Vanilla,
        version: None,
    })
}

fn summary_matches(version: &UpstreamVersionSummary, request: &VersionQuery) -> bool {
    if request
        .release_type
        .is_some_and(|expected| release_type(&version.release_type) != expected)
    {
        return false;
    }
    if request.minecraft_version.as_ref().is_some_and(|expected| {
        !version.targets.iter().any(|target| {
            (target.kind == "game" || target.name.eq_ignore_ascii_case("minecraft"))
                && target.version == *expected
        })
    }) {
        return false;
    }
    if request.loader.is_some_and(|expected| {
        loader_from_targets(&version.targets).map_or(true, |loader| loader.kind != expected)
    }) {
        return false;
    }
    true
}

fn search_item_matches(item: &ModpackSummary, request: &SearchRequest) -> bool {
    if request.minecraft_version.as_ref().is_some_and(|version| {
        !item.minecraft_versions.is_empty() && !item.minecraft_versions.contains(version)
    }) {
        return false;
    }
    if request
        .loader
        .is_some_and(|loader| !item.loaders.contains(&loader))
    {
        return false;
    }
    if request.category.as_ref().is_some_and(|category| {
        !item
            .categories
            .iter()
            .any(|value| value.eq_ignore_ascii_case(category))
    }) {
        return false;
    }
    true
}

fn map_hashes(sha1: Option<&str>, hashes: Option<&UpstreamHashes>) -> Hashes {
    Hashes {
        sha512: hashes.and_then(|value| valid_hash(value.sha512.as_deref(), 128)),
        sha256: hashes.and_then(|value| valid_hash(value.sha256.as_deref(), 64)),
        sha1: hashes
            .and_then(|value| valid_hash(value.sha1.as_deref(), 40))
            .or_else(|| valid_hash(sha1, 40)),
    }
}

fn valid_hash(value: Option<&str>, length: usize) -> Option<String> {
    value
        .filter(|hash| hash.len() == length && hash.chars().all(|value| value.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}

fn memory_from_value(value: &Value) -> slate_modpack_api_contracts::MemoryRecommendation {
    let minimum = value
        .get("minimum")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(2_048);
    let recommended = value
        .get("recommended")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(4_096)
        .max(minimum);
    slate_modpack_api_contracts::MemoryRecommendation {
        minimum_mb: minimum,
        recommended_mb: recommended,
    }
}

fn map_links(links: Vec<UpstreamLink>) -> ModpackLinks {
    let mut result = ModpackLinks::default();
    for link in links {
        let Some(url) = safe_external_url(&link.link) else {
            continue;
        };
        match link.kind.to_ascii_lowercase().as_str() {
            "website" | "modrinth" => result.website.get_or_insert(url),
            "source" => result.source.get_or_insert(url),
            "issues" => result.issues.get_or_insert(url),
            _ => continue,
        };
    }
    result
}

fn map_authors(authors: Vec<UpstreamAuthor>) -> Vec<Author> {
    authors
        .into_iter()
        .filter(|author| !author.name.trim().is_empty())
        .map(|author| Author {
            name: bounded_text(&author.name, 100),
            url: author.website.as_deref().and_then(safe_external_url),
        })
        .collect()
}

fn select_art(art: &[UpstreamArt], kinds: &[&str]) -> Option<String> {
    kinds.iter().find_map(|kind| {
        art.iter()
            .find(|art| art.kind.eq_ignore_ascii_case(kind))
            .and_then(|art| safe_external_url(&art.url))
    })
}

fn slug_from_links(links: &ModpackLinks) -> Option<String> {
    links.website.as_ref().and_then(|value| {
        Url::parse(value)
            .ok()?
            .path_segments()?
            .rfind(|segment| !segment.is_empty())
            .map(slugify)
    })
}

fn safe_external_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    Some(url.to_string())
}

fn safe_download_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    match url.host()? {
        Host::Domain(domain)
            if domain.eq_ignore_ascii_case("localhost")
                || domain.ends_with(".localhost")
                || domain.ends_with(".local") =>
        {
            None
        }
        Host::Ipv4(address)
            if address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_unspecified() =>
        {
            None
        }
        Host::Ipv6(address)
            if address.is_loopback()
                || address.is_unspecified()
                || (address.segments()[0] & 0xfe00) == 0xfc00
                || (address.segments()[0] & 0xffc0) == 0xfe80 =>
        {
            None
        }
        _ => Some(url.to_string()),
    }
}

fn safe_mod_artifact_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let Host::Domain(domain) = url.host()? else {
        return None;
    };
    let domain = domain.to_ascii_lowercase();
    if domain == "cdn.modrinth.com"
        || domain == "edge.forgecdn.net"
        || domain.ends_with(".forgecdn.net")
    {
        Some(url.to_string())
    } else {
        None
    }
}

fn safe_jar_file_name(value: &str) -> Option<String> {
    if value.is_empty()
        || value.len() > 240
        || !value.to_ascii_lowercase().ends_with(".jar")
        || value.contains(['/', '\\', ':', '\0'])
        || matches!(value, "." | "..")
    {
        None
    } else {
        Some(value.to_owned())
    }
}

fn file_name_from_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    let encoded = url.path_segments()?.next_back()?;
    let decoded = percent_encoding::percent_decode_str(encoded)
        .decode_utf8()
        .ok()?;
    safe_jar_file_name(&decoded)
}

fn provider_matches(card: &UpstreamBrowseCard, provider: Provider) -> bool {
    match provider {
        Provider::Ftb => {
            card.provider
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("ftb"))
                || card
                    .platform
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case("modpacksch"))
        }
        _ => card
            .provider
            .as_deref()
            .is_none_or(|value| value.eq_ignore_ascii_case(provider.as_str())),
    }
}

fn validate_identifier(value: &str) -> Result<(), ProviderError> {
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        Err(ProviderError::InvalidIdentifier)
    } else {
        Ok(())
    }
}

fn ensure_success(status: Option<&str>) -> Result<(), ProviderError> {
    if status.is_some_and(|value| value.eq_ignore_ascii_case("error")) {
        Err(ProviderError::InvalidResponse)
    } else {
        Ok(())
    }
}

fn map_upstream_error(error: UpstreamError, missing: MissingResource) -> ProviderError {
    match error {
        UpstreamError::NotFound => ProviderError::NotFound(missing),
        UpstreamError::RateLimited => ProviderError::RateLimited,
        UpstreamError::Unavailable | UpstreamError::Request(_) => ProviderError::Unavailable,
        _ => ProviderError::InvalidResponse,
    }
}

fn release_type(value: &str) -> ReleaseType {
    match value.to_ascii_lowercase().as_str() {
        "release" | "stable" => ReleaseType::Release,
        "beta" => ReleaseType::Beta,
        "alpha" => ReleaseType::Alpha,
        _ => ReleaseType::Unknown,
    }
}

fn file_type(value: &str) -> PackFileType {
    match value.to_ascii_lowercase().replace('-', "_").as_str() {
        "mod" => PackFileType::Mod,
        "config" => PackFileType::Config,
        "resourcepack" | "resource_pack" => PackFileType::ResourcePack,
        "shaderpack" | "shader_pack" => PackFileType::ShaderPack,
        "datapack" | "data_pack" => PackFileType::DataPack,
        "library" => PackFileType::Library,
        "override" => PackFileType::Override,
        "archive" | "cf_extract" | "mr_extract" => PackFileType::Archive,
        _ => PackFileType::Other,
    }
}

fn parse_loader(value: &str) -> Option<LoaderKind> {
    match value
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "")
        .as_str()
    {
        "vanilla" => Some(LoaderKind::Vanilla),
        "forge" => Some(LoaderKind::Forge),
        "neoforge" => Some(LoaderKind::NeoForge),
        "fabric" | "fabricloader" => Some(LoaderKind::Fabric),
        "quilt" | "quiltloader" => Some(LoaderKind::Quilt),
        _ => None,
    }
}

fn loader_name(loader: LoaderKind) -> &'static str {
    match loader {
        LoaderKind::Vanilla => "vanilla",
        LoaderKind::Forge => "forge",
        LoaderKind::NeoForge => "neoforge",
        LoaderKind::Fabric => "fabric",
        LoaderKind::Quilt => "quilt",
    }
}

fn upstream_sort(sort: SearchSort) -> &'static str {
    match sort {
        SearchSort::Relevance | SearchSort::Downloads => "popular",
        SearchSort::Updated => "updated",
        SearchSort::Newest => "new",
    }
}

fn looks_like_minecraft_version(value: &str) -> bool {
    let mut parts = value.split('.');
    parts
        .next()
        .is_some_and(|part| !part.is_empty() && part.chars().all(|value| value.is_ascii_digit()))
        && parts.next().is_some_and(|part| {
            !part.is_empty() && part.chars().all(|value| value.is_ascii_digit())
        })
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    slug.trim_matches('-').to_owned()
}

fn bounded_text(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn nonnegative_u64(value: Option<i64>) -> u64 {
    value.unwrap_or(0).max(0).unsigned_abs()
}

fn timestamp(value: i64) -> String {
    OffsetDateTime::from_unix_timestamp(value)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_owned())
}

fn unique_sorted<T: Ord>(values: impl IntoIterator<Item = T>) -> Vec<T> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum FlexibleId {
    String(String),
    Signed(i64),
    Unsigned(u64),
}

impl Default for FlexibleId {
    fn default() -> Self {
        Self::String(String::new())
    }
}

impl FlexibleId {
    fn to_string_value(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Signed(value) => value.to_string(),
            Self::Unsigned(value) => value.to_string(),
        }
    }

    fn as_u32(&self) -> Option<u32> {
        self.to_string_value().parse().ok()
    }
}

#[derive(Debug, Deserialize)]
struct UpstreamBrowse {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    packs: Vec<UpstreamBrowseCard>,
    #[serde(default)]
    mods: Vec<UpstreamBrowseCard>,
    #[serde(default)]
    page: Option<FlexibleId>,
    #[serde(default)]
    pages: Option<FlexibleId>,
}

#[derive(Debug, Deserialize)]
struct UpstreamModArtifact {
    #[serde(rename = "manifestUrl", default)]
    manifest_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    id: String,
    name: String,
    version_number: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    date_published: String,
    #[serde(default)]
    files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: ModrinthHashes,
}

#[derive(Debug, Default, Deserialize)]
struct ModrinthHashes {
    #[serde(default)]
    sha512: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    sha1: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamBrowseCard {
    id: FlexibleId,
    name: String,
    #[serde(default)]
    synopsis: Option<String>,
    #[serde(default)]
    updated: i64,
    #[serde(default)]
    art: Option<Vec<UpstreamArt>>,
    #[serde(default)]
    authors: Option<Vec<UpstreamAuthor>>,
    #[serde(default)]
    tags: Option<Vec<UpstreamTag>>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    installs: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UpstreamProject {
    #[serde(default)]
    status: Option<String>,
    id: FlexibleId,
    name: String,
    #[serde(default)]
    synopsis: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    updated: i64,
    #[serde(default)]
    art: Option<Vec<UpstreamArt>>,
    #[serde(default)]
    links: Option<Vec<UpstreamLink>>,
    #[serde(default)]
    authors: Option<Vec<UpstreamAuthor>>,
    #[serde(default)]
    versions: Option<Vec<UpstreamVersionSummary>>,
    #[serde(default)]
    installs: Option<i64>,
    #[serde(default)]
    tags: Option<Vec<UpstreamTag>>,
}

#[derive(Debug, Deserialize)]
struct UpstreamVersionHistory {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    versions: Vec<UpstreamVersionSummary>,
}

#[derive(Debug, Deserialize)]
struct UpstreamVersionSummary {
    id: FlexibleId,
    name: String,
    #[serde(rename = "type", default)]
    release_type: String,
    #[serde(default)]
    updated: i64,
    #[serde(default)]
    targets: Vec<UpstreamTarget>,
    #[serde(default)]
    private: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpstreamVersion {
    #[serde(default)]
    status: Option<String>,
    id: FlexibleId,
    name: String,
    #[serde(rename = "type", default)]
    release_type: String,
    #[serde(default)]
    updated: i64,
    #[serde(default)]
    released: i64,
    #[serde(default)]
    targets: Vec<UpstreamTarget>,
    #[serde(default)]
    specs: Value,
    #[serde(default)]
    files: Vec<UpstreamFile>,
    #[serde(default)]
    changelog: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamTarget {
    name: String,
    version: String,
    #[serde(rename = "type", default)]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamFile {
    #[serde(default)]
    id: FlexibleId,
    name: String,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    version: Option<FlexibleId>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    mirrors: Option<Vec<String>>,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    hashes: Option<UpstreamHashes>,
    #[serde(default)]
    size: Option<i64>,
    #[serde(rename = "clientonly", default)]
    client_only: Option<bool>,
    #[serde(rename = "serveronly", default)]
    server_only: Option<bool>,
    #[serde(default)]
    optional: Option<bool>,
    #[serde(default)]
    curseforge: Option<UpstreamCurseForgeReference>,
}

#[derive(Debug, Deserialize)]
struct UpstreamHashes {
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    sha512: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamCurseForgeReference {
    project: FlexibleId,
    file: FlexibleId,
}

#[derive(Clone, Debug, Deserialize)]
struct UpstreamArt {
    #[serde(rename = "type")]
    kind: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamAuthor {
    name: String,
    #[serde(default)]
    website: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamTag {
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamLink {
    link: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamTags {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    tags: Vec<UpstreamTag>,
}
