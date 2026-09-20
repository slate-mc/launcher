use slate_domain::{InstanceId, LoaderFamily, RevisionId};
use slate_installer::{
    InstallRequest, install, load_installed_revision, verify_installed_launch_artifacts,
};
use slate_minecraft::{
    Architecture, JavaRuntime, LaunchIdentity, LaunchLayout, LaunchOptions, LaunchPlanner,
    LaunchRequest, OperatingSystem, ResolvedVersion, RuleContext,
};
use slate_platform::AppPaths;
use std::collections::BTreeMap;
use std::path::PathBuf;

const USAGE: &str =
    "usage: install_smoke <absolute-root> <minecraft-version> <fabric-version> <neoforge-version>";

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
    let fabric_version = string_argument(arguments.next())?;
    let neoforge_version = string_argument(arguments.next())?;
    if arguments.next().is_some() || !root.is_absolute() {
        return Err(USAGE.into());
    }

    let paths = AppPaths::from_roots(root.join("app-data"), root.join("storage"));
    paths.ensure_base_directories()?;
    let matrix = [
        (LoaderFamily::Vanilla, None),
        (LoaderFamily::Fabric, Some(fabric_version)),
        (LoaderFamily::NeoForge, Some(neoforge_version)),
    ];

    for (loader_kind, loader_version) in matrix {
        verify_install(
            paths.clone(),
            &minecraft_version,
            loader_kind,
            loader_version,
        )
        .await?;
    }
    Ok(())
}

async fn verify_install(
    paths: AppPaths,
    minecraft_version: &str,
    loader_kind: LoaderFamily,
    loader_version: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let instance_id = InstanceId::new();
    let revision_id = RevisionId::new();
    println!("installing {loader_kind:?} for Minecraft {minecraft_version}");
    let outcome = install(InstallRequest {
        instance_id,
        revision_id,
        minecraft_version: minecraft_version.to_owned(),
        loader_kind,
        loader_version,
        modpack_plan: None,
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
    println!(
        "verified {loader_kind:?}: {} downloaded, {} reused, Java {}, {} launch artifacts",
        outcome.downloaded_artifacts,
        outcome.reused_artifacts,
        runtime.major_version,
        preparation.required_artifacts.len()
    );
    Ok(())
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
