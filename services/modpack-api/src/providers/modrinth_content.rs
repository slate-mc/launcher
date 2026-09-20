use super::{MissingResource, ProviderError};
use crate::cache::CachePolicy;
use crate::domain::{SearchPage, SearchRequest, SearchSort};
use crate::upstream::{UpstreamClient, UpstreamError};
use serde::Deserialize;
use slate_modpack_api_contracts::{
    Author, ContentKind, Hashes, ModVersionSummary, ModpackSummary, Provider,
};
use url::{Host, Url};

#[derive(Clone, Debug)]
pub struct ModrinthContentProvider {
    upstream: UpstreamClient,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedContent {
    pub project_id: String,
    pub version_id: String,
    pub version_name: String,
    pub file_name: String,
    pub url: String,
    pub size: u64,
    pub hashes: Hashes,
}

impl ModrinthContentProvider {
    #[must_use]
    pub const fn new(upstream: UpstreamClient) -> Self {
        Self { upstream }
    }

    pub async fn search(
        &self,
        kind: ContentKind,
        request: SearchRequest,
    ) -> Result<SearchPage, ProviderError> {
        let minecraft_version = request
            .minecraft_version
            .as_deref()
            .ok_or(ProviderError::InvalidIdentifier)?;
        let facets = serde_json::to_string(&[
            [format!("project_type:{}", project_type(kind))],
            [format!("versions:{minecraft_version}")],
        ])
        .map_err(|_| ProviderError::InvalidResponse)?;
        let offset = request
            .page
            .saturating_sub(1)
            .saturating_mul(u32::try_from(request.limit).unwrap_or(u32::MAX))
            .to_string();
        let limit = request.limit.to_string();
        let mut query = vec![
            ("facets", facets.as_str()),
            ("index", sort_name(request.sort)),
            ("offset", offset.as_str()),
            ("limit", limit.as_str()),
        ];
        if let Some(term) = request.query.as_deref() {
            query.push(("query", term));
        }
        let response: SearchWire = self
            .upstream
            .get_modrinth_json(&["search"], &query, CachePolicy::Search)
            .await
            .map_err(|error| map_upstream(error, MissingResource::Project))?;
        let pages = if response.total_hits == 0 {
            request.page
        } else {
            let limit = u64::try_from(request.limit).unwrap_or(u64::MAX).max(1);
            u32::try_from(response.total_hits.div_ceil(limit)).unwrap_or(u32::MAX)
        };
        let items = response
            .hits
            .into_iter()
            .filter(|item| project_matches(kind, &item.project_type, &item.all_project_types))
            .map(map_search_hit)
            .take(request.limit)
            .collect();
        Ok(SearchPage {
            items,
            page: request.page,
            pages,
        })
    }

    pub async fn versions(
        &self,
        kind: ContentKind,
        project_id: &str,
        minecraft_version: &str,
    ) -> Result<Vec<ModVersionSummary>, ProviderError> {
        self.project(kind, project_id).await?;
        Ok(self
            .version_list(kind, project_id, minecraft_version)
            .await?
            .into_iter()
            .map(|version| ModVersionSummary {
                id: version.id,
                name: bounded_text(&version.name, 160),
            })
            .collect())
    }

    pub async fn resolve(
        &self,
        kind: ContentKind,
        project_id: &str,
        minecraft_version: &str,
        version_id: Option<&str>,
    ) -> Result<ResolvedContent, ProviderError> {
        validate_identifier(project_id)?;
        if version_id.is_some_and(|value| validate_identifier(value).is_err()) {
            return Err(ProviderError::InvalidIdentifier);
        }
        self.project(kind, project_id).await?;
        let versions = self
            .version_list(kind, project_id, minecraft_version)
            .await?;
        let version = if let Some(requested) = version_id {
            versions.into_iter().find(|version| version.id == requested)
        } else {
            versions.into_iter().next()
        };
        let version = version.ok_or(ProviderError::NotFound(MissingResource::Version))?;
        let file = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| version.files.first())
            .ok_or(ProviderError::DownloadUnavailable)?;
        if !safe_archive_name(&file.filename) || !trusted_modrinth_download(&file.url) {
            return Err(ProviderError::DownloadUnavailable);
        }
        let hashes = Hashes {
            sha512: file.hashes.sha512.clone(),
            sha256: file.hashes.sha256.clone(),
            sha1: file.hashes.sha1.clone(),
        };
        if !hashes.has_cryptographic_hash() || file.size == 0 {
            return Err(ProviderError::DownloadUnavailable);
        }
        Ok(ResolvedContent {
            project_id: version.project_id,
            version_id: version.id,
            version_name: bounded_text(&version.name, 160),
            file_name: file.filename.clone(),
            url: file.url.clone(),
            size: file.size,
            hashes,
        })
    }

    async fn project(
        &self,
        kind: ContentKind,
        project_id: &str,
    ) -> Result<ProjectWire, ProviderError> {
        validate_identifier(project_id)?;
        let project: ProjectWire = self
            .upstream
            .get_modrinth_json(&["project", project_id], &[], CachePolicy::Project)
            .await
            .map_err(|error| map_upstream(error, MissingResource::Project))?;
        if !project_matches_details(kind, &project.project_type, &project.loaders) {
            return Err(ProviderError::UnsupportedContent);
        }
        Ok(project)
    }

    async fn version_list(
        &self,
        kind: ContentKind,
        project_id: &str,
        minecraft_version: &str,
    ) -> Result<Vec<VersionWire>, ProviderError> {
        if minecraft_version.is_empty() || minecraft_version.len() > 32 {
            return Err(ProviderError::InvalidIdentifier);
        }
        let game_versions = serde_json::to_string(&[minecraft_version])
            .map_err(|_| ProviderError::InvalidResponse)?;
        let response: Vec<VersionWire> = self
            .upstream
            .get_modrinth_json(
                &["project", project_id, "version"],
                &[
                    ("game_versions", game_versions.as_str()),
                    ("include_changelog", "false"),
                ],
                CachePolicy::Versions,
            )
            .await
            .map_err(|error| map_upstream(error, MissingResource::Version))?;
        Ok(response
            .into_iter()
            .filter(|version| {
                version.project_id == project_id
                    && version
                        .game_versions
                        .iter()
                        .any(|value| value == minecraft_version)
                    && version_matches(kind, &version.loaders)
            })
            .collect())
    }
}

fn map_search_hit(hit: SearchHitWire) -> ModpackSummary {
    ModpackSummary {
        provider: Provider::Modrinth,
        id: hit.project_id,
        slug: hit.slug.unwrap_or_default(),
        name: bounded_text(&hit.title, 160),
        summary: bounded_text(&hit.description, 500),
        authors: vec![Author {
            name: bounded_text(&hit.author, 160),
            url: None,
        }],
        icon_url: hit.icon_url.filter(|url| trusted_modrinth_image(url)),
        downloads: hit.downloads,
        updated_at: hit.date_modified,
        minecraft_versions: hit.versions,
        loaders: Vec::new(),
        categories: hit.categories,
    }
}

const fn project_type(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::ResourcePack => "resourcepack",
        ContentKind::ShaderPack => "shader",
        ContentKind::DataPack => "datapack",
    }
}

fn project_matches(kind: ContentKind, primary: &str, all: &[String]) -> bool {
    primary == project_type(kind) || all.iter().any(|value| value == project_type(kind))
}

fn project_matches_details(kind: ContentKind, primary: &str, loaders: &[String]) -> bool {
    primary == project_type(kind)
        || (kind == ContentKind::DataPack && loaders.iter().any(|value| value == "datapack"))
}

fn version_matches(kind: ContentKind, loaders: &[String]) -> bool {
    match kind {
        ContentKind::ResourcePack => loaders.iter().any(|value| value == "minecraft"),
        ContentKind::ShaderPack => true,
        ContentKind::DataPack => loaders.iter().any(|value| value == "datapack"),
    }
}

const fn sort_name(sort: SearchSort) -> &'static str {
    match sort {
        SearchSort::Relevance => "relevance",
        SearchSort::Downloads => "downloads",
        SearchSort::Updated => "updated",
        SearchSort::Newest => "newest",
    }
}

fn validate_identifier(value: &str) -> Result<(), ProviderError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Err(ProviderError::InvalidIdentifier)
    } else {
        Ok(())
    }
}

fn safe_archive_name(value: &str) -> bool {
    value.len() <= 240
        && value.to_ascii_lowercase().ends_with(".zip")
        && !value.is_empty()
        && !matches!(value, "." | "..")
        && !value.contains(['/', '\\', ':'])
        && !value.chars().any(char::is_control)
}

fn trusted_modrinth_download(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(url.host(), Some(Host::Domain("cdn.modrinth.com")))
    })
}

fn trusted_modrinth_image(value: &str) -> bool {
    trusted_modrinth_download(value)
}

fn bounded_text(value: &str, limit: usize) -> String {
    value
        .trim()
        .chars()
        .filter(|value| !value.is_control())
        .take(limit)
        .collect()
}

fn map_upstream(error: UpstreamError, missing: MissingResource) -> ProviderError {
    match error {
        UpstreamError::NotFound => ProviderError::NotFound(missing),
        UpstreamError::RateLimited => ProviderError::RateLimited,
        UpstreamError::InvalidPathSegment | UpstreamError::InvalidArtifactUrl => {
            ProviderError::InvalidIdentifier
        }
        UpstreamError::InvalidResponse(_) | UpstreamError::ResponseTooLarge => {
            ProviderError::InvalidResponse
        }
        UpstreamError::InvalidBaseUrl
        | UpstreamError::Unavailable
        | UpstreamError::Rejected(_)
        | UpstreamError::ArtifactTooLarge
        | UpstreamError::Request(_) => ProviderError::Unavailable,
    }
}

#[derive(Debug, Deserialize)]
struct SearchWire {
    #[serde(default)]
    hits: Vec<SearchHitWire>,
    #[serde(default)]
    total_hits: u64,
}

#[derive(Debug, Deserialize)]
struct SearchHitWire {
    project_id: String,
    project_type: String,
    #[serde(default)]
    all_project_types: Vec<String>,
    #[serde(default)]
    slug: Option<String>,
    title: String,
    description: String,
    author: String,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    versions: Vec<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    date_modified: String,
}

#[derive(Debug, Deserialize)]
struct ProjectWire {
    project_type: String,
    #[serde(default)]
    loaders: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct VersionWire {
    id: String,
    project_id: String,
    name: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    files: Vec<FileWire>,
}

#[derive(Debug, Deserialize)]
struct FileWire {
    url: String,
    filename: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: HashWire,
}

#[derive(Debug, Default, Deserialize)]
struct HashWire {
    #[serde(default)]
    sha512: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    sha1: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{project_matches, safe_archive_name, trusted_modrinth_download, version_matches};
    use slate_modpack_api_contracts::ContentKind;

    #[test]
    fn content_kinds_match_modrinth_projects_and_versions() {
        assert!(project_matches(
            ContentKind::ResourcePack,
            "resourcepack",
            &[]
        ));
        assert!(project_matches(
            ContentKind::DataPack,
            "mod",
            &["datapack".to_owned()]
        ));
        assert!(version_matches(
            ContentKind::ResourcePack,
            &["minecraft".to_owned()]
        ));
        assert!(version_matches(ContentKind::ShaderPack, &[]));
        assert!(!version_matches(
            ContentKind::DataPack,
            &["fabric".to_owned()]
        ));
    }

    #[test]
    fn content_downloads_require_safe_modrinth_archives() {
        assert!(safe_archive_name("FreshAnimations.zip"));
        assert!(!safe_archive_name("../FreshAnimations.zip"));
        assert!(!safe_archive_name("content.jar"));
        assert!(trusted_modrinth_download(
            "https://cdn.modrinth.com/data/project/version/content.zip"
        ));
        assert!(!trusted_modrinth_download(
            "https://cdn.modrinth.com.example.test/content.zip"
        ));
    }
}
