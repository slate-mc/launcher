use crate::cache::{CachePolicy, ResponseCaches};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use url::Url;
use uuid::Uuid;

const MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_RETRY_AFTER_SECONDS: u64 = 10;

#[derive(Clone, Debug)]
pub struct UpstreamClient {
    base_url: Url,
    client: reqwest::Client,
    caches: ResponseCaches,
}

impl UpstreamClient {
    pub fn new(base_url: Url, user_agent: &str) -> Result<Self, UpstreamError> {
        if base_url.scheme() != "https"
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
        {
            return Err(UpstreamError::InvalidBaseUrl);
        }
        let allowed_redirect_host = base_url
            .host_str()
            .ok_or(UpstreamError::InvalidBaseUrl)?
            .to_owned();
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 5 {
                    return attempt.error(std::io::Error::other("redirect limit exceeded"));
                }
                if attempt.url().scheme() == "https"
                    && attempt.url().host_str() == Some(allowed_redirect_host.as_str())
                {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .user_agent(user_agent)
            .build()?;
        Ok(Self {
            base_url,
            client,
            caches: ResponseCaches::default(),
        })
    }

    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
        policy: CachePolicy,
    ) -> Result<T, UpstreamError> {
        let url = self.url(path, query)?;
        let key = url.as_str().to_owned();
        if let Some(value) = self.caches.get(policy, &key) {
            return serde_json::from_value((*value).clone())
                .map_err(UpstreamError::InvalidResponse);
        }

        let value = Arc::new(self.fetch_json(url, policy == CachePolicy::Version).await?);
        let parsed =
            serde_json::from_value((*value).clone()).map_err(UpstreamError::InvalidResponse)?;
        self.caches.insert(policy, key, value);
        Ok(parsed)
    }

    pub async fn health(&self) -> bool {
        let key = self.base_url.as_str();
        if let Some(healthy) = self.caches.health(key) {
            return healthy;
        }
        let Ok(url) = self.url(&["health"], &[]) else {
            return false;
        };
        let healthy = self
            .client
            .get(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success());
        self.caches.insert_health(key.to_owned(), healthy);
        healthy
    }

    async fn fetch_json(&self, url: Url, manifest: bool) -> Result<Value, UpstreamError> {
        let request_id = Uuid::new_v4();
        let timeout = if manifest {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(15)
        };
        for attempt in 0_u32..=2 {
            let response = self.client.get(url.clone()).timeout(timeout).send().await;
            match response {
                Ok(response) if response.status().is_success() => {
                    if response
                        .content_length()
                        .is_some_and(|length| length > MAX_RESPONSE_BYTES)
                    {
                        return Err(UpstreamError::ResponseTooLarge);
                    }
                    let bytes = response.bytes().await?;
                    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_RESPONSE_BYTES {
                        return Err(UpstreamError::ResponseTooLarge);
                    }
                    return serde_json::from_slice(&bytes).map_err(UpstreamError::InvalidResponse);
                }
                Ok(response) if response.status() == StatusCode::NOT_FOUND => {
                    return Err(UpstreamError::NotFound);
                }
                Ok(response) if response.status() == StatusCode::TOO_MANY_REQUESTS => {
                    if attempt == 2 {
                        return Err(UpstreamError::RateLimited);
                    }
                    let delay = retry_delay(
                        attempt,
                        request_id,
                        response.headers().get(reqwest::header::RETRY_AFTER),
                    );
                    tokio::time::sleep(delay).await;
                }
                Ok(response) if response.status().is_server_error() => {
                    if attempt == 2 {
                        return Err(UpstreamError::Unavailable);
                    }
                    tokio::time::sleep(retry_delay(attempt, request_id, None)).await;
                }
                Ok(response) => return Err(UpstreamError::Rejected(response.status())),
                Err(error) if error.is_timeout() || error.is_connect() => {
                    if attempt == 2 {
                        return Err(UpstreamError::Unavailable);
                    }
                    tokio::time::sleep(retry_delay(attempt, request_id, None)).await;
                }
                Err(error) => return Err(UpstreamError::Request(error)),
            }
        }
        Err(UpstreamError::Unavailable)
    }

    fn url(&self, path: &[&str], query: &[(&str, &str)]) -> Result<Url, UpstreamError> {
        let mut url = self.base_url.clone();
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| UpstreamError::InvalidBaseUrl)?;
            segments.pop_if_empty();
            for segment in path {
                if segment.is_empty() {
                    return Err(UpstreamError::InvalidPathSegment);
                }
                segments.push(segment);
            }
        }
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query.iter().copied());
        }
        Ok(url)
    }
}

fn retry_delay(
    attempt: u32,
    request_id: Uuid,
    retry_after: Option<&reqwest::header::HeaderValue>,
) -> Duration {
    if let Some(seconds) = retry_after
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        return Duration::from_secs(seconds.min(MAX_RETRY_AFTER_SECONDS));
    }
    let jitter = u64::from(request_id.as_bytes()[15] % 100);
    Duration::from_millis((150_u64 * 2_u64.pow(attempt)).saturating_add(jitter))
}

#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    #[error("upstream base URL is not a safe HTTPS URL")]
    InvalidBaseUrl,
    #[error("upstream path segment is empty")]
    InvalidPathSegment,
    #[error("upstream resource was not found")]
    NotFound,
    #[error("upstream rate limit was reached")]
    RateLimited,
    #[error("upstream service is unavailable")]
    Unavailable,
    #[error("upstream rejected the request with {0}")]
    Rejected(StatusCode),
    #[error("upstream response exceeded the size limit")]
    ResponseTooLarge,
    #[error("upstream request failed")]
    Request(#[from] reqwest::Error),
    #[error("upstream returned an invalid response")]
    InvalidResponse(serde_json::Error),
}
