use super::*;

pub(super) const fn install_failure_message(error: &slate_installer::InstallError) -> &'static str {
    use slate_installer::InstallError;

    match error {
        InstallError::Download(_) => {
            "A required file could not be downloaded or verified. Check your connection and try again."
        }
        InstallError::Runtime(_) => {
            "The required Java version could not be prepared. Try again or choose another Java installation."
        }
        InstallError::Content(_) => {
            "Some pack content could not be installed or verified. Try repairing the instance."
        }
        InstallError::Fabric(_)
        | InstallError::NeoForge(_)
        | InstallError::NeoForgeInstallerFailed { .. }
        | InstallError::NeoForgeInstalledMetadataMismatch => {
            "The selected mod loader could not be installed. Check the version and try again."
        }
        InstallError::Mojang(_) => {
            "Minecraft version information could not be loaded. Check your connection and try again."
        }
        InstallError::UnsupportedArchitecture => {
            "This Minecraft version is not available for your computer's architecture."
        }
        _ => "Installation could not be completed. Try again or repair the instance.",
    }
}

pub(super) fn installed_manifest_path(
    paths: &AppPaths,
    instance_id: InstanceId,
    revision_id: RevisionId,
) -> std::path::PathBuf {
    paths
        .instance(instance_id)
        .join("revisions")
        .join(revision_id.to_string())
        .join("metadata")
        .join("installed-revision.json")
}

pub(super) fn launch_layout(
    paths: &AppPaths,
    instance_id: InstanceId,
    revision_id: RevisionId,
) -> LaunchLayout {
    let minecraft_root = paths.artifacts().join("minecraft");
    LaunchLayout {
        game_directory: paths.instance(instance_id).join("game"),
        libraries_directory: minecraft_root.join("libraries"),
        versions_directory: minecraft_root.join("versions"),
        assets_directory: minecraft_root.join("assets"),
        natives_directory: paths
            .instance(instance_id)
            .join("revisions")
            .join(revision_id.to_string())
            .join("natives"),
    }
}

pub(super) fn minecraft_architecture(value: &str) -> Result<Architecture, AppError> {
    match value {
        "x64" | "amd64" | "x86_64" => Ok(Architecture::X86_64),
        "x86" | "x32" => Ok(Architecture::X86),
        "aarch64" | "arm64" => Ok(Architecture::Arm64),
        _ => Err(AppError::new(
            "local.runtime_architecture_unknown",
            "The installed Java runtime reported an unsupported architecture.",
        )),
    }
}

pub(super) const fn platform_architecture(value: JavaArchitecture) -> Option<Architecture> {
    match value {
        JavaArchitecture::X86 => Some(Architecture::X86),
        JavaArchitecture::X86_64 => Some(Architecture::X86_64),
        JavaArchitecture::Arm64 => Some(Architecture::Arm64),
    }
}

pub(super) const fn child_process_priority(value: ProcessPriority) -> ChildProcessPriority {
    match value {
        ProcessPriority::Low => ChildProcessPriority::Low,
        ProcessPriority::BelowNormal => ChildProcessPriority::BelowNormal,
        ProcessPriority::Normal => ChildProcessPriority::Normal,
        ProcessPriority::AboveNormal => ChildProcessPriority::AboveNormal,
        ProcessPriority::High => ChildProcessPriority::High,
    }
}

pub(super) fn launch_jvm_arguments(
    settings: &slate_storage::InstanceSettingsRecord,
) -> Vec<String> {
    let mut arguments = match settings.performance_preset {
        PerformancePreset::Balanced | PerformancePreset::Custom => Vec::new(),
        PerformancePreset::Throughput => vec![
            "-XX:+UseG1GC".to_owned(),
            "-XX:MaxGCPauseMillis=100".to_owned(),
            "-XX:+ParallelRefProcEnabled".to_owned(),
        ],
        PerformancePreset::LowLatency => vec![
            "-XX:+UseG1GC".to_owned(),
            "-XX:MaxGCPauseMillis=50".to_owned(),
            "-XX:+ParallelRefProcEnabled".to_owned(),
        ],
    };
    for argument in &settings.jvm_arguments {
        if !arguments.contains(argument) {
            arguments.push(argument.clone());
        }
    }
    arguments
}

pub(super) async fn apply_managed_game_options(
    game_directory: &std::path::Path,
    language: &str,
    fullscreen: bool,
) -> Result<(), AppError> {
    let updates = BTreeMap::from([
        ("lang".to_owned(), language.to_owned()),
        ("fullscreen".to_owned(), fullscreen.to_string()),
    ]);
    let pending = prepare_options_update(game_directory.join("options.txt"), &updates)
        .await
        .map_err(|_| game_options_error())?;
    pending.commit().await;
    Ok(())
}

pub(super) fn game_options_error() -> AppError {
    AppError::new(
        "local.game_options_unavailable",
        "slate could not apply this instance's game language and window mode.",
    )
}
