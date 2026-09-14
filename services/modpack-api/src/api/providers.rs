use crate::response::{CacheControl, RequestContext, success};
use crate::state::AppState;
use axum::Router;
use axum::extract::{Extension, State};
use axum::response::Response;
use axum::routing::get;
use slate_modpack_api_contracts::{ProviderMetadata, ProvidersResponse};

pub fn routes() -> Router<AppState> {
    Router::new().route("/providers", get(list))
}

async fn list(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
) -> Response {
    let available = state.upstream.health().await;
    let providers = state
        .providers
        .all()
        .map(|(provider, _)| ProviderMetadata {
            id: provider,
            name: provider.display_name().to_owned(),
            available,
        })
        .collect();
    success(
        &context,
        ProvidersResponse { providers },
        CacheControl::PublicShort,
    )
}
