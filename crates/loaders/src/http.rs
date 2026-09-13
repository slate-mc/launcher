use reqwest::{Client, StatusCode, Url, redirect::Policy};
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct BoundedHttpClient {
    client: Client,
}

impl BoundedHttpClient {
    pub(crate) fn new(user_agent: &str) -> Result<Self, HttpError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(Policy::none())
            .user_agent(user_agent)
            .build()?;
        Ok(Self { client })
    }

    pub(crate) async fn get(
        &self,
        url: Url,
        expected_host: &'static str,
        maximum: usize,
    ) -> Result<Vec<u8>, HttpError> {
        validate_source(&url, expected_host)?;
        let mut response = self.client.get(url.clone()).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(HttpError::Status { url, status });
        }
        if let Some(length) = response.content_length()
            && length > maximum as u64
        {
            return Err(HttpError::DocumentTooLarge {
                actual: length,
                maximum,
            });
        }

        let mut bytes = Vec::with_capacity(
            response
                .content_length()
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or_default()
                .min(maximum),
        );
        while let Some(chunk) = response.chunk().await? {
            let next_length =
                bytes
                    .len()
                    .checked_add(chunk.len())
                    .ok_or(HttpError::DocumentTooLarge {
                        actual: u64::MAX,
                        maximum,
                    })?;
            if next_length > maximum {
                return Err(HttpError::DocumentTooLarge {
                    actual: next_length as u64,
                    maximum,
                });
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

fn validate_source(url: &Url, expected_host: &'static str) -> Result<(), HttpError> {
    if url.scheme() != "https" || url.host_str() != Some(expected_host) {
        return Err(HttpError::UntrustedSource {
            expected_host,
            actual: url.to_string(),
        });
    }
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return Err(HttpError::UntrustedSource {
            expected_host,
            actual: url.to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("could not create the loader metadata HTTP client")]
    Client(#[from] reqwest::Error),
    #[error("loader metadata source is not trusted; expected {expected_host}, got {actual}")]
    UntrustedSource {
        expected_host: &'static str,
        actual: String,
    },
    #[error("loader metadata request to {url} returned {status}")]
    Status { url: Url, status: StatusCode },
    #[error("loader response is too large: {actual} bytes exceeds {maximum}")]
    DocumentTooLarge { actual: u64, maximum: usize },
}
