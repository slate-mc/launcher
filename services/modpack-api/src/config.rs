use std::net::SocketAddr;
use url::Url;

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
const DEFAULT_UPSTREAM_URL: &str = "https://api.modpacks.ch";
const DEFAULT_USER_AGENT: &str = "slate-api/0.1 (+https://slate.gg)";
const DEFAULT_RELEASE_MANIFEST_URL: &str =
    "https://github.com/slate-mc/launcher/releases/latest/download/latest.json";
const DEFAULT_POSTHOG_HOST: &str = "https://us.i.posthog.com";

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub upstream_url: Url,
    pub upstream_user_agent: String,
    pub release_manifest_url: Url,
    pub posthog_host: Url,
    pub posthog_project_token: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_address = std::env::var("SLATE_API_BIND")
            .unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_owned())
            .parse()
            .map_err(ConfigError::InvalidBindAddress)?;
        let upstream_url = Url::parse(
            &std::env::var("SLATE_MODPACKS_CH_URL")
                .unwrap_or_else(|_| DEFAULT_UPSTREAM_URL.to_owned()),
        )
        .map_err(ConfigError::InvalidUpstreamUrl)?;
        let upstream_user_agent = std::env::var("SLATE_UPSTREAM_USER_AGENT")
            .unwrap_or_else(|_| DEFAULT_USER_AGENT.to_owned());
        if upstream_user_agent.trim().is_empty() || upstream_user_agent.len() > 256 {
            return Err(ConfigError::InvalidUserAgent);
        }
        let release_manifest_url = Url::parse(
            &std::env::var("SLATE_LAUNCHER_RELEASE_MANIFEST_URL")
                .unwrap_or_else(|_| DEFAULT_RELEASE_MANIFEST_URL.to_owned()),
        )
        .map_err(ConfigError::InvalidReleaseManifestUrl)?;
        if release_manifest_url.scheme() != "https"
            || release_manifest_url.host_str().is_none()
            || !release_manifest_url.username().is_empty()
            || release_manifest_url.password().is_some()
        {
            return Err(ConfigError::UnsafeReleaseManifestUrl);
        }
        let posthog_host = Url::parse(
            &std::env::var("SLATE_POSTHOG_HOST")
                .unwrap_or_else(|_| DEFAULT_POSTHOG_HOST.to_owned()),
        )
        .map_err(ConfigError::InvalidPostHogHost)?;
        if posthog_host.scheme() != "https"
            || posthog_host.host_str().is_none()
            || !posthog_host.username().is_empty()
            || posthog_host.password().is_some()
        {
            return Err(ConfigError::UnsafePostHogHost);
        }
        let posthog_project_token = std::env::var("SLATE_POSTHOG_PROJECT_TOKEN")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if posthog_project_token
            .as_ref()
            .is_some_and(|value| value.len() > 256)
        {
            return Err(ConfigError::InvalidPostHogProjectToken);
        }
        Ok(Self {
            bind_address,
            upstream_url,
            upstream_user_agent,
            release_manifest_url,
            posthog_host,
            posthog_project_token,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("SLATE_API_BIND is not a valid socket address")]
    InvalidBindAddress(std::net::AddrParseError),
    #[error("SLATE_MODPACKS_CH_URL is not a valid URL")]
    InvalidUpstreamUrl(url::ParseError),
    #[error("SLATE_LAUNCHER_RELEASE_MANIFEST_URL is not a valid URL")]
    InvalidReleaseManifestUrl(url::ParseError),
    #[error("SLATE_LAUNCHER_RELEASE_MANIFEST_URL must be a public HTTPS URL")]
    UnsafeReleaseManifestUrl,
    #[error("SLATE_POSTHOG_HOST is not a valid URL")]
    InvalidPostHogHost(url::ParseError),
    #[error("SLATE_POSTHOG_HOST must be a public HTTPS URL")]
    UnsafePostHogHost,
    #[error("SLATE_POSTHOG_PROJECT_TOKEN is too long")]
    InvalidPostHogProjectToken,
    #[error("SLATE_UPSTREAM_USER_AGENT must contain between 1 and 256 characters")]
    InvalidUserAgent,
}
