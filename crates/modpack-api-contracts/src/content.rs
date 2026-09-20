use crate::LoaderKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    ResourcePack,
    ShaderPack,
    DataPack,
}

impl ContentKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourcePack => "resource_pack",
            Self::ShaderPack => "shader_pack",
            Self::DataPack => "data_pack",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ContentInstallPlanRequest {
    pub minecraft_version: String,
    pub loader: LoaderKind,
    pub loader_version: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::ContentKind;

    #[test]
    fn content_kinds_have_stable_public_names() -> Result<(), serde_json::Error> {
        assert_eq!(
            serde_json::to_string(&ContentKind::ResourcePack)?,
            "\"resource_pack\""
        );
        assert_eq!(ContentKind::ShaderPack.as_str(), "shader_pack");
        assert_eq!(ContentKind::DataPack.as_str(), "data_pack");
        Ok(())
    }
}
