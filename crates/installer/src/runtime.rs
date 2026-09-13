use reqwest::Client;
use reqwest::redirect::{Action, Attempt, Policy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use slate_platform::{JavaArchitecture, probe_java_executable};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use url::Url;
use uuid::Uuid;
use zip::ZipArchive;

const MAX_RUNTIME_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RUNTIME_EXTRACTED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_RUNTIME_ENTRIES: usize = 100_000;
const RUNTIME_REDIRECT_HOSTS: [&str; 4] = [
    "api.adoptium.net",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedJavaRuntime {
    pub vendor: String,
    pub release_name: String,
    pub major_version: u32,
    pub architecture: String,
    pub executable: PathBuf,
    pub package_sha256: String,
}

pub async fn ensure_managed_java(
    runtimes_directory: &Path,
    major_version: u32,
    architecture: JavaArchitecture,
) -> Result<ManagedJavaRuntime, RuntimeInstallError> {
    let platform = runtime_platform()?;
    let platform_arch = format!("{platform}-{}", architecture.platform_id());
    let family = runtimes_directory
        .join("java")
        .join(major_version.to_string())
        .join(platform_arch);
    tokio::fs::create_dir_all(&family).await?;
    if let Some(runtime) = load_current_runtime(&family, major_version, architecture).await? {
        return Ok(runtime);
    }

    let client = runtime_http_client()?;
    let release = fetch_release(&client, major_version, architecture, platform).await?;
    validate_release(&release, major_version, architecture, platform)?;
    let safe_release = safe_release_segment(&release.release_name)?;
    let final_directory = family.join(safe_release);
    let executable = final_directory.join(java_relative_path());
    if final_directory.is_dir() {
        let runtime = runtime_from_release(&release, executable);
        validate_runtime_probe(&runtime, architecture).await?;
        write_current_marker(&family, &runtime).await?;
        return Ok(runtime);
    }

    let archive = family.join(format!(".runtime-{}.partial.zip", Uuid::new_v4()));
    download_runtime_archive(&client, &release.binary.package, &archive).await?;
    let staging = family.join(format!(".staging-{}", Uuid::new_v4()));
    tokio::fs::create_dir(&staging).await?;
    let archive_for_extract = archive.clone();
    let staging_for_extract = staging.clone();
    let extraction = tokio::task::spawn_blocking(move || {
        extract_runtime_zip(&archive_for_extract, &staging_for_extract)
    })
    .await?;
    if let Err(error) = extraction {
        let _ = tokio::fs::remove_file(&archive).await;
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }
    let _ = tokio::fs::remove_file(&archive).await;

    let staged_runtime = runtime_from_release(&release, staging.join(java_relative_path()));
    if let Err(error) = validate_runtime_probe(&staged_runtime, architecture).await {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }
    tokio::fs::rename(&staging, &final_directory).await?;
    let runtime = runtime_from_release(&release, final_directory.join(java_relative_path()));
    write_current_marker(&family, &runtime).await?;
    Ok(runtime)
}

async fn load_current_runtime(
    family: &Path,
    major_version: u32,
    architecture: JavaArchitecture,
) -> Result<Option<ManagedJavaRuntime>, RuntimeInstallError> {
    let marker = family.join("current.json");
    let bytes = match tokio::fs::read(&marker).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let runtime: ManagedJavaRuntime = serde_json::from_slice(&bytes)?;
    if runtime.major_version != major_version
        || runtime.architecture != architecture.platform_id()
        || !runtime.executable.is_file()
    {
        return Ok(None);
    }
    if validate_runtime_probe(&runtime, architecture).await.is_err() {
        return Ok(None);
    }
    Ok(Some(runtime))
}

async fn fetch_release(
    client: &Client,
    major: u32,
    architecture: JavaArchitecture,
    platform: &str,
) -> Result<AdoptiumRelease, RuntimeInstallError> {
    let architecture = match architecture {
        JavaArchitecture::X86 => "x32",
        JavaArchitecture::X86_64 => "x64",
        JavaArchitecture::Arm64 => "aarch64",
    };
    let url = Url::parse_with_params(
        &format!("https://api.adoptium.net/v3/assets/latest/{major}/hotspot"),
        &[
            ("architecture", architecture),
            ("heap_size", "normal"),
            ("image_type", "jre"),
            ("jvm_impl", "hotspot"),
            ("os", platform),
            ("project", "jdk"),
            ("vendor", "eclipse"),
        ],
    )?;
    let response = client.get(url).send().await?.error_for_status()?;
    let releases: Vec<AdoptiumRelease> = response.json().await?;
    releases
        .into_iter()
        .next()
        .ok_or(RuntimeInstallError::RuntimeUnavailable { major })
}

async fn download_runtime_archive(
    client: &Client,
    package: &AdoptiumPackage,
    target: &Path,
) -> Result<(), RuntimeInstallError> {
    let url = Url::parse(&package.link)?;
    validate_runtime_url(&url)?;
    if package.size > MAX_RUNTIME_ARCHIVE_BYTES {
        return Err(RuntimeInstallError::ArchiveTooLarge(package.size));
    }
    validate_sha256(&package.checksum)?;
    let mut response = client.get(url).send().await?.error_for_status()?;
    if response
        .content_length()
        .is_some_and(|length| length != package.size)
    {
        return Err(RuntimeInstallError::PackageSizeMismatch);
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(target)
        .await?;
    let mut digest = Sha256::new();
    let mut size = 0_u64;
    while let Some(chunk) = response.chunk().await? {
        size = size
            .checked_add(chunk.len() as u64)
            .ok_or(RuntimeInstallError::ArchiveTooLarge(MAX_RUNTIME_ARCHIVE_BYTES))?;
        if size > package.size || size > MAX_RUNTIME_ARCHIVE_BYTES {
            return Err(RuntimeInstallError::PackageSizeMismatch);
        }
        digest.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    if size != package.size {
        return Err(RuntimeInstallError::PackageSizeMismatch);
    }
    let actual = digest_hex(&digest.finalize());
    if actual != package.checksum.to_ascii_lowercase() {
        return Err(RuntimeInstallError::PackageHashMismatch);
    }
    Ok(())
}

fn extract_runtime_zip(archive: &Path, destination: &Path) -> Result<(), RuntimeInstallError> {
    let file = std::fs::File::open(archive)?;
    let mut zip = ZipArchive::new(file)?;
    if zip.len() > MAX_RUNTIME_ENTRIES {
        return Err(RuntimeInstallError::TooManyArchiveEntries(zip.len()));
    }
    let mut extracted = 0_u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        if entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err(RuntimeInstallError::ArchiveLinkRejected);
        }
        let enclosed = entry
            .enclosed_name()
            .ok_or(RuntimeInstallError::UnsafeArchivePath)?;
        let mut components = enclosed.components();
        let _root = components.next().ok_or(RuntimeInstallError::UnsafeArchivePath)?;
        let relative = components.collect::<PathBuf>();
        if relative.as_os_str().is_empty() {
            continue;
        }
        if relative.components().any(|component| {
            !matches!(component, Component::Normal(_))
        }) {
            return Err(RuntimeInstallError::UnsafeArchivePath);
        }
        let target = destination.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        extracted = extracted
            .checked_add(entry.size())
            .ok_or(RuntimeInstallError::ExtractedRuntimeTooLarge)?;
        if extracted > MAX_RUNTIME_EXTRACTED_BYTES {
            return Err(RuntimeInstallError::ExtractedRuntimeTooLarge);
        }
        let parent = target.parent().ok_or(RuntimeInstallError::UnsafeArchivePath)?;
        std::fs::create_dir_all(parent)?;
        let mut output = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(target)?;
        let size = entry.size();
        std::io::copy(&mut entry.by_ref().take(size), &mut output)?;
        output.flush()?;
    }
    Ok(())
}

async fn validate_runtime_probe(
    runtime: &ManagedJavaRuntime,
    architecture: JavaArchitecture,
) -> Result<(), RuntimeInstallError> {
    let executable = runtime.executable.clone();
    let probe = tokio::task::spawn_blocking(move || probe_java_executable(&executable)).await?;
    if !probe.available
        || probe.major_version != Some(runtime.major_version)
        || probe.architecture != Some(architecture)
    {
        return Err(RuntimeInstallError::RuntimeProbeMismatch);
    }
    Ok(())
}

async fn write_current_marker(
    family: &Path,
    runtime: &ManagedJavaRuntime,
) -> Result<(), RuntimeInstallError> {
    let bytes = serde_json::to_vec_pretty(runtime)?;
    let partial = family.join(format!(".current-{}.partial", Uuid::new_v4()));
    tokio::fs::write(&partial, bytes).await?;
    let marker = family.join("current.json");
    if marker.exists() {
        tokio::fs::remove_file(&marker).await?;
    }
    tokio::fs::rename(partial, marker).await?;
    Ok(())
}

fn runtime_from_release(release: &AdoptiumRelease, executable: PathBuf) -> ManagedJavaRuntime {
    ManagedJavaRuntime {
        vendor: "Eclipse Temurin".to_owned(),
        release_name: release.release_name.clone(),
        major_version: release.version.major,
        architecture: release.binary.architecture.clone(),
        executable,
        package_sha256: release.binary.package.checksum.to_ascii_lowercase(),
    }
}

fn runtime_http_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .user_agent(format!("slate/{} (managed runtime)", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .redirect(Policy::custom(validate_redirect))
        .build()
}

fn validate_redirect(attempt: Attempt<'_>) -> Action {
    if attempt.previous().len() >= 5 {
        return attempt.error("managed runtime download exceeded five redirects");
    }
    if validate_runtime_url(attempt.url()).is_ok() {
        attempt.follow()
    } else {
        attempt.stop()
    }
}

fn validate_runtime_url(url: &Url) -> Result<(), RuntimeInstallError> {
    let allowed = url
        .host_str()
        .is_some_and(|host| RUNTIME_REDIRECT_HOSTS.contains(&host));
    if url.scheme() != "https"
        || !allowed
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(RuntimeInstallError::UntrustedRuntimeOrigin(url.clone()));
    }
    Ok(())
}

fn validate_release(
    release: &AdoptiumRelease,
    major: u32,
    architecture: JavaArchitecture,
    platform: &str,
) -> Result<(), RuntimeInstallError> {
    if release.vendor != "eclipse"
        || release.version.major != major
        || release.binary.os != platform
        || release.binary.architecture != architecture.platform_id()
        || release.binary.image_type != "jre"
    {
        return Err(RuntimeInstallError::ReleaseMetadataMismatch);
    }
    validate_sha256(&release.binary.package.checksum)
}

fn validate_sha256(value: &str) -> Result<(), RuntimeInstallError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RuntimeInstallError::InvalidPackageHash);
    }
    Ok(())
}

fn safe_release_segment(value: &str) -> Result<&str, RuntimeInstallError> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-')
        })
    {
        return Err(RuntimeInstallError::InvalidReleaseName);
    }
    Ok(value)
}

const fn runtime_platform() -> Result<&'static str, RuntimeInstallError> {
    if cfg!(target_os = "windows") {
        Ok("windows")
    } else {
        Err(RuntimeInstallError::UnsupportedPlatform)
    }
}

fn java_relative_path() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("bin/java.exe")
    } else {
        PathBuf::from("bin/java")
    }
}

fn digest_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[derive(Clone, Debug, Deserialize)]
struct AdoptiumRelease {
    binary: AdoptiumBinary,
    release_name: String,
    vendor: String,
    version: AdoptiumVersion,
}

#[derive(Clone, Debug, Deserialize)]
struct AdoptiumBinary {
    architecture: String,
    image_type: String,
    os: String,
    package: AdoptiumPackage,
}

#[derive(Clone, Debug, Deserialize)]
struct AdoptiumPackage {
    checksum: String,
    link: String,
    size: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct AdoptiumVersion {
    major: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeInstallError {
    #[error("managed Java is not yet supported on this platform")]
    UnsupportedPlatform,
    #[error("no managed Java {major} release is available for this platform")]
    RuntimeUnavailable { major: u32 },
    #[error("runtime release metadata does not match the request")]
    ReleaseMetadataMismatch,
    #[error("runtime release name is not a safe path segment")]
    InvalidReleaseName,
    #[error("runtime package SHA-256 is invalid")]
    InvalidPackageHash,
    #[error("runtime archive is too large: {0} bytes")]
    ArchiveTooLarge(u64),
    #[error("runtime package size did not match metadata")]
    PackageSizeMismatch,
    #[error("runtime package SHA-256 did not match metadata")]
    PackageHashMismatch,
    #[error("runtime URL is outside slate's HTTPS origin allowlist: {0}")]
    UntrustedRuntimeOrigin(Url),
    #[error("runtime archive has too many entries: {0}")]
    TooManyArchiveEntries(usize),
    #[error("runtime archive contains an unsafe path")]
    UnsafeArchivePath,
    #[error("runtime archive links are not permitted")]
    ArchiveLinkRejected,
    #[error("extracted runtime exceeds the safety limit")]
    ExtractedRuntimeTooLarge,
    #[error("installed runtime did not report the required major and architecture")]
    RuntimeProbeMismatch,
    #[error("runtime HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("runtime URL is invalid")]
    Url(#[from] url::ParseError),
    #[error("runtime metadata is invalid")]
    Json(#[from] serde_json::Error),
    #[error("runtime archive is invalid")]
    Zip(#[from] zip::result::ZipError),
    #[error("runtime filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("runtime worker failed")]
    Worker(#[from] tokio::task::JoinError),
}

#[cfg(test)]
mod tests {
    use super::{RuntimeInstallError, safe_release_segment, validate_runtime_url};
    use url::Url;

    #[test]
    fn release_names_are_safe_directory_segments() {
        assert_eq!(safe_release_segment("jdk-21.0.12+8").ok(), Some("jdk-21.0.12+8"));
        assert!(matches!(
            safe_release_segment("../java"),
            Err(RuntimeInstallError::InvalidReleaseName)
        ));
    }

    #[test]
    fn runtime_redirects_stay_on_known_https_hosts() -> Result<(), url::ParseError> {
        let github = Url::parse("https://github.com/adoptium/runtime.zip")?;
        let local = Url::parse("https://127.0.0.1/runtime.zip")?;
        assert!(validate_runtime_url(&github).is_ok());
        assert!(validate_runtime_url(&local).is_err());
        Ok(())
    }
}
