use super::modpacks::{
    bounded_optional, encode_cursor, interleave, invalid_path, invalid_query,
    parse_optional_loader, parse_sort, provider_adapter, resolve_page, sort_items, validated_limit,
};
use super::{provider_error, provider_from_str};
use crate::domain::{SearchRequest, java_major_for_minecraft, normalize_install_path};
use crate::providers::ProviderError;
use crate::response::{ApiError, CacheControl, RequestContext, success};
use crate::state::AppState;
use axum::Router;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{Extension, Json, Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use futures_util::future::join_all;
use futures_util::{StreamExt, stream};
use serde::Deserialize;
use slate_modpack_api_contracts::{
    ApiErrorCode, DownloadSource, InstallPlan, InstallPlanDownload, InstallPlanInstance, JavaPlan,
    Loader, LoaderKind, MemoryRecommendation, ModInstallPlanRequest, Provider, ProviderStatus,
    ResolveModsRequest, ResolveModsResponse, ResolvedModProject, RuntimePlan, SearchResponse,
};
use std::collections::{BTreeMap, BTreeSet};

const MAX_RESOLVE_ITEMS: usize = 512;
const RESOLVE_CONCURRENCY: usize = 24;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mods", get(search))
        .route("/mods/resolve", post(resolve))
        .route(
            "/mods/{provider}/{project_id}/install-plan",
            post(install_plan),
        )
}

#[derive(Debug, Default, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    provider: Option<String>,
    minecraft_version: Option<String>,
    loader: Option<String>,
    sort: Option<String>,
    cursor: Option<String>,
    page: Option<u32>,
    limit: Option<usize>,
}

async fn search(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(query) = query.map_err(|_| invalid_query(&context))?;
    let page = resolve_page(&context, query.cursor.as_deref(), query.page)?;
    let limit = validated_limit(&context, query.limit)?;
    let minecraft_version = bounded_optional(
        &context,
        "minecraft_version",
        query.minecraft_version,
        32,
    )?
    .ok_or_else(|| {
        ApiError::invalid_request(&context, "A Minecraft version is required for mod search.")
            .with_field(
                "minecraft_version",
                "Use the exact instance Minecraft version.",
            )
    })?;
    let loader = parse_optional_loader(&context, query.loader.as_deref())?.ok_or_else(|| {
        ApiError::invalid_request(&context, "A mod loader is required for mod search.")
            .with_field("loader", "Use the exact instance mod loader.")
    })?;
    validate_mod_loader(&context, loader)?;
    let sort = parse_sort(&context, query.sort.as_deref())?;
    let request = SearchRequest {
        query: bounded_optional(&context, "q", query.q, 200)?,
        minecraft_version: Some(minecraft_version),
        loader: Some(loader),
        category: None,
        sort,
        page,
        limit,
    };
    let requested_provider = query
        .provider
        .as_deref()
        .unwrap_or("all")
        .to_ascii_lowercase();

    if requested_provider != "all" {
        let provider = provider_from_str(&context, &requested_provider)?;
        validate_mod_provider(&context, provider)?;
        let adapter = provider_adapter(&state, &context, provider)?;
        let page = adapter
            .search_mods(request)
            .await
            .map_err(|error| provider_error(&context, error))?;
        let has_more = page.has_more();
        return Ok(success(
            &context,
            SearchResponse {
                items: page.items,
                next_cursor: has_more.then(|| encode_cursor(page.page.saturating_add(1))),
                has_more,
                provider_status: BTreeMap::from([(provider, ProviderStatus::Ok)]),
            },
            CacheControl::PublicShort,
        ));
    }

    let tasks = [Provider::CurseForge, Provider::Modrinth]
        .into_iter()
        .filter_map(|provider| {
            state.providers.get(provider).map(|adapter| {
                let request = request.clone();
                async move { (provider, adapter.search_mods(request).await) }
            })
        });
    let mut statuses = BTreeMap::new();
    let mut pages = Vec::new();
    let mut has_more = false;
    for (provider, result) in join_all(tasks).await {
        match result {
            Ok(provider_page) => {
                statuses.insert(provider, ProviderStatus::Ok);
                has_more |= provider_page.has_more();
                pages.push(provider_page.items);
            }
            Err(error) => {
                tracing::warn!(provider = %provider, error = %error, "mod provider search failed");
                statuses.insert(provider, ProviderStatus::Unavailable);
            }
        }
    }
    if pages.is_empty() {
        return Err(provider_error(&context, ProviderError::Unavailable));
    }
    let mut items = interleave(pages);
    sort_items(&mut items, sort);
    items.truncate(limit);
    Ok(success(
        &context,
        SearchResponse {
            items,
            next_cursor: has_more.then(|| encode_cursor(page.saturating_add(1))),
            has_more,
            provider_status: statuses,
        },
        CacheControl::PublicShort,
    ))
}

async fn install_plan(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    path: Result<Path<(String, String)>, PathRejection>,
    payload: Result<Json<ModInstallPlanRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Path((provider, project_id)) = path.map_err(|_| invalid_path(&context))?;
    let Json(request) = payload.map_err(|_| {
        ApiError::invalid_request(&context, "The mod install-plan body is invalid.").with_field(
            "body",
            "Provide the exact Minecraft, loader, and loader version from the instance.",
        )
    })?;
    let provider = provider_from_str(&context, &provider)?;
    validate_mod_provider(&context, provider)?;
    validate_mod_loader(&context, request.loader)?;
    let minecraft_version = request.minecraft_version.trim();
    if minecraft_version.is_empty()
        || minecraft_version.chars().count() > 32
        || minecraft_version.chars().any(char::is_control)
    {
        return Err(
            ApiError::invalid_request(&context, "The Minecraft version is invalid.").with_field(
                "minecraft_version",
                "Use the exact instance Minecraft version.",
            ),
        );
    }
    let loader_version = request
        .loader_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.chars().count() <= 64)
        .ok_or_else(|| {
            ApiError::invalid_request(&context, "The loader version is required.")
                .with_field("loader_version", "Use the exact instance loader version.")
        })?;
    let adapter = provider_adapter(&state, &context, provider)?;
    let resolved = adapter
        .resolve_mod(&project_id, minecraft_version, request.loader)
        .await
        .map_err(|error| provider_error(&context, error))?;
    if !resolved.hashes.has_cryptographic_hash() {
        return Err(ApiError::new(
            &context,
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::DownloadUnavailable,
            "The resolved mod has no cryptographic integrity hash.",
            false,
        ));
    }
    let destination =
        normalize_install_path(&format!("mods/{}", resolved.file_name)).map_err(|_| {
            ApiError::new(
                &context,
                StatusCode::BAD_GATEWAY,
                ApiErrorCode::InvalidInstallPath,
                "The resolved mod has an unsafe installation path.",
                false,
            )
        })?;
    let download = InstallPlanDownload {
        id: format!("{provider}:{}:{}", resolved.project_id, resolved.version_id),
        destination,
        size: resolved.size,
        hashes: resolved.hashes,
        sources: vec![DownloadSource::Direct { url: resolved.url }],
        required: true,
    };
    let plan = InstallPlan {
        schema: 1,
        instance: InstallPlanInstance {
            provider,
            project_id: resolved.project_id,
            version_id: resolved.version_id,
            name: resolved.version_name,
        },
        runtime: RuntimePlan {
            minecraft: minecraft_version.to_owned(),
            loader: Loader {
                kind: request.loader,
                version: Some(loader_version.to_owned()),
            },
            java: JavaPlan {
                major: java_major_for_minecraft(minecraft_version),
            },
            memory: MemoryRecommendation {
                minimum_mb: 1_024,
                recommended_mb: 4_096,
            },
        },
        total_download_size: download.size,
        downloads: vec![download],
        extract: Vec::new(),
        delete: Vec::new(),
    };
    Ok(success(&context, plan, CacheControl::NoStore))
}

async fn resolve(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    payload: Result<Json<ResolveModsRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(|_| {
        ApiError::invalid_request(&context, "The mod resolution body is invalid.").with_field(
            "body",
            "Provide an items array of provider project references.",
        )
    })?;
    if request.items.len() > MAX_RESOLVE_ITEMS {
        return Err(ApiError::invalid_request(
            &context,
            "Too many mods were submitted for resolution.",
        )
        .with_field(
            "items",
            format!("Submit no more than {MAX_RESOLVE_ITEMS} items."),
        ));
    }

    let mut references = BTreeSet::new();
    for item in request.items {
        validate_mod_provider(&context, item.provider)?;
        if !valid_identifier(&item.project_id) {
            return Err(ApiError::invalid_request(
                &context,
                "A mod project identifier is invalid.",
            )
            .with_field(
                "items.project_id",
                "Use a provider project identifier containing only letters, numbers, hyphens, underscores, or periods.",
            ));
        }
        references.insert((item.provider, item.project_id));
    }

    let providers = state.providers.clone();
    let items = stream::iter(references)
        .map(|(provider, project_id)| {
            let adapter = providers.get(provider);
            async move {
                let result = match adapter {
                    Some(adapter) => adapter.get_project(&project_id).await,
                    None => Err(ProviderError::Unavailable),
                };
                match result {
                    Ok(project) => Some(ResolvedModProject {
                        provider,
                        project_id,
                        name: project.name,
                        icon_url: project.icon_url,
                    }),
                    Err(error) => {
                        tracing::warn!(
                            provider = %provider,
                            project_id,
                            error = %error,
                            "mod project resolution failed"
                        );
                        None
                    }
                }
            }
        })
        .buffer_unordered(RESOLVE_CONCURRENCY)
        .filter_map(|item| async move { item })
        .collect::<Vec<_>>()
        .await;

    Ok(success(
        &context,
        ResolveModsResponse { items },
        CacheControl::NoStore,
    ))
}

fn validate_mod_provider(context: &RequestContext, provider: Provider) -> Result<(), ApiError> {
    if provider == Provider::Ftb {
        Err(ApiError::new(
            context,
            StatusCode::BAD_REQUEST,
            ApiErrorCode::InvalidProvider,
            "FTB does not provide individual mod discovery.",
            false,
        )
        .with_field("provider", "Use curseforge or modrinth."))
    } else {
        Ok(())
    }
}

fn validate_mod_loader(context: &RequestContext, loader: LoaderKind) -> Result<(), ApiError> {
    if loader == LoaderKind::Vanilla {
        Err(ApiError::new(
            context,
            StatusCode::BAD_REQUEST,
            ApiErrorCode::UnsupportedLoader,
            "Vanilla instances cannot install loader mods.",
            false,
        )
        .with_field("loader", "Choose a modded instance."))
    } else {
        Ok(())
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

#[cfg(test)]
mod tests {
    use super::{valid_identifier, validate_mod_loader, validate_mod_provider};
    use crate::response::RequestContext;
    use slate_modpack_api_contracts::{LoaderKind, Provider};

    #[test]
    fn mod_routes_reject_unsupported_targets() {
        let context = RequestContext::for_test();
        assert!(validate_mod_loader(&context, LoaderKind::Vanilla).is_err());
        assert!(validate_mod_loader(&context, LoaderKind::Fabric).is_ok());
        assert!(validate_mod_provider(&context, Provider::Ftb).is_err());
        assert!(validate_mod_provider(&context, Provider::Modrinth).is_ok());
    }

    #[test]
    fn mod_resolution_identifiers_are_bounded_and_path_safe() {
        assert!(valid_identifier("AANobbMI"));
        assert!(valid_identifier("387638"));
        assert!(!valid_identifier("../387638"));
        assert!(!valid_identifier("project/id"));
        assert!(!valid_identifier(""));
    }
}
