use crate::response::{CacheControl, RequestContext, success};
use crate::state::AppState;
use axum::Router;
use axum::extract::{Extension, State};
use axum::response::Response;
use axum::routing::get;
use slate_modpack_api_contracts::{
    HealthResponse, ProviderStatus, ReadinessResponse, ReadinessStatus,
};
use std::collections::BTreeMap;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}

async fn health(Extension(context): Extension<RequestContext>) -> Response {
    success(
        &context,
        HealthResponse {
            status: "ok".to_owned(),
        },
        CacheControl::NoStore,
    )
}

async fn ready(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
) -> Response {
    let available = state.upstream.health().await;
    let status = if available {
        ReadinessStatus::Ok
    } else {
        ReadinessStatus::Degraded
    };
    let providers = state
        .providers
        .all()
        .map(|(provider, _)| {
            (
                provider,
                if available {
                    ProviderStatus::Ok
                } else {
                    ProviderStatus::Unavailable
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    success(
        &context,
        ReadinessResponse { status, providers },
        CacheControl::NoStore,
    )
}
