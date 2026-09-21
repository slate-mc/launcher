use moka::sync::Cache;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use slate_modpack_api_contracts::{CaptureProductEventRequest, LauncherFeatureConfig};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug)]
pub struct PostHogRelay {
    inner: Option<PostHogRelayInner>,
    feature_cache: Cache<&'static str, Arc<LauncherFeatureConfig>>,
}

#[derive(Clone, Debug)]
struct PostHogRelayInner {
    project_token: String,
    capture_url: Url,
    flags_url: Url,
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

#[derive(Serialize)]
struct PostHogFlagsRequest<'a> {
    api_key: &'a str,
    distinct_id: &'static str,
    evaluation_contexts: [&'static str; 2],
}

#[derive(Debug, Deserialize)]
struct PostHogFlagsResponse {
    #[serde(default)]
    flags: BTreeMap<String, PostHogFlag>,
    #[serde(default, rename = "errorsWhileComputingFlags")]
    errors_while_computing_flags: bool,
}

#[derive(Debug, Deserialize)]
struct PostHogFlag {
    enabled: bool,
    #[serde(default)]
    metadata: PostHogFlagMetadata,
}

#[derive(Debug, Default, Deserialize)]
struct PostHogFlagMetadata {
    #[serde(default)]
    payload: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RolloutPayload {
    rollout_percentage: u16,
}

impl PostHogRelay {
    pub fn new(
        host: Url,
        project_token: Option<String>,
        user_agent: &str,
    ) -> Result<Self, TelemetryError> {
        let Some(project_token) = project_token else {
            return Ok(Self {
                inner: None,
                feature_cache: feature_cache(),
            });
        };
        let mut capture_url = host;
        capture_url.set_path("/i/v0/e/");
        capture_url.set_query(None);
        capture_url.set_fragment(None);
        let mut flags_url = capture_url.clone();
        flags_url.set_path("/flags");
        flags_url.set_query(Some("v=2"));
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
                flags_url,
                client,
            }),
            feature_cache: feature_cache(),
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

    pub async fn feature_config(&self) -> Result<LauncherFeatureConfig, TelemetryError> {
        if let Some(config) = self.feature_cache.get("launcher") {
            return Ok(*config);
        }
        let Some(inner) = &self.inner else {
            return Ok(LauncherFeatureConfig::default());
        };
        let response = inner
            .client
            .post(inner.flags_url.clone())
            .json(&PostHogFlagsRequest {
                api_key: &inner.project_token,
                distinct_id: "slate-global-desktop",
                evaluation_contexts: ["production", "desktop"],
            })
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(TelemetryError::Unavailable);
        }
        if response
            .content_length()
            .is_some_and(|length| length > 1024 * 1024)
        {
            return Err(TelemetryError::InvalidFlags);
        }
        let body = response.bytes().await?;
        if body.len() > 1024 * 1024 {
            return Err(TelemetryError::InvalidFlags);
        }
        let flags: PostHogFlagsResponse = serde_json::from_slice(&body)?;
        if flags.errors_while_computing_flags {
            return Err(TelemetryError::Unavailable);
        }
        let config = LauncherFeatureConfig {
            installs_enabled: !enabled(&flags.flags, "emergency-disable-installs"),
            launch_enabled: !enabled(&flags.flags, "emergency-disable-launch"),
            authentication_enabled: !enabled(&flags.flags, "emergency-disable-authentication"),
            support_reports_enabled: !enabled(&flags.flags, "emergency-disable-support-reports"),
            discover_preview_rollout: rollout(&flags.flags, "discover-preview"),
            cache_seconds: 300,
            remote_available: true,
        };
        self.feature_cache.insert("launcher", Arc::new(config));
        Ok(config)
    }
}

fn feature_cache() -> Cache<&'static str, Arc<LauncherFeatureConfig>> {
    Cache::builder()
        .time_to_live(Duration::from_secs(120))
        .max_capacity(1)
        .build()
}

fn enabled(flags: &BTreeMap<String, PostHogFlag>, key: &str) -> bool {
    flags.get(key).is_some_and(|flag| flag.enabled)
}

fn rollout(flags: &BTreeMap<String, PostHogFlag>, key: &str) -> u8 {
    let Some(flag) = flags.get(key).filter(|flag| flag.enabled) else {
        return 0;
    };
    flag.metadata
        .payload
        .as_deref()
        .and_then(|payload| serde_json::from_str::<RolloutPayload>(payload).ok())
        .map_or(100, |payload| {
            u8::try_from(payload.rollout_percentage.min(100)).unwrap_or(100)
        })
}

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("the analytics relay is unavailable")]
    Unavailable,
    #[error("the analytics event was rejected")]
    Rejected,
    #[error("the analytics request failed")]
    Request(#[from] reqwest::Error),
    #[error("the feature response is invalid")]
    Json(#[from] serde_json::Error),
    #[error("the feature configuration is invalid")]
    InvalidFlags,
}

#[cfg(test)]
mod tests {
    use super::{PostHogFlag, PostHogFlagMetadata, PostHogRelay, enabled, rollout};
    use slate_modpack_api_contracts::{CaptureProductEventRequest, ProductEvent, ProductPlatform};
    use std::collections::BTreeMap;
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
        assert_eq!(relay.feature_config().await?, Default::default());
        Ok(())
    }

    #[test]
    fn feature_controls_are_allowlisted_and_rollouts_are_bounded() {
        let flags = BTreeMap::from([
            (
                "emergency-disable-launch".to_owned(),
                PostHogFlag {
                    enabled: true,
                    metadata: PostHogFlagMetadata::default(),
                },
            ),
            (
                "discover-preview".to_owned(),
                PostHogFlag {
                    enabled: true,
                    metadata: PostHogFlagMetadata {
                        payload: Some(r#"{"rollout_percentage":250}"#.to_owned()),
                    },
                },
            ),
        ]);
        assert!(enabled(&flags, "emergency-disable-launch"));
        assert!(!enabled(&flags, "unknown"));
        assert_eq!(rollout(&flags, "discover-preview"), 100);
        assert_eq!(rollout(&flags, "unknown"), 0);
    }
}
