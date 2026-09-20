use super::*;

#[tauri::command]
pub(super) async fn storage_overview(
    state: tauri::State<'_, DesktopState>,
) -> Result<StorageOverview, AppError> {
    let active = state
        .database
        .list_instances(1_000)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect instance storage."))?;
    let trashed = state
        .database
        .list_trashed_instances(1_000)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load trashed instances."))?;
    let active_roots = active
        .iter()
        .map(|instance| instance.storage_path.clone())
        .collect::<Vec<_>>();
    let trashed_roots = trashed
        .iter()
        .map(|instance| instance.storage_path.clone())
        .collect::<Vec<_>>();
    let paths = state.paths.clone();
    let (usage, measured_trash) = tauri::async_runtime::spawn_blocking(move || {
        let usage = scan_storage_usage(&paths, &active_roots, &trashed_roots)?;
        let measured = trashed_roots
            .iter()
            .map(|root| measure_path(root))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        Ok::<_, std::io::Error>((usage, measured))
    })
    .await
    .map_err(|_| storage_scan_error())?
    .map_err(|_| storage_scan_error())?;

    let trashed_instances = trashed
        .into_iter()
        .zip(measured_trash)
        .map(|(record, usage)| TrashedInstanceSummary {
            id: record.id.as_uuid(),
            name: record.name.to_string(),
            revision: record.revision,
            minecraft_version: record.minecraft_version,
            loader_kind: record.loader_kind.into(),
            loader_version: record.loader_version,
            source_name: record.source_name,
            icon_url: record.icon_url,
            size_bytes: usage.bytes,
            file_count: usage.files,
            files_present: record.storage_path.is_dir(),
            trashed_at: record.trashed_at,
        })
        .collect::<Vec<_>>();
    let trashed_size = trashed_instances
        .iter()
        .fold(0_u64, |total, item| total.saturating_add(item.size_bytes));
    let reclaimable_size_bytes = usage
        .temporary
        .bytes
        .saturating_add(usage.removed_content.bytes)
        .saturating_add(usage.logs.bytes)
        .saturating_add(trashed_size);
    Ok(StorageOverview {
        categories: vec![
            storage_category(StorageCategoryDto::Instances, usage.instances),
            storage_category(StorageCategoryDto::SharedGameFiles, usage.shared_game_files),
            storage_category(StorageCategoryDto::ManagedJava, usage.managed_java),
            storage_category(StorageCategoryDto::Logs, usage.logs),
            storage_category(StorageCategoryDto::TemporaryFiles, usage.temporary),
            storage_category(StorageCategoryDto::RemovedContent, usage.removed_content),
        ],
        total_size_bytes: usage.total.bytes,
        reclaimable_size_bytes,
        trashed_instances,
    })
}

#[tauri::command]
pub(super) async fn storage_clear_category(
    state: tauri::State<'_, DesktopState>,
    request: ClearStorageCategoryRequest,
) -> Result<StorageCleanupResult, AppError> {
    refresh_exited_sessions(state.inner()).await;
    let active_install = state
        .database
        .has_active_install_jobs()
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect active downloads."))?;
    if active_install && request.category != StorageCategoryDto::RemovedContent {
        return Err(AppError::new(
            "local.storage_busy",
            "Wait for active installations to finish before clearing these files.",
        ));
    }
    let active_processes = state.processes.active().map_err(process_state_error)?;
    if !active_processes.is_empty()
        && matches!(
            request.category,
            StorageCategoryDto::Logs
                | StorageCategoryDto::SharedGameFiles
                | StorageCategoryDto::ManagedJava
        )
    {
        return Err(AppError::new(
            "local.storage_in_use",
            "Stop every running Minecraft instance before clearing this storage category.",
        ));
    }
    if matches!(
        request.category,
        StorageCategoryDto::SharedGameFiles | StorageCategoryDto::ManagedJava
    ) && !request.confirm_managed_data
    {
        return Err(AppError::new(
            "validation.managed_storage_confirmation",
            "Confirm that affected instances will need repair before clearing these files.",
        ));
    }
    if request.category == StorageCategoryDto::Instances {
        return Err(AppError::new(
            "validation.storage_category",
            "Remove installed instances from the Library or permanently delete them from trash.",
        ));
    }
    if matches!(
        request.category,
        StorageCategoryDto::SharedGameFiles | StorageCategoryDto::ManagedJava
    ) {
        state
            .database
            .mark_active_instances_for_repair()
            .await
            .map_err(|error| {
                map_storage_error(error, "slate could not mark affected instances for repair.")
            })?;
    }
    let active_roots = state
        .database
        .list_instances(1_000)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect instance storage."))?
        .into_iter()
        .map(|instance| instance.storage_path)
        .collect::<Vec<_>>();
    let paths = state.paths.clone();
    let category = request.category;
    let usage = tauri::async_runtime::spawn_blocking(move || match category {
        StorageCategoryDto::Instances => unreachable!("instances were rejected above"),
        StorageCategoryDto::TemporaryFiles => clear_temporary_files(&paths, &active_roots),
        StorageCategoryDto::RemovedContent => clear_removed_content(&paths),
        StorageCategoryDto::Logs => clear_logs(&paths, &active_roots),
        StorageCategoryDto::SharedGameFiles => clear_shared_game_files(&paths),
        StorageCategoryDto::ManagedJava => clear_managed_java(&paths),
    })
    .await
    .map_err(|_| storage_cleanup_error())?
    .map_err(|_| storage_cleanup_error())?;
    Ok(StorageCleanupResult {
        reclaimed_bytes: usage.bytes,
        removed_files: usage.files,
    })
}

#[tauri::command]
pub(super) async fn trashed_instance_restore(
    state: tauri::State<'_, DesktopState>,
    request: RestoreTrashedInstanceRequest,
) -> Result<InstanceSummary, AppError> {
    state
        .database
        .restore_trashed_instance(InstanceId::from_uuid(request.id), request.expected_revision)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not restore that instance."))
}

#[tauri::command]
pub(super) async fn trashed_instance_delete(
    state: tauri::State<'_, DesktopState>,
    request: DeleteTrashedInstanceRequest,
) -> Result<StorageCleanupResult, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let record = state
        .database
        .get_trashed_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "That trashed instance no longer exists."))?;
    if request.confirmation_name.trim() != record.name.as_str() {
        return Err(AppError::new(
            "validation.instance_delete_confirmation",
            "Enter the exact instance name to permanently delete it.",
        ));
    }
    if record.revision != request.expected_revision {
        return Err(AppError::new(
            "local.instance_changed",
            "That trashed instance changed. Refresh storage and try again.",
        ));
    }
    let usage = tauri::async_runtime::spawn_blocking({
        let root = record.storage_path.clone();
        move || measure_path(&root)
    })
    .await
    .map_err(|_| storage_scan_error())?
    .map_err(|_| storage_scan_error())?;
    let staged = tauri::async_runtime::spawn_blocking({
        let root = record.storage_path.clone();
        move || stage_instance_deletion(&root, instance_id)
    })
    .await
    .map_err(|_| storage_cleanup_error())?
    .map_err(|_| storage_cleanup_error())?;
    if let Err(error) = state
        .database
        .permanently_delete_trashed_instance_record(instance_id, request.expected_revision)
        .await
    {
        rollback_staged_deletions(staged.into_iter().collect()).await;
        return Err(map_storage_error(
            error,
            "slate could not delete that trashed instance.",
        ));
    }
    purge_staged_deletions(staged.into_iter().collect()).await;
    Ok(StorageCleanupResult {
        reclaimed_bytes: usage.bytes,
        removed_files: usage.files,
    })
}

#[tauri::command]
pub(super) async fn trashed_instances_empty(
    state: tauri::State<'_, DesktopState>,
    request: EmptyInstanceTrashRequest,
) -> Result<StorageCleanupResult, AppError> {
    let records = state
        .database
        .list_trashed_instances(1_000)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load trashed instances."))?;
    if usize::try_from(request.expected_count).ok() != Some(records.len()) {
        return Err(AppError::new(
            "local.trash_changed",
            "The trash changed. Refresh storage before permanently deleting its contents.",
        ));
    }
    let staged_result = tauri::async_runtime::spawn_blocking({
        let records = records.clone();
        move || stage_trashed_instances(&records)
    })
    .await
    .map_err(|_| storage_cleanup_error())?;
    let (staged, usage) = staged_result.map_err(|_| storage_cleanup_error())?;
    let revisions = records
        .iter()
        .map(|record| (record.id, record.revision))
        .collect::<Vec<_>>();
    if let Err(error) = state
        .database
        .permanently_delete_trashed_instance_records(&revisions)
        .await
    {
        rollback_staged_deletions(staged).await;
        return Err(map_storage_error(
            error,
            "slate could not empty the instance trash.",
        ));
    }
    purge_staged_deletions(staged).await;
    Ok(StorageCleanupResult {
        reclaimed_bytes: usage.bytes,
        removed_files: usage.files,
    })
}
