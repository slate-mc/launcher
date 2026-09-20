use super::modpacks::{
    bounded_optional, encode_cursor, invalid_path, invalid_query, parse_sort, resolve_page,
    validated_limit,
};
use super::provider_error;
use crate::domain::{SearchRequest, java_major_for_minecraft, normalize_install_path};
use crate::providers::ResolvedContentArtifactKind;
use crate::response::{ApiError, CacheControl, RequestContext, success};
use crate::state::AppState;
use axum::Router;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{Extension, Json, Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use serde::Deserialize;
use slate_modpack_api_contracts::{
    ApiErrorCode, ContentInstallPlanRequest, ContentKind, DownloadSource, InstallPlan,
    InstallPlanDownload, InstallPlanInstance, JavaPlan, Loader, LoaderKind, MemoryRecommendation,
    ModVersionList, Provider, ProviderStatus, RuntimePlan, SearchResponse,
};
use std::collections::BTreeMap;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/content/{kind}", get(search))
        .route(
            "/content/{kind}/modrinth/{project_id}/versions",
            get(versions),
        )
        .route(
            "/content/{kind}/modrinth/{project_id}/install-plan",
            post(install_plan),
        )
}

#[derive(Debug, Default, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    minecraft_version: Option<String>,
    sort: Option<String>,
    cursor: Option<String>,
    page: Option<u32>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct VersionsQuery {
    minecraft_version: Option<String>,
}

async fn search(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Path(kind) = path.map_err(|_| invalid_path(&context))?;
    let kind = parse_content_kind(&context, &kind)?;
    let Query(query) = query.map_err(|_| invalid_query(&context))?;
    let page = resolve_page(&context, query.cursor.as_deref(), query.page)?;
    let limit = validated_limit(&context, query.limit)?;
    let minecraft_version =
        bounded_optional(&context, "minecraft_version", query.minecraft_version, 32)?.ok_or_else(
            || {
                ApiError::invalid_request(&context, "A Minecraft version is required.").with_field(
                    "minecraft_version",
                    "Use the exact instance Minecraft version.",
                )
            },
        )?;
    let request = SearchRequest {
        query: bounded_optional(&context, "q", query.q, 200)?,
        minecraft_version: Some(minecraft_version),
        loader: None,
        category: None,
        sort: parse_sort(&context, query.sort.as_deref())?,
        page,
        limit,
    };
    let page = state
        .content
        .search(kind, request)
        .await
        .map_err(|error| provider_error(&context, error))?;
    let has_more = page.has_more();
    Ok(success(
        &context,
        SearchResponse {
            items: page.items,
            next_cursor: has_more.then(|| encode_cursor(page.page.saturating_add(1))),
            has_more,
            provider_status: BTreeMap::from([(Provider::Modrinth, ProviderStatus::Ok)]),
        },
        CacheControl::PublicShort,
    ))
}

async fn versions(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    path: Result<Path<(String, String)>, PathRejection>,
    query: Result<Query<VersionsQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Path((kind, project_id)) = path.map_err(|_| invalid_path(&context))?;
    let kind = parse_content_kind(&context, &kind)?;
    let Query(query) = query.map_err(|_| invalid_query(&context))?;
    let minecraft_version =
        bounded_optional(&context, "minecraft_version", query.minecraft_version, 32)?.ok_or_else(
            || {
                ApiError::invalid_request(&context, "A Minecraft version is required.").with_field(
                    "minecraft_version",
                    "Use the exact instance Minecraft version.",
                )
            },
        )?;
    let items = state
        .content
        .versions(kind, &project_id, &minecraft_version)
        .await
        .map_err(|error| provider_error(&context, error))?;
    Ok(success(
        &context,
        ModVersionList { items },
        CacheControl::PublicShort,
    ))
}

async fn install_plan(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    path: Result<Path<(String, String)>, PathRejection>,
    payload: Result<Json<ContentInstallPlanRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Path((kind, project_id)) = path.map_err(|_| invalid_path(&context))?;
    let kind = parse_content_kind(&context, &kind)?;
    let Json(request) = payload.map_err(|_| {
        ApiError::invalid_request(&context, "The content install request is invalid.")
            .with_field("body", "Use the exact runtime details from the instance.")
    })?;
    let minecraft_version = request.minecraft_version.trim();
    if minecraft_version.is_empty()
        || minecraft_version.chars().count() > 32
        || minecraft_version.chars().any(char::is_control)
    {
        return Err(ApiError::invalid_request(
            &context,
            "The Minecraft version is invalid.",
        ));
    }
    validate_loader_version(&context, request.loader, request.loader_version.as_deref())?;
    let version_id = request.version_id.as_deref().map(str::trim);
    let resolved = state
        .content
        .resolve(
            kind,
            &project_id,
            minecraft_version,
            version_id,
            request.loader,
        )
        .await
        .map_err(|error| provider_error(&context, error))?;
    let downloads = resolved
        .artifacts
        .into_iter()
        .map(|artifact| {
            let (id, directory) = match artifact.kind {
                ResolvedContentArtifactKind::Content(artifact_kind) => (
                    format!(
                        "content:{}:modrinth:{}:{}",
                        artifact_kind.as_str(),
                        artifact.project_id,
                        artifact.version_id
                    ),
                    content_destination(artifact_kind),
                ),
                ResolvedContentArtifactKind::Mod => (
                    format!("modrinth:{}:{}", artifact.project_id, artifact.version_id),
                    "mods",
                ),
            };
            let destination =
                normalize_install_path(&format!("{directory}/{}", artifact.file_name)).map_err(
                    |_| {
                        ApiError::new(
                            &context,
                            StatusCode::BAD_GATEWAY,
                            ApiErrorCode::InvalidInstallPath,
                            "The selected content has an unsafe file name.",
                            false,
                        )
                    },
                )?;
            Ok(InstallPlanDownload {
                id,
                destination,
                size: artifact.size,
                hashes: artifact.hashes,
                sources: vec![DownloadSource::Direct { url: artifact.url }],
                required: true,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let total_download_size = downloads.iter().try_fold(0_u64, |total, download| {
        total.checked_add(download.size).ok_or_else(|| {
            ApiError::new(
                &context,
                StatusCode::BAD_GATEWAY,
                ApiErrorCode::InternalError,
                "The content download size is invalid.",
                false,
            )
        })
    })?;
    let plan = InstallPlan {
        schema: 1,
        instance: InstallPlanInstance {
            provider: Provider::Modrinth,
            project_id: resolved.project_id.clone(),
            version_id: resolved.version_id.clone(),
            name: resolved.version_name,
        },
        runtime: RuntimePlan {
            minecraft: minecraft_version.to_owned(),
            loader: Loader {
                kind: request.loader,
                version: request.loader_version,
            },
            java: JavaPlan {
                major: java_major_for_minecraft(minecraft_version),
            },
            memory: MemoryRecommendation {
                minimum_mb: 1_024,
                recommended_mb: 4_096,
            },
        },
        downloads,
        extract: Vec::new(),
        delete: Vec::new(),
        total_download_size,
    };
    Ok(success(&context, plan, CacheControl::NoStore))
}

fn parse_content_kind(context: &RequestContext, value: &str) -> Result<ContentKind, ApiError> {
    match value {
        "resource_pack" => Ok(ContentKind::ResourcePack),
        "shader_pack" => Ok(ContentKind::ShaderPack),
        "data_pack" => Ok(ContentKind::DataPack),
        _ => Err(
            ApiError::invalid_request(context, "The content type is invalid.")
                .with_field("kind", "Use resource_pack, shader_pack, or data_pack."),
        ),
    }
}

fn validate_loader_version(
    context: &RequestContext,
    loader: LoaderKind,
    loader_version: Option<&str>,
) -> Result<(), ApiError> {
    let loader_version = loader_version
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let valid = match loader {
        LoaderKind::Vanilla => loader_version.is_none(),
        LoaderKind::Fabric | LoaderKind::NeoForge => {
            loader_version.is_some_and(|value| value.len() <= 64)
        }
        LoaderKind::Forge | LoaderKind::Quilt => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ApiError::new(
            context,
            StatusCode::BAD_REQUEST,
            ApiErrorCode::UnsupportedLoader,
            "The instance loader details are invalid.",
            false,
        ))
    }
}

const fn content_destination(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::ResourcePack => "resourcepacks",
        ContentKind::ShaderPack => "shaderpacks",
        ContentKind::DataPack => "datapacks",
    }
}

#[cfg(test)]
mod tests {
    use super::{content_destination, parse_content_kind, validate_loader_version};
    use crate::response::RequestContext;
    use slate_modpack_api_contracts::{ContentKind, LoaderKind};

    #[test]
    fn content_routes_validate_kinds_and_runtime_targets() {
        let context = RequestContext::for_test();
        assert_eq!(
            parse_content_kind(&context, "resource_pack").ok(),
            Some(ContentKind::ResourcePack)
        );
        assert!(parse_content_kind(&context, "mod").is_err());
        assert!(validate_loader_version(&context, LoaderKind::Vanilla, None).is_ok());
        assert!(validate_loader_version(&context, LoaderKind::Fabric, Some("0.16.10")).is_ok());
        assert!(validate_loader_version(&context, LoaderKind::Fabric, None).is_err());
        assert_eq!(content_destination(ContentKind::ShaderPack), "shaderpacks");
    }
}
