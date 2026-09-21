use crate::release::{ReleaseChannel, ReleaseError};
use crate::state::AppState;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
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
    headers: HeaderMap,
) -> Response {
    let channel = headers
        .get("x-slate-update-channel")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("stable");
    let Ok(channel) = ReleaseChannel::parse(channel) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let cohort_id = match headers.get("x-slate-update-cohort") {
        Some(value) => match value
            .to_str()
            .ok()
            .and_then(|value| uuid::Uuid::parse_str(value).ok())
        {
            Some(value) => Some(value),
            None => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };
    match state
        .releases
        .update_for(&target, &architecture, &current_version, channel, cohort_id)
        .await
    {
        Ok(Some(update)) => axum::Json(update).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(
            ReleaseError::InvalidChannel
            | ReleaseError::UnsupportedTarget
            | ReleaseError::InvalidVersion,
        ) => StatusCode::BAD_REQUEST.into_response(),
        Err(
            ReleaseError::InvalidManifest
            | ReleaseError::Unavailable
            | ReleaseError::Request(_)
            | ReleaseError::Json(_),
        ) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
