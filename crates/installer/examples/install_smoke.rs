use serde::Serialize;
use slate_domain::{InstanceId, LoaderFamily, RevisionId};
use slate_installer::{
    InstallRequest, install, load_installed_revision, verify_installed_launch_artifacts,
};
use slate_loaders::{FabricAdapter, NeoForgeAdapter};
use slate_minecraft::{
    Architecture, JavaRuntime, LaunchIdentity, LaunchLayout, LaunchOptions, LaunchPlanner,
    LaunchRequest, OperatingSystem, ResolvedVersion, RuleContext,
};
use slate_modpack_api_contracts::{
    Architecture as ApiArchitecture, InstallPlan, InstallPlanRequest, LoaderKind, Platform,
    Provider,
};
use slate_modpack_client::ModpackApiClient;
use slate_platform::AppPaths;
use std::collections::BTreeMap;
use std::path::PathBuf;
use url::Url;

const USAGE: &str = "usage: install_smoke <absolute-empty-root> <minecraft-version> <fabric-version|auto> <neoforge-version|auto> [<api-url> <provider> <project-id> <version-id>]";

#[derive(Serialize)]
struct AcceptanceReport<'a> {
    schema: u32,
    complete: bool,
    targets: &'a [AcceptanceResult],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AcceptanceResult {
    label: String,
    minecraft_version: String,
    loader: String,
    loader_version: Option<String>,
    downloaded_artifacts: usize,
    reused_artifacts: usize,
    installed_content_files: usize,
    java_major: u32,
    launch_artifacts: usize,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("install smoke failed: {error}");
        let mut source = error.source();
        while let Some(cause) = source {
            eprintln!("caused by: {cause}");
            source = cause.source();
        }
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let root = PathBuf::from(arguments.next().ok_or(USAGE)?);
    let minecraft_version = string_argument(arguments.next())?;
    let requested_fabric_version = string_argument(arguments.next())?;
    let requested_neoforge_version = string_argument(arguments.next())?;
    let remaining = arguments
        .map(|value| value.into_string().map_err(|_| USAGE))
        .collect::<Result<Vec<_>, _>>()?;
    if !matches!(remaining.len(), 0 | 4) || !root.is_absolute() {
        return Err(USAGE.into());
    }
    ensure_clean_root(&root)?;

    let fabric_version =
        resolve_fabric_version(&minecraft_version, &requested_fabric_version).await?;
    let neoforge_version =
        resolve_neoforge_version(&minecraft_version, &requested_neoforge_version).await?;

    let paths = AppPaths::from_roots(root.join("app-data"), root.join("storage"));
    paths.ensure_base_directories()?;
    let matrix = [
        (LoaderFamily::Vanilla, None),
        (LoaderFamily::Fabric, Some(fabric_version)),
        (LoaderFamily::NeoForge, Some(neoforge_version)),
    ];
    let mut results = Vec::new();

    for (loader_kind, loader_version) in matrix {
        results.push(
            verify_install(
                paths.clone(),
                &minecraft_version,
                loader_kind,
                loader_version,
                None,
                format!("{loader_kind:?}"),
            )
            .await?,
        );
        write_report(&root, &results, false).await?;
    }
    if let [api_url, provider, project_id, version_id] = remaining.as_slice() {
        let provider = provider.parse::<Provider>()?;
        let client = ModpackApiClient::new(Url::parse(api_url)?)?;
        let plan = client
            .install_plan(
                provider,
                project_id,
                version_id,
                &InstallPlanRequest {
                    platform: current_api_platform(),
                    arch: current_api_architecture()?,
                    include_optional: Vec::new(),
                },
            )
            .await?;
        let loader_kind = loader_family(plan.runtime.loader.kind)?;
        let loader_version = plan.runtime.loader.version.clone();
        let pack_name = plan.instance.name.clone();
        let pack_minecraft = plan.runtime.minecraft.clone();
        results.push(
            verify_install(
                paths,
                &pack_minecraft,
                loader_kind,
                loader_version,
                Some(plan),
                format!("modpack {pack_name}"),
            )
            .await?,
        );
    }
    write_report(&root, &results, true).await?;
    println!(
        "acceptance report: {}",
        root.join("acceptance-result.json").display()
    );
    Ok(())
}

async fn verify_install(
    paths: AppPaths,
    minecraft_version: &str,
    loader_kind: LoaderFamily,
    loader_version: Option<String>,
    modpack_plan: Option<InstallPlan>,
    label: String,
) -> Result<AcceptanceResult, Box<dyn std::error::Error>> {
    let instance_id = InstanceId::new();
    let revision_id = RevisionId::new();
    println!("installing {label} for Minecraft {minecraft_version}");
    let expected_content_files = modpack_plan.as_ref().map_or(0, |plan| plan.downloads.len());
    let mut outcome = install(InstallRequest {
        instance_id,
        revision_id,
        minecraft_version: minecraft_version.to_owned(),
        loader_kind,
        loader_version: loader_version.clone(),
        modpack_plan,
        download_concurrency: 8,
        download_bandwidth_limit_mib: 0,
        paths: paths.clone(),
    })
    .await?;

    let installed = load_installed_revision(
        &outcome.manifest_path,
        instance_id,
        revision_id,
        &outcome.manifest_digest,
    )
    .await?;
    let runtime = installed.runtime;
    let resolved = ResolvedVersion::resolve(installed.metadata_layers)?;
    let architecture = current_architecture()?;
    let layout = launch_layout(&paths, instance_id, revision_id);
    let preparation = LaunchPlanner::prepare(
        &resolved,
        LaunchRequest {
            runtime: JavaRuntime::new(
                runtime.executable.clone(),
                runtime.major_version,
                architecture,
            )?,
            layout,
            identity: LaunchIdentity::new(
                "slate_smoke",
                uuid::Uuid::nil(),
                "smoke-access-token",
                "73823938-7288-4e14-9ede-33d98af655cf",
                "0",
            )?,
            rules: RuleContext::new(current_os(), architecture, std::env::consts::OS),
            options: LaunchOptions::default(),
            environment: BTreeMap::new(),
        },
    )?;
    for requirement in &preparation.required_artifacts {
        if !requirement.target_path().is_file() {
            return Err(format!(
                "launch artifact is missing after install: {}",
                requirement.target_path().display()
            )
            .into());
        }
    }
    verify_installed_launch_artifacts(
        paths.storage_root(),
        &preparation.required_artifacts,
        &installed.launch_artifacts,
    )
    .await?;
    if expected_content_files > 0 && outcome.installed_content_files == 0 {
        return Err("modpack plan contained downloads but installed no content files".into());
    }
    if let Some(transaction) = outcome.content_transaction.take() {
        transaction.commit()?;
    }
    println!(
        "verified {label}: {} downloaded, {} reused, {} content files, Java {}, {} launch artifacts",
        outcome.downloaded_artifacts,
        outcome.reused_artifacts,
        outcome.installed_content_files,
        runtime.major_version,
        preparation.required_artifacts.len()
    );
    Ok(AcceptanceResult {
        label,
        minecraft_version: minecraft_version.to_owned(),
        loader: loader_kind.as_storage_value().to_owned(),
        loader_version,
        downloaded_artifacts: outcome.downloaded_artifacts,
        reused_artifacts: outcome.reused_artifacts,
        installed_content_files: outcome.installed_content_files,
        java_major: runtime.major_version,
        launch_artifacts: preparation.required_artifacts.len(),
    })
}

async fn write_report(
    root: &std::path::Path,
    targets: &[AcceptanceResult],
    complete: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(&AcceptanceReport {
        schema: 1,
        complete,
        targets,
    })?;
    tokio::fs::write(root.join("acceptance-result.json"), bytes).await?;
    Ok(())
}

fn ensure_clean_root(root: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    if root.exists() && std::fs::read_dir(root)?.next().transpose()?.is_some() {
        return Err("acceptance root must not exist or must be empty".into());
    }
    Ok(())
}

async fn resolve_fabric_version(
    minecraft_version: &str,
    requested: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if requested != "auto" {
        return Ok(requested.to_owned());
    }
    FabricAdapter::new()?
        .fetch_loader_versions(minecraft_version)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| "Fabric has no compatible loader version".into())
}

async fn resolve_neoforge_version(
    minecraft_version: &str,
    requested: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if requested != "auto" {
        return Ok(requested.to_owned());
    }
    NeoForgeAdapter::new()?
        .fetch_loader_versions(minecraft_version)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| "NeoForge has no compatible loader version".into())
}

fn loader_family(kind: LoaderKind) -> Result<LoaderFamily, Box<dyn std::error::Error>> {
    match kind {
        LoaderKind::Vanilla => Ok(LoaderFamily::Vanilla),
        LoaderKind::Fabric => Ok(LoaderFamily::Fabric),
        LoaderKind::NeoForge => Ok(LoaderFamily::NeoForge),
        LoaderKind::Forge | LoaderKind::Quilt => {
            Err("the desktop launcher does not support this modpack loader yet".into())
        }
    }
}

const fn current_api_platform() -> Platform {
    if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::Macos
    } else {
        Platform::Linux
    }
}

fn current_api_architecture() -> Result<ApiArchitecture, Box<dyn std::error::Error>> {
    if cfg!(target_arch = "x86_64") {
        Ok(ApiArchitecture::X86_64)
    } else if cfg!(target_arch = "aarch64") {
        Ok(ApiArchitecture::Aarch64)
    } else {
        Err("the modpack API does not support this acceptance-test architecture".into())
    }
}

fn string_argument(value: Option<std::ffi::OsString>) -> Result<String, &'static str> {
    value
        .and_then(|value| value.into_string().ok())
        .ok_or(USAGE)
}

fn launch_layout(
    paths: &AppPaths,
    instance_id: InstanceId,
    revision_id: RevisionId,
) -> LaunchLayout {
    let minecraft = paths.artifacts().join("minecraft");
    LaunchLayout {
        game_directory: paths.instance(instance_id).join("game"),
        libraries_directory: minecraft.join("libraries"),
        versions_directory: minecraft.join("versions"),
        assets_directory: minecraft.join("assets"),
        natives_directory: paths
            .instance(instance_id)
            .join("revisions")
            .join(revision_id.to_string())
            .join("natives"),
    }
}

const fn current_os() -> OperatingSystem {
    if cfg!(target_os = "windows") {
        OperatingSystem::Windows
    } else if cfg!(target_os = "macos") {
        OperatingSystem::MacOs
    } else {
        OperatingSystem::Linux
    }
}

fn current_architecture() -> Result<Architecture, Box<dyn std::error::Error>> {
    if cfg!(target_arch = "x86") {
        Ok(Architecture::X86)
    } else if cfg!(target_arch = "x86_64") {
        Ok(Architecture::X86_64)
    } else if cfg!(target_arch = "aarch64") {
        Ok(Architecture::Arm64)
    } else {
        Err("unsupported smoke-test architecture".into())
    }
}
