use super::*;

pub(super) fn instance_files_error() -> AppError {
    AppError::new(
        "local.instance_files_unavailable",
        "slate could not safely copy or open those instance files.",
    )
}

pub(super) fn portable_instance_error() -> AppError {
    AppError::new(
        "local.instance_archive_invalid",
        "slate could not read or write that portable instance archive. The archive may be damaged, unsafe, or too large.",
    )
}

pub(super) async fn ensure_instance_stopped_and_current(
    state: &DesktopState,
    instance_id: InstanceId,
    expected_revision: u64,
) -> Result<InstanceRecord, AppError> {
    if state
        .processes
        .active_for_instance(instance_id)
        .map_err(process_state_error)?
        .is_some()
    {
        return Err(AppError::new(
            "local.instance_running",
            "Stop Minecraft before changing or copying its files.",
        ));
    }
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if instance.revision != expected_revision {
        return Err(AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ));
    }
    Ok(instance)
}

pub(super) fn snapshot_summary(record: InstanceSnapshotRecord) -> InstanceSnapshotSummary {
    InstanceSnapshotSummary {
        id: record.id,
        size_bytes: record.size_bytes,
        pinned: record.pinned,
        created_at: record.created_at,
    }
}

pub(super) async fn prune_instance_snapshots(state: &DesktopState, instance_id: InstanceId) {
    let Ok(instance) = state.database.get_instance(instance_id).await else {
        return;
    };
    let Ok(snapshots) = state.database.list_instance_snapshots(instance_id).await else {
        return;
    };
    let retention = usize::from(instance.settings.backup_retention);
    let mut unpinned_seen = 0_usize;
    for snapshot in snapshots {
        if snapshot.pinned {
            continue;
        }
        unpinned_seen += 1;
        if unpinned_seen <= retention {
            continue;
        }
        if state
            .database
            .delete_instance_snapshot(instance_id, snapshot.id)
            .await
            .is_ok()
        {
            let _ = tokio::fs::remove_dir_all(
                instance
                    .storage_path
                    .join("snapshots")
                    .join(snapshot.id.to_string()),
            )
            .await;
        }
    }
}

pub(super) async fn create_automatic_snapshot_if_enabled(
    state: &DesktopState,
    instance: &InstanceRecord,
) -> Result<(), AppError> {
    if !instance.settings.backup_before_changes {
        return Ok(());
    }
    let game_directory = instance.storage_path.join("game");
    let has_files = tauri::async_runtime::spawn_blocking(move || {
        game_directory
            .read_dir()
            .ok()
            .and_then(|mut entries| entries.next())
            .is_some()
    })
    .await
    .unwrap_or(false);
    if !has_files {
        return Ok(());
    }
    let snapshot_id = Uuid::new_v4();
    let mods = state
        .database
        .list_instance_mods(instance.id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not snapshot installed mods."))?;
    let instance_root = instance.storage_path.clone();
    let (manifest, size_bytes) =
        tauri::async_runtime::spawn_blocking(move || create_snapshot(&instance_root, snapshot_id))
            .await
            .map_err(|_| instance_files_error())?
            .map_err(|_| instance_files_error())?;
    let manifest_ref = manifest.to_string_lossy().replace('\\', "/");
    if let Err(error) = state
        .database
        .create_instance_snapshot(instance.id, snapshot_id, &manifest_ref, size_bytes, &mods)
        .await
    {
        let _ = tokio::fs::remove_dir_all(
            instance
                .storage_path
                .join("snapshots")
                .join(snapshot_id.to_string()),
        )
        .await;
        return Err(map_storage_error(
            error,
            "slate could not record the safety snapshot.",
        ));
    }
    prune_instance_snapshots(state, instance.id).await;
    Ok(())
}

pub(super) fn prune_instance_logs(instance_root: &std::path::Path, retention_days: u16) {
    let maximum_age = Duration::from_secs(u64::from(retention_days) * 24 * 60 * 60);
    let now = std::time::SystemTime::now();
    for directory in [
        instance_root.join("logs"),
        instance_root.join("game").join("logs"),
        instance_root.join("game").join("crash-reports"),
    ] {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                continue;
            }
            let extension = entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase);
            if !matches!(extension.as_deref(), Some("log" | "txt" | "gz")) {
                continue;
            }
            let expired = metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age > maximum_age);
            if expired {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

pub(super) fn instance_options_path(paths: &AppPaths, instance_id: InstanceId) -> PathBuf {
    paths.instance(instance_id).join("game").join("options.txt")
}

pub(super) fn current_rule_context(architecture: Architecture) -> RuleContext {
    RuleContext::new(
        if cfg!(target_os = "windows") {
            OperatingSystem::Windows
        } else if cfg!(target_os = "macos") {
            OperatingSystem::MacOs
        } else {
            OperatingSystem::Linux
        },
        architecture,
        std::env::consts::OS,
    )
}

pub(super) async fn refresh_exited_sessions(state: &DesktopState) {
    let Ok(exited) = state.processes.refresh() else {
        return;
    };
    for process in exited {
        let _ = state
            .database
            .finish_session(process.session_id, process.exit_code, process.force_stopped)
            .await;
    }
}

pub(super) async fn restore_window_after_session(
    state: DesktopState,
    window: tauri::WebviewWindow,
    session_id: SessionId,
) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        refresh_exited_sessions(&state).await;
        let running = state
            .processes
            .active_for_session(session_id)
            .ok()
            .flatten()
            .is_some();
        if !running {
            let _ = window.show();
            let _ = window.set_focus();
            break;
        }
    }
}
