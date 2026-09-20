use super::*;

#[tauri::command]
pub(super) async fn install_job_retry(
    state: tauri::State<'_, DesktopState>,
    request: RetryInstallJobRequest,
) -> Result<InstallJobSummary, AppError> {
    let job = state
        .database
        .get_install_job(JobId::from_uuid(request.job_id))
        .await
        .map_err(|error| map_storage_error(error, "That installation is no longer available."))?;
    if !matches!(job.state, JobState::Failed | JobState::Cancelled) {
        return Err(AppError::new(
            "local.install_not_retryable",
            "Only failed or cancelled installations can be retried.",
        ));
    }
    let instance = state
        .database
        .get_instance(job.instance_id)
        .await
        .map_err(|error| map_storage_error(error, "That instance is no longer available."))?;
    let instance_id = instance.id.as_uuid();
    let expected_revision = instance.revision;

    match job.operation.as_deref() {
        Some("instance_install" | "external_instance_import") => {
            instance_install_inner(
                state.inner(),
                InstallInstanceRequest {
                    id: instance_id,
                    expected_revision,
                },
            )
            .await
        }
        Some("mod_install") => {
            let mods = job
                .retry_payload
                .and_then(|payload| payload.get("mods").cloned())
                .and_then(|mods| serde_json::from_value::<Vec<InstallModSelection>>(mods).ok())
                .filter(|mods| !mods.is_empty())
                .ok_or_else(retry_context_missing)?;
            instance_mod_install_inner(
                state.inner(),
                InstallModRequest {
                    instance_id,
                    expected_revision,
                    mods,
                },
            )
            .await
        }
        Some("content_install") => {
            let payload = job
                .retry_payload
                .as_ref()
                .ok_or_else(retry_context_missing)?;
            let kind = payload
                .get("kind")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok())
                .ok_or_else(retry_context_missing)?;
            if let Some(target_version_id) = payload
                .get("targetVersionId")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
            {
                instance_content_update_inner(
                    state.inner(),
                    UpdateInstanceContentRequest {
                        instance_id,
                        expected_revision,
                        kind,
                        provider: payload
                            .get("provider")
                            .cloned()
                            .and_then(|value| serde_json::from_value(value).ok())
                            .ok_or_else(retry_context_missing)?,
                        project_id: retry_string(payload, "projectId")?,
                        file_path: retry_string(payload, "filePath")?,
                        display_name: retry_string(payload, "displayName")?,
                        target_version_id: target_version_id.to_owned(),
                    },
                )
                .await
            } else {
                let world_name = payload
                    .get("worldName")
                    .cloned()
                    .and_then(|value| serde_json::from_value(value).ok());
                let content = payload
                    .get("content")
                    .cloned()
                    .and_then(|value| {
                        serde_json::from_value::<Vec<InstallContentSelection>>(value).ok()
                    })
                    .filter(|items| !items.is_empty())
                    .ok_or_else(retry_context_missing)?;
                instance_content_install_inner(
                    state.inner(),
                    InstallContentRequest {
                        instance_id,
                        expected_revision,
                        kind,
                        world_name,
                        content,
                    },
                )
                .await
            }
        }
        Some("mod_update") => {
            let payload = job
                .retry_payload
                .as_ref()
                .ok_or_else(retry_context_missing)?;
            let provider = payload
                .get("provider")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok())
                .ok_or_else(retry_context_missing)?;
            let project_id = payload
                .get("projectId")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(retry_context_missing)?;
            let target_version_id = payload
                .get("targetVersionId")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(retry_context_missing)?;
            let file_path = payload
                .get("filePath")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(retry_context_missing)?;
            let display_name = payload
                .get("displayName")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(retry_context_missing)?;
            instance_mod_update_inner(
                state.inner(),
                UpdateInstanceModRequest {
                    instance_id,
                    expected_revision,
                    provider,
                    project_id,
                    file_path,
                    display_name,
                    target_version_id,
                },
            )
            .await
        }
        Some("modpack_update") => {
            let target_version_id = job
                .retry_payload
                .as_ref()
                .and_then(|payload| payload.get("targetVersionId"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(retry_context_missing)?;
            modpack_update_apply_inner(
                state.inner(),
                ApplyModpackUpdateRequest {
                    instance_id,
                    expected_revision,
                    target_version_id,
                },
            )
            .await
        }
        Some("imported_pack_install") => {
            let payload = job
                .retry_payload
                .as_ref()
                .ok_or_else(retry_context_missing)?;
            let plan = payload
                .get("plan")
                .cloned()
                .and_then(|value| serde_json::from_value::<InstallPlan>(value).ok())
                .ok_or_else(retry_context_missing)?;
            let override_prefixes = payload
                .get("overridePrefixes")
                .cloned()
                .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
                .ok_or_else(retry_context_missing)?;
            let archive_path = instance.storage_path.join("metadata/import-source.zip");
            queue_instance_install(
                state.inner(),
                instance,
                expected_revision,
                Some(plan.clone()),
                PendingModChanges {
                    imported_overrides: Some(PendingImportedOverrides {
                        archive_path,
                        prefixes: override_prefixes.clone(),
                    }),
                    ..PendingModChanges::default()
                },
                None,
                RetryableInstallOperation::ImportedPackInstall {
                    plan: Box::new(plan),
                    override_prefixes,
                },
            )
            .await
        }
        _ => Err(retry_context_missing()),
    }
}

fn retry_context_missing() -> AppError {
    AppError::new(
        "local.install_retry_unavailable",
        "This older installation cannot be retried here. Open the instance and try again.",
    )
}

fn retry_string(payload: &serde_json::Value, field: &str) -> Result<String, AppError> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(retry_context_missing)
}

pub(super) async fn fetch_instance_modpack_plan(
    state: &DesktopState,
    source: &slate_storage::ModpackSourceRecord,
) -> Result<InstallPlan, AppError> {
    state
        .modpacks
        .install_plan(
            source.provider,
            &source.project_id,
            &source.version_id,
            &InstallPlanRequest {
                platform: current_modpack_platform(),
                arch: current_modpack_architecture()?,
                include_optional: source.selected_optional.clone(),
            },
        )
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
pub(super) async fn modpack_update_check(
    state: tauri::State<'_, DesktopState>,
    request: CheckModpackUpdateRequest,
) -> Result<ModpackUpdateSummary, AppError> {
    let instance = state
        .database
        .get_instance(InstanceId::from_uuid(request.instance_id))
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let source = instance.modpack_source.as_ref().ok_or_else(|| {
        AppError::new(
            "modpack.not_managed",
            "This instance is not linked to an installed modpack.",
        )
    })?;
    let response = state
        .modpacks
        .update(
            source.provider,
            &source.project_id,
            &source.version_id,
            &instance.minecraft_version,
            modpack_loader_kind(instance.loader_kind)?,
        )
        .await
        .map_err(modpack_api_error)?;
    Ok(ModpackUpdateSummary {
        update_available: response.update_available,
        current_version_id: source.version_id.clone(),
        current_version_name: response.current.map(|version| version.name),
        latest_version_id: response.latest.as_ref().map(|version| version.id.clone()),
        latest_version_name: response.latest.map(|version| version.name),
    })
}

#[tauri::command]
pub(super) async fn modpack_update_apply(
    state: tauri::State<'_, DesktopState>,
    request: ApplyModpackUpdateRequest,
) -> Result<InstallJobSummary, AppError> {
    modpack_update_apply_inner(state.inner(), request).await
}

pub(super) async fn modpack_update_apply_inner(
    state: &DesktopState,
    request: ApplyModpackUpdateRequest,
) -> Result<InstallJobSummary, AppError> {
    refresh_exited_sessions(state).await;
    if request.target_version_id.trim().is_empty() || request.target_version_id.len() > 128 {
        return Err(AppError::new(
            "modpack.invalid_version",
            "The selected pack version is invalid.",
        ));
    }
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state, instance_id, request.expected_revision).await?;
    let source = instance.modpack_source.clone().ok_or_else(|| {
        AppError::new(
            "modpack.not_managed",
            "This instance is not linked to an installed modpack.",
        )
    })?;
    let available = state
        .modpacks
        .update(
            source.provider,
            &source.project_id,
            &source.version_id,
            &instance.minecraft_version,
            modpack_loader_kind(instance.loader_kind)?,
        )
        .await
        .map_err(modpack_api_error)?;
    let latest = available
        .latest
        .ok_or_else(|| AppError::new("modpack.no_update", "This modpack is already up to date."))?;
    if !available.update_available || latest.id != request.target_version_id {
        return Err(AppError::new(
            "modpack.update_changed",
            "A newer pack version became available. Check for updates again.",
        ));
    }

    let current_version = state
        .modpacks
        .version(source.provider, &source.project_id, &source.version_id)
        .await
        .map_err(modpack_api_error)?;
    let target_version = state
        .modpacks
        .version(source.provider, &source.project_id, &latest.id)
        .await
        .map_err(modpack_api_error)?;
    let mut plan = state
        .modpacks
        .install_plan(
            source.provider,
            &source.project_id,
            &latest.id,
            &InstallPlanRequest {
                platform: current_modpack_platform(),
                arch: current_modpack_architecture()?,
                include_optional: source.selected_optional.clone(),
            },
        )
        .await
        .map_err(modpack_api_error)?;
    if target_version.provider != source.provider
        || target_version.project_id != source.project_id
        || target_version.id != latest.id
        || target_version.minecraft.version != instance.minecraft_version
        || target_version.loader != plan.runtime.loader
        || plan.instance.provider != source.provider
        || plan.instance.project_id != source.project_id
        || plan.instance.version_id != latest.id
        || plan.runtime.minecraft != instance.minecraft_version
    {
        return Err(AppError::new(
            "modpack.resolution_mismatch",
            "The updated pack does not match this instance. Nothing was changed.",
        ));
    }
    let target_loader = launcher_loader_kind(target_version.loader.kind)?;
    let configured_loader = launcher_loader_kind(modpack_loader_kind(instance.loader_kind)?)?;
    if target_loader != configured_loader {
        return Err(AppError::new(
            "modpack.incompatible_update",
            "The updated pack requires a different mod loader.",
        ));
    }
    let target_loader_version = validate_selected_loader_version(
        &instance.minecraft_version,
        target_loader,
        target_version.loader.version.as_deref(),
    )
    .await?;

    let user_mod_paths = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not check installed mods."))?
        .into_iter()
        .map(|item| normalized_content_path(&item.file_path))
        .collect::<HashSet<_>>();
    let target_paths = target_version
        .files
        .iter()
        .filter(|file| file.side != Side::Server)
        .map(|file| normalized_content_path(&file.path))
        .collect::<HashSet<_>>();
    for file in current_version.files {
        let normalized = normalized_content_path(&file.path);
        if file.side != Side::Server
            && !target_paths.contains(&normalized)
            && !user_mod_paths.contains(&normalized)
        {
            plan.delete.push(file.path);
        }
    }
    plan.delete.sort();
    plan.delete.dedup();

    queue_instance_install(
        state,
        instance,
        request.expected_revision,
        Some(plan),
        PendingModChanges::default(),
        Some(PendingModpackUpdate {
            version_id: latest.id.clone(),
            loader_version: target_loader_version,
        }),
        RetryableInstallOperation::ModpackUpdate {
            target_version_id: latest.id,
        },
    )
    .await
}

fn modpack_loader_kind(loader: LoaderFamily) -> Result<LoaderKind, AppError> {
    match loader {
        LoaderFamily::Vanilla => Ok(LoaderKind::Vanilla),
        LoaderFamily::Fabric => Ok(LoaderKind::Fabric),
        LoaderFamily::NeoForge => Ok(LoaderKind::NeoForge),
    }
}

#[tauri::command]
pub(super) async fn modpack_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallModpackRequest,
) -> Result<ModpackInstallStarted, AppError> {
    refresh_exited_sessions(state.inner()).await;
    if request.include_optional.len() > 1_000 {
        return Err(AppError::new(
            "modpack.too_many_options",
            "Too many optional files were selected.",
        ));
    }
    let name = parse_instance_name(&request.instance_name).map_err(instance_name_app_error)?;
    let project = state
        .modpacks
        .project(request.provider, &request.project_id)
        .await
        .map_err(modpack_api_error)?;
    let version = state
        .modpacks
        .version(request.provider, &request.project_id, &request.version_id)
        .await
        .map_err(modpack_api_error)?;
    let plan = state
        .modpacks
        .install_plan(
            request.provider,
            &request.project_id,
            &request.version_id,
            &InstallPlanRequest {
                platform: current_modpack_platform(),
                arch: current_modpack_architecture()?,
                include_optional: request.include_optional.clone(),
            },
        )
        .await
        .map_err(modpack_api_error)?;
    validate_resolved_modpack(&request, &version, &plan)?;

    let loader_kind = launcher_loader_kind(plan.runtime.loader.kind)?;
    let loader_version = validate_selected_loader_version(
        &plan.runtime.minecraft,
        loader_kind,
        plan.runtime.loader.version.as_deref(),
    )
    .await?;
    let memory_mb = plan.runtime.memory.recommended_mb.clamp(1_024, 32_768);
    validate_instance_configuration(
        InstanceModeDto::Modded,
        &plan.runtime.minecraft,
        loader_kind,
        loader_version.as_deref(),
        memory_mb,
    )
    .map_err(configuration_app_error)?;

    let root_id = preferred_storage_root_id(state.inner()).await?;
    let record = state
        .database
        .create_instance(NewInstance {
            name,
            mode: slate_domain::InstanceMode::Modded,
            management_mode: ManagementMode::Local,
            root_id,
            minecraft_version: plan.runtime.minecraft.clone(),
            loader_kind: loader_kind.into(),
            loader_version,
            memory_mb,
            modpack_source: Some(NewModpackSource {
                provider: request.provider,
                project_id: request.project_id,
                version_id: request.version_id,
                selected_optional: request.include_optional,
                display_name: project.name,
                icon_url: project.icon_url,
                banner_url: project.banner_url,
            }),
        })
        .await
        .map_err(|error| {
            map_storage_error(error, "slate could not create the modpack instance.")
        })?;
    if std::fs::create_dir_all(&record.storage_path).is_err() {
        let _ = state
            .database
            .trash_instance(record.id, record.revision)
            .await;
        return Err(AppError::new(
            "local.instance_directory_unavailable",
            "slate could not finish creating the instance. The incomplete instance was moved to trash.",
        ));
    }
    let job = match queue_instance_install(
        state.inner(),
        record.clone(),
        record.revision,
        Some(plan),
        PendingModChanges::default(),
        None,
        RetryableInstallOperation::InstanceInstall,
    )
    .await
    {
        Ok(job) => job,
        Err(error) => {
            let _ = state
                .database
                .trash_instance(record.id, record.revision)
                .await;
            return Err(error);
        }
    };
    let installed_record = state
        .database
        .get_instance(record.id)
        .await
        .map_err(|error| {
            map_storage_error(error, "slate could not reload the modpack instance.")
        })?;
    Ok(ModpackInstallStarted {
        instance: instance_summary(installed_record),
        job,
    })
}
