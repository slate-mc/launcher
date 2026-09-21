//! Trusted native client for Slate's normalized modpack content API.

use reqwest::{Method, StatusCode};
use slate_modpack_api_contracts::{
    ApiEnvelope, ApiErrorCode, CaptureProductEventRequest, CaptureProductEventResponse,
    CategoriesResponse, ContentInstallPlanRequest, ContentKind, ImportPackPlanRequest,
    ImportedPackPlan, InstallPlan, InstallPlanRequest, LauncherFeatureConfig, LoaderKind,
    ModInstallPlanRequest, ModVersionList, Modpack, ModpackVersion, Provider, ProvidersResponse,
    ReleaseType, ResolveModsRequest, ResolveModsResponse, SearchResponse, SupportReportReceipt,
    UpdateResponse, VersionPage,
};
use std::time::Duration;
use url::Url;
use uuid::Uuid;

pub const DEVELOPMENT_API_URL: &str = "http://127.0.0.1:8080";
pub const PRODUCTION_API_URL: &str = "https://api.slatelauncher.org";
const MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_SUPPORT_REPORT_BYTES: usize = 20 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ModpackApiClient {
    base_url: Url,
    client: reqwest::Client,
}

impl ModpackApiClient {
    pub fn for_current_build() -> Result<Self, ClientError> {
        let base = if cfg!(debug_assertions) {
            DEVELOPMENT_API_URL
        } else {
            PRODUCTION_API_URL
        };
        Self::new(Url::parse(base)?)
    }

    pub fn new(mut base_url: Url) -> Result<Self, ClientError> {
        validate_base_url(&base_url)?;
        base_url.set_path("/");
        base_url.set_query(None);
        base_url.set_fragment(None);
        let client = reqwest::Client::builder()
            .user_agent(format!(
                "slate/{} (modpack content client)",
                env!("CARGO_PKG_VERSION")
            ))
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { base_url, client })
    }

    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub async fn providers(&self) -> Result<ProvidersResponse, ClientError> {
        self.get(self.endpoint(&["v1", "providers"])?).await
    }

    pub async fn capture_product_event(
        &self,
        request: &CaptureProductEventRequest,
    ) -> Result<CaptureProductEventResponse, ClientError> {
        self.send(
            Method::POST,
            self.endpoint(&["v1", "telemetry", "events"])?,
            Some(serde_json::to_vec(request)?),
        )
        .await
    }

    pub async fn launcher_feature_config(&self) -> Result<LauncherFeatureConfig, ClientError> {
        self.get(self.endpoint(&["v1", "launcher", "config"])?)
            .await
    }

    pub async fn upload_support_report(
        &self,
        report_id: Uuid,
        archive: Vec<u8>,
    ) -> Result<SupportReportReceipt, ClientError> {
        if archive.len() > MAX_SUPPORT_REPORT_BYTES {
            return Err(ClientError::SupportReportTooLarge);
        }
        let response = self
            .client
            .post(self.endpoint(&["v1", "support", "reports"])?)
            .header(reqwest::header::CONTENT_TYPE, "application/zip")
            .header("x-slate-report-id", report_id.to_string())
            .body(archive)
            .send()
            .await?;
        Self::read_response(response).await
    }

    pub async fn import_plan(
        &self,
        request: &ImportPackPlanRequest,
    ) -> Result<ImportedPackPlan, ClientError> {
        self.send(
            Method::POST,
            self.endpoint(&["v1", "import-plan"])?,
            Some(serde_json::to_vec(request)?),
        )
        .await
    }

    pub async fn search(&self, options: &SearchOptions) -> Result<SearchResponse, ClientError> {
        options.validate()?;
        let mut url = self.endpoint(&["v1", "modpacks"])?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(value) = options.query.as_deref() {
                query.append_pair("q", value);
            }
            if let Some(value) = options.provider.map(Provider::as_str) {
                query.append_pair("provider", value);
            }
            if let Some(value) = options.minecraft_version.as_deref() {
                query.append_pair("minecraft_version", value);
            }
            if let Some(value) = options.loader.map(loader_name) {
                query.append_pair("loader", value);
            }
            if let Some(value) = options.category.as_deref() {
                query.append_pair("category", value);
            }
            query.append_pair("sort", options.sort.as_str());
            if let Some(cursor) = &options.cursor {
                query.append_pair("cursor", cursor);
            }
            if let Some(page) = options.page {
                query.append_pair("page", &page.to_string());
            }
            query.append_pair("limit", &options.limit.to_string());
        }
        self.get(url).await
    }

    pub async fn search_mods(
        &self,
        options: &SearchOptions,
    ) -> Result<SearchResponse, ClientError> {
        options.validate()?;
        let minecraft_version = options
            .minecraft_version
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or(ClientError::MissingModTarget)?;
        let loader = options
            .loader
            .filter(|value| *value != LoaderKind::Vanilla)
            .ok_or(ClientError::MissingModTarget)?;
        if options.provider == Some(Provider::Ftb) {
            return Err(ClientError::UnsupportedModProvider);
        }
        let mut url = self.endpoint(&["v1", "mods"])?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(value) = options.query.as_deref() {
                query.append_pair("q", value);
            }
            if let Some(value) = options.provider.map(Provider::as_str) {
                query.append_pair("provider", value);
            }
            query.append_pair("minecraft_version", minecraft_version);
            query.append_pair("loader", loader_name(loader));
            query.append_pair("sort", options.sort.as_str());
            if let Some(cursor) = &options.cursor {
                query.append_pair("cursor", cursor);
            }
            if let Some(page) = options.page {
                query.append_pair("page", &page.to_string());
            }
            query.append_pair("limit", &options.limit.to_string());
        }
        self.get(url).await
    }

    pub async fn search_content(
        &self,
        kind: ContentKind,
        options: &SearchOptions,
    ) -> Result<SearchResponse, ClientError> {
        options.validate()?;
        let minecraft_version = options
            .minecraft_version
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or(ClientError::MissingContentTarget)?;
        let mut url = self.endpoint(&["v1", "content", kind.as_str()])?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(value) = options.query.as_deref() {
                query.append_pair("q", value);
            }
            query.append_pair("minecraft_version", minecraft_version);
            query.append_pair("sort", options.sort.as_str());
            if let Some(cursor) = &options.cursor {
                query.append_pair("cursor", cursor);
            }
            if let Some(page) = options.page {
                query.append_pair("page", &page.to_string());
            }
            query.append_pair("limit", &options.limit.to_string());
        }
        self.get(url).await
    }

    pub async fn content_versions(
        &self,
        kind: ContentKind,
        project_id: &str,
        minecraft_version: &str,
    ) -> Result<ModVersionList, ClientError> {
        if minecraft_version.trim().is_empty() {
            return Err(ClientError::MissingContentTarget);
        }
        let mut url = self.endpoint(&[
            "v1",
            "content",
            kind.as_str(),
            "modrinth",
            project_id,
            "versions",
        ])?;
        url.query_pairs_mut()
            .append_pair("minecraft_version", minecraft_version);
        self.get(url).await
    }

    pub async fn content_install_plan(
        &self,
        kind: ContentKind,
        project_id: &str,
        request: &ContentInstallPlanRequest,
    ) -> Result<InstallPlan, ClientError> {
        if request.minecraft_version.trim().is_empty() {
            return Err(ClientError::MissingContentTarget);
        }
        let url = self.endpoint(&[
            "v1",
            "content",
            kind.as_str(),
            "modrinth",
            project_id,
            "install-plan",
        ])?;
        self.send(Method::POST, url, Some(serde_json::to_vec(request)?))
            .await
    }

    pub async fn project(
        &self,
        provider: Provider,
        project_id: &str,
    ) -> Result<Modpack, ClientError> {
        self.get(self.endpoint(&["v1", "modpacks", provider.as_str(), project_id])?)
            .await
    }

    pub async fn versions(
        &self,
        provider: Provider,
        project_id: &str,
        options: &VersionOptions,
    ) -> Result<VersionPage, ClientError> {
        options.validate()?;
        let mut url =
            self.endpoint(&["v1", "modpacks", provider.as_str(), project_id, "versions"])?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(value) = options.minecraft_version.as_deref() {
                query.append_pair("minecraft_version", value);
            }
            if let Some(value) = options.loader.map(loader_name) {
                query.append_pair("loader", value);
            }
            if let Some(value) = options.release_type.map(release_type_name) {
                query.append_pair("release_type", value);
            }
            if let Some(cursor) = &options.cursor {
                query.append_pair("cursor", cursor);
            }
            if let Some(page) = options.page {
                query.append_pair("page", &page.to_string());
            }
            query.append_pair("limit", &options.limit.to_string());
        }
        self.get(url).await
    }

    pub async fn version(
        &self,
        provider: Provider,
        project_id: &str,
        version_id: &str,
    ) -> Result<ModpackVersion, ClientError> {
        self.get(self.endpoint(&[
            "v1",
            "modpacks",
            provider.as_str(),
            project_id,
            "versions",
            version_id,
        ])?)
        .await
    }

    pub async fn update(
        &self,
        provider: Provider,
        project_id: &str,
        current_version: &str,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<UpdateResponse, ClientError> {
        let mut url =
            self.endpoint(&["v1", "modpacks", provider.as_str(), project_id, "update"])?;
        url.query_pairs_mut()
            .append_pair("current_version", current_version)
            .append_pair("minecraft_version", minecraft_version)
            .append_pair("loader", loader_name(loader));
        self.get(url).await
    }

    pub async fn install_plan(
        &self,
        provider: Provider,
        project_id: &str,
        version_id: &str,
        request: &InstallPlanRequest,
    ) -> Result<InstallPlan, ClientError> {
        let url = self.endpoint(&[
            "v1",
            "modpacks",
            provider.as_str(),
            project_id,
            "versions",
            version_id,
            "install-plan",
        ])?;
        let body = serde_json::to_vec(request)?;
        self.send(Method::POST, url, Some(body)).await
    }

    pub async fn mod_install_plan(
        &self,
        provider: Provider,
        project_id: &str,
        request: &ModInstallPlanRequest,
    ) -> Result<InstallPlan, ClientError> {
        if provider == Provider::Ftb {
            return Err(ClientError::UnsupportedModProvider);
        }
        if request.minecraft_version.trim().is_empty()
            || request.loader == LoaderKind::Vanilla
            || request
                .loader_version
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(ClientError::MissingModTarget);
        }
        let url = self.endpoint(&["v1", "mods", provider.as_str(), project_id, "install-plan"])?;
        self.send(Method::POST, url, Some(serde_json::to_vec(request)?))
            .await
    }

    pub async fn mod_versions(
        &self,
        provider: Provider,
        project_id: &str,
        minecraft_version: &str,
        loader: LoaderKind,
    ) -> Result<ModVersionList, ClientError> {
        if provider == Provider::Ftb {
            return Err(ClientError::UnsupportedModProvider);
        }
        if minecraft_version.trim().is_empty() || loader == LoaderKind::Vanilla {
            return Err(ClientError::MissingModTarget);
        }
        let mut url = self.endpoint(&["v1", "mods", provider.as_str(), project_id, "versions"])?;
        url.query_pairs_mut()
            .append_pair("minecraft_version", minecraft_version)
            .append_pair("loader", loader_name(loader));
        self.get(url).await
    }

    pub async fn resolve_mods(
        &self,
        request: &ResolveModsRequest,
    ) -> Result<ResolveModsResponse, ClientError> {
        let url = self.endpoint(&["v1", "mods", "resolve"])?;
        self.send(Method::POST, url, Some(serde_json::to_vec(request)?))
            .await
    }

    pub async fn categories(
        &self,
        provider: Option<Provider>,
    ) -> Result<CategoriesResponse, ClientError> {
        let mut url = self.endpoint(&["v1", "modpacks", "categories"])?;
        if let Some(provider) = provider {
            url.query_pairs_mut()
                .append_pair("provider", provider.as_str());
        }
        self.get(url).await
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, url: Url) -> Result<T, ClientError> {
        self.send(Method::GET, url, None).await
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        body: Option<Vec<u8>>,
    ) -> Result<T, ClientError> {
        let mut request = self.client.request(method, url);
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        }
        let response = request.send().await?;
        Self::read_response(response).await
    }

    async fn read_response<T: serde::de::DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<T, ClientError> {
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES)
        {
            return Err(ClientError::ResponseTooLarge);
        }
        let bytes = response.bytes().await?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_RESPONSE_BYTES {
            return Err(ClientError::ResponseTooLarge);
        }
        let envelope: ApiEnvelope<T> = serde_json::from_slice(&bytes)?;
        if status.is_success() && envelope.success {
            return envelope.data.ok_or(ClientError::InvalidEnvelope);
        }
        let error = envelope.error.ok_or(ClientError::InvalidEnvelope)?;
        Err(ClientError::Api {
            status,
            code: error.code,
            message: error.message,
            request_id: envelope.meta.request_id,
            retryable: error.retryable,
        })
    }

    fn endpoint(&self, segments: &[&str]) -> Result<Url, ClientError> {
        let mut url = self.base_url.clone();
        let mut path = url
            .path_segments_mut()
            .map_err(|_| ClientError::InvalidBaseUrl)?;
        path.clear();
        for segment in segments {
            validate_identifier(segment)?;
            path.push(segment);
        }
        drop(path);
        Ok(url)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchOptions {
    pub query: Option<String>,
    pub provider: Option<Provider>,
    pub minecraft_version: Option<String>,
    pub loader: Option<LoaderKind>,
    pub category: Option<String>,
    pub sort: SearchSort,
    pub cursor: Option<String>,
    pub page: Option<u32>,
    pub limit: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: None,
            provider: None,
            minecraft_version: None,
            loader: None,
            category: None,
            sort: SearchSort::Relevance,
            cursor: None,
            page: None,
            limit: 20,
        }
    }
}

impl SearchOptions {
    fn validate(&self) -> Result<(), ClientError> {
        validate_pagination(self.page, self.cursor.as_deref(), self.limit)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SearchSort {
    #[default]
    Relevance,
    Downloads,
    Updated,
    Newest,
}

impl SearchSort {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Updated => "updated",
            Self::Newest => "newest",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionOptions {
    pub minecraft_version: Option<String>,
    pub loader: Option<LoaderKind>,
    pub release_type: Option<ReleaseType>,
    pub cursor: Option<String>,
    pub page: Option<u32>,
    pub limit: usize,
}

impl Default for VersionOptions {
    fn default() -> Self {
        Self {
            minecraft_version: None,
            loader: None,
            release_type: None,
            cursor: None,
            page: None,
            limit: 20,
        }
    }
}

impl VersionOptions {
    fn validate(&self) -> Result<(), ClientError> {
        validate_pagination(self.page, self.cursor.as_deref(), self.limit)
    }
}

fn validate_base_url(url: &Url) -> Result<(), ClientError> {
    let loopback_development = cfg!(debug_assertions)
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost"));
    if (url.scheme() == "https" || loopback_development)
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
    {
        Ok(())
    } else {
        Err(ClientError::InvalidBaseUrl)
    }
}

fn validate_identifier(value: &str) -> Result<(), ClientError> {
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        Err(ClientError::InvalidIdentifier)
    } else {
        Ok(())
    }
}

fn validate_pagination(
    page: Option<u32>,
    cursor: Option<&str>,
    limit: usize,
) -> Result<(), ClientError> {
    if page.is_some() && cursor.is_some() {
        return Err(ClientError::AmbiguousPagination);
    }
    if page.is_some_and(|page| !(1..=10_000).contains(&page)) || !(1..=50).contains(&limit) {
        return Err(ClientError::InvalidPagination);
    }
    if cursor.is_some_and(|cursor| cursor.is_empty() || cursor.len() > 256) {
        return Err(ClientError::InvalidPagination);
    }
    Ok(())
}

const fn loader_name(loader: LoaderKind) -> &'static str {
    match loader {
        LoaderKind::Vanilla => "vanilla",
        LoaderKind::Forge => "forge",
        LoaderKind::NeoForge => "neoforge",
        LoaderKind::Fabric => "fabric",
        LoaderKind::Quilt => "quilt",
    }
}

const fn release_type_name(release_type: ReleaseType) -> &'static str {
    match release_type {
        ReleaseType::Release => "release",
        ReleaseType::Beta => "beta",
        ReleaseType::Alpha => "alpha",
        ReleaseType::Unknown => "unknown",
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("the Slate modpack API URL is invalid")]
    InvalidBaseUrl,
    #[error("a modpack API identifier is invalid")]
    InvalidIdentifier,
    #[error("page and cursor cannot be used together")]
    AmbiguousPagination,
    #[error("modpack pagination is invalid")]
    InvalidPagination,
    #[error("individual mod operations require an exact modded Minecraft target")]
    MissingModTarget,
    #[error("the selected provider does not support individual mods")]
    UnsupportedModProvider,
    #[error("content operations require an exact Minecraft target")]
    MissingContentTarget,
    #[error("the Slate modpack API response exceeded its size limit")]
    ResponseTooLarge,
    #[error("the support report exceeds the upload limit")]
    SupportReportTooLarge,
    #[error("the Slate modpack API returned an invalid response envelope")]
    InvalidEnvelope,
    #[error("the Slate modpack API returned {code:?} for request {request_id}: {message}")]
    Api {
        status: StatusCode,
        code: ApiErrorCode,
        message: String,
        request_id: String,
        retryable: bool,
    },
    #[error("the Slate modpack API request failed")]
    Http(#[from] reqwest::Error),
    #[error("the Slate modpack API response was invalid")]
    Json(#[from] serde_json::Error),
    #[error("the Slate modpack API URL could not be parsed")]
    Url(#[from] url::ParseError),
}

impl ClientError {
    #[must_use]
    pub fn user_message(&self) -> String {
        match self {
            Self::Api {
                message,
                request_id,
                retryable,
                ..
            } => format!(
                "{message} Request ID: {request_id}.{}",
                if *retryable {
                    " Try again shortly."
                } else {
                    ""
                }
            ),
            Self::Http(_) => {
                "slate could not reach the modpack service. Check your connection and try again."
                    .to_owned()
            }
            _ => "slate could not read the modpack service response. Try again.".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEVELOPMENT_API_URL, ModpackApiClient, PRODUCTION_API_URL, SearchOptions};
    use url::Url;

    #[test]
    fn build_endpoints_are_explicit_and_secure() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(DEVELOPMENT_API_URL, "http://127.0.0.1:8080");
        assert_eq!(PRODUCTION_API_URL, "https://api.slatelauncher.org");
        assert!(ModpackApiClient::new(Url::parse(PRODUCTION_API_URL)?).is_ok());
        assert!(ModpackApiClient::new(Url::parse("http://example.test")?).is_err());
        Ok(())
    }

    #[test]
    fn explicit_page_and_cursor_cannot_be_mixed() {
        let options = SearchOptions {
            page: Some(2),
            cursor: Some("opaque".to_owned()),
            ..SearchOptions::default()
        };
        assert!(options.validate().is_err());
    }
}
