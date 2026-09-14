use crate::{LoaderKind, Provider, ProviderStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Author {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct VersionReference {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ModpackLinks {
    pub website: Option<String>,
    pub source: Option<String>,
    pub issues: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ModpackSummary {
    pub provider: Provider,
    pub id: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub authors: Vec<Author>,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub updated_at: String,
    pub minecraft_versions: Vec<String>,
    pub loaders: Vec<LoaderKind>,
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct SearchResponse {
    pub items: Vec<ModpackSummary>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub provider_status: BTreeMap<Provider, ProviderStatus>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Modpack {
    pub provider: Provider,
    pub id: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub authors: Vec<Author>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub minecraft_versions: Vec<String>,
    pub loaders: Vec<LoaderKind>,
    pub links: ModpackLinks,
    pub updated_at: String,
    pub latest_version: Option<VersionReference>,
}
