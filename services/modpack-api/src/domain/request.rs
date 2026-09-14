use slate_modpack_api_contracts::{LoaderKind, ModpackSummary, ReleaseType};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchSort {
    Relevance,
    Downloads,
    Updated,
    Newest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRequest {
    pub query: Option<String>,
    pub minecraft_version: Option<String>,
    pub loader: Option<LoaderKind>,
    pub category: Option<String>,
    pub sort: SearchSort,
    pub page: u32,
    pub limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPage {
    pub items: Vec<ModpackSummary>,
    pub page: u32,
    pub pages: u32,
}

impl SearchPage {
    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.page < self.pages
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionQuery {
    pub minecraft_version: Option<String>,
    pub loader: Option<LoaderKind>,
    pub release_type: Option<ReleaseType>,
    pub page: u32,
    pub limit: usize,
}
