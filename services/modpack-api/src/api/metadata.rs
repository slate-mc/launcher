use super::{provider_error, provider_from_str};
use crate::response::{ApiError, CacheControl, RequestContext, success};
use crate::state::AppState;
use axum::Router;
use axum::extract::{Extension, Query, State, rejection::QueryRejection};
use axum::response::Response;
use axum::routing::get;
use futures_util::future::join_all;
use serde::Deserialize;
use slate_modpack_api_contracts::{
    CategoriesResponse, CategorySummary, LoaderKind, LoaderTypesResponse, MinecraftReleaseKind,
    MinecraftVersionSummary, MinecraftVersionsResponse,
};
use std::collections::BTreeMap;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/modpacks/categories", get(categories))
        .route("/minecraft/versions", get(minecraft_versions))
        .route("/loaders", get(loaders))
}

#[derive(Debug, Default, Deserialize)]
struct CategoryQuery {
    provider: Option<String>,
}

async fn categories(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    query: Result<Query<CategoryQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(query) = query.map_err(|_| {
        ApiError::invalid_request(&context, "The category query is invalid.")
            .with_field("query", "Use a valid provider value.")
    })?;
    let mut categories = BTreeMap::<String, CategorySummary>::new();
    if let Some(provider) = query.provider.as_deref() {
        let provider = provider_from_str(&context, provider)?;
        let adapter = state.providers.get(provider).ok_or_else(|| {
            ApiError::invalid_request(&context, "The modpack provider is not configured.")
        })?;
        for category in adapter
            .categories()
            .await
            .map_err(|error| provider_error(&context, error))?
        {
            categories.entry(category.id.clone()).or_insert(category);
        }
    } else {
        let tasks = state
            .providers
            .all()
            .map(|(_, adapter)| async move { adapter.categories().await });
        let mut succeeded = false;
        for items in join_all(tasks).await.into_iter().flatten() {
            succeeded = true;
            for category in items {
                categories.entry(category.id.clone()).or_insert(category);
            }
        }
        if !succeeded {
            return Err(provider_error(
                &context,
                crate::providers::ProviderError::Unavailable,
            ));
        }
    }
    Ok(success(
        &context,
        CategoriesResponse {
            items: categories.into_values().collect(),
        },
        CacheControl::PublicProject,
    ))
}

async fn minecraft_versions(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
) -> Result<Response, ApiError> {
    let manifest = state.minecraft.manifest().await.map_err(|error| {
        tracing::error!(error = %error, "official Minecraft version manifest failed");
        provider_error(&context, crate::providers::ProviderError::Unavailable)
    })?;
    let items = manifest
        .versions
        .iter()
        .map(|version| MinecraftVersionSummary {
            version: version.id.clone(),
            kind: match version.version_type.as_str() {
                "release" => MinecraftReleaseKind::Release,
                "snapshot" => MinecraftReleaseKind::Snapshot,
                "old_alpha" | "old_beta" => MinecraftReleaseKind::Old,
                _ => MinecraftReleaseKind::Unknown,
            },
        })
        .collect();
    Ok(success(
        &context,
        MinecraftVersionsResponse { items },
        CacheControl::PublicProject,
    ))
}

async fn loaders(Extension(context): Extension<RequestContext>) -> Response {
    success(
        &context,
        LoaderTypesResponse {
            items: vec![
                LoaderKind::Vanilla,
                LoaderKind::Fabric,
                LoaderKind::Quilt,
                LoaderKind::Forge,
                LoaderKind::NeoForge,
            ],
        },
        CacheControl::PublicProject,
    )
}
