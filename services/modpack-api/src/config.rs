use std::net::SocketAddr;
use url::Url;

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
const DEFAULT_UPSTREAM_URL: &str = "https://api.modpacks.ch";
const DEFAULT_USER_AGENT: &str = "slate-api/0.1 (+https://slate.gg)";

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub upstream_url: Url,
    pub upstream_user_agent: String,
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
        Ok(Self {
            bind_address,
            upstream_url,
            upstream_user_agent,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("SLATE_API_BIND is not a valid socket address")]
    InvalidBindAddress(std::net::AddrParseError),
    #[error("SLATE_MODPACKS_CH_URL is not a valid URL")]
    InvalidUpstreamUrl(url::ParseError),
    #[error("SLATE_UPSTREAM_USER_AGENT must contain between 1 and 256 characters")]
    InvalidUserAgent,
}
