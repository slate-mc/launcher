pub mod api;
pub mod cache;
pub mod config;
pub mod domain;
pub mod providers;
pub mod rate_limit;
pub mod release;
pub mod response;
pub mod state;
pub mod upstream;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::Method;
use axum::http::header::CONTENT_TYPE;
use axum::middleware;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};

pub fn router(state: state::AppState) -> Router {
    let middleware_state = state.clone();
    api::router()
        .fallback(api::not_found)
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers([CONTENT_TYPE]),
        )
        .layer(middleware::from_fn_with_state(
            middleware_state,
            rate_limit::enforce,
        ))
        .layer(middleware::from_fn(response::request_context))
        .with_state(state)
}
