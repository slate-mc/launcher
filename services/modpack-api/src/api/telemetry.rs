use crate::response::{ApiError, CacheControl, RequestContext, success};
use crate::state::AppState;
use crate::telemetry::TelemetryError;
use axum::Json;
use axum::Router;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use slate_modpack_api_contracts::{
    ApiErrorCode, CaptureProductEventRequest, CaptureProductEventResponse,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/telemetry/events", post(capture))
}

async fn capture(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    Json(request): Json<CaptureProductEventRequest>,
) -> Result<Response, ApiError> {
    validate(&context, &request)?;
    match state.telemetry.capture(&request).await {
        Ok(accepted) => Ok(success(
            &context,
            CaptureProductEventResponse { accepted },
            CacheControl::NoStore,
        )),
        Err(TelemetryError::Unavailable | TelemetryError::Request(_)) => Err(ApiError::new(
            &context,
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ProviderUnavailable,
            "Anonymous usage reporting is temporarily unavailable.",
            true,
        )),
        Err(TelemetryError::Rejected) => Err(ApiError::invalid_request(
            &context,
            "The anonymous usage event is invalid.",
        )),
    }
}

fn validate(
    context: &RequestContext,
    request: &CaptureProductEventRequest,
) -> Result<(), ApiError> {
    for (field, value, maximum) in [
        ("appVersion", request.app_version.as_str(), 64_usize),
        ("architecture", request.architecture.as_str(), 32),
    ] {
        if value.is_empty()
            || value.len() > maximum
            || !value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-_+".contains(character))
        {
            return Err(ApiError::invalid_request(
                context,
                "The anonymous usage event is invalid.",
            )
            .with_field(field, "Use a short version or platform identifier."));
        }
    }
    for (field, value) in [
        ("loader", request.loader.as_deref()),
        ("provider", request.provider.as_deref()),
    ] {
        if value.is_some_and(|value| {
            value.is_empty()
                || value.len() > 32
                || !value
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character))
        }) {
            return Err(ApiError::invalid_request(
                context,
                "The anonymous usage event is invalid.",
            )
            .with_field(field, "Use a supported content identifier."));
        }
    }
    Ok(())
}
