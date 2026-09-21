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
    pub observability: ObservabilityConfig,
    pub sentry_dsn: Option<sentry::types::Dsn>,
}

#[derive(Clone, Debug)]
pub struct ObservabilityConfig {
    pub enabled: bool,
    pub sample_ratio: f64,
    pub environment: String,
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
        let observability = ObservabilityConfig::from_env()?;
        let sentry_dsn = std::env::var("SLATE_SENTRY_DSN")
            .ok()
            .map(|value| value.parse())
            .transpose()
            .map_err(ConfigError::InvalidSentryDsn)?;
        Ok(Self {
            bind_address,
            upstream_url,
            upstream_user_agent,
            release_manifest_url,
            posthog_host,
            posthog_project_token,
            observability,
            sentry_dsn,
        })
    }
}

impl ObservabilityConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let enabled = std::env::var("SLATE_OTEL_ENABLED")
            .ok()
            .map(|value| parse_enabled(&value))
            .transpose()?
            .unwrap_or(false);
        let sample_ratio = std::env::var("SLATE_OTEL_SAMPLE_RATIO")
            .ok()
            .map(|value| value.parse::<f64>())
            .transpose()
            .map_err(ConfigError::InvalidOtelSampleRatio)?
            .unwrap_or(0.1);
        if !sample_ratio.is_finite() || !(0.0..=1.0).contains(&sample_ratio) {
            return Err(ConfigError::OtelSampleRatioOutOfRange);
        }
        let environment = std::env::var("SLATE_DEPLOYMENT_ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_owned());
        if environment.is_empty()
            || environment.len() > 64
            || !environment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(ConfigError::InvalidDeploymentEnvironment);
        }
        if enabled {
            let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .map_err(|_| ConfigError::MissingOtelEndpoint)?;
            validate_otel_endpoint(&endpoint)?;
        }
        Ok(Self {
            enabled,
            sample_ratio,
            environment,
        })
    }
}

fn parse_enabled(value: &str) -> Result<bool, ConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::InvalidOtelEnabled),
    }
}

fn validate_otel_endpoint(value: &str) -> Result<(), ConfigError> {
    let endpoint = Url::parse(value).map_err(ConfigError::InvalidOtelEndpoint)?;
    if !endpoint.username().is_empty() || endpoint.password().is_some() || endpoint.host().is_none()
    {
        return Err(ConfigError::UnsafeOtelEndpoint);
    }
    let secure = endpoint.scheme() == "https";
    let local_http = endpoint.scheme() == "http"
        && endpoint
            .host_str()
            .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"));
    if !secure && !local_http {
        return Err(ConfigError::UnsafeOtelEndpoint);
    }
    Ok(())
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
    #[error("SLATE_OTEL_ENABLED must be true or false")]
    InvalidOtelEnabled,
    #[error("SLATE_OTEL_SAMPLE_RATIO is not a number")]
    InvalidOtelSampleRatio(std::num::ParseFloatError),
    #[error("SLATE_OTEL_SAMPLE_RATIO must be between 0 and 1")]
    OtelSampleRatioOutOfRange,
    #[error("SLATE_DEPLOYMENT_ENVIRONMENT is invalid")]
    InvalidDeploymentEnvironment,
    #[error("OTEL_EXPORTER_OTLP_ENDPOINT is required when telemetry export is enabled")]
    MissingOtelEndpoint,
    #[error("OTEL_EXPORTER_OTLP_ENDPOINT is not a valid URL")]
    InvalidOtelEndpoint(url::ParseError),
    #[error("OTEL_EXPORTER_OTLP_ENDPOINT must use HTTPS, except for a local collector")]
    UnsafeOtelEndpoint,
    #[error("SLATE_SENTRY_DSN is not a valid Sentry DSN")]
    InvalidSentryDsn(sentry::types::ParseDsnError),
}

#[cfg(test)]
mod tests {
    use super::{parse_enabled, validate_otel_endpoint};

    #[test]
    fn otel_switch_accepts_explicit_boolean_values() {
        assert!(parse_enabled("true").is_ok_and(|value| value));
        assert!(parse_enabled("0").is_ok_and(|value| !value));
        assert!(parse_enabled("sometimes").is_err());
    }

    #[test]
    fn otel_endpoint_requires_https_except_for_local_collectors() {
        assert!(validate_otel_endpoint("https://otlp-gateway.example.com").is_ok());
        assert!(validate_otel_endpoint("http://127.0.0.1:4318").is_ok());
        assert!(validate_otel_endpoint("http://collector.internal:4318").is_err());
        assert!(validate_otel_endpoint("https://user:secret@example.com").is_err());
    }
}
