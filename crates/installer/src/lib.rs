//! Transactional Minecraft installation and managed Java acquisition for slate.

mod download;
mod modpack;
mod natives;
mod runtime;

use natives::extract_natives;

pub use download::{DownloadError, DownloadProgress, DownloadSummary, Downloader};
pub use modpack::{ContentInstallError, PendingContentCommit};
pub use runtime::{ManagedJavaRuntime, RuntimeInstallError, ensure_managed_java};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use slate_domain::{InstanceId, LoaderFamily, RevisionId};
use slate_loaders::{FabricAdapter, NeoForgeAdapter, NeoForgeInstallerBundle};
use slate_minecraft::{
    Architecture, ArtifactRequirement, ExpectedHash, HashAlgorithm, LaunchLayout, LaunchPlanner,
    MojangMetadataClient, OperatingSystem, ResolvedVersion, RuleContext, VersionMetadata,
};
use slate_modpack_api_contracts::InstallPlan;
use slate_platform::{
    AppPaths, JavaArchitecture, ManagedRelativePath, restricted_child_environment,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use uuid::Uuid;

const MAX_ASSET_INDEX_BYTES: usize = 16 * 1024 * 1024;
const MAX_ASSET_OBJECTS: usize = 1_000_000;
const MAX_INSTALLED_MANIFEST_BYTES: usize = 32 * 1024 * 1024;
const MAX_LAUNCH_ARTIFACTS: usize = 16_384;

#[derive(Clone, Debug)]
pub struct InstallRequest {
    pub instance_id: InstanceId,
    pub revision_id: RevisionId,
    pub minecraft_version: String,
    pub loader_kind: LoaderFamily,
    pub loader_version: Option<String>,
    pub modpack_plan: Option<InstallPlan>,
    pub download_concurrency: u8,
    pub download_bandwidth_limit_mib: u32,
    pub paths: AppPaths,
}

#[derive(Clone, Debug)]
pub struct ContentUpdateRequest {
    pub instance_id: InstanceId,
    pub revision_id: RevisionId,
    pub parent_revision_id: RevisionId,
    pub parent_manifest_digest: String,
    pub minecraft_version: String,
    pub loader_kind: LoaderFamily,
    pub loader_version: Option<String>,
    pub plan: InstallPlan,
    pub download_concurrency: u8,
    pub download_bandwidth_limit_mib: u32,
    pub paths: AppPaths,
}

#[derive(Debug)]
pub struct InstallOutcome {
    pub manifest_digest: String,
    pub manifest_path: PathBuf,
    pub resolved_version_id: String,
    pub runtime: ManagedJavaRuntime,
    pub downloaded_artifacts: usize,
    pub reused_artifacts: usize,
    pub installed_content_files: usize,
    pub content_transaction: Option<PendingContentCommit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallPhase {
    Metadata,
    BaseGame,
    Assets,
    Runtime,
    Loader,
    LaunchFiles,
    Natives,
    Content,
    Verification,
    Commit,
}

impl InstallPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::BaseGame => "base-game",
            Self::Assets => "assets",
            Self::Runtime => "runtime",
            Self::Loader => "loader",
            Self::LaunchFiles => "launch-files",
            Self::Natives => "natives",
            Self::Content => "content",
            Self::Verification => "verification",
            Self::Commit => "commit",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallProgress {
    pub phase: InstallPhase,
    pub message: String,
    pub completed_items: Option<u64>,
    pub total_items: Option<u64>,
}

impl InstallProgress {
    #[must_use]
    pub fn indeterminate(phase: InstallPhase, message: impl Into<String>) -> Self {
        Self {
            phase,
            message: message.into(),
            completed_items: None,
            total_items: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct InstalledRevision {
    pub metadata_layers: Vec<VersionMetadata>,
    pub runtime: ManagedJavaRuntime,
    pub launch_artifacts: Vec<InstalledArtifactDigest>,
    pub content_artifacts: Vec<InstalledArtifactDigest>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledArtifactDigest {
    pub relative_path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledRevisionManifest {
    schema_version: u32,
    instance_id: InstanceId,
    revision_id: RevisionId,
    minecraft_version: String,
    loader_kind: LoaderFamily,
    loader_version: Option<String>,
    resolved_version_id: String,
    metadata_layers: Vec<VersionMetadata>,
    runtime: ManagedJavaRuntime,
    launch_artifacts: Vec<InstalledArtifactDigest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    content_artifacts: Vec<InstalledArtifactDigest>,
}

pub async fn install(request: InstallRequest) -> Result<InstallOutcome, InstallError> {
    install_with_progress(request, |_| {}).await
}

pub async fn install_with_progress<F>(
    request: InstallRequest,
    on_progress: F,
) -> Result<InstallOutcome, InstallError>
where
    F: Fn(InstallProgress) + Send + Sync,
{
    validate_request(&request)?;
    on_progress(InstallProgress::indeterminate(
        InstallPhase::Metadata,
        format!("Resolving Minecraft {} metadata", request.minecraft_version),
    ));
    let minecraft_root = request.paths.artifacts().join("minecraft");
    let revision_directory = request
        .paths
        .instance(request.instance_id)
        .join("revisions")
        .join(request.revision_id.to_string());
    let game_directory = request.paths.instance(request.instance_id).join("game");
    let metadata_directory = revision_directory.join("metadata");
    let native_directory = revision_directory.join("natives");
    for directory in [
        &minecraft_root,
        &metadata_directory,
        &game_directory,
        &native_directory,
    ] {
        tokio::fs::create_dir_all(directory).await?;
    }

    let layout = LaunchLayout {
        game_directory,
        libraries_directory: minecraft_root.join("libraries"),
        versions_directory: minecraft_root.join("versions"),
        assets_directory: minecraft_root.join("assets"),
        natives_directory: native_directory,
    };
    let rules = current_rule_context();
    let metadata = MojangMetadataClient::new()?;
    let manifest = metadata.fetch_manifest().await?;
    let base = metadata
        .fetch_version(&manifest, &request.minecraft_version)
        .await?;
    write_version_metadata(&layout.versions_directory, &base).await?;

    let base_resolved = ResolvedVersion::resolve(vec![base.clone()])?;
    let base_install = LaunchPlanner::prepare_install(&base_resolved, &layout, &rules)?;
    if let Some(plan) = &request.modpack_plan {
        modpack::validate_plan_compatibility(&request, plan, base_install.required_java_major)?;
    }
    let downloader = Downloader::with_bandwidth_limit(
        request.download_concurrency,
        request.download_bandwidth_limit_mib,
    )?;
    let mut summary = download_install_phase(
        &downloader,
        base_install.required_artifacts,
        InstallPhase::BaseGame,
        "Checking base game files",
        &on_progress,
    )
    .await?;
    let assets = asset_requirements(&layout.assets_directory, &base_resolved)?;
    merge_summary(
        &mut summary,
        download_install_phase(
            &downloader,
            assets,
            InstallPhase::Assets,
            "Checking game assets",
            &on_progress,
        )
        .await?,
    );

    let java_architecture = current_java_architecture()?;
    on_progress(InstallProgress::indeterminate(
        InstallPhase::Runtime,
        format!(
            "Preparing managed Java {}",
            base_install.required_java_major
        ),
    ));
    let runtime = ensure_managed_java(
        &request.paths.runtimes(),
        base_install.required_java_major,
        java_architecture,
    )
    .await?;

    let layers = match request.loader_kind {
        LoaderFamily::Vanilla => {
            on_progress(InstallProgress::indeterminate(
                InstallPhase::Loader,
                "Preparing the Vanilla launch profile",
            ));
            vec![base]
        }
        LoaderFamily::Fabric => {
            let loader_version = request
                .loader_version
                .as_deref()
                .ok_or(InstallError::LoaderVersionRequired)?;
            on_progress(InstallProgress::indeterminate(
                InstallPhase::Loader,
                format!("Resolving Fabric {loader_version}"),
            ));
            let overlay = FabricAdapter::new()?
                .fetch_profile(&request.minecraft_version, loader_version)
                .await?;
            write_version_metadata(&layout.versions_directory, &overlay).await?;
            vec![base, overlay]
        }
        LoaderFamily::NeoForge => {
            let loader_version = request
                .loader_version
                .as_deref()
                .ok_or(InstallError::LoaderVersionRequired)?;
            on_progress(InstallProgress::indeterminate(
                InstallPhase::Loader,
                format!("Running the NeoForge {loader_version} client installer"),
            ));
            let bundle = NeoForgeAdapter::new()?
                .fetch_installer(&request.minecraft_version, loader_version)
                .await?;
            run_neoforge_installer(
                &bundle,
                loader_version,
                &runtime,
                &minecraft_root,
                &metadata_directory,
            )
            .await?;
            let installed = read_installed_neoforge_version(
                &layout.versions_directory,
                &request.minecraft_version,
                loader_version,
            )
            .await?;
            vec![base, installed]
        }
    };

    let resolved = ResolvedVersion::resolve(layers.clone())?;
    let full_install = LaunchPlanner::prepare_install(&resolved, &layout, &rules)?;
    let requirements = if request.loader_kind == LoaderFamily::NeoForge {
        trust_neoforge_processor_outputs(full_install.required_artifacts, &layout, &request).await?
    } else {
        full_install.required_artifacts
    };
    merge_summary(
        &mut summary,
        download_install_phase(
            &downloader,
            requirements.clone(),
            InstallPhase::LaunchFiles,
            "Checking loader and launch files",
            &on_progress,
        )
        .await?,
    );
    on_progress(InstallProgress::indeterminate(
        InstallPhase::Natives,
        "Extracting native libraries",
    ));
    extract_natives(full_install.native_extractions).await?;
    let installed_content = if let Some(plan) = &request.modpack_plan {
        Some(
            modpack::install_plan_content(
                plan,
                &layout.game_directory,
                &revision_directory,
                request.paths.storage_root(),
                request.download_concurrency,
                request.download_bandwidth_limit_mib,
                |completed, total, message| {
                    on_progress(InstallProgress {
                        phase: InstallPhase::Content,
                        message,
                        completed_items: Some(completed),
                        total_items: Some(total),
                    });
                },
            )
            .await?,
        )
    } else {
        None
    };
    let (content_artifacts, content_transaction) = installed_content.map_or_else(
        || (Vec::new(), None),
        |installed| (installed.artifacts, Some(installed.transaction)),
    );
    on_progress(InstallProgress::indeterminate(
        InstallPhase::Verification,
        "Creating the verified launch-file index",
    ));
    let launch_artifacts = hash_launch_artifacts(&request.paths, requirements).await?;

    on_progress(InstallProgress::indeterminate(
        InstallPhase::Commit,
        "Finalizing the installed revision",
    ));
    let installed_content_files = content_artifacts.len();
    let installed_manifest = InstalledRevisionManifest {
        schema_version: 2,
        instance_id: request.instance_id,
        revision_id: request.revision_id,
        minecraft_version: request.minecraft_version,
        loader_kind: request.loader_kind,
        loader_version: request.loader_version,
        resolved_version_id: resolved.id().to_owned(),
        metadata_layers: layers,
        runtime: runtime.clone(),
        launch_artifacts,
        content_artifacts,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&installed_manifest)?;
    let manifest_digest = digest_hex(&Sha256::digest(&manifest_bytes));
    let manifest_path = metadata_directory.join("installed-revision.json");
    atomic_write(&manifest_path, &manifest_bytes).await?;

    Ok(InstallOutcome {
        manifest_digest,
        manifest_path,
        resolved_version_id: resolved.id().to_owned(),
        runtime,
        downloaded_artifacts: summary.downloaded,
        reused_artifacts: summary.reused,
        installed_content_files,
        content_transaction,
    })
}

pub async fn update_content_with_progress<F>(
    request: ContentUpdateRequest,
    on_progress: F,
) -> Result<InstallOutcome, InstallError>
where
    F: Fn(InstallProgress) + Send + Sync,
{
    if request.minecraft_version.trim().is_empty() {
        return Err(InstallError::MinecraftVersionRequired);
    }
    if request.loader_kind.requires_version()
        && request.loader_version.as_deref().is_none_or(str::is_empty)
    {
        return Err(InstallError::LoaderVersionRequired);
    }
    let instance_directory = request.paths.instance(request.instance_id);
    let parent_manifest_path = instance_directory
        .join("revisions")
        .join(request.parent_revision_id.to_string())
        .join("metadata")
        .join("installed-revision.json");
    on_progress(InstallProgress::indeterminate(
        InstallPhase::Metadata,
        "Loading the verified instance revision",
    ));
    let installed = load_installed_revision(
        &parent_manifest_path,
        request.instance_id,
        request.parent_revision_id,
        &request.parent_manifest_digest,
    )
    .await?;
    let resolved = ResolvedVersion::resolve(installed.metadata_layers.clone())?;
    modpack::validate_plan_target(
        &request.minecraft_version,
        request.loader_kind,
        request.loader_version.as_deref(),
        &request.plan,
        installed.runtime.major_version,
    )?;

    let revision_directory = instance_directory
        .join("revisions")
        .join(request.revision_id.to_string());
    let metadata_directory = revision_directory.join("metadata");
    let game_directory = instance_directory.join("game");
    tokio::fs::create_dir_all(&metadata_directory).await?;
    tokio::fs::create_dir_all(&game_directory).await?;
    let parent_natives = instance_directory
        .join("revisions")
        .join(request.parent_revision_id.to_string())
        .join("natives");
    let revision_natives = revision_directory.join("natives");
    tokio::task::spawn_blocking(move || clone_directory_tree(&parent_natives, &revision_natives))
        .await??;
    let installed_content = modpack::install_plan_content(
        &request.plan,
        &game_directory,
        &revision_directory,
        request.paths.storage_root(),
        request.download_concurrency,
        request.download_bandwidth_limit_mib,
        |completed, total, message| {
            on_progress(InstallProgress {
                phase: InstallPhase::Content,
                message,
                completed_items: Some(completed),
                total_items: Some(total),
            });
        },
    )
    .await?;
    let added_content = installed_content.artifacts;
    let content_transaction = installed_content.transaction;

    on_progress(InstallProgress::indeterminate(
        InstallPhase::Commit,
        "Updating the verified content index",
    ));
    let installed_content_files = added_content.len();
    let mut content_by_path = installed
        .content_artifacts
        .into_iter()
        .map(|artifact| (artifact.relative_path.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    remove_deleted_content_artifacts(
        &mut content_by_path,
        &request.plan.delete,
        &game_directory,
        request.paths.storage_root(),
    )?;
    for artifact in added_content {
        content_by_path.insert(artifact.relative_path.clone(), artifact);
    }
    let installed_manifest = InstalledRevisionManifest {
        schema_version: 2,
        instance_id: request.instance_id,
        revision_id: request.revision_id,
        minecraft_version: request.minecraft_version,
        loader_kind: request.loader_kind,
        loader_version: request.loader_version,
        resolved_version_id: resolved.id().to_owned(),
        metadata_layers: installed.metadata_layers,
        runtime: installed.runtime.clone(),
        launch_artifacts: installed.launch_artifacts,
        content_artifacts: content_by_path.into_values().collect(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&installed_manifest)?;
    let manifest_digest = digest_hex(&Sha256::digest(&manifest_bytes));
    let manifest_path = metadata_directory.join("installed-revision.json");
    atomic_write(&manifest_path, &manifest_bytes).await?;

    Ok(InstallOutcome {
        manifest_digest,
        manifest_path,
        resolved_version_id: resolved.id().to_owned(),
        runtime: installed.runtime,
        downloaded_artifacts: installed_content_files,
        reused_artifacts: 0,
        installed_content_files,
        content_transaction: Some(content_transaction),
    })
}

fn remove_deleted_content_artifacts(
    content_by_path: &mut BTreeMap<String, InstalledArtifactDigest>,
    deleted_paths: &[String],
    game_directory: &Path,
    storage_root: &Path,
) -> Result<(), InstallError> {
    for deleted in deleted_paths {
        let stored_path = relative_artifact_path(storage_root, &game_directory.join(deleted))?;
        content_by_path.remove(&stored_path);
    }
    Ok(())
}

fn clone_directory_tree(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            clone_directory_tree(&entry.path(), &target)?;
        } else if file_type.is_file() && std::fs::hard_link(entry.path(), &target).is_err() {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

async fn download_install_phase<F>(
    downloader: &Downloader,
    requirements: Vec<ArtifactRequirement>,
    phase: InstallPhase,
    message: &'static str,
    on_progress: &F,
) -> Result<DownloadSummary, InstallError>
where
    F: Fn(InstallProgress) + Send + Sync,
{
    let total = u64::try_from(requirements.len()).unwrap_or(u64::MAX);
    on_progress(InstallProgress {
        phase,
        message: message.to_owned(),
        completed_items: Some(0),
        total_items: Some(total),
    });
    let summary = downloader
        .download_all_with_progress(requirements, |progress| {
            on_progress(InstallProgress {
                phase,
                message: message.to_owned(),
                completed_items: Some(u64::try_from(progress.completed).unwrap_or(u64::MAX)),
                total_items: Some(u64::try_from(progress.total).unwrap_or(u64::MAX)),
            });
        })
        .await?;
    Ok(summary)
}

pub async fn load_installed_revision(
    path: &Path,
    expected_instance_id: InstanceId,
    expected_revision_id: RevisionId,
    expected_digest: &str,
) -> Result<InstalledRevision, InstallError> {
    let bytes = tokio::fs::read(path).await?;
    if bytes.len() > MAX_INSTALLED_MANIFEST_BYTES || !valid_sha256(expected_digest) {
        return Err(InstallError::InvalidInstalledManifest);
    }
    let actual_digest = digest_hex(&Sha256::digest(&bytes));
    if actual_digest != expected_digest.to_ascii_lowercase() {
        return Err(InstallError::InstalledManifestDigestMismatch);
    }
    let manifest: InstalledRevisionManifest = serde_json::from_slice(&bytes)?;
    if manifest.schema_version != 2
        || manifest.instance_id != expected_instance_id
        || manifest.revision_id != expected_revision_id
        || manifest.metadata_layers.is_empty()
        || manifest.launch_artifacts.is_empty()
        || manifest.launch_artifacts.len() > MAX_LAUNCH_ARTIFACTS
    {
        return Err(InstallError::InvalidInstalledManifest);
    }
    validate_installed_artifact_declarations(&manifest.launch_artifacts)?;
    Ok(InstalledRevision {
        metadata_layers: manifest.metadata_layers,
        runtime: manifest.runtime,
        launch_artifacts: manifest.launch_artifacts,
        content_artifacts: manifest.content_artifacts,
    })
}

fn validate_request(request: &InstallRequest) -> Result<(), InstallError> {
    if request.minecraft_version.trim().is_empty() {
        return Err(InstallError::MinecraftVersionRequired);
    }
    if request.loader_kind.requires_version()
        && request.loader_version.as_deref().is_none_or(str::is_empty)
    {
        return Err(InstallError::LoaderVersionRequired);
    }
    if !request.loader_kind.requires_version() && request.loader_version.is_some() {
        return Err(InstallError::UnexpectedLoaderVersion);
    }
    Ok(())
}

fn current_rule_context() -> RuleContext {
    RuleContext::new(
        if cfg!(target_os = "windows") {
            OperatingSystem::Windows
        } else if cfg!(target_os = "macos") {
            OperatingSystem::MacOs
        } else {
            OperatingSystem::Linux
        },
        if cfg!(target_arch = "x86") {
            Architecture::X86
        } else if cfg!(target_arch = "aarch64") {
            Architecture::Arm64
        } else {
            Architecture::X86_64
        },
        std::env::consts::OS,
    )
}

const fn current_java_architecture() -> Result<JavaArchitecture, InstallError> {
    if cfg!(target_arch = "x86") {
        Ok(JavaArchitecture::X86)
    } else if cfg!(target_arch = "x86_64") {
        Ok(JavaArchitecture::X86_64)
    } else if cfg!(target_arch = "aarch64") {
        Ok(JavaArchitecture::Arm64)
    } else {
        Err(InstallError::UnsupportedArchitecture)
    }
}

async fn write_version_metadata(
    versions_directory: &Path,
    metadata: &VersionMetadata,
) -> Result<(), InstallError> {
    let directory = versions_directory.join(&metadata.id);
    tokio::fs::create_dir_all(&directory).await?;
    atomic_write(
        &directory.join(format!("{}.json", metadata.id)),
        &serde_json::to_vec_pretty(metadata)?,
    )
    .await
}

fn asset_requirements(
    assets_directory: &Path,
    version: &ResolvedVersion,
) -> Result<Vec<ArtifactRequirement>, InstallError> {
    let path = assets_directory
        .join("indexes")
        .join(format!("{}.json", version.asset_index().id));
    let bytes = std::fs::read(path)?;
    if bytes.len() > MAX_ASSET_INDEX_BYTES {
        return Err(InstallError::AssetIndexTooLarge(bytes.len()));
    }
    let index: AssetIndexDocument = serde_json::from_slice(&bytes)?;
    if index.objects.len() > MAX_ASSET_OBJECTS {
        return Err(InstallError::TooManyAssetObjects(index.objects.len()));
    }
    index
        .objects
        .into_values()
        .map(|object| {
            ArtifactRequirement::official_asset_object(assets_directory, &object.hash, object.size)
                .map_err(InstallError::from)
        })
        .collect()
}

#[derive(Deserialize)]
struct AssetIndexDocument {
    objects: BTreeMap<String, AssetObject>,
}

#[derive(Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

async fn run_neoforge_installer(
    bundle: &NeoForgeInstallerBundle,
    loader_version: &str,
    runtime: &ManagedJavaRuntime,
    minecraft_root: &Path,
    metadata_directory: &Path,
) -> Result<(), InstallError> {
    let installer_directory = minecraft_root
        .join("installers")
        .join("neoforge")
        .join(loader_version);
    tokio::fs::create_dir_all(&installer_directory).await?;
    let installer_path =
        installer_directory.join(format!("neoforge-{loader_version}-installer.jar"));
    atomic_write(&installer_path, &bundle.installer_bytes).await?;
    ensure_launcher_profile_file(minecraft_root).await?;

    let log_path = metadata_directory.join("neoforge-installer.log");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&log_path)?;
    let stderr = log.try_clone()?;
    let mut command = tokio::process::Command::new(&runtime.executable);
    command
        .args(["-jar"])
        .arg(&installer_path)
        .arg("--install-client")
        .arg(minecraft_root)
        .current_dir(metadata_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr))
        .kill_on_drop(true);
    restrict_installer_environment(&mut command, runtime);
    let status = command.status().await?;
    if !status.success() {
        return Err(InstallError::NeoForgeInstallerFailed {
            code: status.code(),
            log_path,
        });
    }
    Ok(())
}

fn restrict_installer_environment(
    command: &mut tokio::process::Command,
    runtime: &ManagedJavaRuntime,
) {
    command.env_clear();
    command.envs(restricted_child_environment(Some(&runtime.executable)));
}

async fn ensure_launcher_profile_file(minecraft_root: &Path) -> Result<(), InstallError> {
    let path = minecraft_root.join("launcher_profiles.json");
    if !path.exists() {
        atomic_write(&path, br#"{"profiles":{},"settings":{},"version":3}"#).await?;
    }
    Ok(())
}

async fn read_installed_neoforge_version(
    versions_directory: &Path,
    minecraft_version: &str,
    loader_version: &str,
) -> Result<VersionMetadata, InstallError> {
    let id = format!("neoforge-{loader_version}");
    let bytes = tokio::fs::read(versions_directory.join(&id).join(format!("{id}.json"))).await?;
    let metadata = VersionMetadata::from_json_slice(&bytes)?;
    if metadata.id != id || metadata.inherits_from.as_deref() != Some(minecraft_version) {
        return Err(InstallError::NeoForgeInstalledMetadataMismatch);
    }
    Ok(metadata)
}

async fn trust_neoforge_processor_outputs(
    requirements: Vec<ArtifactRequirement>,
    layout: &LaunchLayout,
    request: &InstallRequest,
) -> Result<Vec<ArtifactRequirement>, InstallError> {
    let loader_version = request
        .loader_version
        .as_deref()
        .ok_or(InstallError::LoaderVersionRequired)?;
    let generated_root = layout
        .libraries_directory
        .join("net")
        .join("neoforged")
        .join("neoforge")
        .join(loader_version);
    let mut trusted = Vec::with_capacity(requirements.len());
    for requirement in requirements {
        if requirement.expected_hashes().is_empty()
            && requirement.target_path().is_file()
            && requirement.target_path().starts_with(&generated_root)
        {
            let path = requirement.target_path().to_path_buf();
            let hash = tokio::task::spawn_blocking(move || sha256_file(&path)).await??;
            trusted.push(
                requirement.with_expected_hash(ExpectedHash::new(HashAlgorithm::Sha256, &hash)?),
            );
        } else {
            trusted.push(requirement);
        }
    }
    Ok(trusted)
}

async fn hash_launch_artifacts(
    paths: &AppPaths,
    requirements: Vec<ArtifactRequirement>,
) -> Result<Vec<InstalledArtifactDigest>, InstallError> {
    let storage_root = paths.storage_root().to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut artifacts = Vec::with_capacity(requirements.len());
        for requirement in requirements {
            artifacts.push(InstalledArtifactDigest {
                relative_path: relative_artifact_path(&storage_root, requirement.target_path())?,
                sha256: sha256_file(requirement.target_path())?,
            });
        }
        artifacts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        validate_installed_artifact_declarations(&artifacts)?;
        Ok(artifacts)
    })
    .await?
}

pub async fn verify_installed_launch_artifacts(
    storage_root: &Path,
    requirements: &[ArtifactRequirement],
    installed: &[InstalledArtifactDigest],
) -> Result<(), InstallError> {
    let storage_root = storage_root.to_path_buf();
    let requirements = requirements.to_vec();
    let installed = installed.to_vec();
    tokio::task::spawn_blocking(move || {
        validate_installed_artifact_declarations(&installed)?;
        let declared = installed
            .into_iter()
            .map(|artifact| (artifact.relative_path, artifact.sha256))
            .collect::<BTreeMap<_, _>>();
        for requirement in requirements {
            let relative = relative_artifact_path(&storage_root, requirement.target_path())?;
            let expected = declared
                .get(&relative)
                .ok_or_else(|| InstallError::LaunchArtifactNotDeclared(relative.clone()))?;
            let actual = sha256_file(requirement.target_path())?;
            if actual != *expected {
                return Err(InstallError::InstalledArtifactHashMismatch(relative));
            }
        }
        Ok(())
    })
    .await?
}

fn validate_installed_artifact_declarations(
    artifacts: &[InstalledArtifactDigest],
) -> Result<(), InstallError> {
    if artifacts.is_empty() || artifacts.len() > MAX_LAUNCH_ARTIFACTS {
        return Err(InstallError::InvalidInstalledManifest);
    }
    let mut unique = BTreeSet::new();
    for artifact in artifacts {
        let path = ManagedRelativePath::parse(&artifact.relative_path)
            .map_err(|_| InstallError::InvalidInstalledManifest)?;
        if path.as_str() != artifact.relative_path
            || !valid_sha256(&artifact.sha256)
            || !unique.insert(artifact.relative_path.as_str())
        {
            return Err(InstallError::InvalidInstalledManifest);
        }
    }
    Ok(())
}

fn relative_artifact_path(root: &Path, target: &Path) -> Result<String, InstallError> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| InstallError::ArtifactOutsideStorageRoot)?;
    let mut segments = Vec::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(InstallError::ArtifactOutsideStorageRoot);
        };
        segments.push(
            segment
                .to_str()
                .ok_or(InstallError::ArtifactOutsideStorageRoot)?,
        );
    }
    let relative = segments.join("/");
    let managed = ManagedRelativePath::parse(&relative)
        .map_err(|_| InstallError::ArtifactOutsideStorageRoot)?;
    Ok(managed.as_str().to_owned())
}

async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), InstallError> {
    let parent = path
        .parent()
        .ok_or_else(|| InstallError::ParentMissing(path.to_path_buf()))?;
    tokio::fs::create_dir_all(parent).await?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| InstallError::ParentMissing(path.to_path_buf()))?;
    let partial = path.with_file_name(format!(".{name}.partial-{}", Uuid::new_v4()));
    tokio::fs::write(&partial, bytes).await?;
    if path.exists() {
        tokio::fs::remove_file(path).await?;
    }
    tokio::fs::rename(partial, path).await?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest_hex(&digest.finalize()))
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

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn merge_summary(target: &mut DownloadSummary, value: DownloadSummary) {
    target.downloaded += value.downloaded;
    target.reused += value.reused;
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Minecraft version is required")]
    MinecraftVersionRequired,
    #[error("the selected loader requires a version")]
    LoaderVersionRequired,
    #[error("Vanilla cannot have a loader version")]
    UnexpectedLoaderVersion,
    #[error("this CPU architecture is not supported")]
    UnsupportedArchitecture,
    #[error("asset index is too large: {0} bytes")]
    AssetIndexTooLarge(usize),
    #[error("asset index contains too many objects: {0}")]
    TooManyAssetObjects(usize),
    #[error("native archive contains too many entries: {0}")]
    TooManyNativeEntries(usize),
    #[error("native archive path or link is unsafe")]
    UnsafeArchivePath,
    #[error("native extraction exceeds the safety limit")]
    NativeSetTooLarge,
    #[error("NeoForge installer failed with {code:?}; see {log_path}")]
    NeoForgeInstallerFailed {
        code: Option<i32>,
        log_path: PathBuf,
    },
    #[error("NeoForge installed unexpected version metadata")]
    NeoForgeInstalledMetadataMismatch,
    #[error("installed revision manifest is invalid")]
    InvalidInstalledManifest,
    #[error("installed revision manifest digest does not match its database revision")]
    InstalledManifestDigestMismatch,
    #[error("an installed artifact is outside the managed storage root")]
    ArtifactOutsideStorageRoot,
    #[error("launch artifact was not declared by the installed revision: {0}")]
    LaunchArtifactNotDeclared(String),
    #[error("installed launch artifact failed SHA-256 verification: {0}")]
    InstalledArtifactHashMismatch(String),
    #[error("managed path has no parent: {0}")]
    ParentMissing(PathBuf),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Runtime(#[from] RuntimeInstallError),
    #[error(transparent)]
    Content(#[from] ContentInstallError),
    #[error(transparent)]
    Mojang(#[from] slate_minecraft::MetadataFetchError),
    #[error(transparent)]
    Fabric(#[from] slate_loaders::FabricError),
    #[error(transparent)]
    NeoForge(#[from] slate_loaders::NeoForgeError),
    #[error(transparent)]
    Resolve(#[from] slate_minecraft::ResolveError),
    #[error(transparent)]
    Plan(#[from] slate_minecraft::LaunchBuildError),
    #[error(transparent)]
    Artifact(#[from] slate_minecraft::ArtifactError),
    #[error(transparent)]
    Metadata(#[from] slate_minecraft::MetadataError),
    #[error("installation JSON is invalid")]
    Json(#[from] serde_json::Error),
    #[error("installation filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("installation archive is invalid")]
    Zip(#[from] zip::result::ZipError),
    #[error("installation worker failed")]
    Worker(#[from] tokio::task::JoinError),
}

#[cfg(test)]
mod tests;
