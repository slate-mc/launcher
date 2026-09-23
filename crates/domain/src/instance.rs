use serde::{Deserialize, Serialize};
use std::fmt;

const MAX_INSTANCE_NAME_CHARACTERS: usize = 80;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct InstanceName(String);

impl InstanceName {
    pub fn parse(candidate: impl Into<String>) -> Result<Self, InstanceNameError> {
        let candidate = candidate.into();
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            return Err(InstanceNameError::Empty);
        }
        if trimmed.chars().count() > MAX_INSTANCE_NAME_CHARACTERS {
            return Err(InstanceNameError::TooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(InstanceNameError::ControlCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstanceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum InstanceNameError {
    #[error("instance name is empty")]
    Empty,
    #[error("instance name exceeds 80 characters")]
    TooLong,
    #[error("instance name contains a control character")]
    ControlCharacter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceMode {
    Vanilla,
    Modded,
    SlateClient,
}

impl InstanceMode {
    #[must_use]
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Modded => "modded",
            // Keep the legacy value readable until the instance table is rebuilt in a later
            // storage migration. It is never exposed through the public contract.
            Self::SlateClient => "pvp",
        }
    }
}

impl TryFrom<&str> for InstanceMode {
    type Error = UnknownInstanceMode;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "vanilla" => Ok(Self::Vanilla),
            "modded" => Ok(Self::Modded),
            "pvp" | "slate_client" => Ok(Self::SlateClient),
            other => Err(UnknownInstanceMode(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown instance mode: {0}")]
pub struct UnknownInstanceMode(String);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ManagementMode {
    Local,
    Community,
}

impl ManagementMode {
    #[must_use]
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Community => "community",
        }
    }
}

impl TryFrom<&str> for ManagementMode {
    type Error = UnknownManagementMode;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "local" => Ok(Self::Local),
            "community" => Ok(Self::Community),
            other => Err(UnknownManagementMode(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown management mode: {0}")]
pub struct UnknownManagementMode(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "version")]
pub enum LoaderKind {
    Fabric(String),
    NeoForge(String),
    Forge(String),
    Quilt(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LoaderFamily {
    Vanilla,
    Fabric,
    NeoForge,
}

impl LoaderFamily {
    #[must_use]
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::NeoForge => "neoforge",
        }
    }

    #[must_use]
    pub const fn requires_version(self) -> bool {
        !matches!(self, Self::Vanilla)
    }
}

impl TryFrom<&str> for LoaderFamily {
    type Error = UnknownLoaderFamily;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "vanilla" => Ok(Self::Vanilla),
            "fabric" => Ok(Self::Fabric),
            "neoforge" => Ok(Self::NeoForge),
            other => Err(UnknownLoaderFamily(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown loader family: {0}")]
pub struct UnknownLoaderFamily(String);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceSetupState {
    Configured,
    Preparing,
    Ready,
    Blocked,
}

impl InstanceSetupState {
    #[must_use]
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Preparing => "preparing",
            Self::Ready => "ready",
            Self::Blocked => "blocked",
        }
    }
}

impl TryFrom<&str> for InstanceSetupState {
    type Error = UnknownInstanceSetupState;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "configured" => Ok(Self::Configured),
            "preparing" => Ok(Self::Preparing),
            "ready" => Ok(Self::Ready),
            "blocked" => Ok(Self::Blocked),
            other => Err(UnknownInstanceSetupState(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown instance setup state: {0}")]
pub struct UnknownInstanceSetupState(String);

#[cfg(test)]
mod tests {
    use super::{InstanceMode, InstanceName, LoaderFamily};

    #[test]
    fn trims_and_validates_instance_names() -> Result<(), Box<dyn std::error::Error>> {
        let name = InstanceName::parse("  Survival  ")?;

        assert_eq!(name.as_str(), "Survival");
        assert!(InstanceName::parse("\n").is_err());
        assert!(InstanceName::parse("x".repeat(81)).is_err());
        Ok(())
    }

    #[test]
    fn vanilla_is_the_only_loader_without_a_version() {
        assert!(!LoaderFamily::Vanilla.requires_version());
        assert!(LoaderFamily::Fabric.requires_version());
        assert!(LoaderFamily::NeoForge.requires_version());
    }

    #[test]
    fn legacy_pvp_storage_value_loads_as_slate_client() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(InstanceMode::try_from("pvp")?, InstanceMode::SlateClient);
        assert_eq!(
            InstanceMode::try_from("slate_client")?,
            InstanceMode::SlateClient
        );
        assert_eq!(InstanceMode::SlateClient.as_storage_value(), "pvp");
        Ok(())
    }
}
