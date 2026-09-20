use super::provider_error;
use crate::domain::{java_major_for_minecraft, normalize_install_path};
use crate::response::{ApiError, CacheControl, RequestContext, success};
use crate::state::AppState;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Json, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use futures_util::{StreamExt, TryStreamExt, stream};
use serde::Deserialize;
use slate_modpack_api_contracts::{
    ApiErrorCode, DownloadSource, Hashes, ImportPackFormat, ImportPackPlanRequest,
    ImportedPackPlan, InstallPlan, InstallPlanDownload, InstallPlanInstance, JavaPlan, Loader,
    LoaderKind, MemoryRecommendation, Provider, RuntimePlan,
};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

const MAX_IMPORTED_FILES: usize = 2_048;
const IMPORT_RESOLUTION_CONCURRENCY: usize = 12;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/import-plan", post(import_plan))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
}

async fn import_plan(
    State(state): State<AppState>,
    Extension(context): Extension<RequestContext>,
    payload: Result<Json<ImportPackPlanRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(|_| {
        ApiError::invalid_request(&context, "The imported pack manifest is invalid.").with_field(
            "manifest",
            "Choose a valid CurseForge ZIP or Modrinth .mrpack file.",
        )
    })?;
    let _target = (request.platform, request.arch);
    let imported = match request.format {
        ImportPackFormat::CurseForge => {
            let manifest: CurseForgeManifest =
                serde_json::from_value(request.manifest).map_err(|_| invalid_manifest(&context))?;
            curseforge_plan(&state, &context, manifest).await?
        }
        ImportPackFormat::Modrinth => {
            let manifest: ModrinthManifest =
                serde_json::from_value(request.manifest).map_err(|_| invalid_manifest(&context))?;
            modrinth_plan(&context, manifest)?
        }
    };
    Ok(success(&context, imported, CacheControl::NoStore))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeManifest {
    manifest_type: String,
    manifest_version: u32,
    name: String,
    version: String,
    minecraft: CurseForgeMinecraft,
    files: Vec<CurseForgeFile>,
    overrides: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeMinecraft {
    version: String,
    mod_loaders: Vec<CurseForgeLoader>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeFile {
    project_id: u64,
    file_id: u64,
    #[serde(default = "required_by_default")]
    required: bool,
}

const fn required_by_default() -> bool {
    true
}

async fn curseforge_plan(
    state: &AppState,
    context: &RequestContext,
    manifest: CurseForgeManifest,
) -> Result<ImportedPackPlan, ApiError> {
    if manifest.manifest_type != "minecraftModpack"
        || manifest.manifest_version != 1
        || manifest.files.is_empty()
        || manifest.files.len() > MAX_IMPORTED_FILES
        || manifest
            .files
            .iter()
            .any(|file| file.project_id == 0 || file.file_id == 0)
    {
        return Err(invalid_manifest(context));
    }
    let name = imported_text(&manifest.name, 160).ok_or_else(|| invalid_manifest(context))?;
    let version_name =
        imported_text(&manifest.version, 160).unwrap_or_else(|| "Imported release".to_owned());
    let minecraft =
        imported_version(&manifest.minecraft.version).ok_or_else(|| invalid_manifest(context))?;
    let loader_entry = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|loader| loader.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first())
        .ok_or_else(|| unsupported_loader(context))?;
    let (loader_kind, loader_version) =
        parse_curseforge_loader(&loader_entry.id).ok_or_else(|| unsupported_loader(context))?;
    let adapter = state
        .providers
        .get(Provider::CurseForge)
        .ok_or_else(|| provider_error(context, crate::providers::ProviderError::Unavailable))?;
    let resolved = stream::iter(manifest.files.into_iter().map(|file| {
        let adapter = adapter.clone();
        let minecraft = minecraft.clone();
        async move {
            let item = adapter
                .resolve_mod(
                    &file.project_id.to_string(),
                    &minecraft,
                    loader_kind,
                    &file.file_id.to_string(),
                )
                .await?;
            Ok::<_, crate::providers::ProviderError>((file.required, item))
        }
    }))
    .buffer_unordered(IMPORT_RESOLUTION_CONCURRENCY)
    .try_collect::<Vec<_>>()
    .await
    .map_err(|error| provider_error(context, error))?;

    let mut destinations = BTreeSet::new();
    let mut downloads = Vec::with_capacity(resolved.len());
    let mut total_download_size = 0_u64;
    for (required, item) in resolved {
        let destination = normalize_install_path(&format!("mods/{}", item.file_name))
            .map_err(|_| invalid_manifest(context))?;
        if !destinations.insert(destination.clone()) {
            return Err(ApiError::new(
                context,
                StatusCode::BAD_REQUEST,
                ApiErrorCode::InvalidInstallPath,
                "Two imported files use the same destination.",
                false,
            ));
        }
        total_download_size = total_download_size
            .checked_add(item.size)
            .ok_or_else(|| invalid_manifest(context))?;
        downloads.push(InstallPlanDownload {
            id: format!("curseforge:{}:{}", item.project_id, item.version_id),
            destination,
            size: item.size,
            hashes: item.hashes,
            sources: vec![DownloadSource::Direct { url: item.url }],
            required,
        });
    }
    let override_directories = manifest
        .overrides
        .map(|value| normalize_install_path(&value).map_err(|_| invalid_manifest(context)))
        .transpose()?
        .into_iter()
        .collect();
    Ok(imported_plan(
        Provider::CurseForge,
        name,
        version_name,
        minecraft,
        loader_kind,
        Some(loader_version),
        downloads,
        total_download_size,
        override_directories,
    ))
}

fn parse_curseforge_loader(value: &str) -> Option<(LoaderKind, String)> {
    let (kind, version) = value.split_once('-')?;
    let loader = match kind.to_ascii_lowercase().as_str() {
        "fabric" => LoaderKind::Fabric,
        "neoforge" => LoaderKind::NeoForge,
        _ => return None,
    };
    imported_version(version).map(|version| (loader, version))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModrinthManifest {
    format_version: u32,
    game: String,
    version_id: String,
    name: String,
    files: Vec<ModrinthFile>,
    dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModrinthFile {
    path: String,
    hashes: BTreeMap<String, String>,
    env: Option<ModrinthEnvironment>,
    downloads: Vec<String>,
    file_size: u64,
}

#[derive(Debug, Deserialize)]
struct ModrinthEnvironment {
    client: String,
}

fn modrinth_plan(
    context: &RequestContext,
    manifest: ModrinthManifest,
) -> Result<ImportedPackPlan, ApiError> {
    if manifest.format_version != 1
        || manifest.game != "minecraft"
        || manifest.files.is_empty()
        || manifest.files.len() > MAX_IMPORTED_FILES
    {
        return Err(invalid_manifest(context));
    }
    let name = imported_text(&manifest.name, 160).ok_or_else(|| invalid_manifest(context))?;
    let version_name =
        imported_text(&manifest.version_id, 160).unwrap_or_else(|| "Imported release".to_owned());
    let minecraft = manifest
        .dependencies
        .get("minecraft")
        .and_then(|value| imported_version(value))
        .ok_or_else(|| invalid_manifest(context))?;
    let (loader_kind, loader_version) =
        modrinth_loader(&manifest.dependencies).ok_or_else(|| unsupported_loader(context))?;
    let mut destinations = BTreeSet::new();
    let mut downloads = Vec::new();
    let mut total_download_size = 0_u64;
    for (index, file) in manifest.files.into_iter().enumerate() {
        let client = file
            .env
            .as_ref()
            .map_or("required", |environment| environment.client.as_str());
        if client == "unsupported" {
            continue;
        }
        if !matches!(client, "required" | "optional") || file.file_size == 0 {
            return Err(invalid_manifest(context));
        }
        let destination =
            normalize_install_path(&file.path).map_err(|_| invalid_manifest(context))?;
        if !destinations.insert(destination.clone()) {
            return Err(ApiError::new(
                context,
                StatusCode::BAD_REQUEST,
                ApiErrorCode::InvalidInstallPath,
                "Two imported files use the same destination.",
                false,
            ));
        }
        let hashes = Hashes {
            sha512: valid_hash(file.hashes.get("sha512"), 128),
            sha256: valid_hash(file.hashes.get("sha256"), 64),
            sha1: valid_hash(file.hashes.get("sha1"), 40),
        };
        if !hashes.has_cryptographic_hash() {
            return Err(invalid_manifest(context));
        }
        let sources = file
            .downloads
            .into_iter()
            .filter(|url| safe_import_url(url))
            .map(|url| DownloadSource::Direct { url })
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Err(ApiError::new(
                context,
                StatusCode::BAD_REQUEST,
                ApiErrorCode::DownloadUnavailable,
                "An imported file has no supported download source.",
                false,
            ));
        }
        total_download_size = total_download_size
            .checked_add(file.file_size)
            .ok_or_else(|| invalid_manifest(context))?;
        downloads.push(InstallPlanDownload {
            id: format!("modrinth:import:{index}"),
            destination,
            size: file.file_size,
            hashes,
            sources,
            required: client == "required",
        });
    }
    if downloads.is_empty() {
        return Err(invalid_manifest(context));
    }
    Ok(imported_plan(
        Provider::Modrinth,
        name,
        version_name,
        minecraft,
        loader_kind,
        loader_version,
        downloads,
        total_download_size,
        vec!["overrides".to_owned(), "client-overrides".to_owned()],
    ))
}

fn modrinth_loader(
    dependencies: &BTreeMap<String, String>,
) -> Option<(LoaderKind, Option<String>)> {
    let supported = [
        ("fabric-loader", LoaderKind::Fabric),
        ("neoforge", LoaderKind::NeoForge),
    ];
    let mut selected = supported.into_iter().filter_map(|(key, kind)| {
        dependencies
            .get(key)
            .and_then(|value| imported_version(value))
            .map(|version| (kind, Some(version)))
    });
    let first = selected.next();
    if selected.next().is_some() {
        return None;
    }
    first.or_else(|| {
        (!dependencies.contains_key("forge") && !dependencies.contains_key("quilt-loader"))
            .then_some((LoaderKind::Vanilla, None))
    })
}

#[allow(clippy::too_many_arguments)]
fn imported_plan(
    provider: Provider,
    name: String,
    version_name: String,
    minecraft: String,
    loader_kind: LoaderKind,
    loader_version: Option<String>,
    downloads: Vec<InstallPlanDownload>,
    total_download_size: u64,
    override_directories: Vec<String>,
) -> ImportedPackPlan {
    ImportedPackPlan {
        plan: InstallPlan {
            schema: 1,
            instance: InstallPlanInstance {
                provider,
                project_id: "local-import".to_owned(),
                version_id: version_name.clone(),
                name: name.clone(),
            },
            runtime: RuntimePlan {
                minecraft: minecraft.clone(),
                loader: Loader {
                    kind: loader_kind,
                    version: loader_version,
                },
                java: JavaPlan {
                    major: java_major_for_minecraft(&minecraft),
                },
                memory: MemoryRecommendation {
                    minimum_mb: 4_096,
                    recommended_mb: 8_192,
                },
            },
            downloads,
            extract: Vec::new(),
            delete: Vec::new(),
            total_download_size,
        },
        name,
        version_name,
        override_directories,
    }
}

fn imported_text(value: &str, maximum: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && !value.chars().any(char::is_control))
        .then(|| value.chars().take(maximum).collect())
}

fn imported_version(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-_+".contains(character)))
    .then(|| value.to_owned())
}

fn valid_hash(value: Option<&String>, expected_length: usize) -> Option<String> {
    value
        .filter(|hash| {
            hash.len() == expected_length && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .map(|hash| hash.to_ascii_lowercase())
}

fn safe_import_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("cdn.modrinth.com"))
}

fn invalid_manifest(context: &RequestContext) -> ApiError {
    ApiError::invalid_request(context, "The imported pack manifest is invalid.").with_field(
        "manifest",
        "Choose a complete CurseForge ZIP or Modrinth .mrpack file.",
    )
}

fn unsupported_loader(context: &RequestContext) -> ApiError {
    ApiError::new(
        context,
        StatusCode::BAD_REQUEST,
        ApiErrorCode::UnsupportedLoader,
        "The imported pack uses a loader this version of slate does not support.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::{ModrinthManifest, modrinth_loader, modrinth_plan, parse_curseforge_loader};
    use crate::response::RequestContext;
    use slate_modpack_api_contracts::LoaderKind;

    #[test]
    fn parses_supported_curseforge_loaders() {
        assert_eq!(
            parse_curseforge_loader("fabric-0.16.10"),
            Some((LoaderKind::Fabric, "0.16.10".to_owned()))
        );
        assert_eq!(
            parse_curseforge_loader("neoforge-21.1.172"),
            Some((LoaderKind::NeoForge, "21.1.172".to_owned()))
        );
        assert_eq!(parse_curseforge_loader("forge-47.3.0"), None);
    }

    #[test]
    fn normalizes_modrinth_client_files() -> Result<(), Box<dyn std::error::Error>> {
        let manifest: ModrinthManifest = serde_json::from_value(serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0.0",
            "name": "Example Pack",
            "dependencies": {
                "minecraft": "1.21.1",
                "fabric-loader": "0.16.10"
            },
            "files": [{
                "path": "mods/example.jar",
                "hashes": { "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" },
                "env": { "client": "required", "server": "required" },
                "downloads": ["https://cdn.modrinth.com/data/example.jar"],
                "fileSize": 42
            }]
        }))?;
        assert_eq!(
            modrinth_loader(&manifest.dependencies),
            Some((LoaderKind::Fabric, Some("0.16.10".to_owned())))
        );
        let context = RequestContext::for_test();
        let imported = match modrinth_plan(&context, manifest) {
            Ok(imported) => imported,
            Err(_) => return Err(std::io::Error::other("valid Modrinth manifest").into()),
        };
        assert_eq!(imported.plan.downloads.len(), 1);
        assert_eq!(imported.plan.runtime.minecraft, "1.21.1");
        assert_eq!(imported.override_directories.len(), 2);
        Ok(())
    }

    #[test]
    fn rejects_unsafe_modrinth_paths_and_download_hosts() -> Result<(), Box<dyn std::error::Error>>
    {
        let context = RequestContext::for_test();
        for (path, download) in [
            (
                "../mods/example.jar",
                "https://cdn.modrinth.com/data/example.jar",
            ),
            (
                "mods/example.jar",
                "https://downloads.example.com/example.jar",
            ),
        ] {
            let manifest: ModrinthManifest = serde_json::from_value(serde_json::json!({
                "formatVersion": 1,
                "game": "minecraft",
                "versionId": "1.0.0",
                "name": "Unsafe Pack",
                "dependencies": {
                    "minecraft": "1.21.1",
                    "fabric-loader": "0.16.10"
                },
                "files": [{
                    "path": path,
                    "hashes": { "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" },
                    "env": { "client": "required", "server": "required" },
                    "downloads": [download],
                    "fileSize": 42
                }]
            }))?;
            assert!(modrinth_plan(&context, manifest).is_err());
        }
        Ok(())
    }
}
