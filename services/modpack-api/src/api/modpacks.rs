use super::{provider_error, provider_from_str};
use crate::domain::{
    InstallPlanError, SearchRequest, SearchSort, VersionQuery, generate_install_plan,
};
use crate::providers::ProviderError;
use crate::response::{ApiError, CacheControl, RequestContext, success};
use crate::state::AppState;
use axum::Router;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{Extension, Json, Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use slate_modpack_api_contracts::{
    ApiErrorCode, InstallPlanRequest, LoaderKind, ModpackSummary, Provider, ProviderStatus,
    ReleaseType, SearchResponse, UpdateResponse, VersionPage, VersionReference,
};
use std::collections::BTreeMap;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;
const MAX_CURSOR_LENGTH: usize = 256;
const MAX_PAGE: u32 = 10_000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/modpacks", get(search))
        .route("/modpacks/{provider}/{project_id}", get(project))
        .route("/modpacks/{provider}/{project_id}/versions", get(versions))
        .route(
            "/modpacks/{provider}/{project_id}/versions/{version_id}",
            get(version),
        )
        .route(
            "/modpacks/{provider}/{project_id}/versions/{version_id}/install-plan",
            post(install_plan),
        )
        .route("/modpacks/{provider}/{project_id}/update", get(update))
}

#[derive(Debug, Default, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    provider: Option<String>,
    minecraft_version: Option<String>,
    loader: Option<String>,
    category: Option<String>,
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
    let loader = parse_optional_loader(&context, query.loader.as_deref())?;
    let sort = parse_sort(&context, query.sort.as_deref())?;
    let request = SearchRequest {
        query: bounded_optional(&context, "q", query.q, 200)?,
        minecraft_version: bounded_optional(
            &context,
            "minecraft_version",
            query.minecraft_version,
            32,
        )?,
        loader,
        category: bounded_optional(&context, "category", query.category, 100)?,
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
        let adapter = state.providers.get(provider).ok_or_else(|| {
            ApiError::invalid_request(&context, "The modpack provider is not configured.")
        })?;
        let page = adapter
            .search(request)
            .await
            .map_err(|error| provider_error(&context, error))?;
        let has_more = page.has_more();
        let response = SearchResponse {
            items: page.items,
            next_cursor: has_more.then(|| encode_cursor(page.page.saturating_add(1))),
            has_more,
            provider_status: BTreeMap::from([(provider, ProviderStatus::Ok)]),
        };
        return Ok(success(&context, response, CacheControl::PublicShort));
    }

    let tasks = state.providers.all().map(|(provider, adapter)| {
        let request = request.clone();
        async move { (provider, adapter.search(request).await) }
    });
    let mut statuses = BTreeMap::new();
    let mut pages = Vec::new();
    let mut has_more = false;
    for (provider, result) in join_all(tasks).await {
        match result {
            Ok(page) => {
                statuses.insert(provider, ProviderStatus::Ok);
                has_more |= page.has_more();
                pages.push(page.items);
            }
            Err(error) => {
                tracing::warn!(provider = %provider, error = %error, "modpack provider search failed");
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

async fn project(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Response, ApiError> {
    let Path((provider, project_id)) = path.map_err(|_| invalid_path(&context))?;
    let provider = provider_from_str(&context, &provider)?;
    let adapter = provider_adapter(&state, &context, provider)?;
    let data = adapter
        .get_project(&project_id)
        .await
        .map_err(|error| provider_error(&context, error))?;
    Ok(success(&context, data, CacheControl::PublicProject))
}

#[derive(Debug, Default, Deserialize)]
struct VersionsQuery {
    minecraft_version: Option<String>,
    loader: Option<String>,
    release_type: Option<String>,
    cursor: Option<String>,
    page: Option<u32>,
    limit: Option<usize>,
}

async fn versions(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    path: Result<Path<(String, String)>, PathRejection>,
    query: Result<Query<VersionsQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Path((provider, project_id)) = path.map_err(|_| invalid_path(&context))?;
    let Query(query) = query.map_err(|_| invalid_query(&context))?;
    let provider = provider_from_str(&context, &provider)?;
    let page = resolve_page(&context, query.cursor.as_deref(), query.page)?;
    let limit = validated_limit(&context, query.limit)?;
    let request = VersionQuery {
        minecraft_version: bounded_optional(
            &context,
            "minecraft_version",
            query.minecraft_version,
            32,
        )?,
        loader: parse_optional_loader(&context, query.loader.as_deref())?,
        release_type: parse_optional_release_type(&context, query.release_type.as_deref())?,
        page,
        limit,
    };
    let adapter = provider_adapter(&state, &context, provider)?;
    let items = adapter
        .list_versions(&project_id, request)
        .await
        .map_err(|error| provider_error(&context, error))?;
    let has_more = items.len() == limit;
    Ok(success(
        &context,
        VersionPage {
            items,
            next_cursor: has_more.then(|| encode_cursor(page.saturating_add(1))),
            has_more,
        },
        CacheControl::PublicProject,
    ))
}

async fn version(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    path: Result<Path<(String, String, String)>, PathRejection>,
) -> Result<Response, ApiError> {
    let Path((provider, project_id, version_id)) = path.map_err(|_| invalid_path(&context))?;
    let provider = provider_from_str(&context, &provider)?;
    let adapter = provider_adapter(&state, &context, provider)?;
    let data = adapter
        .get_version(&project_id, &version_id)
        .await
        .map_err(|error| provider_error(&context, error))?;
    Ok(success(&context, data, CacheControl::PublicManifest))
}

async fn install_plan(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    path: Result<Path<(String, String, String)>, PathRejection>,
    payload: Result<Json<InstallPlanRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Path((provider, project_id, version_id)) = path.map_err(|_| invalid_path(&context))?;
    let Json(request) = payload.map_err(|_| {
        ApiError::invalid_request(&context, "The install-plan body is invalid.").with_field(
            "body",
            "Provide a supported platform, architecture, and optional files.",
        )
    })?;
    let provider = provider_from_str(&context, &provider)?;
    let adapter = provider_adapter(&state, &context, provider)?;
    let (version, project) = tokio::join!(
        adapter.get_version(&project_id, &version_id),
        adapter.get_project(&project_id),
    );
    let version = version.map_err(|error| provider_error(&context, error))?;
    let project = project.map_err(|error| provider_error(&context, error))?;
    let mut plan = generate_install_plan(&version, &request)
        .map_err(|error| install_plan_error(&context, error))?;
    plan.instance.name = project.name;
    Ok(success(&context, plan, CacheControl::NoStore))
}

#[derive(Debug, Deserialize)]
struct UpdateQuery {
    current_version: String,
    minecraft_version: Option<String>,
    loader: Option<String>,
}

async fn update(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    path: Result<Path<(String, String)>, PathRejection>,
    query: Result<Query<UpdateQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Path((provider, project_id)) = path.map_err(|_| invalid_path(&context))?;
    let Query(query) = query.map_err(|_| invalid_query(&context))?;
    let provider = provider_from_str(&context, &provider)?;
    let adapter = provider_adapter(&state, &context, provider)?;
    let current = adapter
        .get_version(&project_id, &query.current_version)
        .await
        .map_err(|error| provider_error(&context, error))?;
    let minecraft_version =
        bounded_optional(&context, "minecraft_version", query.minecraft_version, 32)?
            .unwrap_or_else(|| current.minecraft.version.clone());
    let loader =
        parse_optional_loader(&context, query.loader.as_deref())?.unwrap_or(current.loader.kind);
    let candidates = adapter
        .list_versions(
            &project_id,
            VersionQuery {
                minecraft_version: Some(minecraft_version),
                loader: Some(loader),
                release_type: Some(current.release_type),
                page: 1,
                limit: MAX_LIMIT,
            },
        )
        .await
        .map_err(|error| provider_error(&context, error))?;
    let latest = candidates.into_iter().next();
    let update_available = latest
        .as_ref()
        .is_some_and(|candidate| candidate.id != current.id);
    let current_reference = update_available.then(|| VersionReference {
        id: current.id.clone(),
        name: current.name,
    });
    let latest_reference = latest
        .filter(|version| version.id != current.id)
        .map(|version| VersionReference {
            id: version.id,
            name: version.name,
        });
    Ok(success(
        &context,
        UpdateResponse {
            update_available,
            current: current_reference,
            latest: latest_reference,
        },
        CacheControl::PublicShort,
    ))
}

fn provider_adapter(
    state: &AppState,
    context: &RequestContext,
    provider: Provider,
) -> Result<std::sync::Arc<dyn crate::providers::ModpackProvider>, ApiError> {
    state.providers.get(provider).ok_or_else(|| {
        ApiError::invalid_request(context, "The modpack provider is not configured.")
    })
}

fn validated_limit(context: &RequestContext, value: Option<usize>) -> Result<usize, ApiError> {
    let limit = value.unwrap_or(DEFAULT_LIMIT);
    if (1..=MAX_LIMIT).contains(&limit) {
        Ok(limit)
    } else {
        Err(
            ApiError::invalid_request(context, "The requested page size is invalid.")
                .with_field("limit", "Use an integer from 1 through 50."),
        )
    }
}

fn bounded_optional(
    context: &RequestContext,
    field: &str,
    value: Option<String>,
    maximum: usize,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > maximum || value.chars().any(char::is_control) {
        return Err(
            ApiError::invalid_request(context, "A query value is invalid.").with_field(
                field,
                format!("Use at most {maximum} printable characters."),
            ),
        );
    }
    Ok(Some(value.to_owned()))
}

fn parse_optional_loader(
    context: &RequestContext,
    value: Option<&str>,
) -> Result<Option<LoaderKind>, ApiError> {
    value.map(|value| parse_loader(context, value)).transpose()
}

fn parse_loader(context: &RequestContext, value: &str) -> Result<LoaderKind, ApiError> {
    match value.to_ascii_lowercase().as_str() {
        "vanilla" => Ok(LoaderKind::Vanilla),
        "forge" => Ok(LoaderKind::Forge),
        "neoforge" => Ok(LoaderKind::NeoForge),
        "fabric" => Ok(LoaderKind::Fabric),
        "quilt" => Ok(LoaderKind::Quilt),
        _ => Err(ApiError::new(
            context,
            StatusCode::BAD_REQUEST,
            ApiErrorCode::UnsupportedLoader,
            "The requested mod loader is not supported.",
            false,
        )
        .with_field("loader", "Use vanilla, forge, neoforge, fabric, or quilt.")),
    }
}

fn parse_sort(context: &RequestContext, value: Option<&str>) -> Result<SearchSort, ApiError> {
    match value.unwrap_or("relevance").to_ascii_lowercase().as_str() {
        "relevance" => Ok(SearchSort::Relevance),
        "downloads" => Ok(SearchSort::Downloads),
        "updated" => Ok(SearchSort::Updated),
        "newest" => Ok(SearchSort::Newest),
        _ => Err(
            ApiError::invalid_request(context, "The requested sort order is invalid.")
                .with_field("sort", "Use relevance, downloads, updated, or newest."),
        ),
    }
}

fn parse_optional_release_type(
    context: &RequestContext,
    value: Option<&str>,
) -> Result<Option<ReleaseType>, ApiError> {
    value
        .map(|value| match value.to_ascii_lowercase().as_str() {
            "release" => Ok(ReleaseType::Release),
            "beta" => Ok(ReleaseType::Beta),
            "alpha" => Ok(ReleaseType::Alpha),
            "unknown" => Ok(ReleaseType::Unknown),
            _ => Err(
                ApiError::invalid_request(context, "The requested release type is invalid.")
                    .with_field("release_type", "Use release, beta, alpha, or unknown."),
            ),
        })
        .transpose()
}

#[derive(Debug, Deserialize, Serialize)]
struct PageCursor {
    page: u32,
}

fn decode_cursor(context: &RequestContext, cursor: Option<&str>) -> Result<u32, ApiError> {
    let Some(cursor) = cursor else {
        return Ok(1);
    };
    if cursor.is_empty() || cursor.len() > MAX_CURSOR_LENGTH {
        return Err(invalid_cursor(context));
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| invalid_cursor(context))?;
    let cursor: PageCursor =
        serde_json::from_slice(&decoded).map_err(|_| invalid_cursor(context))?;
    if cursor.page == 0 || cursor.page > MAX_PAGE {
        return Err(invalid_cursor(context));
    }
    Ok(cursor.page)
}

fn resolve_page(
    context: &RequestContext,
    cursor: Option<&str>,
    page: Option<u32>,
) -> Result<u32, ApiError> {
    if cursor.is_some() && page.is_some() {
        return Err(ApiError::invalid_request(
            context,
            "Choose either a page number or a pagination cursor.",
        )
        .with_field(
            "pagination",
            "The page and cursor parameters cannot be used together.",
        ));
    }
    if let Some(page) = page {
        if (1..=MAX_PAGE).contains(&page) {
            return Ok(page);
        }
        return Err(
            ApiError::invalid_request(context, "The page number is invalid.")
                .with_field("page", "Use an integer from 1 through 10000."),
        );
    }
    decode_cursor(context, cursor)
}

fn encode_cursor(page: u32) -> String {
    let bytes = serde_json::to_vec(&PageCursor { page }).unwrap_or_default();
    URL_SAFE_NO_PAD.encode(bytes)
}

fn invalid_cursor(context: &RequestContext) -> ApiError {
    ApiError::invalid_request(context, "The pagination cursor is invalid.")
        .with_field("cursor", "Use the opaque cursor returned by this endpoint.")
}

fn invalid_query(context: &RequestContext) -> ApiError {
    ApiError::invalid_request(context, "The request query is invalid.")
        .with_field("query", "Check the query parameter names and value types.")
}

fn invalid_path(context: &RequestContext) -> ApiError {
    ApiError::invalid_request(context, "The request path is invalid.").with_field(
        "path",
        "Use valid provider, project, and version identifiers.",
    )
}

fn install_plan_error(context: &RequestContext, error: InstallPlanError) -> ApiError {
    match error {
        InstallPlanError::UnknownOptionalFile => ApiError::invalid_request(
            context,
            "The install plan selected an unknown optional file.",
        )
        .with_field(
            "include_optional",
            "Select only option IDs returned by the version endpoint.",
        ),
        InstallPlanError::InvalidPath => ApiError::new(
            context,
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::InvalidInstallPath,
            "The resolved modpack contains an unsafe installation path.",
            false,
        ),
        InstallPlanError::MissingHash => ApiError::new(
            context,
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::DownloadUnavailable,
            "A required modpack file has no cryptographic integrity hash.",
            false,
        ),
        InstallPlanError::DuplicateDestination | InstallPlanError::DownloadSizeOverflow => {
            ApiError::new(
                context,
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorCode::InternalError,
                "The install plan could not be generated safely.",
                false,
            )
        }
    }
}

fn interleave(mut pages: Vec<Vec<ModpackSummary>>) -> Vec<ModpackSummary> {
    let mut items = Vec::new();
    let maximum = pages.iter().map(Vec::len).max().unwrap_or(0);
    for index in 0..maximum {
        for page in &mut pages {
            if index < page.len() {
                items.push(page[index].clone());
            }
        }
    }
    items
}

fn sort_items(items: &mut [ModpackSummary], sort: SearchSort) {
    match sort {
        SearchSort::Relevance => {}
        SearchSort::Downloads => {
            items.sort_by_key(|item| std::cmp::Reverse(item.downloads));
        }
        SearchSort::Updated | SearchSort::Newest => {
            items.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PageCursor, URL_SAFE_NO_PAD, decode_cursor, resolve_page};
    use crate::response::RequestContext;
    use base64::Engine;
    use serde_json::json;

    #[test]
    fn pagination_cursor_is_opaque_and_bounded() {
        let context = RequestContext::for_test();
        let encoded =
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&PageCursor { page: 3 }).unwrap_or_default());
        assert!(matches!(decode_cursor(&context, Some(&encoded)), Ok(3)));
        let oversized =
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&json!({"page": 10001})).unwrap_or_default());
        assert!(decode_cursor(&context, Some(&oversized)).is_err());
    }

    #[test]
    fn page_number_and_cursor_are_mutually_exclusive() {
        let context = RequestContext::for_test();
        let cursor =
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&PageCursor { page: 2 }).unwrap_or_default());
        assert!(matches!(resolve_page(&context, None, Some(4)), Ok(4)));
        assert!(resolve_page(&context, Some(&cursor), Some(4)).is_err());
        assert!(resolve_page(&context, None, Some(0)).is_err());
    }
}
