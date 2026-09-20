use super::mapping::parse_loader;
use serde::Deserialize;
use serde_json::Value;
use slate_modpack_api_contracts::LoaderKind;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum FlexibleId {
    String(String),
    Signed(i64),
    Unsigned(u64),
}

impl Default for FlexibleId {
    fn default() -> Self {
        Self::String(String::new())
    }
}

impl FlexibleId {
    pub(super) fn to_string_value(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Signed(value) => value.to_string(),
            Self::Unsigned(value) => value.to_string(),
        }
    }

    pub(super) fn as_u32(&self) -> Option<u32> {
        self.to_string_value().parse().ok()
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamBrowse {
    #[serde(default)]
    pub(super) status: Option<String>,
    #[serde(default)]
    pub(super) packs: Vec<UpstreamBrowseCard>,
    #[serde(default)]
    pub(super) mods: Vec<UpstreamBrowseCard>,
    #[serde(default)]
    pub(super) page: Option<FlexibleId>,
    #[serde(default)]
    pub(super) pages: Option<FlexibleId>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ModrinthVersion {
    pub(super) id: String,
    pub(super) project_id: String,
    pub(super) name: String,
    pub(super) version_number: String,
    #[serde(default)]
    pub(super) game_versions: Vec<String>,
    #[serde(default)]
    pub(super) loaders: Vec<String>,
    #[serde(default)]
    pub(super) date_published: String,
    #[serde(default)]
    pub(super) files: Vec<ModrinthFile>,
    #[serde(default)]
    pub(super) dependencies: Vec<ModrinthDependency>,
}

pub(super) fn modrinth_version_matches(
    version: &ModrinthVersion,
    minecraft_version: &str,
    loader: LoaderKind,
) -> bool {
    version
        .game_versions
        .iter()
        .any(|value| value == minecraft_version)
        && version
            .loaders
            .iter()
            .any(|value| parse_loader(value) == Some(loader))
}

#[derive(Debug, Deserialize)]
pub(super) struct ModrinthDependency {
    #[serde(default)]
    pub(super) version_id: Option<String>,
    #[serde(default)]
    pub(super) project_id: Option<String>,
    pub(super) dependency_type: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ModrinthFile {
    pub(super) url: String,
    pub(super) filename: String,
    #[serde(default)]
    pub(super) size: u64,
    #[serde(default)]
    pub(super) primary: bool,
    #[serde(default)]
    pub(super) hashes: ModrinthHashes,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ModrinthHashes {
    #[serde(default)]
    pub(super) sha512: Option<String>,
    #[serde(default)]
    pub(super) sha256: Option<String>,
    #[serde(default)]
    pub(super) sha1: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamBrowseCard {
    pub(super) id: FlexibleId,
    pub(super) name: String,
    #[serde(default)]
    pub(super) synopsis: Option<String>,
    #[serde(default)]
    pub(super) updated: i64,
    #[serde(default)]
    pub(super) art: Option<Vec<UpstreamArt>>,
    #[serde(default)]
    pub(super) authors: Option<Vec<UpstreamAuthor>>,
    #[serde(default)]
    pub(super) tags: Option<Vec<UpstreamTag>>,
    #[serde(default)]
    pub(super) provider: Option<String>,
    #[serde(default)]
    pub(super) platform: Option<String>,
    #[serde(default)]
    pub(super) installs: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamProject {
    #[serde(default)]
    pub(super) status: Option<String>,
    pub(super) id: FlexibleId,
    pub(super) name: String,
    #[serde(default)]
    pub(super) synopsis: Option<String>,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) updated: i64,
    #[serde(default)]
    pub(super) art: Option<Vec<UpstreamArt>>,
    #[serde(default)]
    pub(super) links: Option<Vec<UpstreamLink>>,
    #[serde(default)]
    pub(super) authors: Option<Vec<UpstreamAuthor>>,
    #[serde(default)]
    pub(super) versions: Option<Vec<UpstreamVersionSummary>>,
    #[serde(default)]
    pub(super) installs: Option<i64>,
    #[serde(default)]
    pub(super) tags: Option<Vec<UpstreamTag>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamVersionHistory {
    #[serde(default)]
    pub(super) status: Option<String>,
    #[serde(default)]
    pub(super) versions: Vec<UpstreamVersionSummary>,
    #[serde(default)]
    pub(super) pages: Option<FlexibleId>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamVersionSummary {
    pub(super) id: FlexibleId,
    pub(super) name: String,
    #[serde(rename = "type", default)]
    pub(super) release_type: String,
    #[serde(default)]
    pub(super) updated: i64,
    #[serde(default)]
    pub(super) targets: Vec<UpstreamTarget>,
    #[serde(default)]
    pub(super) private: Option<bool>,
    #[serde(default)]
    pub(super) url: Option<String>,
    #[serde(default)]
    pub(super) mirrors: Option<Vec<String>>,
    #[serde(default)]
    pub(super) sha1: Option<String>,
    #[serde(default)]
    pub(super) hashes: Option<UpstreamHashes>,
    #[serde(default)]
    pub(super) size: Option<i64>,
    #[serde(default)]
    pub(super) dependencies: Vec<UpstreamModDependency>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamModDependency {
    pub(super) id: FlexibleId,
    #[serde(default)]
    pub(super) file: Option<FlexibleId>,
    #[serde(default)]
    pub(super) required: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamVersion {
    #[serde(default)]
    pub(super) status: Option<String>,
    pub(super) id: FlexibleId,
    pub(super) name: String,
    #[serde(rename = "type", default)]
    pub(super) release_type: String,
    #[serde(default)]
    pub(super) updated: i64,
    #[serde(default)]
    pub(super) released: i64,
    #[serde(default)]
    pub(super) targets: Vec<UpstreamTarget>,
    #[serde(default)]
    pub(super) specs: Value,
    #[serde(default)]
    pub(super) files: Vec<UpstreamFile>,
    #[serde(default)]
    pub(super) changelog: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamTarget {
    pub(super) name: String,
    pub(super) version: String,
    #[serde(rename = "type", default)]
    pub(super) kind: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamFile {
    #[serde(default)]
    pub(super) id: FlexibleId,
    pub(super) name: String,
    #[serde(rename = "type", default)]
    pub(super) kind: Option<String>,
    #[serde(default)]
    pub(super) version: Option<FlexibleId>,
    #[serde(default)]
    pub(super) path: Option<String>,
    #[serde(default)]
    pub(super) url: Option<String>,
    #[serde(default)]
    pub(super) mirrors: Option<Vec<String>>,
    #[serde(default)]
    pub(super) sha1: Option<String>,
    #[serde(default)]
    pub(super) hashes: Option<UpstreamHashes>,
    #[serde(default)]
    pub(super) size: Option<i64>,
    #[serde(rename = "clientonly", default)]
    pub(super) client_only: Option<bool>,
    #[serde(rename = "serveronly", default)]
    pub(super) server_only: Option<bool>,
    #[serde(default)]
    pub(super) optional: Option<bool>,
    #[serde(default)]
    pub(super) curseforge: Option<UpstreamCurseForgeReference>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamHashes {
    #[serde(default)]
    pub(super) sha1: Option<String>,
    #[serde(default)]
    pub(super) sha256: Option<String>,
    #[serde(default)]
    pub(super) sha512: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamCurseForgeReference {
    pub(super) project: FlexibleId,
    pub(super) file: FlexibleId,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct UpstreamArt {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) url: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamAuthor {
    pub(super) name: String,
    #[serde(default)]
    pub(super) website: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamTag {
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamLink {
    pub(super) link: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamTags {
    #[serde(default)]
    pub(super) status: Option<String>,
    #[serde(default)]
    pub(super) tags: Vec<UpstreamTag>,
}
