use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    CurseForge,
    Modrinth,
    Ftb,
}

impl Provider {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CurseForge => "curseforge",
            Self::Modrinth => "modrinth",
            Self::Ftb => "ftb",
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::CurseForge => "CurseForge",
            Self::Modrinth => "Modrinth",
            Self::Ftb => "FTB",
        }
    }
}

impl Display for Provider {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = InvalidProvider;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "curseforge" => Ok(Self::CurseForge),
            "modrinth" => Ok(Self::Modrinth),
            "ftb" => Ok(Self::Ftb),
            _ => Err(InvalidProvider),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown modpack provider")]
pub struct InvalidProvider;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ProviderMetadata {
    pub id: Provider,
    pub name: String,
    pub available: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderMetadata>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderStatus {
    Ok,
    Unavailable,
}
