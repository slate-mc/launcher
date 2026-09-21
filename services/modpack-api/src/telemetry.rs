use reqwest::StatusCode;
use serde::Serialize;
use slate_modpack_api_contracts::CaptureProductEventRequest;
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug)]
pub struct PostHogRelay {
    inner: Option<PostHogRelayInner>,
}

#[derive(Clone, Debug)]
struct PostHogRelayInner {
    project_token: String,
    capture_url: Url,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct PostHogEvent<'a> {
    api_key: &'a str,
    distinct_id: String,
    event: &'static str,
    properties: PostHogProperties<'a>,
}

#[derive(Serialize)]
struct PostHogProperties<'a> {
    #[serde(rename = "$process_person_profile")]
    process_person_profile: bool,
    #[serde(rename = "$lib")]
    library: &'static str,
    #[serde(rename = "$lib_version")]
    library_version: &'static str,
    app_version: &'a str,
    platform: &'a str,
    architecture: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    loader: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<&'a str>,
}

impl PostHogRelay {
    pub fn new(
        host: Url,
        project_token: Option<String>,
        user_agent: &str,
    ) -> Result<Self, TelemetryError> {
        let Some(project_token) = project_token else {
            return Ok(Self { inner: None });
        };
        let mut capture_url = host;
        capture_url.set_path("/i/v0/e/");
        capture_url.set_query(None);
        capture_url.set_fragment(None);
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(user_agent)
            .build()?;
        Ok(Self {
            inner: Some(PostHogRelayInner {
                project_token,
                capture_url,
                client,
            }),
        })
    }

    #[must_use]
    pub const fn available(&self) -> bool {
        self.inner.is_some()
    }

    pub async fn capture(
        &self,
        request: &CaptureProductEventRequest,
    ) -> Result<bool, TelemetryError> {
        let Some(inner) = &self.inner else {
            return Ok(false);
        };
        let event = PostHogEvent {
            api_key: &inner.project_token,
            distinct_id: request.installation_id.to_string(),
            event: request.event.as_str(),
            properties: PostHogProperties {
                process_person_profile: false,
                library: "slate-api",
                library_version: env!("CARGO_PKG_VERSION"),
                app_version: &request.app_version,
                platform: request.platform.as_str(),
                architecture: &request.architecture,
                loader: request.loader.as_deref(),
                provider: request.provider.as_deref(),
            },
        };
        let response = inner
            .client
            .post(inner.capture_url.clone())
            .json(&event)
            .send()
            .await?;
        if response.status().is_success() {
            Ok(true)
        } else if response.status() == StatusCode::TOO_MANY_REQUESTS
            || response.status().is_server_error()
        {
            Err(TelemetryError::Unavailable)
        } else {
            Err(TelemetryError::Rejected)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("the analytics relay is unavailable")]
    Unavailable,
    #[error("the analytics event was rejected")]
    Rejected,
    #[error("the analytics request failed")]
    Request(#[from] reqwest::Error),
}

#[cfg(test)]
mod tests {
    use super::PostHogRelay;
    use slate_modpack_api_contracts::{CaptureProductEventRequest, ProductEvent, ProductPlatform};
    use url::Url;
    use uuid::Uuid;

    #[tokio::test]
    async fn disabled_relay_accepts_no_data() -> Result<(), Box<dyn std::error::Error>> {
        let relay = PostHogRelay::new(Url::parse("https://us.i.posthog.com")?, None, "slate-test")?;
        let accepted = relay
            .capture(&CaptureProductEventRequest {
                installation_id: Uuid::new_v4(),
                event: ProductEvent::LauncherStarted,
                app_version: "0.1.0".to_owned(),
                platform: ProductPlatform::Windows,
                architecture: "x86_64".to_owned(),
                loader: None,
                provider: None,
            })
            .await?;
        assert!(!accepted);
        assert!(!relay.available());
        Ok(())
    }
}
