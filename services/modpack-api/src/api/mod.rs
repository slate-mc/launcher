mod content;
mod health;
mod imports;
mod metadata;
mod modpacks;
mod mods;
mod providers;
mod releases;

use crate::providers::{MissingResource, ProviderError};
use crate::response::{ApiError, RequestContext};
use crate::state::AppState;
use axum::Router;
use axum::extract::Extension;
use axum::http::StatusCode;
use slate_modpack_api_contracts::{ApiErrorCode, Provider};
use std::str::FromStr;

pub fn router() -> Router<AppState> {
    Router::new().merge(health::routes()).nest(
        "/v1",
        metadata::routes()
            .merge(content::routes())
            .merge(imports::routes())
            .merge(providers::routes())
            .merge(releases::routes())
            .merge(mods::routes())
            .merge(modpacks::routes()),
    )
}

pub async fn not_found(Extension(context): Extension<RequestContext>) -> ApiError {
    ApiError::new(
        &context,
        StatusCode::NOT_FOUND,
        ApiErrorCode::InvalidRequest,
        "The requested API route does not exist.",
        false,
    )
}

pub fn provider_error(context: &RequestContext, error: ProviderError) -> ApiError {
    match error {
        ProviderError::InvalidIdentifier => ApiError::invalid_request(
            context,
            "The provider project or version identifier is invalid.",
        ),
        ProviderError::NotFound(MissingResource::Project) => ApiError::new(
            context,
            StatusCode::NOT_FOUND,
            ApiErrorCode::ModpackNotFound,
            "The requested modpack could not be found.",
            false,
        ),
        ProviderError::NotFound(MissingResource::Version) => ApiError::new(
            context,
            StatusCode::NOT_FOUND,
            ApiErrorCode::VersionNotFound,
            "The requested modpack version could not be found.",
            false,
        ),
        ProviderError::Unavailable | ProviderError::InvalidResponse => ApiError::new(
            context,
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::ProviderUnavailable,
            "The modpack provider is temporarily unavailable.",
            true,
        ),
        ProviderError::RateLimited => ApiError::new(
            context,
            StatusCode::TOO_MANY_REQUESTS,
            ApiErrorCode::UpstreamRateLimited,
            "The modpack provider is temporarily rate limited.",
            true,
        ),
        ProviderError::InvalidInstallPath => ApiError::new(
            context,
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::InvalidInstallPath,
            "The provider returned an unsafe installation path.",
            false,
        ),
        ProviderError::DownloadUnavailable => ApiError::new(
            context,
            StatusCode::BAD_GATEWAY,
            ApiErrorCode::DownloadUnavailable,
            "A required modpack file has no safe download source.",
            true,
        ),
        ProviderError::UnsupportedLoader => ApiError::new(
            context,
            StatusCode::BAD_REQUEST,
            ApiErrorCode::UnsupportedLoader,
            "The modpack loader is not supported.",
            false,
        ),
        ProviderError::UnsupportedContent => ApiError::new(
            context,
            StatusCode::BAD_REQUEST,
            ApiErrorCode::InvalidProvider,
            "The provider does not support this content type.",
            false,
        ),
    }
}

pub fn provider_from_str(context: &RequestContext, value: &str) -> Result<Provider, ApiError> {
    Provider::from_str(value).map_err(|_| {
        ApiError::new(
            context,
            StatusCode::BAD_REQUEST,
            ApiErrorCode::InvalidProvider,
            "The requested modpack provider is invalid.",
            false,
        )
        .with_field("provider", "Use curseforge, modrinth, or ftb.")
    })
}
