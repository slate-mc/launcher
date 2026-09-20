use super::*;

pub(super) struct InstalledModIndex {
    pub(super) identities: HashSet<String>,
    pub(super) artifacts: HashMap<String, InstalledModArtifact>,
}

pub(super) struct InstalledModArtifact {
    pub(super) size: u64,
    pub(super) known_hashes: Vec<Hashes>,
}

pub(super) struct PlannedModDestination {
    pub(super) size: u64,
    pub(super) hashes: Hashes,
    pub(super) requested_name: String,
}

pub(super) fn same_mod_artifact(
    left_size: u64,
    left: &Hashes,
    right_size: u64,
    right: &Hashes,
) -> bool {
    if left_size != right_size {
        return false;
    }
    let comparisons = [
        (left.sha512.as_deref(), right.sha512.as_deref()),
        (left.sha256.as_deref(), right.sha256.as_deref()),
        (left.sha1.as_deref(), right.sha1.as_deref()),
    ];
    let mut matched = false;
    for (left, right) in comparisons {
        if let (Some(left), Some(right)) = (left, right) {
            if !left.eq_ignore_ascii_case(right) {
                return false;
            }
            matched = true;
        }
    }
    matched
}

pub(super) fn installed_artifact_matches(
    installed: &InstalledModArtifact,
    planned_size: u64,
    planned_hashes: &Hashes,
) -> bool {
    installed.known_hashes.iter().any(|known_hashes| {
        same_mod_artifact(installed.size, known_hashes, planned_size, planned_hashes)
    })
}

pub(super) async fn installed_mod_index(
    state: &DesktopState,
    instance: &InstanceRecord,
) -> Result<InstalledModIndex, AppError> {
    let paths = paths_for_instance(state, instance);
    let instance_id = instance.id;
    let files = tokio::task::spawn_blocking(move || scan_instance_mods(&paths, instance_id))
        .await
        .map_err(|_| content_file_error())?
        .map_err(|_| content_file_error())?;
    let mut artifacts = files
        .into_iter()
        .map(|file| {
            (
                normalized_content_path(&file.file_path),
                InstalledModArtifact {
                    size: file.size,
                    known_hashes: Vec::new(),
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let mut identities = HashSet::new();
    for installed in state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect installed mods."))?
    {
        if let Some(artifact) = artifacts.get_mut(&normalized_content_path(&installed.file_path)) {
            identities.insert(format!("{}:{}", installed.provider, installed.project_id));
            artifact.known_hashes.push(installed.hashes);
        }
    }
    if let Some(source) = &instance.modpack_source {
        let version = state
            .modpacks
            .version(source.provider, &source.project_id, &source.version_id)
            .await
            .map_err(modpack_api_error)?;
        for file in version.files {
            if file.kind == PackFileType::Mod
                && let Some(artifact) = artifacts.get_mut(&normalized_content_path(&file.path))
            {
                artifact.known_hashes.push(file.hashes);
                if let Some(reference) = file.source {
                    identities.insert(format!("{}:{}", reference.provider, reference.project_id));
                }
            }
        }
    }
    Ok(InstalledModIndex {
        identities,
        artifacts,
    })
}

pub(super) fn parse_mod_download_id(
    value: &str,
) -> Option<(slate_modpack_api_contracts::Provider, String, String)> {
    let mut parts = value.splitn(3, ':');
    let provider = slate_modpack_api_contracts::Provider::from_str(parts.next()?).ok()?;
    let project_id = parts.next()?.trim();
    let version_id = parts.next()?.trim();
    if provider == slate_modpack_api_contracts::Provider::Ftb
        || project_id.is_empty()
        || version_id.is_empty()
    {
        return None;
    }
    Some((provider, project_id.to_owned(), version_id.to_owned()))
}

pub(super) fn instance_mod_target(
    instance: &InstanceRecord,
) -> Result<(LoaderKind, String), AppError> {
    let loader = match instance.loader_kind {
        LoaderFamily::Vanilla => {
            return Err(AppError::new(
                "mod.vanilla_instance",
                "Vanilla instances cannot install loader mods.",
            ));
        }
        LoaderFamily::Fabric => LoaderKind::Fabric,
        LoaderFamily::NeoForge => LoaderKind::NeoForge,
    };
    let version = instance
        .loader_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "mod.loader_version_missing",
                "This instance does not have an exact loader version.",
            )
        })?;
    Ok((loader, version.to_owned()))
}

pub(super) fn validate_instance_mod_plan(
    instance: &InstanceRecord,
    provider: slate_modpack_api_contracts::Provider,
    project_id: &str,
    plan: &InstallPlan,
) -> Result<(), AppError> {
    let (loader, loader_version) = instance_mod_target(instance)?;
    let valid_downloads = !plan.downloads.is_empty()
        && plan.downloads.len() <= 257
        && plan.extract.is_empty()
        && plan.delete.is_empty()
        && plan.downloads.iter().all(|download| {
            download.destination.starts_with("mods/")
                && download.hashes.has_cryptographic_hash()
                && parse_mod_download_id(&download.id).is_some()
        });
    if plan.schema != 1
        || plan.instance.provider != provider
        || plan.instance.project_id != project_id
        || plan.runtime.minecraft != instance.minecraft_version
        || plan.runtime.loader.kind != loader
        || plan.runtime.loader.version.as_deref() != Some(loader_version.as_str())
        || !valid_downloads
    {
        return Err(AppError::new(
            "mod.resolution_mismatch",
            "A compatible version of this mod could not be confirmed. Nothing was installed.",
        ));
    }
    Ok(())
}

pub(super) fn validate_resolved_modpack(
    request: &InstallModpackRequest,
    version: &ModpackVersion,
    plan: &InstallPlan,
) -> Result<(), AppError> {
    if version.provider != request.provider
        || version.project_id != request.project_id
        || version.id != request.version_id
        || plan.instance.provider != request.provider
        || plan.instance.project_id != request.project_id
        || plan.instance.version_id != request.version_id
        || version.minecraft.version != plan.runtime.minecraft
        || version.loader != plan.runtime.loader
    {
        return Err(AppError::new(
            "modpack.resolution_mismatch",
            "This modpack release has conflicting version details. Nothing was installed.",
        ));
    }
    Ok(())
}

pub(super) fn launcher_loader_kind(loader: LoaderKind) -> Result<LoaderKindDto, AppError> {
    match loader {
        LoaderKind::Vanilla => Ok(LoaderKindDto::Vanilla),
        LoaderKind::Fabric => Ok(LoaderKindDto::Fabric),
        LoaderKind::NeoForge => Ok(LoaderKindDto::NeoForge),
        LoaderKind::Forge | LoaderKind::Quilt => Err(AppError::new(
            "modpack.loader_not_supported",
            "This pack uses a loader that slate cannot install yet.",
        )),
    }
}

pub(super) const fn current_modpack_platform() -> ModpackPlatform {
    if cfg!(target_os = "windows") {
        ModpackPlatform::Windows
    } else if cfg!(target_os = "macos") {
        ModpackPlatform::Macos
    } else {
        ModpackPlatform::Linux
    }
}

pub(super) fn current_modpack_architecture() -> Result<ModpackArchitecture, AppError> {
    if cfg!(target_arch = "x86_64") {
        Ok(ModpackArchitecture::X86_64)
    } else if cfg!(target_arch = "aarch64") {
        Ok(ModpackArchitecture::Aarch64)
    } else {
        Err(AppError::new(
            "modpack.unsupported_architecture",
            "Modpack installation currently supports x86-64 and ARM64 systems.",
        ))
    }
}

#[tauri::command]
pub(super) async fn install_jobs_list(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<InstallJobSummary>, AppError> {
    refresh_exited_sessions(state.inner()).await;
    state
        .database
        .list_install_jobs(100)
        .await
        .map(|jobs| jobs.into_iter().map(install_job_summary).collect())
        .map_err(|error| map_storage_error(error, "slate could not load installation activity."))
}
