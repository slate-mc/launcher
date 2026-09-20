use super::*;

#[derive(Clone, Debug)]
pub(super) struct PendingModpackUpdate {
    pub(super) version_id: String,
    pub(super) loader_version: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct PendingModChanges {
    pub(super) installed: Vec<NewInstanceMod>,
    pub(super) replaced_paths: Vec<String>,
    pub(super) dependency_sets: Vec<NewInstanceModDependencySet>,
    pub(super) provider_content: Vec<NewInstanceProviderContent>,
    pub(super) replaced_content_paths: Vec<String>,
    pub(super) imported_overrides: Option<PendingImportedOverrides>,
}

#[derive(Debug)]
pub(super) struct PendingImportedOverrides {
    pub(super) archive_path: PathBuf,
    pub(super) prefixes: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) enum RetryableInstallOperation {
    InstanceInstall,
    ExternalInstanceImport,
    ModInstall {
        mods: Vec<InstallModSelection>,
    },
    ContentInstall {
        kind: InstanceContentKindDto,
        world_name: Option<String>,
        content: Vec<InstallContentSelection>,
    },
    ContentUpdate {
        kind: InstanceContentKindDto,
        provider: slate_modpack_api_contracts::Provider,
        project_id: String,
        file_path: String,
        display_name: String,
        target_version_id: String,
    },
    ModUpdate {
        provider: slate_modpack_api_contracts::Provider,
        project_id: String,
        file_path: String,
        display_name: String,
        target_version_id: String,
    },
    ModpackUpdate {
        target_version_id: String,
    },
    ImportedPackInstall {
        plan: Box<InstallPlan>,
        override_prefixes: Vec<String>,
    },
}

impl RetryableInstallOperation {
    const fn storage_name(&self) -> &'static str {
        match self {
            Self::InstanceInstall => "instance_install",
            Self::ExternalInstanceImport => "external_instance_import",
            Self::ModInstall { .. } => "mod_install",
            Self::ContentInstall { .. } => "content_install",
            Self::ContentUpdate { .. } => "content_install",
            Self::ModUpdate { .. } => "mod_update",
            Self::ModpackUpdate { .. } => "modpack_update",
            Self::ImportedPackInstall { .. } => "imported_pack_install",
        }
    }

    fn payload(&self) -> Option<serde_json::Value> {
        match self {
            Self::InstanceInstall | Self::ExternalInstanceImport => None,
            Self::ModInstall { mods } => Some(serde_json::json!({ "mods": mods })),
            Self::ContentInstall {
                kind,
                world_name,
                content,
            } => Some(serde_json::json!({
                "kind": kind,
                "worldName": world_name,
                "content": content,
            })),
            Self::ContentUpdate {
                kind,
                provider,
                project_id,
                file_path,
                display_name,
                target_version_id,
            } => Some(serde_json::json!({
                "kind": kind,
                "provider": provider,
                "projectId": project_id,
                "filePath": file_path,
                "displayName": display_name,
                "targetVersionId": target_version_id,
            })),
            Self::ModUpdate {
                provider,
                project_id,
                file_path,
                display_name,
                target_version_id,
            } => Some(serde_json::json!({
                "provider": provider,
                "projectId": project_id,
                "filePath": file_path,
                "displayName": display_name,
                "targetVersionId": target_version_id,
            })),
            Self::ModpackUpdate { target_version_id } => {
                Some(serde_json::json!({ "targetVersionId": target_version_id }))
            }
            Self::ImportedPackInstall {
                plan,
                override_prefixes,
            } => Some(serde_json::json!({
                "plan": plan,
                "overridePrefixes": override_prefixes,
            })),
        }
    }

    const fn skips_automatic_snapshot(&self) -> bool {
        matches!(self, Self::ExternalInstanceImport)
    }
}

#[tauri::command]
pub(super) async fn instance_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallInstanceRequest,
) -> Result<InstallJobSummary, AppError> {
    instance_install_inner(state.inner(), request).await
}

pub(super) async fn instance_install_inner(
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
        PendingModChanges::default(),
        None,
        RetryableInstallOperation::InstanceInstall,
    )
    .await
}

pub(super) async fn queue_instance_install(
    state: &DesktopState,
    instance: InstanceRecord,
    expected_revision: u64,
    mut modpack_plan: Option<InstallPlan>,
    mod_changes: PendingModChanges,
    modpack_update: Option<PendingModpackUpdate>,
    operation: RetryableInstallOperation,
) -> Result<InstallJobSummary, AppError> {
    let PendingModChanges {
        installed: pending_mods,
        replaced_paths: replaced_mod_paths,
        dependency_sets,
        provider_content,
        replaced_content_paths,
        imported_overrides,
    } = mod_changes;
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
    if !operation.skips_automatic_snapshot() {
        create_automatic_snapshot_if_enabled(state, &instance).await?;
    }
    let content_update = if pending_mods.is_empty() && provider_content.is_empty() {
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
    let control = state.installs.register(pending.job.id);
    tracing::info!(
        instance_id = %instance_id,
        job_id = %pending.job.id,
        revision_id = %pending.revision_id,
        operation = operation.storage_name(),
        "installation queued"
    );
    let response = supervised_install_job_summary(state, pending.job.clone());
    let install_paths = paths_for_instance(state, &instance);
    let target_loader_version = modpack_update.as_ref().map_or_else(
        || instance.loader_version.clone(),
        |update| update.loader_version.clone(),
    );
    let task_state = state.clone();
    tauri::async_runtime::spawn(async move {
        let mut cancellation = control.cancellation;
        let mut pause = control.pause;
        let mut start = control.start;
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
        let started = tokio::select! {
            biased;
            _ = &mut cancellation => false,
            result = &mut start => result.is_ok(),
        };
        let result = if started {
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
            loop {
                if *pause.borrow() {
                    tokio::select! {
                        biased;
                        _ = &mut cancellation => break None,
                        changed = pause.changed() => {
                            if changed.is_err() {
                                break None;
                            }
                        }
                    }
                } else {
                    tokio::select! {
                        biased;
                        _ = &mut cancellation => break None,
                        changed = pause.changed() => {
                            if changed.is_err() {
                                break None;
                            }
                        }
                        result = &mut installation => break Some(result),
                    }
                }
            }
        } else {
            None
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
                if let Some(imported_overrides) = imported_overrides {
                    let _ = task_state
                        .database
                        .update_install_progress(
                            pending.job.id,
                            "content",
                            "Applying imported pack settings",
                            None,
                            None,
                        )
                        .await;
                    let instance_root = task_state.paths.instance(instance_id);
                    let extraction = tauri::async_runtime::spawn_blocking(move || {
                        extract_import_overrides(
                            &imported_overrides.archive_path,
                            &instance_root,
                            &imported_overrides.prefixes,
                        )
                    })
                    .await;
                    if !matches!(extraction, Ok(Ok(()))) {
                        let _ = task_state
                            .database
                            .fail_instance_install(
                                pending.job.id,
                                pending.revision_id,
                                "The pack files were installed, but its local overrides could not be applied.",
                            )
                            .await;
                        tracing::error!(
                            job_id = %pending.job.id,
                            instance_id = %instance_id,
                            "imported pack overrides could not be applied"
                        );
                        task_state.installs.finish(pending.job.id);
                        return;
                    }
                }
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
                } else if !provider_content.is_empty() && !pending_mods.is_empty() {
                    format!(
                        "Installed {} verified content and dependency files",
                        outcome.installed_content_files
                    )
                } else if !replaced_mod_paths.is_empty() {
                    format!(
                        "Updated {} verified mod files without reinstalling the instance",
                        outcome.installed_content_files
                    )
                } else if !pending_mods.is_empty() {
                    format!(
                        "Installed {} verified mod files without reinstalling the base instance",
                        outcome.installed_content_files
                    )
                } else if !provider_content.is_empty() {
                    format!(
                        "Installed {} verified content files without reinstalling the base instance",
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
                } else if !provider_content.is_empty() && !pending_mods.is_empty() {
                    task_state
                        .database
                        .complete_instance_mixed_content_install(
                            CompletedInstall {
                                job_id: pending.job.id,
                                revision_id: pending.revision_id,
                                manifest_digest: outcome.manifest_digest,
                                client_version: outcome.resolved_version_id,
                                runtime,
                                message,
                            },
                            pending_mods,
                            replaced_mod_paths,
                            dependency_sets,
                            provider_content,
                            replaced_content_paths,
                        )
                        .await
                } else if !replaced_mod_paths.is_empty() {
                    task_state
                        .database
                        .complete_instance_mod_update(
                            CompletedInstall {
                                job_id: pending.job.id,
                                revision_id: pending.revision_id,
                                manifest_digest: outcome.manifest_digest,
                                client_version: outcome.resolved_version_id,
                                runtime,
                                message,
                            },
                            pending_mods,
                            replaced_mod_paths,
                            dependency_sets,
                        )
                        .await
                } else if pending_mods.is_empty() {
                    if provider_content.is_empty() {
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
                            .complete_instance_provider_content_install(
                                CompletedInstall {
                                    job_id: pending.job.id,
                                    revision_id: pending.revision_id,
                                    manifest_digest: outcome.manifest_digest,
                                    client_version: outcome.resolved_version_id,
                                    runtime,
                                    message,
                                },
                                provider_content,
                                replaced_content_paths,
                            )
                            .await
                    }
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
                            dependency_sets,
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
        || !matches!(
            job.state,
            JobState::Queued | JobState::Running | JobState::Paused
        )
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
        .map(|job| supervised_install_job_summary(state.inner(), job))
        .map_err(|error| map_storage_error(error, "slate could not reload that installation."))
}

#[tauri::command]
pub(super) async fn install_job_set_paused(
    state: tauri::State<'_, DesktopState>,
    request: SetInstallJobPausedRequest,
) -> Result<InstallJobSummary, AppError> {
    let job_id = JobId::from_uuid(request.job_id);
    let revision_id = RevisionId::from_uuid(request.revision_id);
    let job = state
        .database
        .get_install_job(job_id)
        .await
        .map_err(|error| map_storage_error(error, "That installation is no longer active."))?;
    let expected_state = if request.paused {
        matches!(job.state, JobState::Queued | JobState::Running)
    } else {
        job.state == JobState::Paused
    };
    if job.revision_id != revision_id
        || !expected_state
        || !state.installs.set_paused(job_id, request.paused)
    {
        return Err(AppError::new(
            "local.install_not_active",
            "That installation has already finished or changed state.",
        ));
    }
    let changed = state
        .database
        .set_install_paused(job_id, request.paused)
        .await
        .map_err(|error| map_storage_error(error, "slate could not update that installation."))?;
    if !changed {
        let _ = state.installs.set_paused(job_id, !request.paused);
        return Err(AppError::new(
            "local.install_not_active",
            "That installation has already finished or changed state.",
        ));
    }
    state
        .database
        .get_install_job(job_id)
        .await
        .map(|job| supervised_install_job_summary(state.inner(), job))
        .map_err(|error| map_storage_error(error, "slate could not reload that installation."))
}

#[tauri::command]
pub(super) async fn install_job_move(
    state: tauri::State<'_, DesktopState>,
    request: MoveInstallJobRequest,
) -> Result<InstallJobSummary, AppError> {
    let job_id = JobId::from_uuid(request.job_id);
    let direction = match request.direction {
        InstallQueueDirectionDto::Up => QueueDirection::Up,
        InstallQueueDirectionDto::Down => QueueDirection::Down,
    };
    if !state.installs.move_queued(job_id, direction) {
        return Err(AppError::new(
            "local.install_not_queued",
            "That installation has started or cannot move any farther.",
        ));
    }
    state
        .database
        .get_install_job(job_id)
        .await
        .map(|job| supervised_install_job_summary(state.inner(), job))
        .map_err(|error| map_storage_error(error, "slate could not reload that installation."))
}
