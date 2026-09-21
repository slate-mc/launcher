use crate::release::ReleaseError;
use crate::state::AppState;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/launcher/updates/{target}/{architecture}/{current_version}",
        get(check),
    )
}

async fn check(
    State(state): State<AppState>,
    Path((target, architecture, current_version)): Path<(String, String, String)>,
) -> Response {
    match state
        .releases
        .update_for(&target, &architecture, &current_version)
        .await
    {
        Ok(Some(update)) => axum::Json(update).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(ReleaseError::UnsupportedTarget | ReleaseError::InvalidVersion) => {
            StatusCode::BAD_REQUEST.into_response()
        }
        Err(
            ReleaseError::InvalidManifest
            | ReleaseError::Unavailable
            | ReleaseError::Request(_)
            | ReleaseError::Json(_),
        ) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
