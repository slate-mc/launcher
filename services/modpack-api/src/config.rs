use std::net::SocketAddr;
use url::Url;

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
const DEFAULT_UPSTREAM_URL: &str = "https://api.modpacks.ch";
const DEFAULT_USER_AGENT: &str = "slate-api/0.1 (+https://slate.gg)";
const DEFAULT_RELEASE_MANIFEST_URL: &str =
    "https://github.com/slate-mc/launcher/releases/latest/download/latest.json";
const DEFAULT_POSTHOG_HOST: &str = "https://us.i.posthog.com";
const DEFAULT_SUPPORT_REPORT_PREFIX: &str = "support-reports";

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub upstream_url: Url,
    pub upstream_user_agent: String,
    pub release_manifest_url: Url,
    pub beta_release_manifest_url: Option<Url>,
    pub stable_release_rollout: u8,
    pub beta_release_rollout: u8,
    pub posthog_host: Url,
    pub posthog_project_token: Option<String>,
    pub observability: ObservabilityConfig,
    pub sentry_dsn: Option<sentry::types::Dsn>,
    pub support_reports: SupportReportStorageConfig,
}

#[derive(Clone, Debug)]
pub struct ObservabilityConfig {
    pub enabled: bool,
    pub sample_ratio: f64,
    pub environment: String,
}

#[derive(Clone, Debug)]
pub struct SupportReportStorageConfig {
    pub bucket: Option<String>,
    pub prefix: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReadinessSummary {
    pub environment: String,
    pub sentry_configured: bool,
    pub posthog_configured: bool,
    pub otel_configured: bool,
    pub support_report_storage_configured: bool,
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
        let beta_release_manifest_url = std::env::var("SLATE_LAUNCHER_BETA_RELEASE_MANIFEST_URL")
            .ok()
            .map(|value| Url::parse(&value))
            .transpose()
            .map_err(ConfigError::InvalidBetaReleaseManifestUrl)?;
        if beta_release_manifest_url.as_ref().is_some_and(|url| {
            url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
        }) {
            return Err(ConfigError::UnsafeBetaReleaseManifestUrl);
        }
        let stable_release_rollout = rollout_percentage("SLATE_LAUNCHER_STABLE_ROLLOUT_PERCENT")?;
        let beta_release_rollout = rollout_percentage("SLATE_LAUNCHER_BETA_ROLLOUT_PERCENT")?;
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
        let support_reports = SupportReportStorageConfig::from_env()?;
        let config = Self {
            bind_address,
            upstream_url,
            upstream_user_agent,
            release_manifest_url,
            beta_release_manifest_url,
            stable_release_rollout,
            beta_release_rollout,
            posthog_host,
            posthog_project_token,
            observability,
            sentry_dsn,
            support_reports,
        };
        config.validate_production_services()?;
        Ok(config)
    }

    #[must_use]
    pub fn production_readiness_summary(&self) -> ProductionReadinessSummary {
        ProductionReadinessSummary {
            environment: self.observability.environment.clone(),
            sentry_configured: self.sentry_dsn.is_some(),
            posthog_configured: self.posthog_project_token.is_some(),
            otel_configured: self.observability.enabled,
            support_report_storage_configured: self.support_reports.bucket.is_some(),
        }
    }

    fn validate_production_services(&self) -> Result<(), ConfigError> {
        validate_production_services(
            &self.observability.environment,
            self.sentry_dsn.is_some(),
            self.posthog_project_token.is_some(),
            self.observability.enabled,
            self.support_reports.bucket.is_some(),
        )
    }
}

fn validate_production_services(
    environment: &str,
    sentry_configured: bool,
    posthog_configured: bool,
    otel_configured: bool,
    support_reports_configured: bool,
) -> Result<(), ConfigError> {
    if environment != "production" {
        return Ok(());
    }
    for (configured, variable) in [
        (sentry_configured, "SLATE_SENTRY_DSN"),
        (posthog_configured, "SLATE_POSTHOG_PROJECT_TOKEN"),
        (otel_configured, "SLATE_OTEL_ENABLED=true"),
        (support_reports_configured, "SLATE_SUPPORT_REPORTS_BUCKET"),
    ] {
        if !configured {
            return Err(ConfigError::MissingProductionService(variable));
        }
    }
    Ok(())
}

fn rollout_percentage(variable: &'static str) -> Result<u8, ConfigError> {
    let value = std::env::var(variable).unwrap_or_else(|_| "100".to_owned());
    let parsed = value
        .parse::<u8>()
        .map_err(|source| ConfigError::InvalidRolloutPercentage { variable, source })?;
    if parsed > 100 {
        return Err(ConfigError::RolloutPercentageOutOfRange { variable });
    }
    Ok(parsed)
}

impl SupportReportStorageConfig {
    fn from_env() -> Result<Self, ConfigError> {
        if let Ok(endpoint) = std::env::var("AWS_ENDPOINT_URL_S3") {
            validate_support_endpoint(&endpoint)?;
        }
        let bucket = std::env::var("SLATE_SUPPORT_REPORTS_BUCKET")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if bucket.as_ref().is_some_and(|value| {
            value.len() > 255
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        }) {
            return Err(ConfigError::InvalidSupportReportsBucket);
        }
        let prefix = std::env::var("SLATE_SUPPORT_REPORTS_PREFIX")
            .unwrap_or_else(|_| DEFAULT_SUPPORT_REPORT_PREFIX.to_owned())
            .trim_matches('/')
            .to_owned();
        if prefix.is_empty()
            || prefix.len() > 128
            || !prefix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_'))
            || prefix.split('/').any(|segment| segment.is_empty())
        {
            return Err(ConfigError::InvalidSupportReportsPrefix);
        }
        Ok(Self { bucket, prefix })
    }
}

fn validate_support_endpoint(value: &str) -> Result<(), ConfigError> {
    let endpoint = Url::parse(value).map_err(ConfigError::InvalidSupportReportsEndpoint)?;
    if !endpoint.username().is_empty() || endpoint.password().is_some() || endpoint.host().is_none()
    {
        return Err(ConfigError::UnsafeSupportReportsEndpoint);
    }
    let secure = endpoint.scheme() == "https";
    let local_http = endpoint.scheme() == "http"
        && endpoint
            .host_str()
            .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"));
    if secure || local_http {
        Ok(())
    } else {
        Err(ConfigError::UnsafeSupportReportsEndpoint)
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
    #[error("SLATE_LAUNCHER_BETA_RELEASE_MANIFEST_URL is not a valid URL")]
    InvalidBetaReleaseManifestUrl(url::ParseError),
    #[error("SLATE_LAUNCHER_BETA_RELEASE_MANIFEST_URL must be a public HTTPS URL")]
    UnsafeBetaReleaseManifestUrl,
    #[error("{variable} must be an integer from 0 through 100")]
    InvalidRolloutPercentage {
        variable: &'static str,
        source: std::num::ParseIntError,
    },
    #[error("{variable} must be from 0 through 100")]
    RolloutPercentageOutOfRange { variable: &'static str },
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
    #[error("SLATE_SUPPORT_REPORTS_BUCKET is invalid")]
    InvalidSupportReportsBucket,
    #[error("SLATE_SUPPORT_REPORTS_PREFIX is invalid")]
    InvalidSupportReportsPrefix,
    #[error("AWS_ENDPOINT_URL_S3 is not a valid URL")]
    InvalidSupportReportsEndpoint(url::ParseError),
    #[error("AWS_ENDPOINT_URL_S3 must use HTTPS, except for a local development endpoint")]
    UnsafeSupportReportsEndpoint,
    #[error("production requires {0}")]
    MissingProductionService(&'static str),
}

#[cfg(test)]
mod tests {
    use super::{
        parse_enabled, validate_otel_endpoint, validate_production_services,
        validate_support_endpoint,
    };

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

    #[test]
    fn support_storage_requires_https_except_for_local_development() {
        assert!(validate_support_endpoint("https://account.r2.cloudflarestorage.com").is_ok());
        assert!(validate_support_endpoint("http://127.0.0.1:9000").is_ok());
        assert!(validate_support_endpoint("http://object-storage.example.com").is_err());
        assert!(validate_support_endpoint("https://user:secret@example.com").is_err());
    }

    #[test]
    fn production_requires_every_operational_service() {
        assert!(validate_production_services("development", false, false, false, false).is_ok());
        assert!(validate_production_services("production", true, true, true, true).is_ok());
        assert!(validate_production_services("production", true, false, true, true).is_err());
        assert!(validate_production_services("production", true, true, false, true).is_err());
        assert!(validate_production_services("production", true, true, true, false).is_err());
    }
}
