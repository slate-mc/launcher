use super::{INSTALL_METADATA_CONCURRENCY, MissingResource, ProviderError, wire::*};
use crate::domain::{SearchRequest, SearchSort, VersionQuery, normalize_install_path};
use crate::upstream::{UpstreamClient, UpstreamError};
use futures_util::{StreamExt, TryStreamExt, stream};
use serde_json::Value;
use slate_modpack_api_contracts::{
    Author, DownloadSource, FileOption, Hashes, Loader, LoaderKind, Modpack, ModpackFile,
    ModpackLinks, ModpackSummary, ModpackVersion, ModpackVersionSummary, PackFileType, Provider,
    ProviderReference, ReleaseType, Side, VersionReference,
};
use std::collections::BTreeSet;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use url::{Host, Url};

pub(super) fn map_search_card(
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

pub(super) fn map_project(
    project: UpstreamProject,
    provider: Provider,
) -> Result<Modpack, ProviderError> {
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

pub(super) fn map_version(
    version: UpstreamVersion,
    provider: Provider,
    project_id: &str,
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
        let mapped = map_file(file, provider, project_id)?;
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

pub(super) async fn resolve_install_metadata(
    mut version: ModpackVersion,
    upstream: &UpstreamClient,
) -> Result<ModpackVersion, ProviderError> {
    let files = stream::iter(version.files.into_iter().map(|mut file| {
        let upstream = upstream.clone();
        async move {
            if !file.hashes.has_cryptographic_hash() {
                let DownloadSource::Direct { url } = &file.download else {
                    return Err(ProviderError::DownloadUnavailable);
                };
                let metadata = upstream
                    .artifact_metadata(url)
                    .await
                    .map_err(|_| ProviderError::DownloadUnavailable)?;
                file.size = metadata.size;
                file.hashes.sha256 = Some(metadata.sha256.clone());
                file.hashes.sha512 = Some(metadata.sha512.clone());
            }
            Ok(file)
        }
    }))
    .buffered(INSTALL_METADATA_CONCURRENCY)
    .try_collect::<Vec<_>>()
    .await?;
    version.total_download_size = files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.size)
            .ok_or(ProviderError::InvalidResponse)
    })?;
    version.files = files;
    Ok(version)
}

pub(super) fn map_file(
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
        raw_id.clone()
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
                project_id: if provider == Provider::Modrinth && !raw_id.is_empty() && raw_id != "0"
                {
                    raw_id
                } else {
                    project_id.to_owned()
                },
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

pub(super) fn loader_from_targets(targets: &[UpstreamTarget]) -> Result<Loader, ProviderError> {
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

pub(super) fn summary_matches(version: &UpstreamVersionSummary, request: &VersionQuery) -> bool {
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

pub(super) fn map_version_summary(
    version: UpstreamVersionSummary,
) -> Result<ModpackVersionSummary, ProviderError> {
    let minecraft_version = version
        .targets
        .iter()
        .find(|target| target.kind == "game" || target.name.eq_ignore_ascii_case("minecraft"))
        .map(|target| target.version.clone())
        .ok_or(ProviderError::InvalidResponse)?;
    let loader = loader_from_targets(&version.targets)?;
    Ok(ModpackVersionSummary {
        id: version.id.to_string_value(),
        name: bounded_text(&version.name, 160),
        release_type: release_type(&version.release_type),
        minecraft_version,
        loader,
        published_at: timestamp(version.updated),
        changelog: None,
    })
}

pub(super) fn search_item_matches(item: &ModpackSummary, request: &SearchRequest) -> bool {
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

pub(super) fn map_hashes(sha1: Option<&str>, hashes: Option<&UpstreamHashes>) -> Hashes {
    Hashes {
        sha512: hashes.and_then(|value| valid_hash(value.sha512.as_deref(), 128)),
        sha256: hashes.and_then(|value| valid_hash(value.sha256.as_deref(), 64)),
        sha1: hashes
            .and_then(|value| valid_hash(value.sha1.as_deref(), 40))
            .or_else(|| valid_hash(sha1, 40)),
    }
}

pub(super) fn valid_hash(value: Option<&str>, length: usize) -> Option<String> {
    value
        .filter(|hash| hash.len() == length && hash.chars().all(|value| value.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}

pub(super) fn memory_from_value(
    value: &Value,
) -> slate_modpack_api_contracts::MemoryRecommendation {
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

pub(super) fn map_links(links: Vec<UpstreamLink>) -> ModpackLinks {
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

pub(super) fn map_authors(authors: Vec<UpstreamAuthor>) -> Vec<Author> {
    authors
        .into_iter()
        .filter(|author| !author.name.trim().is_empty())
        .map(|author| Author {
            name: bounded_text(&author.name, 100),
            url: author.website.as_deref().and_then(safe_external_url),
        })
        .collect()
}

pub(super) fn select_art(art: &[UpstreamArt], kinds: &[&str]) -> Option<String> {
    kinds.iter().find_map(|kind| {
        art.iter()
            .find(|art| art.kind.eq_ignore_ascii_case(kind))
            .and_then(|art| safe_external_url(&art.url))
    })
}

pub(super) fn slug_from_links(links: &ModpackLinks) -> Option<String> {
    links.website.as_ref().and_then(|value| {
        Url::parse(value)
            .ok()?
            .path_segments()?
            .rfind(|segment| !segment.is_empty())
            .map(slugify)
    })
}

pub(super) fn safe_external_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    Some(url.to_string())
}

pub(super) fn safe_download_url(value: &str) -> Option<String> {
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

pub(super) fn safe_mod_artifact_url(value: &str) -> Option<String> {
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

pub(super) fn safe_jar_file_name(value: &str) -> Option<String> {
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

pub(super) fn file_name_from_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    let encoded = url.path_segments()?.next_back()?;
    let decoded = percent_encoding::percent_decode_str(encoded)
        .decode_utf8()
        .ok()?;
    safe_jar_file_name(&decoded)
}

pub(super) fn provider_matches(card: &UpstreamBrowseCard, provider: Provider) -> bool {
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

pub(super) fn validate_identifier(value: &str) -> Result<(), ProviderError> {
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

pub(super) fn ensure_success(status: Option<&str>) -> Result<(), ProviderError> {
    if status.is_some_and(|value| value.eq_ignore_ascii_case("error")) {
        Err(ProviderError::InvalidResponse)
    } else {
        Ok(())
    }
}

pub(super) fn map_upstream_error(error: UpstreamError, missing: MissingResource) -> ProviderError {
    match error {
        UpstreamError::NotFound => ProviderError::NotFound(missing),
        UpstreamError::RateLimited => ProviderError::RateLimited,
        UpstreamError::Unavailable | UpstreamError::Request(_) => ProviderError::Unavailable,
        _ => ProviderError::InvalidResponse,
    }
}

pub(super) fn release_type(value: &str) -> ReleaseType {
    match value.to_ascii_lowercase().as_str() {
        "release" | "stable" => ReleaseType::Release,
        "beta" => ReleaseType::Beta,
        "alpha" => ReleaseType::Alpha,
        _ => ReleaseType::Unknown,
    }
}

pub(super) fn file_type(value: &str) -> PackFileType {
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

pub(super) fn parse_loader(value: &str) -> Option<LoaderKind> {
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

pub(super) fn loader_name(loader: LoaderKind) -> &'static str {
    match loader {
        LoaderKind::Vanilla => "vanilla",
        LoaderKind::Forge => "forge",
        LoaderKind::NeoForge => "neoforge",
        LoaderKind::Fabric => "fabric",
        LoaderKind::Quilt => "quilt",
    }
}

pub(super) fn upstream_sort(sort: SearchSort) -> &'static str {
    match sort {
        SearchSort::Relevance | SearchSort::Downloads => "popular",
        SearchSort::Updated => "updated",
        SearchSort::Newest => "new",
    }
}

pub(super) fn looks_like_minecraft_version(value: &str) -> bool {
    let mut parts = value.split('.');
    parts
        .next()
        .is_some_and(|part| !part.is_empty() && part.chars().all(|value| value.is_ascii_digit()))
        && parts.next().is_some_and(|part| {
            !part.is_empty() && part.chars().all(|value| value.is_ascii_digit())
        })
}

pub(super) fn slugify(value: &str) -> String {
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

pub(super) fn bounded_text(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

pub(super) fn nonnegative_u64(value: Option<i64>) -> u64 {
    value.unwrap_or(0).max(0).unsigned_abs()
}

pub(super) fn timestamp(value: i64) -> String {
    OffsetDateTime::from_unix_timestamp(value)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_owned())
}

pub(super) fn unique_sorted<T: Ord>(values: impl IntoIterator<Item = T>) -> Vec<T> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
