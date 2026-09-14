use axum::Json;
use axum::extract::Request;
use axum::http::header::{CACHE_CONTROL, HeaderName};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use slate_modpack_api_contracts::{
    ApiEnvelope, ApiErrorCode, ApiErrorDetail, ApiFieldError, ApiMeta,
};
use std::time::Instant;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");
const TRACEPARENT_HEADER: HeaderName = HeaderName::from_static("traceparent");

#[derive(Clone, Debug)]
pub struct RequestContext {
    pub request_id: String,
    pub trace_id: String,
    pub span_id: String,
    started_at: Instant,
}

impl RequestContext {
    fn from_request(request: &Request) -> Self {
        let trace_id = request
            .headers()
            .get(&TRACEPARENT_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(trace_id_from_traceparent)
            .unwrap_or_else(|| Uuid::new_v4().simple().to_string());
        Self {
            request_id: Uuid::new_v4().to_string(),
            trace_id,
            span_id: Uuid::new_v4()
                .simple()
                .to_string()
                .chars()
                .take(16)
                .collect(),
            started_at: Instant::now(),
        }
    }

    fn meta(&self) -> ApiMeta {
        ApiMeta {
            request_id: self.request_id.clone(),
            trace_id: self.trace_id.clone(),
            span_id: self.span_id.clone(),
            timestamp: OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned()),
            duration_ms: self.started_at.elapsed().as_secs_f64() * 1_000.0,
        }
    }

    fn traceparent(&self) -> String {
        format!("00-{}-{}-01", self.trace_id, self.span_id)
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            trace_id: Uuid::new_v4().simple().to_string(),
            span_id: Uuid::new_v4()
                .simple()
                .to_string()
                .chars()
                .take(16)
                .collect(),
            started_at: Instant::now(),
        }
    }
}

pub async fn request_context(mut request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let route = request.uri().path().to_owned();
    let context = RequestContext::from_request(&request);
    request.extensions_mut().insert(context.clone());
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&context.request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    if let Ok(value) = HeaderValue::from_str(&context.traceparent()) {
        response.headers_mut().insert(TRACEPARENT_HEADER, value);
    }
    tracing::info!(
        request_id = %context.request_id,
        trace_id = %context.trace_id,
        span_id = %context.span_id,
        method = %method,
        route,
        status = response.status().as_u16(),
        duration_ms = context.started_at.elapsed().as_secs_f64() * 1_000.0,
        "request completed"
    );
    response
}

pub fn success<T: Serialize>(context: &RequestContext, data: T, cache: CacheControl) -> Response {
    let envelope = ApiEnvelope {
        success: true,
        data: Some(data),
        error: None,
        meta: context.meta(),
    };
    let mut response = Json(envelope).into_response();
    if let Some(value) = cache.value()
        && let Ok(value) = HeaderValue::from_str(value)
    {
        response.headers_mut().insert(CACHE_CONTROL, value);
    }
    response
}

#[derive(Clone, Copy, Debug)]
pub enum CacheControl {
    NoStore,
    PublicShort,
    PublicProject,
    PublicManifest,
}

impl CacheControl {
    const fn value(self) -> Option<&'static str> {
        match self {
            Self::NoStore => Some("no-store"),
            Self::PublicShort => Some("public, max-age=60, stale-while-revalidate=120"),
            Self::PublicProject => Some("public, max-age=300, stale-while-revalidate=600"),
            Self::PublicManifest => Some("public, max-age=900, stale-while-revalidate=1800"),
        }
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    context: Box<RequestContext>,
    detail: Box<ApiErrorDetail>,
}

impl ApiError {
    pub fn new(
        context: &RequestContext,
        status: StatusCode,
        code: ApiErrorCode,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            context: Box::new(context.clone()),
            detail: Box::new(ApiErrorDetail {
                code,
                message: message.into(),
                retryable,
                details: Vec::new(),
            }),
        }
    }

    pub fn invalid_request(context: &RequestContext, message: impl Into<String>) -> Self {
        Self::new(
            context,
            StatusCode::BAD_REQUEST,
            ApiErrorCode::InvalidRequest,
            message,
            false,
        )
    }

    pub fn with_field(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.detail.details.push(ApiFieldError {
            field: field.into(),
            message: message.into(),
        });
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::warn!(
            request_id = %self.context.request_id,
            trace_id = %self.context.trace_id,
            span_id = %self.context.span_id,
            status = self.status.as_u16(),
            error_code = ?self.detail.code,
            "request failed"
        );
        let envelope = ApiEnvelope::<()> {
            success: false,
            data: None,
            error: Some(*self.detail),
            meta: self.context.meta(),
        };
        let mut response = (self.status, Json(envelope)).into_response();
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

pub fn internal_error(context: &RequestContext) -> ApiError {
    ApiError::new(
        context,
        StatusCode::INTERNAL_SERVER_ERROR,
        ApiErrorCode::InternalError,
        "The request could not be completed.",
        false,
    )
}

fn trace_id_from_traceparent(value: &str) -> Option<String> {
    let mut segments = value.split('-');
    let version = segments.next()?;
    let trace_id = segments.next()?;
    let parent_id = segments.next()?;
    let flags = segments.next()?;
    if segments.next().is_some()
        || version.len() != 2
        || trace_id.len() != 32
        || parent_id.len() != 16
        || flags.len() != 2
        || !trace_id.bytes().all(|value| value.is_ascii_hexdigit())
        || !parent_id.bytes().all(|value| value.is_ascii_hexdigit())
        || trace_id.bytes().all(|value| value == b'0')
        || parent_id.bytes().all(|value| value == b'0')
    {
        return None;
    }
    Some(trace_id.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::trace_id_from_traceparent;

    #[test]
    fn validates_w3c_traceparent_identifiers() {
        assert_eq!(
            trace_id_from_traceparent("00-15a4f7b8baa13dbc3d5e74933202e79d-f45b8f3c6e246a66-01"),
            Some("15a4f7b8baa13dbc3d5e74933202e79d".to_owned())
        );
        assert!(trace_id_from_traceparent("00-zero-00-01").is_none());
    }
}
