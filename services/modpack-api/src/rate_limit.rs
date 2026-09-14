use crate::response::{ApiError, RequestContext};
use crate::state::AppState;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::RETRY_AFTER;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use moka::sync::Cache;
use slate_modpack_api_contracts::ApiErrorCode;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(60);
const RATE_LIMIT_HEADER: HeaderName = HeaderName::from_static("x-ratelimit-limit");
const RATE_REMAINING_HEADER: HeaderName = HeaderName::from_static("x-ratelimit-remaining");

#[derive(Clone, Debug)]
pub struct RateLimiter {
    windows: Cache<RateKey, Arc<Mutex<RateWindow>>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            windows: Cache::builder()
                .time_to_idle(Duration::from_secs(2 * 60))
                .max_capacity(100_000)
                .build(),
        }
    }
}

impl RateLimiter {
    fn check(&self, key: RateKey, limit: u32) -> RateDecision {
        let window = self.windows.get_with(key, || {
            Arc::new(Mutex::new(RateWindow {
                started_at: Instant::now(),
                count: 0,
            }))
        });
        let mut window = window
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if window.started_at.elapsed() >= WINDOW {
            window.started_at = Instant::now();
            window.count = 0;
        }
        if window.count >= limit {
            return RateDecision {
                allowed: false,
                limit,
                remaining: 0,
                retry_after: WINDOW
                    .saturating_sub(window.started_at.elapsed())
                    .as_secs()
                    .max(1),
            };
        }
        window.count = window.count.saturating_add(1);
        RateDecision {
            allowed: true,
            limit,
            remaining: limit.saturating_sub(window.count),
            retry_after: 0,
        }
    }
}

pub async fn enforce(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(category) = RateCategory::for_request(&request) else {
        return next.run(request).await;
    };
    let Some(context) = request.extensions().get::<RequestContext>().cloned() else {
        return next.run(request).await;
    };
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::V4(Ipv4Addr::LOCALHOST), |address| address.ip());
    let limit = category.limit();
    let decision = state.rate_limiter.check(RateKey { ip, category }, limit);
    let mut response = if decision.allowed {
        next.run(request).await
    } else {
        ApiError::new(
            &context,
            StatusCode::TOO_MANY_REQUESTS,
            ApiErrorCode::RateLimited,
            "Too many requests were sent to this endpoint.",
            true,
        )
        .into_response()
    };
    insert_number_header(&mut response, RATE_LIMIT_HEADER, decision.limit);
    insert_number_header(&mut response, RATE_REMAINING_HEADER, decision.remaining);
    if !decision.allowed {
        insert_number_header(&mut response, RETRY_AFTER, decision.retry_after);
    }
    response
}

fn insert_number_header(response: &mut Response, name: HeaderName, value: impl ToString) {
    if let Ok(value) = HeaderValue::from_str(&value.to_string()) {
        response.headers_mut().insert(name, value);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum RateCategory {
    Search,
    Metadata,
    InstallPlan,
}

impl RateCategory {
    fn for_request(request: &Request) -> Option<Self> {
        let path = request.uri().path();
        if path == "/v1/modpacks" || path == "/v1/mods" {
            Some(Self::Search)
        } else if path.ends_with("/install-plan") {
            Some(Self::InstallPlan)
        } else if path.starts_with("/v1/") {
            Some(Self::Metadata)
        } else {
            None
        }
    }

    const fn limit(self) -> u32 {
        match self {
            Self::Search => 60,
            Self::Metadata => 120,
            Self::InstallPlan => 30,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RateKey {
    ip: IpAddr,
    category: RateCategory,
}

#[derive(Debug)]
struct RateWindow {
    started_at: Instant,
    count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RateDecision {
    allowed: bool,
    limit: u32,
    remaining: u32,
    retry_after: u64,
}

#[cfg(test)]
mod tests {
    use super::{RateCategory, RateKey, RateLimiter};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn fixed_window_enforces_its_limit() {
        let limiter = RateLimiter::default();
        let key = RateKey {
            ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            category: RateCategory::Search,
        };
        assert!(limiter.check(key, 2).allowed);
        assert!(limiter.check(key, 2).allowed);
        let rejected = limiter.check(key, 2);
        assert!(!rejected.allowed);
        assert_eq!(rejected.remaining, 0);
        assert!(rejected.retry_after > 0);
    }
}
