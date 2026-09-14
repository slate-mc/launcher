use crate::cache::{CachePolicy, ResponseCaches};
use futures_util::StreamExt;
use moka::sync::Cache;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256, Sha512};
use std::sync::Arc;
use std::time::Duration;
use url::{Host, Url};
use uuid::Uuid;

const MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RETRY_AFTER_SECONDS: u64 = 10;

#[derive(Clone, Debug)]
pub struct UpstreamClient {
    base_url: Url,
    client: reqwest::Client,
    artifact_client: reqwest::Client,
    modrinth_client: reqwest::Client,
    artifact_metadata: Cache<String, Arc<ArtifactMetadata>>,
    caches: ResponseCaches,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactMetadata {
    pub size: u64,
    pub sha256: String,
    pub sha512: String,
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
        let artifact_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 5 {
                    return attempt.error(std::io::Error::other("redirect limit exceeded"));
                }
                if trusted_artifact_url(attempt.url()) {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .user_agent(user_agent)
            .build()?;
        let modrinth_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 {
                    return attempt.error(std::io::Error::other("redirect limit exceeded"));
                }
                if attempt.url().scheme() == "https"
                    && attempt.url().host_str() == Some("api.modrinth.com")
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
            artifact_client,
            modrinth_client,
            artifact_metadata: Cache::builder()
                .time_to_live(Duration::from_secs(30 * 60))
                .max_capacity(256)
                .build(),
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

    pub async fn artifact_metadata(
        &self,
        value: &str,
    ) -> Result<Arc<ArtifactMetadata>, UpstreamError> {
        let url = Url::parse(value).map_err(|_| UpstreamError::InvalidArtifactUrl)?;
        if !trusted_artifact_url(&url) {
            return Err(UpstreamError::InvalidArtifactUrl);
        }
        let key = url.as_str().to_owned();
        if let Some(metadata) = self.artifact_metadata.get(&key) {
            return Ok(metadata);
        }
        let metadata = Arc::new(self.fetch_artifact_metadata(url).await?);
        self.artifact_metadata.insert(key, metadata.clone());
        Ok(metadata)
    }

    pub async fn get_modrinth_json<T: DeserializeOwned>(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
        policy: CachePolicy,
    ) -> Result<T, UpstreamError> {
        let mut url = Url::parse("https://api.modrinth.com/v2/")
            .map_err(|_| UpstreamError::InvalidBaseUrl)?;
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
        let key = url.as_str().to_owned();
        if let Some(value) = self.caches.get(policy, &key) {
            return serde_json::from_value((*value).clone())
                .map_err(UpstreamError::InvalidResponse);
        }
        let value = Arc::new(fetch_json_with_client(&self.modrinth_client, url).await?);
        let parsed =
            serde_json::from_value((*value).clone()).map_err(UpstreamError::InvalidResponse)?;
        self.caches.insert(policy, key, value);
        Ok(parsed)
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

    async fn fetch_artifact_metadata(&self, url: Url) -> Result<ArtifactMetadata, UpstreamError> {
        let request_id = Uuid::new_v4();
        for attempt in 0_u32..=2 {
            let response = self.artifact_client.get(url.clone()).send().await;
            match response {
                Ok(response) if response.status().is_success() => {
                    if response
                        .content_length()
                        .is_some_and(|length| length > MAX_ARTIFACT_BYTES)
                    {
                        return Err(UpstreamError::ArtifactTooLarge);
                    }
                    let mut size = 0_u64;
                    let mut sha256 = Sha256::new();
                    let mut sha512 = Sha512::new();
                    let mut body = response.bytes_stream();
                    while let Some(chunk) = body.next().await {
                        let chunk = chunk?;
                        size = size
                            .checked_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX))
                            .ok_or(UpstreamError::ArtifactTooLarge)?;
                        if size > MAX_ARTIFACT_BYTES {
                            return Err(UpstreamError::ArtifactTooLarge);
                        }
                        sha256.update(&chunk);
                        sha512.update(&chunk);
                    }
                    let sha256 = sha256.finalize();
                    let sha512 = sha512.finalize();
                    return Ok(ArtifactMetadata {
                        size,
                        sha256: lowercase_hex(sha256.as_ref()),
                        sha512: lowercase_hex(sha512.as_ref()),
                    });
                }
                Ok(response) if response.status() == StatusCode::NOT_FOUND => {
                    return Err(UpstreamError::NotFound);
                }
                Ok(response)
                    if response.status() == StatusCode::TOO_MANY_REQUESTS
                        || response.status().is_server_error() =>
                {
                    if attempt == 2 {
                        return Err(if response.status() == StatusCode::TOO_MANY_REQUESTS {
                            UpstreamError::RateLimited
                        } else {
                            UpstreamError::Unavailable
                        });
                    }
                    let retry_after = response.headers().get(reqwest::header::RETRY_AFTER);
                    tokio::time::sleep(retry_delay(attempt, request_id, retry_after)).await;
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

async fn fetch_json_with_client(
    client: &reqwest::Client,
    url: Url,
) -> Result<Value, UpstreamError> {
    let request_id = Uuid::new_v4();
    for attempt in 0_u32..=2 {
        let response = client.get(url.clone()).send().await;
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
            Ok(response)
                if response.status() == StatusCode::TOO_MANY_REQUESTS
                    || response.status().is_server_error() =>
            {
                if attempt == 2 {
                    return Err(if response.status() == StatusCode::TOO_MANY_REQUESTS {
                        UpstreamError::RateLimited
                    } else {
                        UpstreamError::Unavailable
                    });
                }
                tokio::time::sleep(retry_delay(
                    attempt,
                    request_id,
                    response.headers().get(reqwest::header::RETRY_AFTER),
                ))
                .await;
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

fn trusted_artifact_url(url: &Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    match url.host() {
        Some(Host::Domain(domain)) => {
            let domain = domain.to_ascii_lowercase();
            domain == "edge.forgecdn.net"
                || domain.ends_with(".forgecdn.net")
                || domain == "cdn.modrinth.com"
                || domain.ends_with(".modpacks.ch")
        }
        _ => false,
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(DIGITS[usize::from(byte >> 4)]));
        value.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    value
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
    #[error("artifact URL is not an allowlisted HTTPS provider URL")]
    InvalidArtifactUrl,
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
    #[error("provider artifact exceeded the size limit")]
    ArtifactTooLarge,
    #[error("upstream request failed")]
    Request(#[from] reqwest::Error),
    #[error("upstream returned an invalid response")]
    InvalidResponse(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::{lowercase_hex, trusted_artifact_url};
    use url::Url;

    #[test]
    fn artifact_inspection_is_restricted_to_known_provider_hosts() {
        for allowed in [
            "https://edge.forgecdn.net/files/example.zip",
            "https://mediafilez.forgecdn.net/files/example.zip",
            "https://cdn.modrinth.com/data/example.jar",
            "https://apps.modpacks.ch/modpacks/example.zip",
        ] {
            assert!(
                Url::parse(allowed)
                    .ok()
                    .as_ref()
                    .is_some_and(trusted_artifact_url),
                "{allowed}"
            );
        }
        for blocked in [
            "http://edge.forgecdn.net/files/example.zip",
            "https://localhost/example.zip",
            "https://127.0.0.1/example.zip",
            "https://forgecdn.net.example.test/example.zip",
        ] {
            assert!(
                !Url::parse(blocked)
                    .ok()
                    .as_ref()
                    .is_some_and(trusted_artifact_url),
                "{blocked}"
            );
        }
    }

    #[test]
    fn digest_bytes_are_encoded_as_lowercase_hex() {
        assert_eq!(lowercase_hex(&[0x00, 0x1f, 0xa0, 0xff]), "001fa0ff");
    }
}
