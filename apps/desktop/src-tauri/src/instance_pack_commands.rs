use super::*;

async fn rollback_content_file_move(file_move: FileMove) -> Result<(), AppError> {
    let rollback = tokio::task::spawn_blocking(move || file_move.rollback()).await;
    if matches!(rollback, Ok(Ok(()))) {
        Ok(())
    } else {
        Err(AppError::new(
            "local.content_rollback_failed",
            "slate could not restore the content file after the change failed. The instance needs attention.",
        ))
    }
}

async fn finish_resource_pack_file_change(
    state: &DesktopState,
    instance_id: InstanceId,
    file_move: FileMove,
    options_change: PendingGameOptionsWrite,
    database_result: Result<(), StorageError>,
    fallback: &'static str,
) -> Result<InstanceSummary, AppError> {
    if let Err(error) = database_result {
        options_change.rollback().await;
        rollback_content_file_move(file_move).await?;
        return Err(map_storage_error(error, fallback));
    }
    options_change.commit().await;
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not refresh the instance."))
}
#[tauri::command]
pub(super) async fn instance_worlds_list(
    state: tauri::State<'_, DesktopState>,
    request: InstanceWorldsRequest,
) -> Result<Vec<String>, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let paths = paths_for_instance(state.inner(), &instance);
    tauri::async_runtime::spawn_blocking(move || scan_instance_worlds(&paths, instance_id))
        .await
        .map_err(|_| content_file_error())?
        .map_err(|_| content_file_error())
}

#[tauri::command]
pub(super) async fn instance_content_file_import(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: ImportLocalContentFileRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    local_pack_import_kind(&instance, request.kind, request.world_name.as_deref())?;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Add a content pack")
            .add_filter("Minecraft content pack", &["zip"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| local_pack_import_error(LocalContentImportError::InvalidArchive))?;
    let Some(source) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(instance_summary(instance));
    };

    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    let import_kind =
        local_pack_import_kind(&instance, request.kind, request.world_name.as_deref())?;
    let paths = paths_for_instance(state.inner(), &instance);
    if let LocalContentImportKind::DataPack(world_name) = &import_kind {
        let paths_for_worlds = paths.clone();
        let worlds = tauri::async_runtime::spawn_blocking(move || {
            scan_instance_worlds(&paths_for_worlds, instance_id)
        })
        .await
        .map_err(|_| content_file_error())?
        .map_err(|_| content_file_error())?;
        if !worlds.contains(world_name) {
            return Err(AppError::new(
                "content.world_unavailable",
                "That world is no longer available. Choose another world and try again.",
            ));
        }
    }
    let paths_for_validation = paths.clone();
    let source_for_validation = source.clone();
    let validation_kind = import_kind.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), LocalContentImportError> {
        validate_local_content_source(&source_for_validation, &validation_kind)?;
        ensure_local_content_not_installed(
            &paths_for_validation,
            instance_id,
            &source_for_validation,
            &validation_kind,
        )
    })
    .await
    .map_err(|_| local_pack_import_error(LocalContentImportError::InvalidArchive))?
    .map_err(local_pack_import_error)?;
    create_automatic_snapshot_if_enabled(state.inner(), &instance).await?;

    let imported = tauri::async_runtime::spawn_blocking(move || {
        import_local_content(&paths, instance_id, &source, &import_kind)
    })
    .await
    .map_err(|_| local_pack_import_error(LocalContentImportError::InvalidArchive))?
    .map_err(local_pack_import_error)?;
    if let Err(error) = state
        .database
        .advance_instance_revision(instance_id, request.expected_revision)
        .await
    {
        rollback_imported_content(imported).await?;
        return Err(map_storage_error(
            error,
            "slate could not record the imported content pack.",
        ));
    }
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not refresh the instance."))
}

fn local_pack_import_kind(
    instance: &InstanceRecord,
    kind: InstanceContentKindDto,
    world_name: Option<&str>,
) -> Result<LocalContentImportKind, AppError> {
    if instance.setup_state != slate_domain::InstanceSetupState::Ready {
        return Err(AppError::new(
            "local.instance_not_ready",
            "Finish installing or repair this instance before adding content packs.",
        ));
    }
    match kind {
        InstanceContentKindDto::ResourcePack => {
            Ok(LocalContentImportKind::Pack(InstanceContentKind::Resource))
        }
        InstanceContentKindDto::ShaderPack => {
            Ok(LocalContentImportKind::Pack(InstanceContentKind::Shader))
        }
        InstanceContentKindDto::DataPack => {
            let world_name = world_name.filter(|value| {
                !value.is_empty()
                    && value.len() <= 240
                    && !matches!(*value, "." | "..")
                    && !value.contains(['/', '\\', ':'])
                    && !value.chars().any(char::is_control)
            });
            world_name
                .map(|value| LocalContentImportKind::DataPack(value.to_owned()))
                .ok_or_else(|| {
                    AppError::new(
                        "content.world_required",
                        "Choose a world before importing a data pack.",
                    )
                })
        }
    }
}

fn local_pack_import_error(error: LocalContentImportError) -> AppError {
    match error {
        LocalContentImportError::AlreadyInstalled => AppError::new(
            "content.already_installed",
            "A content pack with that file name is already installed.",
        ),
        LocalContentImportError::TooLarge => AppError::new(
            "content.file_too_large",
            "That content pack is larger than the 1 GB import limit.",
        ),
        LocalContentImportError::InvalidArchive | LocalContentImportError::WrongLoader => {
            AppError::new(
                "content.invalid_archive",
                "Choose a valid content pack ZIP for this section.",
            )
        }
        LocalContentImportError::Io(_) => AppError::new(
            "local.content_import_failed",
            "slate could not safely copy that content pack.",
        ),
    }
}

#[tauri::command]
pub(super) async fn instance_content_files_list(
    state: tauri::State<'_, DesktopState>,
    request: InstanceContentFilesRequest,
) -> Result<Vec<InstanceContentFileSummary>, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let internal_kind = content_kind(request.kind);
    let paths = paths_for_instance(state.inner(), &instance);
    let scan_paths = paths.clone();
    let files = tokio::task::spawn_blocking(move || {
        scan_instance_content(&scan_paths, instance_id, internal_kind)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let mut managed_paths = HashSet::new();
    let mut provider_content = state
        .database
        .list_instance_provider_content(instance_id, provider_content_kind(request.kind))
        .await
        .map_err(|error| map_storage_error(error, "slate could not load installed content."))?
        .into_iter()
        .map(|item| (normalized_content_path(&item.file_path), item))
        .collect::<HashMap<_, _>>();
    if let Some(source) = &instance.modpack_source {
        match state
            .modpacks
            .version(source.provider, &source.project_id, &source.version_id)
            .await
        {
            Ok(version) => {
                for file in version.files {
                    if pack_file_matches_content(file.kind, request.kind) {
                        managed_paths.insert(normalized_content_path(&file.path));
                    }
                }
            }
            Err(error) => tracing::warn!(
                instance_id = %instance_id,
                error = %error,
                "could not resolve pack-managed content paths"
            ),
        }
    }
    let mut summaries = files
        .into_iter()
        .map(|file| {
            let source = provider_content.remove(&normalized_content_path(&file.file_path));
            content_file_summary(file, request.kind, &managed_paths, source)
        })
        .collect::<Vec<_>>();
    if request.kind == InstanceContentKindDto::ResourcePack {
        let active_packs = read_resource_packs(&instance_options_path(&paths, instance_id))
            .await
            .map_err(|_| resource_pack_options_error())?;
        apply_resource_pack_state(&mut summaries, &active_packs);
    }
    Ok(summaries)
}

fn apply_resource_pack_state(files: &mut [InstanceContentFileSummary], active_packs: &[String]) {
    let ordered_files = active_packs
        .iter()
        .rev()
        .filter(|entry| entry.starts_with("file/"))
        .collect::<Vec<_>>();
    let priorities = ordered_files
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.as_str(), u32::try_from(index + 1).unwrap_or(u32::MAX)))
        .collect::<HashMap<_, _>>();
    let active_count = u32::try_from(ordered_files.len()).unwrap_or(u32::MAX);
    for file in files.iter_mut() {
        let identity = resource_pack_identity(&file.file_path);
        file.priority = identity.and_then(|entry| priorities.get(entry.as_str()).copied());
        file.active = Some(file.priority.is_some());
        file.can_move_higher = file.priority.is_some_and(|priority| priority > 1);
        file.can_move_lower = file
            .priority
            .is_some_and(|priority| priority < active_count);
    }
    files.sort_by(|left, right| {
        left.priority
            .unwrap_or(u32::MAX)
            .cmp(&right.priority.unwrap_or(u32::MAX))
            .then_with(|| {
                left.display_name
                    .to_ascii_lowercase()
                    .cmp(&right.display_name.to_ascii_lowercase())
            })
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
}

fn resource_pack_identity(file_path: &str) -> Option<String> {
    let file_name = file_path.strip_prefix("resourcepacks/")?;
    if file_name.is_empty() || file_name.contains(['/', '\\']) {
        return None;
    }
    Some(format!("file/{file_name}"))
}

fn resource_pack_file_name(file_path: &str) -> Result<&str, AppError> {
    file_path
        .strip_prefix("resourcepacks/")
        .filter(|name| !name.is_empty() && !name.contains(['/', '\\']))
        .ok_or_else(resource_pack_options_error)
}

fn resource_pack_options_error() -> AppError {
    AppError::new(
        "content.resource_pack_options_unavailable",
        "slate could not update the resource pack list. Open Minecraft once, then try again.",
    )
}

#[tauri::command]
pub(super) async fn instance_resource_pack_set_active(
    state: tauri::State<'_, DesktopState>,
    request: SetInstanceResourcePackActiveRequest,
) -> Result<InstanceSummary, AppError> {
    update_resource_pack_configuration(
        state.inner(),
        InstanceId::from_uuid(request.instance_id),
        request.expected_revision,
        &request.file_path,
        ResourcePackOrderChange::SetActive(request.active),
    )
    .await
}

#[tauri::command]
pub(super) async fn instance_resource_pack_move(
    state: tauri::State<'_, DesktopState>,
    request: MoveInstanceResourcePackRequest,
) -> Result<InstanceSummary, AppError> {
    let change = match request.direction {
        ResourcePackOrderDirectionDto::Higher => ResourcePackOrderChange::MoveHigher,
        ResourcePackOrderDirectionDto::Lower => ResourcePackOrderChange::MoveLower,
    };
    update_resource_pack_configuration(
        state.inner(),
        InstanceId::from_uuid(request.instance_id),
        request.expected_revision,
        &request.file_path,
        change,
    )
    .await
}

async fn update_resource_pack_configuration(
    state: &DesktopState,
    instance_id: InstanceId,
    expected_revision: u64,
    file_path: &str,
    change: ResourcePackOrderChange,
) -> Result<InstanceSummary, AppError> {
    let instance = prepare_instance_content_change(state, instance_id, expected_revision).await?;
    let paths = paths_for_instance(state, &instance);
    let scan_paths = paths.clone();
    let files = tokio::task::spawn_blocking(move || {
        scan_instance_content(&scan_paths, instance_id, InstanceContentKind::Resource)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let file = files
        .iter()
        .find(|candidate| candidate.file_path == file_path)
        .ok_or_else(resource_pack_options_error)?;
    if !file.enabled && change != ResourcePackOrderChange::SetActive(false) {
        return Err(AppError::new(
            "content.resource_pack_hidden",
            "Restore that resource pack before activating or reordering it.",
        ));
    }
    let pending = prepare_resource_pack_update(
        instance_options_path(&paths, instance_id),
        resource_pack_file_name(file_path)?,
        change,
    )
    .await
    .map_err(|_| resource_pack_options_error())?;
    if let Err(error) = state
        .database
        .advance_instance_revision(instance_id, expected_revision)
        .await
    {
        pending.rollback().await;
        return Err(map_storage_error(
            error,
            "slate could not record the resource pack change.",
        ));
    }
    pending.commit().await;
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not refresh the instance."))
}

#[tauri::command]
pub(super) async fn instance_content_file_set_enabled(
    state: tauri::State<'_, DesktopState>,
    request: SetInstanceContentFileEnabledRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    let paths = paths_for_instance(state.inner(), &instance);
    let file_path = request.file_path.clone();
    let kind = content_kind(request.kind);
    let enabled = request.enabled;
    let file_move = tokio::task::spawn_blocking(move || {
        set_instance_content_enabled(&paths, instance_id, kind, &file_path, enabled)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let options_change = if request.kind == InstanceContentKindDto::ResourcePack && !enabled {
        match prepare_resource_pack_update(
            instance_options_path(&paths_for_instance(state.inner(), &instance), instance_id),
            resource_pack_file_name(&request.file_path)?,
            ResourcePackOrderChange::SetActive(false),
        )
        .await
        {
            Ok(pending) => Some(pending),
            Err(_) => {
                rollback_content_file_move(file_move).await?;
                return Err(resource_pack_options_error());
            }
        }
    } else {
        None
    };
    let database_result = state
        .database
        .move_instance_provider_content(
            instance_id,
            request.expected_revision,
            &request.file_path,
            file_move
                .updated_file_path
                .as_deref()
                .ok_or_else(content_file_error)?,
        )
        .await;
    if let Some(options_change) = options_change {
        finish_resource_pack_file_change(
            state.inner(),
            instance_id,
            file_move,
            options_change,
            database_result,
            "slate could not update that content file.",
        )
        .await
    } else {
        finish_instance_content_change(
            state.inner(),
            instance_id,
            file_move,
            database_result,
            "slate could not update that content file.",
        )
        .await
    }
}

#[tauri::command]
pub(super) async fn instance_content_set_pinned(
    state: tauri::State<'_, DesktopState>,
    request: SetInstanceContentPinnedRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    if request.provider != slate_modpack_api_contracts::Provider::Modrinth
        || request.project_id.trim().is_empty()
    {
        return Err(AppError::new(
            "content.provider_unavailable",
            "Updates are not available for that content file.",
        ));
    }
    state
        .database
        .set_instance_provider_content_pinned(
            instance_id,
            request.expected_revision,
            provider_content_kind(request.kind),
            request.provider,
            request.project_id.trim(),
            request.pinned,
        )
        .await
        .map_err(|error| map_storage_error(error, "slate could not update that content file."))?;
    let updated = state
        .database
        .get_instance(instance.id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not reload that instance."))?;
    Ok(instance_summary(updated))
}

#[tauri::command]
pub(super) async fn instance_content_file_remove(
    state: tauri::State<'_, DesktopState>,
    request: RemoveInstanceContentFileRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    let paths = paths_for_instance(state.inner(), &instance);
    let file_path = request.file_path.clone();
    let kind = content_kind(request.kind);
    let file_move = tokio::task::spawn_blocking(move || {
        trash_instance_content(&paths, instance_id, kind, &file_path)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let options_change = if request.kind == InstanceContentKindDto::ResourcePack {
        match prepare_resource_pack_update(
            instance_options_path(&paths_for_instance(state.inner(), &instance), instance_id),
            resource_pack_file_name(&request.file_path)?,
            ResourcePackOrderChange::SetActive(false),
        )
        .await
        {
            Ok(pending) => Some(pending),
            Err(_) => {
                rollback_content_file_move(file_move).await?;
                return Err(resource_pack_options_error());
            }
        }
    } else {
        None
    };
    let database_result = state
        .database
        .remove_instance_provider_content(
            instance_id,
            request.expected_revision,
            &request.file_path,
        )
        .await;
    if let Some(options_change) = options_change {
        finish_resource_pack_file_change(
            state.inner(),
            instance_id,
            file_move,
            options_change,
            database_result,
            "slate could not remove that content file.",
        )
        .await
    } else {
        finish_instance_content_change(
            state.inner(),
            instance_id,
            file_move,
            database_result,
            "slate could not remove that content file.",
        )
        .await
    }
}
