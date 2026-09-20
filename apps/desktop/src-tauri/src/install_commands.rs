use super::*;

#[derive(Clone, Debug)]
pub(super) struct PendingModpackUpdate {
    version_id: String,
    loader_version: Option<String>,
}

#[derive(Clone, Debug)]
enum RetryableInstallOperation {
    InstanceInstall,
    ModInstall { mods: Vec<InstallModSelection> },
    ModpackUpdate { target_version_id: String },
}

impl RetryableInstallOperation {
    const fn storage_name(&self) -> &'static str {
        match self {
            Self::InstanceInstall => "instance_install",
            Self::ModInstall { .. } => "mod_install",
            Self::ModpackUpdate { .. } => "modpack_update",
        }
    }

    fn payload(&self) -> Option<serde_json::Value> {
        match self {
            Self::InstanceInstall => None,
            Self::ModInstall { mods } => Some(serde_json::json!({ "mods": mods })),
            Self::ModpackUpdate { target_version_id } => {
                Some(serde_json::json!({ "targetVersionId": target_version_id }))
            }
        }
    }
}

#[tauri::command]
pub(super) async fn instance_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallInstanceRequest,
) -> Result<InstallJobSummary, AppError> {
    instance_install_inner(state.inner(), request).await
}

async fn instance_install_inner(
    state: &DesktopState,
    request: InstallInstanceRequest,
) -> Result<InstallJobSummary, AppError> {
    refresh_exited_sessions(state).await;
    let instance_id = InstanceId::from_uuid(request.id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let plan = if let Some(source) = &instance.modpack_source {
        Some(fetch_instance_modpack_plan(state, source).await?)
    } else {
        None
    };
    queue_instance_install(
        state,
        instance,
        request.expected_revision,
        plan,
        Vec::new(),
        None,
        RetryableInstallOperation::InstanceInstall,
    )
    .await
}

async fn queue_instance_install(
    state: &DesktopState,
    instance: InstanceRecord,
    expected_revision: u64,
    mut modpack_plan: Option<InstallPlan>,
    pending_mods: Vec<NewInstanceMod>,
    modpack_update: Option<PendingModpackUpdate>,
    operation: RetryableInstallOperation,
) -> Result<InstallJobSummary, AppError> {
    let instance_id = instance.id;
    if state
        .processes
        .active_for_instance(instance_id)
        .map_err(process_state_error)?
        .is_some()
    {
        return Err(AppError::new(
            "local.instance_running",
            "Minecraft is already running for this instance.",
        ));
    }
    if instance.setup_state == slate_domain::InstanceSetupState::Preparing {
        return Err(AppError::new(
            "local.install_already_running",
            "That instance already has an installation in progress.",
        ));
    }
    if instance.revision != expected_revision {
        return Err(AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ));
    }
    create_automatic_snapshot_if_enabled(state, &instance).await?;
    let content_update = if pending_mods.is_empty() {
        None
    } else {
        let plan = modpack_plan.take().ok_or_else(|| {
            AppError::new(
                "mod.invalid_plan",
                "The mod service returned an empty content plan.",
            )
        })?;
        let parent = state
            .database
            .get_installed_revision(instance_id)
            .await
            .map_err(|error| {
                map_storage_error(
                    error,
                    "Install the base instance before adding individual mods.",
                )
            })?;
        Some((parent, plan))
    };
    let preferences = state
        .database
        .get_app_preferences()
        .await
        .map_err(|error| map_storage_error(error, "slate could not load download settings."))?;
    let pending = state
        .database
        .begin_instance_install_with_context(
            instance_id,
            expected_revision,
            RequestId::new(),
            operation.storage_name(),
            operation.payload(),
        )
        .await
        .map_err(|error| map_storage_error(error, "slate could not queue the installation."))?;
    let cancellation = state.installs.register(pending.job.id);
    tracing::info!(
        instance_id = %instance_id,
        job_id = %pending.job.id,
        revision_id = %pending.revision_id,
        operation = operation.storage_name(),
        "installation queued"
    );
    let response = install_job_summary(pending.job.clone());
    let install_paths = paths_for_instance(state, &instance);
    let target_loader_version = modpack_update.as_ref().map_or_else(
        || instance.loader_version.clone(),
        |update| update.loader_version.clone(),
    );
    let task_state = state.clone();
    tauri::async_runtime::spawn(async move {
        let (progress_tx, mut progress_rx) =
            tokio::sync::mpsc::unbounded_channel::<InstallProgress>();
        let progress_gate = Arc::new(Mutex::new((None, Instant::now() - Duration::from_secs(1))));
        let progress_database = task_state.database.clone();
        let progress_job_id = pending.job.id;
        let progress_task = tauri::async_runtime::spawn(async move {
            while let Some(progress) = progress_rx.recv().await {
                let _ = progress_database
                    .update_install_progress(
                        progress_job_id,
                        progress.phase.as_str(),
                        &progress.message,
                        progress.completed_items,
                        progress.total_items,
                    )
                    .await;
            }
        });
        let progress_callback = move |progress: InstallProgress| {
            let now = Instant::now();
            let finished_stage =
                progress.completed_items == progress.total_items && progress.total_items.is_some();
            let should_send = progress_gate.lock().is_ok_and(|mut gate| {
                let phase_changed = gate.0 != Some(progress.phase);
                if phase_changed
                    || finished_stage
                    || now.duration_since(gate.1) >= Duration::from_millis(125)
                {
                    *gate = (Some(progress.phase), now);
                    true
                } else {
                    false
                }
            });
            if should_send {
                let _ = progress_tx.send(progress);
            }
        };
        let result = {
            let installation = async {
                if let Some((parent, plan)) = content_update {
                    update_content_with_progress(
                        ContentUpdateRequest {
                            instance_id,
                            revision_id: pending.revision_id,
                            parent_revision_id: parent.id,
                            parent_manifest_digest: parent.manifest_digest,
                            minecraft_version: instance.minecraft_version,
                            loader_kind: instance.loader_kind,
                            loader_version: instance.loader_version,
                            plan,
                            download_concurrency: preferences.download_concurrency,
                            download_bandwidth_limit_mib: preferences.download_bandwidth_limit_mib,
                            paths: install_paths.clone(),
                        },
                        progress_callback,
                    )
                    .await
                } else {
                    install_with_progress(
                        NativeInstallRequest {
                            instance_id,
                            revision_id: pending.revision_id,
                            minecraft_version: instance.minecraft_version,
                            loader_kind: instance.loader_kind,
                            loader_version: target_loader_version,
                            modpack_plan,
                            download_concurrency: preferences.download_concurrency,
                            download_bandwidth_limit_mib: preferences.download_bandwidth_limit_mib,
                            paths: install_paths,
                        },
                        progress_callback,
                    )
                    .await
                }
            };
            tokio::pin!(installation);
            tokio::select! {
                biased;
                _ = cancellation => None,
                result = &mut installation => Some(result),
            }
        };
        let _ = progress_task.await;
        match result {
            None => {
                let _ = task_state
                    .database
                    .cancel_instance_install(
                        pending.job.id,
                        pending.revision_id,
                        "Installation cancelled",
                    )
                    .await;
                tracing::info!(job_id = %pending.job.id, instance_id = %instance_id, "installation cancelled");
            }
            Some(Ok(outcome)) => {
                let runtime = InstalledRuntime {
                    vendor: outcome.runtime.vendor,
                    release_name: outcome.runtime.release_name.clone(),
                    java_version: outcome.runtime.release_name,
                    major: outcome.runtime.major_version,
                    os: std::env::consts::OS.to_owned(),
                    arch: outcome.runtime.architecture,
                    executable_ref: outcome.runtime.executable.to_string_lossy().into_owned(),
                    source_digest: outcome.runtime.package_sha256,
                };
                let message = if let Some(update) = &modpack_update {
                    format!(
                        "Updated the pack to {} with {} verified content files",
                        update.version_id, outcome.installed_content_files
                    )
                } else if !pending_mods.is_empty() {
                    format!(
                        "Installed {} verified mod files without reinstalling the base instance",
                        outcome.installed_content_files
                    )
                } else if outcome.installed_content_files > 0 {
                    format!(
                        "Ready with {} verified pack files; downloaded {} game files and reused {} cached files",
                        outcome.installed_content_files,
                        outcome.downloaded_artifacts,
                        outcome.reused_artifacts
                    )
                } else {
                    format!(
                        "Installed and verified {} files; reused {} cached files",
                        outcome.downloaded_artifacts, outcome.reused_artifacts
                    )
                };
                let completion = if let Some(update) = modpack_update {
                    task_state
                        .database
                        .complete_instance_modpack_update(
                            CompletedInstall {
                                job_id: pending.job.id,
                                revision_id: pending.revision_id,
                                manifest_digest: outcome.manifest_digest,
                                client_version: outcome.resolved_version_id,
                                runtime,
                                message,
                            },
                            CompletedModpackUpdate {
                                version_id: update.version_id,
                                loader_version: update.loader_version,
                            },
                        )
                        .await
                } else if pending_mods.is_empty() {
                    task_state
                        .database
                        .complete_instance_install(
                            pending.job.id,
                            pending.revision_id,
                            &outcome.manifest_digest,
                            &outcome.resolved_version_id,
                            runtime,
                            &message,
                        )
                        .await
                } else {
                    task_state
                        .database
                        .complete_instance_mod_install(
                            CompletedInstall {
                                job_id: pending.job.id,
                                revision_id: pending.revision_id,
                                manifest_digest: outcome.manifest_digest,
                                client_version: outcome.resolved_version_id,
                                runtime,
                                message,
                            },
                            pending_mods,
                        )
                        .await
                };
                if let Err(error) = completion {
                    tracing::error!(
                        job_id = %pending.job.id,
                        error = %error,
                        "could not commit a completed instance installation"
                    );
                } else {
                    tracing::info!(
                        job_id = %pending.job.id,
                        instance_id = %instance_id,
                        downloaded = outcome.downloaded_artifacts,
                        reused = outcome.reused_artifacts,
                        content_files = outcome.installed_content_files,
                        "installation completed"
                    );
                }
            }
            Some(Err(error)) => {
                tracing::error!(
                    job_id = %pending.job.id,
                    instance_id = %instance_id,
                    error = %error,
                    "instance installation failed"
                );
                let message = install_failure_message(&error);
                let _ = task_state
                    .database
                    .fail_instance_install(pending.job.id, pending.revision_id, message)
                    .await;
            }
        }
        task_state.installs.finish(pending.job.id);
    });
    Ok(response)
}

#[tauri::command]
pub(super) async fn install_job_cancel(
    state: tauri::State<'_, DesktopState>,
    request: CancelInstallJobRequest,
) -> Result<InstallJobSummary, AppError> {
    let job_id = JobId::from_uuid(request.job_id);
    let revision_id = RevisionId::from_uuid(request.revision_id);
    let job = state
        .database
        .get_install_job(job_id)
        .await
        .map_err(|error| map_storage_error(error, "That installation is no longer active."))?;
    if job.revision_id != revision_id
        || !matches!(job.state, JobState::Queued | JobState::Running)
        || !state.installs.cancel(job_id)
    {
        return Err(AppError::new(
            "local.install_not_active",
            "That installation has already finished.",
        ));
    }
    state
        .database
        .cancel_instance_install(job_id, revision_id, "Installation cancelled")
        .await
        .map_err(|error| map_storage_error(error, "slate could not cancel that installation."))?;
    state
        .database
        .get_install_job(job_id)
        .await
        .map(install_job_summary)
        .map_err(|error| map_storage_error(error, "slate could not reload that installation."))
}

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
        Some("instance_install") => {
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
        _ => Err(retry_context_missing()),
    }
}

fn retry_context_missing() -> AppError {
    AppError::new(
        "local.install_retry_unavailable",
        "This older installation cannot be retried here. Open the instance and try again.",
    )
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

async fn modpack_update_apply_inner(
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
        Vec::new(),
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
        Vec::new(),
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

#[tauri::command]
pub(super) async fn instance_mod_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallModRequest,
) -> Result<InstallJobSummary, AppError> {
    instance_mod_install_inner(state.inner(), request).await
}

async fn instance_mod_install_inner(
    state: &DesktopState,
    request: InstallModRequest,
) -> Result<InstallJobSummary, AppError> {
    refresh_exited_sessions(state).await;
    if request.mods.is_empty() || request.mods.len() > 50 {
        return Err(AppError::new(
            "mod.invalid_selection",
            "Select between 1 and 50 mods to install at once.",
        ));
    }
    let mut requested = HashSet::new();
    for selection in &request.mods {
        let display_name = selection.display_name.trim();
        let project_id = selection.project_id.trim();
        if selection.provider == slate_modpack_api_contracts::Provider::Ftb
            || display_name.is_empty()
            || display_name.chars().count() > 160
            || display_name.chars().any(char::is_control)
            || project_id.is_empty()
            || project_id.len() > 128
            || !requested.insert(format!("{}:{project_id}", selection.provider))
        {
            return Err(AppError::new(
                "mod.invalid_selection",
                "The selected mod list contains an invalid or duplicate project.",
            ));
        }
    }
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let installed = installed_mod_index(state, &instance).await?;
    let (loader, loader_version) = instance_mod_target(&instance)?;
    let mut merged_plan: Option<InstallPlan> = None;
    let mut pending_by_identity = BTreeMap::<String, NewInstanceMod>::new();
    let mut planned_destinations = BTreeMap::<String, PlannedModDestination>::new();
    for selection in &request.mods {
        let root_identity = format!("{}:{}", selection.provider, selection.project_id.trim());
        if installed.identities.contains(&root_identity) {
            return Err(AppError::new(
                "mod.already_installed",
                format!(
                    "{} is already installed in this instance.",
                    selection.display_name
                ),
            ));
        }
        let mut plan = state
            .modpacks
            .mod_install_plan(
                selection.provider,
                selection.project_id.trim(),
                &ModInstallPlanRequest {
                    minecraft_version: instance.minecraft_version.clone(),
                    loader,
                    loader_version: Some(loader_version.clone()),
                },
            )
            .await
            .map_err(modpack_api_error)?;
        validate_instance_mod_plan(
            &instance,
            selection.provider,
            selection.project_id.trim(),
            &plan,
        )?;
        let mut accepted_downloads = Vec::new();
        for download in plan.downloads {
            let (provider, project_id, version_id) = parse_mod_download_id(&download.id)
                .ok_or_else(|| {
                    AppError::new(
                        "mod.invalid_plan",
                        "The mod service returned an invalid dependency identity.",
                    )
                })?;
            let dependency_identity = format!("{provider}:{project_id}");
            if installed.identities.contains(&dependency_identity) {
                continue;
            }
            if let Some(existing) = pending_by_identity.get_mut(&dependency_identity) {
                if existing.version_id != version_id {
                    return Err(AppError::new(
                        "mod.dependency_conflict",
                        "The selected mods require conflicting versions of the same dependency.",
                    ));
                }
                if dependency_identity == root_identity {
                    existing.display_name = selection.display_name.trim().to_owned();
                }
                continue;
            }
            let display_name = if dependency_identity == root_identity {
                selection.display_name.trim().to_owned()
            } else {
                let file_name = download
                    .destination
                    .rsplit('/')
                    .next()
                    .unwrap_or(project_id.as_str());
                file_name
                    .strip_suffix(".jar")
                    .unwrap_or(file_name)
                    .to_owned()
            };
            let normalized_destination = normalized_content_path(&download.destination);
            if let Some(existing) = installed.artifacts.get(&normalized_destination) {
                if !installed_artifact_matches(existing, download.size, &download.hashes) {
                    return Err(AppError::new(
                        "mod.file_conflict",
                        format!(
                            "{} conflicts with an existing mod file named {}.",
                            selection.display_name, download.destination
                        ),
                    ));
                }
                pending_by_identity.insert(
                    dependency_identity,
                    NewInstanceMod {
                        provider,
                        project_id,
                        version_id,
                        display_name,
                        file_path: download.destination,
                        hashes: download.hashes,
                    },
                );
                continue;
            }
            if let Some(existing) = planned_destinations.get(&normalized_destination) {
                if !same_mod_artifact(
                    existing.size,
                    &existing.hashes,
                    download.size,
                    &download.hashes,
                ) {
                    return Err(AppError::new(
                        "mod.file_conflict",
                        format!(
                            "{} and {} resolve to different files named {}. Deselect one and try again.",
                            existing.requested_name, selection.display_name, download.destination
                        ),
                    ));
                }
                pending_by_identity.insert(
                    dependency_identity,
                    NewInstanceMod {
                        provider,
                        project_id,
                        version_id,
                        display_name,
                        file_path: download.destination,
                        hashes: download.hashes,
                    },
                );
                continue;
            }
            planned_destinations.insert(
                normalized_destination,
                PlannedModDestination {
                    size: download.size,
                    hashes: download.hashes.clone(),
                    requested_name: selection.display_name.trim().to_owned(),
                },
            );
            pending_by_identity.insert(
                dependency_identity,
                NewInstanceMod {
                    provider,
                    project_id,
                    version_id,
                    display_name,
                    file_path: download.destination.clone(),
                    hashes: download.hashes.clone(),
                },
            );
            accepted_downloads.push(download);
        }
        plan.downloads = accepted_downloads;
        if let Some(merged) = &mut merged_plan {
            merged.downloads.extend(plan.downloads);
        } else {
            merged_plan = Some(plan);
        }
    }
    let mut plan = merged_plan.ok_or_else(|| {
        AppError::new(
            "mod.already_installed",
            "Every selected mod and dependency is already installed.",
        )
    })?;
    if plan.downloads.is_empty() || pending_by_identity.is_empty() {
        return Err(AppError::new(
            "mod.already_installed",
            "Every selected mod and dependency is already installed.",
        ));
    }
    plan.total_download_size = plan.downloads.iter().try_fold(0_u64, |total, download| {
        total.checked_add(download.size).ok_or_else(|| {
            AppError::new(
                "mod.invalid_plan",
                "The selected mod download size is invalid.",
            )
        })
    })?;
    queue_instance_install(
        state,
        instance,
        request.expected_revision,
        Some(plan),
        pending_by_identity.into_values().collect(),
        None,
        RetryableInstallOperation::ModInstall { mods: request.mods },
    )
    .await
}
