use super::*;

#[tauri::command]
pub(super) async fn instance_duplicate(
    state: tauri::State<'_, DesktopState>,
    request: DuplicateInstanceRequest,
) -> Result<InstanceSummary, AppError> {
    let source_id = InstanceId::from_uuid(request.id);
    if state
        .processes
        .active_for_instance(source_id)
        .map_err(process_state_error)?
        .is_some()
    {
        return Err(AppError::new(
            "local.instance_running",
            "Stop Minecraft before duplicating this instance.",
        ));
    }
    let source = state
        .database
        .get_instance(source_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if source.revision != request.expected_revision {
        return Err(AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ));
    }
    let name = parse_instance_name(&request.name).map_err(instance_name_app_error)?;
    let root_id = preferred_storage_root_id(state.inner()).await?;
    let duplicate = state
        .database
        .create_instance(NewInstance {
            name,
            mode: source.mode,
            management_mode: ManagementMode::Local,
            root_id,
            minecraft_version: source.minecraft_version.clone(),
            loader_kind: source.loader_kind,
            loader_version: source.loader_version.clone(),
            memory_mb: source.memory_mb,
            modpack_source: source.modpack_source.as_ref().map(|pack| NewModpackSource {
                provider: pack.provider,
                project_id: pack.project_id.clone(),
                version_id: pack.version_id.clone(),
                selected_optional: pack.selected_optional.clone(),
                display_name: pack.display_name.clone(),
                icon_url: pack.icon_url.clone(),
                banner_url: pack.banner_url.clone(),
            }),
        })
        .await
        .map_err(|error| map_storage_error(error, "slate could not create the duplicate."))?;
    let destination_root = duplicate.storage_path.clone();
    if tokio::fs::create_dir_all(&destination_root).await.is_err() {
        let _ = state
            .database
            .trash_instance(duplicate.id, duplicate.revision)
            .await;
        return Err(instance_files_error());
    }
    if request.include_settings
        && state
            .database
            .copy_instance_profile(source_id, duplicate.id)
            .await
            .is_err()
    {
        let _ = state
            .database
            .trash_instance(duplicate.id, duplicate.revision)
            .await;
        return Err(instance_files_error());
    }
    let source_root = source.storage_path.clone();
    let destination_for_copy = destination_root.clone();
    let copy = tauri::async_runtime::spawn_blocking(move || {
        copy_duplicate_personal_data(
            &source_root,
            &destination_for_copy,
            request.include_worlds,
            request.include_screenshots,
            request.include_settings,
        )
    })
    .await;
    if !matches!(copy, Ok(Ok(()))) {
        let _ = state
            .database
            .trash_instance(duplicate.id, duplicate.revision)
            .await;
        return Err(instance_files_error());
    }
    state
        .database
        .get_instance(duplicate.id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not reload the duplicate."))
}

#[tauri::command]
pub(super) async fn instance_move_storage(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: MoveInstanceStorageRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let instance =
        ensure_instance_stopped_and_current(state.inner(), instance_id, request.expected_revision)
            .await?;
    if instance.setup_state == slate_domain::InstanceSetupState::Preparing {
        return Err(AppError::new(
            "local.install_running",
            "Wait for the active installation to finish before moving this instance.",
        ));
    }
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Choose a slate storage folder")
            .blocking_pick_folder()
    })
    .await
    .map_err(|_| instance_files_error())?;
    let Some(selected) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(instance_summary(instance));
    };
    let selected = tauri::async_runtime::spawn_blocking(move || std::fs::canonicalize(selected))
        .await
        .map_err(|_| instance_files_error())?
        .map_err(|_| instance_files_error())?;
    let canonical_root = selected.to_string_lossy().into_owned();
    let destination = selected.join("instances").join(instance_id.to_string());
    let source = instance.storage_path.clone();
    let pending = tauri::async_runtime::spawn_blocking(move || {
        prepare_instance_relocation(&source, &destination)
    })
    .await
    .map_err(|_| instance_files_error())?
    .map_err(|_| {
        AppError::new(
            "local.instance_move_failed",
            "slate could not copy the instance to that storage folder. Choose an empty, writable location outside the current instance.",
        )
    })?;
    let root_id = match state
        .database
        .get_or_create_storage_root(&canonical_root)
        .await
    {
        Ok(root_id) => root_id,
        Err(error) => {
            let _ = tauri::async_runtime::spawn_blocking(move || pending.rollback()).await;
            return Err(map_storage_error(
                error,
                "slate could not register that storage folder.",
            ));
        }
    };
    let relative_path = ManagedRelativePath::parse(&format!("instances/{instance_id}"))
        .map_err(|_| instance_files_error())?;
    let updated = match state
        .database
        .relocate_instance(
            instance_id,
            root_id,
            &relative_path,
            request.expected_revision,
        )
        .await
    {
        Ok(updated) => updated,
        Err(error) => {
            let _ = tauri::async_runtime::spawn_blocking(move || pending.rollback()).await;
            return Err(map_storage_error(
                error,
                "slate could not commit the instance move.",
            ));
        }
    };
    if !matches!(
        tauri::async_runtime::spawn_blocking(move || pending.commit()).await,
        Ok(Ok(()))
    ) {
        tracing::warn!(instance_id = %instance_id, "instance moved but the old copy could not be removed");
    }
    Ok(instance_summary(updated))
}

#[tauri::command]
pub(super) async fn instance_export(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: ExportInstanceRequest,
) -> Result<(), AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let instance =
        ensure_instance_stopped_and_current(state.inner(), instance_id, request.expected_revision)
            .await?;
    if instance.setup_state == slate_domain::InstanceSetupState::Preparing {
        return Err(AppError::new(
            "local.install_running",
            "Wait for the active installation to finish before exporting this instance.",
        ));
    }
    let manifest = serde_json::to_vec_pretty(&portable_manifest(&instance))
        .map_err(|_| portable_instance_error())?;
    let filename = format!(
        "{}.slate-instance.zip",
        portable_filename(instance.name.as_str())
    );
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Export portable slate instance")
            .set_file_name(filename)
            .add_filter("slate instance", &["zip"])
            .blocking_save_file()
    })
    .await
    .map_err(|_| portable_instance_error())?;
    let Some(destination) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(());
    };
    let source = instance.storage_path;
    tauri::async_runtime::spawn_blocking(move || {
        export_portable_archive(&source, &destination, &manifest)
    })
    .await
    .map_err(|_| portable_instance_error())?
    .map_err(|_| portable_instance_error())
}

#[tauri::command]
pub(super) async fn instance_import(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    _request: ImportInstanceRequest,
) -> Result<Option<InstanceSummary>, AppError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Import a portable slate instance")
            .add_filter("slate instance", &["zip"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| portable_instance_error())?;
    let Some(source) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(None);
    };
    let source_for_manifest = source.clone();
    let manifest_bytes =
        tauri::async_runtime::spawn_blocking(move || read_portable_manifest(&source_for_manifest))
            .await
            .map_err(|_| portable_instance_error())?
            .map_err(|_| portable_instance_error())?;
    let manifest: PortableInstanceManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| portable_instance_error())?;
    if manifest.schema != 1 {
        return Err(AppError::new(
            "local.instance_archive_version",
            "This instance export was created by an unsupported version of slate.",
        ));
    }
    validate_instance_configuration(
        manifest.mode,
        &manifest.minecraft_version,
        manifest.loader_kind,
        manifest.loader_version.as_deref(),
        manifest.memory_mb,
    )
    .map_err(configuration_app_error)?;
    let name = parse_instance_name(&manifest.name).map_err(instance_name_app_error)?;
    let modpack_source = validated_portable_modpack_source(manifest.modpack_source)?;
    let root_id = preferred_storage_root_id(state.inner()).await?;
    let mut record = state
        .database
        .create_instance(NewInstance {
            name,
            mode: manifest.mode.into(),
            management_mode: ManagementMode::Local,
            root_id,
            minecraft_version: manifest.minecraft_version.trim().to_owned(),
            loader_kind: manifest.loader_kind.into(),
            loader_version: manifest.loader_version.clone(),
            memory_mb: manifest.memory_mb,
            modpack_source,
        })
        .await
        .map_err(|error| {
            map_storage_error(error, "slate could not create the imported instance.")
        })?;
    let destination = record.storage_path.clone();
    let extraction = tauri::async_runtime::spawn_blocking(move || {
        extract_portable_archive(&source, &destination)
    })
    .await;
    if !matches!(extraction, Ok(Ok(()))) {
        cleanup_failed_import(state.inner(), &record).await;
        return Err(portable_instance_error());
    }
    let settings_request = portable_settings_request(&record, manifest.settings.clone());
    let settings = match validated_instance_settings(&record, settings_request) {
        Ok(settings) => settings,
        Err(error) => {
            cleanup_failed_import(state.inner(), &record).await;
            return Err(error);
        }
    };
    if let Err(error) = state
        .database
        .update_instance_settings(record.id, record.revision, settings)
        .await
    {
        cleanup_failed_import(state.inner(), &record).await;
        return Err(map_storage_error(
            error,
            "slate could not apply the imported profile.",
        ));
    }
    record = state
        .database
        .get_instance(record.id)
        .await
        .map_err(|error| {
            map_storage_error(error, "slate could not reload the imported profile.")
        })?;
    record = match restore_imported_artwork(state.inner(), record.clone(), &manifest.settings).await
    {
        Ok(record) => record,
        Err(error) => {
            cleanup_failed_import(state.inner(), &record).await;
            return Err(error);
        }
    };
    Ok(Some(instance_summary(record)))
}

#[tauri::command]
pub(super) async fn instance_snapshots_list(
    state: tauri::State<'_, DesktopState>,
    request: InstanceSnapshotsRequest,
) -> Result<Vec<InstanceSnapshotSummary>, AppError> {
    state
        .database
        .list_instance_snapshots(InstanceId::from_uuid(request.id))
        .await
        .map(|snapshots| snapshots.into_iter().map(snapshot_summary).collect())
        .map_err(|error| map_storage_error(error, "slate could not load instance snapshots."))
}

#[tauri::command]
pub(super) async fn instance_snapshot_create(
    state: tauri::State<'_, DesktopState>,
    request: CreateInstanceSnapshotRequest,
) -> Result<InstanceSnapshotSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let instance =
        ensure_instance_stopped_and_current(state.inner(), instance_id, request.expected_revision)
            .await?;
    let snapshot_id = Uuid::new_v4();
    let mods = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not snapshot installed mods."))?;
    let instance_root = instance.storage_path.clone();
    let snapshot =
        tauri::async_runtime::spawn_blocking(move || create_snapshot(&instance_root, snapshot_id))
            .await
            .map_err(|_| instance_files_error())?
            .map_err(|_| instance_files_error())?;
    let manifest_ref = snapshot.0.to_string_lossy().replace('\\', "/");
    let record = match state
        .database
        .create_instance_snapshot(instance_id, snapshot_id, &manifest_ref, snapshot.1, &mods)
        .await
    {
        Ok(record) => record,
        Err(error) => {
            let directory = instance
                .storage_path
                .join("snapshots")
                .join(snapshot_id.to_string());
            let _ = tokio::fs::remove_dir_all(directory).await;
            return Err(map_storage_error(
                error,
                "slate could not record the snapshot.",
            ));
        }
    };
    prune_instance_snapshots(state.inner(), instance_id).await;
    Ok(snapshot_summary(record))
}

#[tauri::command]
pub(super) async fn instance_snapshot_restore(
    state: tauri::State<'_, DesktopState>,
    request: RestoreInstanceSnapshotRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let instance =
        ensure_instance_stopped_and_current(state.inner(), instance_id, request.expected_revision)
            .await?;
    let snapshot = state
        .database
        .get_instance_snapshot(instance_id, request.snapshot_id)
        .await
        .map_err(|error| map_storage_error(error, "That snapshot no longer exists."))?;
    let expected_ref = format!("snapshots/{}", request.snapshot_id);
    if snapshot.manifest_ref.replace('\\', "/") != expected_ref {
        return Err(instance_files_error());
    }
    let instance_root = instance.storage_path;
    let snapshot_id = request.snapshot_id;
    let pending = tauri::async_runtime::spawn_blocking(move || {
        prepare_snapshot_restore(&instance_root, snapshot_id)
    })
    .await
    .map_err(|_| instance_files_error())?
    .map_err(|_| instance_files_error())?;
    if let Err(error) = state
        .database
        .restore_instance_snapshot_state(
            instance_id,
            request.snapshot_id,
            request.expected_revision,
        )
        .await
    {
        let _ = tauri::async_runtime::spawn_blocking(move || pending.rollback()).await;
        return Err(map_storage_error(
            error,
            "slate could not record the restore.",
        ));
    }
    let _ = tauri::async_runtime::spawn_blocking(move || pending.commit()).await;
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not reload that instance."))
}

#[tauri::command]
pub(super) async fn instance_snapshot_delete(
    state: tauri::State<'_, DesktopState>,
    request: DeleteInstanceSnapshotRequest,
) -> Result<(), AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let snapshot = state
        .database
        .delete_instance_snapshot(instance_id, request.snapshot_id)
        .await
        .map_err(|error| map_storage_error(error, "That snapshot no longer exists."))?;
    let expected_ref = format!("snapshots/{}", request.snapshot_id);
    if snapshot.manifest_ref.replace('\\', "/") == expected_ref {
        let _ = tokio::fs::remove_dir_all(
            instance
                .storage_path
                .join("snapshots")
                .join(request.snapshot_id.to_string()),
        )
        .await;
    }
    Ok(())
}

#[tauri::command]
pub(super) async fn instance_snapshot_set_pinned(
    state: tauri::State<'_, DesktopState>,
    request: SetInstanceSnapshotPinnedRequest,
) -> Result<InstanceSnapshotSummary, AppError> {
    state
        .database
        .set_instance_snapshot_pinned(
            InstanceId::from_uuid(request.id),
            request.snapshot_id,
            request.pinned,
        )
        .await
        .map(snapshot_summary)
        .map_err(|error| map_storage_error(error, "slate could not update that snapshot."))
}

#[tauri::command]
pub(super) async fn instance_set_favorite(
    state: tauri::State<'_, DesktopState>,
    request: SetFavoriteRequest,
) -> Result<InstanceSummary, AppError> {
    state
        .database
        .set_instance_favorite(
            InstanceId::from_uuid(request.id),
            request.favorite,
            request.expected_revision,
        )
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not update the favorite."))
}

#[tauri::command]
pub(super) async fn instance_trash(
    state: tauri::State<'_, DesktopState>,
    request: TrashInstanceRequest,
) -> Result<(), AppError> {
    refresh_exited_sessions(state.inner()).await;
    let instance_id = InstanceId::from_uuid(request.id);
    if state
        .processes
        .active_for_instance(instance_id)
        .map_err(process_state_error)?
        .is_some()
    {
        return Err(AppError::new(
            "local.instance_running",
            "Stop Minecraft before moving this instance to trash.",
        ));
    }
    state
        .database
        .trash_instance(instance_id, request.expected_revision)
        .await
        .map_err(|error| map_storage_error(error, "slate could not move the instance to trash."))
}
