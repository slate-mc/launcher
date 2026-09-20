use super::{InstallRequest, InstalledArtifactDigest};
use futures_util::{StreamExt, TryStreamExt, stream};
use reqwest::{Client, StatusCode};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use slate_domain::LoaderFamily;
use slate_modpack_api_contracts::{
    DownloadSource, Hashes, InstallPlan, InstallPlanDownload, LoaderKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use url::Url;
use uuid::Uuid;
use zip::ZipArchive;

const MAX_DOWNLOADS: usize = 100_000;
const MAX_EXTRACT_ACTIONS: usize = 32;
const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_TOTAL_DOWNLOAD_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 200_000;
const MAX_EXTRACTED_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;

pub(super) fn validate_plan_compatibility(
    request: &InstallRequest,
    plan: &InstallPlan,
    required_java_major: u32,
) -> Result<(), ContentInstallError> {
    validate_plan_target(
        &request.minecraft_version,
        request.loader_kind,
        request.loader_version.as_deref(),
        plan,
        required_java_major,
    )
}

pub(super) fn validate_plan_target(
    minecraft_version: &str,
    loader_kind: LoaderFamily,
    loader_version: Option<&str>,
    plan: &InstallPlan,
    required_java_major: u32,
) -> Result<(), ContentInstallError> {
    if plan.schema != 1 {
        return Err(ContentInstallError::UnsupportedSchema(plan.schema));
    }
    if plan.runtime.minecraft != minecraft_version {
        return Err(ContentInstallError::MinecraftMismatch {
            expected: minecraft_version.to_owned(),
            actual: plan.runtime.minecraft.clone(),
        });
    }
    let plan_loader = match plan.runtime.loader.kind {
        LoaderKind::Vanilla => LoaderFamily::Vanilla,
        LoaderKind::Fabric => LoaderFamily::Fabric,
        LoaderKind::NeoForge => LoaderFamily::NeoForge,
        LoaderKind::Forge | LoaderKind::Quilt => {
            return Err(ContentInstallError::UnsupportedLoader(
                format!("{:?}", plan.runtime.loader.kind).to_ascii_lowercase(),
            ));
        }
    };
    if plan_loader != loader_kind {
        return Err(ContentInstallError::LoaderMismatch);
    }
    if plan.runtime.loader.version.as_deref() != loader_version {
        return Err(ContentInstallError::LoaderVersionMismatch);
    }
    if plan.runtime.java.major != required_java_major {
        return Err(ContentInstallError::JavaMismatch {
            expected: required_java_major,
            actual: plan.runtime.java.major,
        });
    }
    validate_plan_shape(plan)
}

pub(super) async fn install_plan_content<F>(
    plan: &InstallPlan,
    game_directory: &Path,
    revision_directory: &Path,
    storage_root: &Path,
    download_concurrency: u8,
    on_progress: F,
) -> Result<Vec<InstalledArtifactDigest>, ContentInstallError>
where
    F: Fn(u64, u64, String) + Send + Sync,
{
    validate_plan_shape(plan)?;
    let staging = revision_directory.join("content-staging");
    let backup = revision_directory.join("content-backup");
    remove_managed_path(&staging).await?;
    remove_managed_path(&backup).await?;
    tokio::fs::create_dir_all(&staging).await?;

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("slate-launcher/0.1 (+https://slatelauncher.org)")
        .build()?;
    let total = u64::try_from(plan.downloads.len()).unwrap_or(u64::MAX);
    on_progress(0, total, "Preparing modpack content".to_owned());
    let completed = AtomicU64::new(0);
    let staged_downloads = stream::iter(plan.downloads.iter().cloned())
        .map(|download| {
            let client = client.clone();
            let staging = staging.clone();
            let completed = &completed;
            let on_progress = &on_progress;
            async move {
                let relative = validate_relative_path(&download.destination, true)?;
                let destination = staging.join(&relative);
                if let Some(parent) = destination.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                let label = content_download_label(&download);
                let already_completed = completed.load(Ordering::Relaxed);
                on_progress(
                    already_completed,
                    total,
                    format!(
                        "Downloading {label} · {} · {already_completed} of {total} verified",
                        format_download_size(download.size)
                    ),
                );
                download_verified(&client, &download, &destination).await?;
                let now_completed = completed.fetch_add(1, Ordering::Relaxed) + 1;
                on_progress(
                    now_completed,
                    total,
                    format!("Verified {label} · {now_completed} of {total} files"),
                );
                Ok::<_, ContentInstallError>((download.id.clone(), destination))
            }
        })
        .buffer_unordered(usize::from(download_concurrency.max(1)))
        .try_collect::<BTreeMap<_, _>>()
        .await?;

    let extract_total = plan.extract.len();
    for (index, action) in plan.extract.iter().enumerate() {
        on_progress(
            total,
            total,
            format!(
                "Extracting pack overrides · {} of {extract_total} archives",
                index + 1
            ),
        );
        let archive = staged_downloads
            .get(&action.download_id)
            .ok_or_else(|| ContentInstallError::UnknownArchive(action.download_id.clone()))?
            .clone();
        let destination = if action.destination == "." {
            staging.clone()
        } else {
            staging.join(validate_relative_path(&action.destination, false)?)
        };
        let prefix = action.source_prefix.clone();
        tokio::task::spawn_blocking(move || {
            extract_archive(&archive, &destination, prefix.as_deref())
        })
        .await??;
    }

    on_progress(
        total,
        total,
        "Applying verified content to the instance".to_owned(),
    );
    let staging_for_commit = staging.clone();
    let game_for_commit = game_directory.to_path_buf();
    let backup_for_commit = backup.clone();
    let deletes = plan.delete.clone();
    let installed_paths = tokio::task::spawn_blocking(move || {
        commit_staged_content(
            &staging_for_commit,
            &game_for_commit,
            &backup_for_commit,
            &deletes,
        )
    })
    .await??;

    on_progress(total, total, "Indexing installed pack content".to_owned());
    let storage_root = storage_root.to_path_buf();
    let artifacts = tokio::task::spawn_blocking(move || {
        let mut artifacts = Vec::with_capacity(installed_paths.len());
        for path in installed_paths {
            artifacts.push(InstalledArtifactDigest {
                relative_path: super::relative_artifact_path(&storage_root, &path)
                    .map_err(|_| ContentInstallError::UnsafePath)?,
                sha256: super::sha256_file(&path)?,
            });
        }
        artifacts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok::<_, ContentInstallError>(artifacts)
    })
    .await??;
    on_progress(
        total,
        total,
        format!("Installed and indexed {} pack files", artifacts.len()),
    );
    remove_managed_path(&staging).await?;
    remove_managed_path(&backup).await?;
    Ok(artifacts)
}

fn content_download_label(download: &InstallPlanDownload) -> String {
    let name = download
        .destination
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(download.id.as_str());
    if name.ends_with(".archive") {
        "pack overrides".to_owned()
    } else {
        name.chars().take(96).collect()
    }
}

fn format_download_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    if bytes == 0 {
        "size pending".to_owned()
    } else if bytes < 1024 {
        format!("{bytes} B")
    } else if (bytes as f64) < MIB {
        format!("{:.1} KB", bytes as f64 / KIB)
    } else if (bytes as f64) < GIB {
        format!("{:.1} MB", bytes as f64 / MIB)
    } else {
        format!("{:.2} GB", bytes as f64 / GIB)
    }
}

fn validate_plan_shape(plan: &InstallPlan) -> Result<(), ContentInstallError> {
    if plan.downloads.len() > MAX_DOWNLOADS || plan.extract.len() > MAX_EXTRACT_ACTIONS {
        return Err(ContentInstallError::PlanTooLarge);
    }
    if plan.total_download_size > MAX_TOTAL_DOWNLOAD_BYTES {
        return Err(ContentInstallError::PlanTooLarge);
    }
    let mut ids = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    let mut calculated_size = 0_u64;
    for download in &plan.downloads {
        if download.id.is_empty()
            || !ids.insert(download.id.as_str())
            || download.size > MAX_DOWNLOAD_BYTES
            || !download.hashes.has_cryptographic_hash()
            || download.sources.is_empty()
        {
            return Err(ContentInstallError::InvalidPlan);
        }
        validate_hashes(&download.hashes)?;
        let destination = validate_relative_path(&download.destination, true)?;
        if !destinations.insert(destination) {
            return Err(ContentInstallError::DuplicateDestination);
        }
        calculated_size = calculated_size
            .checked_add(download.size)
            .ok_or(ContentInstallError::PlanTooLarge)?;
    }
    if calculated_size != plan.total_download_size {
        return Err(ContentInstallError::InvalidPlan);
    }
    for action in &plan.extract {
        if !ids.contains(action.download_id.as_str()) {
            return Err(ContentInstallError::UnknownArchive(
                action.download_id.clone(),
            ));
        }
        if action.destination != "." {
            validate_relative_path(&action.destination, false)?;
        }
        if let Some(prefix) = &action.source_prefix {
            validate_archive_prefix(prefix)?;
        }
    }
    for path in &plan.delete {
        validate_relative_path(path, false)?;
    }
    Ok(())
}

fn validate_hashes(hashes: &Hashes) -> Result<(), ContentInstallError> {
    for (value, length) in [
        (hashes.sha512.as_deref(), 128),
        (hashes.sha256.as_deref(), 64),
        (hashes.sha1.as_deref(), 40),
    ] {
        if value.is_some_and(|hash| {
            hash.len() != length || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(ContentInstallError::InvalidHash);
        }
    }
    Ok(())
}

async fn download_verified(
    client: &Client,
    download: &InstallPlanDownload,
    destination: &Path,
) -> Result<(), ContentInstallError> {
    let partial = destination.with_extension(format!("partial-{}", Uuid::new_v4()));
    let mut last_error = None;
    for source in &download.sources {
        for attempt in 0..3_u32 {
            let _ = tokio::fs::remove_file(&partial).await;
            match download_once(client, source, &partial, download.size, &download.hashes).await {
                Ok(()) => {
                    tokio::fs::rename(&partial, destination).await?;
                    return Ok(());
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt < 2 {
                        tokio::time::sleep(Duration::from_millis(250_u64 << attempt)).await;
                    }
                }
            }
        }
    }
    let _ = tokio::fs::remove_file(&partial).await;
    Err(last_error.unwrap_or(ContentInstallError::DownloadUnavailable))
}

async fn download_once(
    client: &Client,
    source: &DownloadSource,
    destination: &Path,
    expected_size: u64,
    hashes: &Hashes,
) -> Result<(), ContentInstallError> {
    let (raw_url, proxy) = match source {
        DownloadSource::Direct { url } => (url, false),
        DownloadSource::Proxy { url } => (url, true),
    };
    let mut url = validate_download_url(raw_url, proxy)?;
    let mut response = None;
    for redirects in 0..=MAX_REDIRECTS {
        let current = client.get(url.clone()).send().await?;
        if current.status().is_redirection() {
            if redirects == MAX_REDIRECTS {
                return Err(ContentInstallError::TooManyRedirects);
            }
            let location = current
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(ContentInstallError::UnsafeDownloadUrl)?;
            url = validate_download_url(url.join(location)?.as_str(), proxy)?;
            continue;
        }
        response = Some(current);
        break;
    }
    let mut response = response.ok_or(ContentInstallError::TooManyRedirects)?;
    if !response.status().is_success() {
        return Err(ContentInstallError::HttpStatus(response.status()));
    }
    if response.content_length().is_some_and(|size| {
        size > MAX_DOWNLOAD_BYTES || (expected_size > 0 && size != expected_size)
    }) {
        return Err(ContentInstallError::SizeMismatch);
    }

    let mut file = tokio::fs::File::create(destination).await?;
    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut sha512 = Sha512::new();
    let mut received = 0_u64;
    while let Some(chunk) = response.chunk().await? {
        received = received
            .checked_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX))
            .ok_or(ContentInstallError::SizeMismatch)?;
        if received > MAX_DOWNLOAD_BYTES || (expected_size > 0 && received > expected_size) {
            return Err(ContentInstallError::SizeMismatch);
        }
        sha1.update(&chunk);
        sha256.update(&chunk);
        sha512.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    if expected_size > 0 && received != expected_size {
        return Err(ContentInstallError::SizeMismatch);
    }
    let actual = Hashes {
        sha512: Some(hex(&sha512.finalize())),
        sha256: Some(hex(&sha256.finalize())),
        sha1: Some(hex(&sha1.finalize())),
    };
    for (expected, actual) in [
        (hashes.sha512.as_deref(), actual.sha512.as_deref()),
        (hashes.sha256.as_deref(), actual.sha256.as_deref()),
        (hashes.sha1.as_deref(), actual.sha1.as_deref()),
    ] {
        if expected
            .is_some_and(|value| actual.is_none_or(|actual| !value.eq_ignore_ascii_case(actual)))
        {
            return Err(ContentInstallError::HashMismatch);
        }
    }
    Ok(())
}

fn validate_download_url(value: &str, proxy: bool) -> Result<Url, ContentInstallError> {
    let url = Url::parse(value)?;
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(ContentInstallError::UnsafeDownloadUrl);
    }
    let host = url
        .host_str()
        .map(str::to_ascii_lowercase)
        .ok_or(ContentInstallError::UnsafeDownloadUrl)?;
    let development_proxy = cfg!(debug_assertions)
        && url.scheme() == "http"
        && host == "127.0.0.1"
        && url.port() == Some(8080);
    if development_proxy {
        return Ok(url);
    }
    if url.scheme() != "https" {
        return Err(ContentInstallError::UnsafeDownloadUrl);
    }
    let allowed = if proxy {
        host == "api.slatelauncher.org"
    } else {
        host == "cdn.modrinth.com"
            || host == "files.feed-the-beast.com"
            || host == "cdn.feed-the-beast.com"
            || host == "api.slatelauncher.org"
            || host == "api.modpacks.ch"
            || host.ends_with(".forgecdn.net")
            || host.ends_with(".modpacks.ch")
    };
    if !allowed {
        return Err(ContentInstallError::UnsafeDownloadUrl);
    }
    Ok(url)
}

fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    prefix: Option<&str>,
) -> Result<(), ContentInstallError> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(ContentInstallError::ArchiveTooLarge);
    }
    let prefix = prefix.map(validate_archive_prefix).transpose()?;
    let mut extracted = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(ContentInstallError::UnsafeArchivePath);
        }
        let raw_name = entry.name();
        if raw_name.contains('\\') || raw_name.contains('\0') {
            return Err(ContentInstallError::UnsafeArchivePath);
        }
        let relative_name = if let Some(prefix) = &prefix {
            let Some(remainder) = raw_name.strip_prefix(prefix) else {
                continue;
            };
            if remainder.is_empty() {
                continue;
            }
            remainder
        } else {
            raw_name
        };
        let relative = validate_relative_path(relative_name, false)?;
        let output = destination.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(output)?;
            continue;
        }
        extracted = extracted
            .checked_add(entry.size())
            .ok_or(ContentInstallError::ArchiveTooLarge)?;
        if extracted > MAX_EXTRACTED_BYTES {
            return Err(ContentInstallError::ArchiveTooLarge);
        }
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if output.exists() {
            return Err(ContentInstallError::DuplicateDestination);
        }
        let mut output_file = std::fs::File::create(output)?;
        let copied = std::io::copy(&mut entry, &mut output_file)?;
        if copied != entry.size() {
            return Err(ContentInstallError::ArchiveTooLarge);
        }
        output_file.flush()?;
    }
    Ok(())
}

fn commit_staged_content(
    staging: &Path,
    game_directory: &Path,
    backup: &Path,
    deletes: &[String],
) -> Result<Vec<PathBuf>, ContentInstallError> {
    let mut staged_files = Vec::new();
    collect_files(staging, staging, &mut staged_files)?;
    staged_files.retain(|relative| {
        relative
            .components()
            .next()
            .is_none_or(|component| component.as_os_str() != ".slate")
    });
    staged_files.sort();
    let mut delete_paths = deletes
        .iter()
        .map(|path| validate_relative_path(path, false))
        .collect::<Result<Vec<_>, _>>()?;
    delete_paths.sort();

    std::fs::create_dir_all(backup)?;
    let mut backed_up = Vec::new();
    let mut written = Vec::new();
    let mut affected = delete_paths.iter().cloned().collect::<BTreeSet<_>>();
    affected.extend(staged_files.iter().cloned());
    let operation = (|| {
        for relative in &affected {
            let target = game_directory.join(relative);
            if std::fs::symlink_metadata(&target).is_ok() {
                let backup_target = backup.join(relative);
                if let Some(parent) = backup_target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(&target, &backup_target)?;
                backed_up.push((target, backup_target));
            }
        }
        for relative in &staged_files {
            let source = staging.join(relative);
            let target = game_directory.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(source, &target)?;
            written.push(target);
        }
        Ok::<_, std::io::Error>(())
    })();
    if let Err(error) = operation {
        for target in written.iter().rev() {
            let _ = std::fs::remove_file(target);
        }
        for (target, backup_target) in backed_up.iter().rev() {
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::rename(backup_target, target);
        }
        return Err(ContentInstallError::Io(error));
    }
    Ok(written)
}

fn collect_files(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), ContentInstallError> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = entry.file_type()?;
        if metadata.is_symlink() {
            return Err(ContentInstallError::UnsafePath);
        }
        if metadata.is_dir() {
            collect_files(root, &entry.path(), output)?;
        } else if metadata.is_file() {
            output.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| ContentInstallError::UnsafePath)?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn validate_relative_path(
    value: &str,
    allow_archive_staging: bool,
) -> Result<PathBuf, ContentInstallError> {
    let normalized = value.replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.starts_with("//")
        || normalized.starts_with("~/")
    {
        return Err(ContentInstallError::UnsafePath);
    }
    let path = Path::new(&normalized);
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => {
                let text = segment.to_string_lossy();
                if text.is_empty() || text.contains(':') || text.contains('\0') {
                    return Err(ContentInstallError::UnsafePath);
                }
                clean.push(segment);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ContentInstallError::UnsafePath);
            }
        }
    }
    let first = clean
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        });
    if clean.as_os_str().is_empty()
        || (first.as_deref() == Some(".slate") && !allow_archive_staging)
        || (first.as_deref() == Some(".slate")
            && !clean
                .to_string_lossy()
                .replace('\\', "/")
                .starts_with(".slate/archives/"))
    {
        return Err(ContentInstallError::UnsafePath);
    }
    Ok(clean)
}

fn validate_archive_prefix(value: &str) -> Result<String, ContentInstallError> {
    let normalized = value.replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    let clean = validate_relative_path(trimmed, false)?;
    Ok(format!("{}/", clean.to_string_lossy().replace('\\', "/")))
}

async fn remove_managed_path(path: &Path) -> Result<(), ContentInstallError> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.is_dir() => tokio::fs::remove_dir_all(path).await?,
        Ok(_) => tokio::fs::remove_file(path).await?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(ContentInstallError::Io(error)),
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[derive(Debug, thiserror::Error)]
pub enum ContentInstallError {
    #[error("the content plan uses unsupported schema {0}")]
    UnsupportedSchema(u32),
    #[error("the content plan targets Minecraft {actual}, not {expected}")]
    MinecraftMismatch { expected: String, actual: String },
    #[error("the content plan uses unsupported loader {0}")]
    UnsupportedLoader(String),
    #[error("the content plan loader does not match the instance")]
    LoaderMismatch,
    #[error("the content plan loader version does not exactly match the instance")]
    LoaderVersionMismatch,
    #[error(
        "the content plan requires Java {actual}, but Minecraft metadata requires Java {expected}"
    )]
    JavaMismatch { expected: u32, actual: u32 },
    #[error("the content plan is invalid")]
    InvalidPlan,
    #[error("the content plan exceeds launcher safety limits")]
    PlanTooLarge,
    #[error("the content plan contains an unsafe path")]
    UnsafePath,
    #[error("the content plan contains duplicate destinations")]
    DuplicateDestination,
    #[error("the content plan contains an invalid hash")]
    InvalidHash,
    #[error("content download URL is not trusted")]
    UnsafeDownloadUrl,
    #[error("content download redirected too many times")]
    TooManyRedirects,
    #[error("content download returned HTTP {0}")]
    HttpStatus(StatusCode),
    #[error("content download size does not match its plan")]
    SizeMismatch,
    #[error("content download hash does not match its plan")]
    HashMismatch,
    #[error("no content download source succeeded")]
    DownloadUnavailable,
    #[error("content plan references unknown archive {0}")]
    UnknownArchive(String),
    #[error("content archive contains an unsafe path or link")]
    UnsafeArchivePath,
    #[error("content archive exceeds launcher extraction limits")]
    ArchiveTooLarge,
    #[error("content request URL is invalid")]
    Url(#[from] url::ParseError),
    #[error("content request failed")]
    Request(#[from] reqwest::Error),
    #[error("content archive is invalid")]
    Zip(#[from] zip::result::ZipError),
    #[error("content filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("content installation worker failed")]
    Worker(#[from] tokio::task::JoinError),
}

#[cfg(test)]
mod tests {
    use super::{
        ContentInstallError, content_download_label, format_download_size, validate_archive_prefix,
        validate_download_url, validate_relative_path,
    };
    use slate_modpack_api_contracts::{DownloadSource, Hashes, InstallPlanDownload};

    #[test]
    fn paths_cannot_escape_or_use_reserved_storage() {
        assert!(validate_relative_path("mods/example.jar", false).is_ok());
        assert!(validate_relative_path(".slate/archives/pack.archive", true).is_ok());
        for invalid in [
            "../evil",
            "C:/evil",
            "//server/share",
            "/etc/passwd",
            "mods/x:ads",
            ".slate/state",
        ] {
            assert!(matches!(
                validate_relative_path(invalid, false),
                Err(ContentInstallError::UnsafePath)
            ));
        }
        assert_eq!(
            validate_archive_prefix("overrides").ok().as_deref(),
            Some("overrides/")
        );
    }

    #[test]
    fn download_hosts_are_allowlisted() {
        assert!(validate_download_url("https://cdn.modrinth.com/data/a.jar", false).is_ok());
        assert!(
            validate_download_url("https://mediafilez.forgecdn.net/files/a.jar", false).is_ok()
        );
        assert!(validate_download_url("https://evil.example/a.jar", false).is_err());
        assert!(validate_download_url("http://cdn.modrinth.com/a.jar", false).is_err());
    }

    #[test]
    fn progress_messages_use_bounded_human_file_details() {
        let download = InstallPlanDownload {
            id: "curseforge:pack:file".to_owned(),
            destination: "mods/architectury-13.0.11-neoforge.jar".to_owned(),
            size: 584_734,
            hashes: Hashes::default(),
            sources: vec![DownloadSource::Direct {
                url: "https://cdn.modrinth.com/example.jar".to_owned(),
            }],
            required: true,
        };

        assert_eq!(
            content_download_label(&download),
            "architectury-13.0.11-neoforge.jar"
        );
        assert_eq!(format_download_size(download.size), "571.0 KB");
    }
}
