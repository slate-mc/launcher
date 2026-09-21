use super::*;

#[tauri::command]
pub(super) async fn instances_list(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<InstanceSummary>, AppError> {
    state
        .database
        .list_instances(250)
        .await
        .map(|records| records.into_iter().map(instance_summary).collect())
        .map_err(|error| map_storage_error(error, "slate could not load your instances."))
}

#[tauri::command]
pub(super) async fn instance_get(
    state: tauri::State<'_, DesktopState>,
    id: Uuid,
) -> Result<InstanceSummary, AppError> {
    state
        .database
        .get_instance(InstanceId::from_uuid(id))
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))
}

#[tauri::command]
pub(super) async fn instance_create(
    state: tauri::State<'_, DesktopState>,
    request: CreateInstanceRequest,
) -> Result<InstanceSummary, AppError> {
    let loader_version = validate_selected_loader_version(
        &request.minecraft_version,
        request.loader_kind,
        request.loader_version.as_deref(),
    )
    .await?;
    validate_instance_configuration(
        request.mode,
        &request.minecraft_version,
        request.loader_kind,
        loader_version.as_deref(),
        request.memory_mb,
    )
    .map_err(configuration_app_error)?;
    let name = parse_instance_name(&request.name).map_err(instance_name_app_error)?;
    let root_id = preferred_storage_root_id(state.inner()).await?;
    let record = state
        .database
        .create_instance(NewInstance {
            name,
            mode: request.mode.into(),
            management_mode: ManagementMode::Local,
            root_id,
            minecraft_version: request.minecraft_version.trim().to_owned(),
            loader_kind: request.loader_kind.into(),
            loader_version,
            memory_mb: request.memory_mb,
            modpack_source: None,
        })
        .await
        .map_err(|error| map_storage_error(error, "slate could not create the instance."))?;

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

    let _ = state
        .telemetry
        .capture(
            slate_modpack_api_contracts::ProductEvent::InstanceCreated,
            Some(record.loader_kind.as_storage_value()),
            None,
        )
        .await;

    Ok(instance_summary(record))
}

#[tauri::command]
pub(super) async fn instance_rename(
    state: tauri::State<'_, DesktopState>,
    request: RenameInstanceRequest,
) -> Result<InstanceSummary, AppError> {
    let name = parse_instance_name(&request.name).map_err(instance_name_app_error)?;
    state
        .database
        .rename_instance(
            InstanceId::from_uuid(request.id),
            &name,
            request.expected_revision,
        )
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not rename the instance."))
}

#[tauri::command]
pub(super) async fn instance_update_configuration(
    state: tauri::State<'_, DesktopState>,
    request: UpdateInstanceConfigurationRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let current = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let requested_loader_kind: LoaderFamily = request.loader_kind.into();
    let requested_loader_version = request
        .loader_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let runtime_unchanged = request.minecraft_version.trim() == current.minecraft_version
        && requested_loader_kind == current.loader_kind
        && requested_loader_version == current.loader_version.as_deref();
    let loader_version = if runtime_unchanged {
        current.loader_version.clone()
    } else {
        validate_selected_loader_version(
            &request.minecraft_version,
            request.loader_kind,
            request.loader_version.as_deref(),
        )
        .await?
    };
    validate_instance_configuration(
        current.mode.into(),
        &request.minecraft_version,
        request.loader_kind,
        loader_version.as_deref(),
        request.memory_mb,
    )
    .map_err(configuration_app_error)?;
    if !runtime_unchanged {
        ensure_instance_stopped_and_current(state.inner(), instance_id, request.expected_revision)
            .await?;
        create_automatic_snapshot_if_enabled(state.inner(), &current).await?;
    }

    state
        .database
        .update_instance_configuration(
            instance_id,
            &request.minecraft_version,
            request.loader_kind.into(),
            loader_version.as_deref(),
            request.memory_mb,
            request.expected_revision,
        )
        .await
        .map(instance_summary)
        .map_err(|error| {
            map_storage_error(error, "slate could not update the instance configuration.")
        })
}

#[tauri::command]
pub(super) async fn instance_update_settings(
    state: tauri::State<'_, DesktopState>,
    request: UpdateInstanceSettingsRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let expected_revision = request.expected_revision;
    let current = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if current.revision != expected_revision {
        return Err(AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ));
    }
    if let Some(account_id) = request.preferred_account_id {
        state
            .database
            .get_account(AccountId::from_uuid(account_id))
            .await
            .map_err(|error| map_storage_error(error, "Choose a connected Minecraft account."))?;
    }
    let settings = validated_instance_settings(&current, request)?;
    state
        .database
        .update_instance_settings(instance_id, expected_revision, settings)
        .await
        .map_err(|error| map_storage_error(error, "slate could not save the instance settings."))?;
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| {
            map_storage_error(error, "slate saved the settings but could not reload them.")
        })
}

#[tauri::command]
pub(super) async fn instance_select_java(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: SelectInstanceJavaRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if instance.revision != request.expected_revision {
        return Err(AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ));
    }
    let (mode, path, label) = match request.mode {
        JavaSelectionModeDto::Managed => (JavaSelectionMode::Managed, None, None),
        JavaSelectionModeDto::Detected => {
            let probe = tauri::async_runtime::spawn_blocking(detect_java_runtime)
                .await
                .map_err(|_| java_selection_error())?;
            validated_java_selection(&instance, probe, JavaSelectionMode::Detected)?
        }
        JavaSelectionModeDto::Custom => {
            let picked = tauri::async_runtime::spawn_blocking(move || {
                app.dialog()
                    .file()
                    .set_title("Choose a Java executable")
                    .add_filter("Java executable", &["exe"])
                    .blocking_pick_file()
            })
            .await
            .map_err(|_| java_selection_error())?;
            let Some(path) = picked.and_then(|path| path.into_path().ok()) else {
                return Ok(instance_summary(instance));
            };
            let probe_path = path.clone();
            let probe =
                tauri::async_runtime::spawn_blocking(move || probe_java_executable(&probe_path))
                    .await
                    .map_err(|_| java_selection_error())?;
            validated_java_selection(&instance, probe, JavaSelectionMode::Custom)?
        }
    };
    state
        .database
        .set_instance_java_selection(
            instance_id,
            request.expected_revision,
            mode,
            path.as_deref(),
            label.as_deref(),
        )
        .await
        .map_err(|error| map_storage_error(error, "slate could not save the Java selection."))?;
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not reload the Java selection."))
}

#[tauri::command]
pub(super) async fn instance_select_artwork(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: SelectInstanceArtworkRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let current = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if current.revision != request.expected_revision {
        return Err(AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ));
    }
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Choose instance artwork")
            .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| artwork_error())?;
    let Some(source) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(instance_summary(current));
    };
    let kind = request.kind;
    let instance_paths = paths_for_instance(state.inner(), &current);
    let destination = instance_artwork_path(&instance_paths, instance_id, kind);
    let maximum = match kind {
        InstanceArtworkKindDto::Icon => 2 * 1024 * 1024,
        InstanceArtworkKindDto::Banner => 8 * 1024 * 1024,
    };
    let source_for_read = source.clone();
    let (bytes, mime) = tauri::async_runtime::spawn_blocking(move || {
        let bytes = std::fs::read(source_for_read)?;
        if bytes.len() > maximum {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "image is too large",
            ));
        }
        let mime = image_mime(&bytes).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "unsupported image")
        })?;
        Ok::<_, std::io::Error>((bytes, mime))
    })
    .await
    .map_err(|_| artwork_error())?
    .map_err(|_| artwork_error())?;
    let replacement = replace_managed_file(destination, bytes).await?;
    let column = match kind {
        InstanceArtworkKindDto::Icon => "icon_mime",
        InstanceArtworkKindDto::Banner => "banner_mime",
    };
    if let Err(error) = state
        .database
        .set_instance_artwork_mime(instance_id, request.expected_revision, column, Some(mime))
        .await
    {
        replacement.rollback().await;
        return Err(map_storage_error(
            error,
            "slate could not save the artwork.",
        ));
    }
    replacement.commit().await;
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not reload the artwork."))
}

#[tauri::command]
pub(super) async fn instance_reset_artwork(
    state: tauri::State<'_, DesktopState>,
    request: SelectInstanceArtworkRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let current = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let instance_paths = paths_for_instance(state.inner(), &current);
    let destination = instance_artwork_path(&instance_paths, instance_id, request.kind);
    let replacement = remove_managed_file(destination).await?;
    let column = match request.kind {
        InstanceArtworkKindDto::Icon => "icon_mime",
        InstanceArtworkKindDto::Banner => "banner_mime",
    };
    if let Err(error) = state
        .database
        .set_instance_artwork_mime(instance_id, request.expected_revision, column, None)
        .await
    {
        replacement.rollback().await;
        return Err(map_storage_error(
            error,
            "slate could not reset the artwork.",
        ));
    }
    replacement.commit().await;
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not reload the artwork."))
}

#[tauri::command]
pub(super) async fn instance_get_artwork(
    state: tauri::State<'_, DesktopState>,
    request: GetInstanceArtworkRequest,
) -> Result<Option<InstanceArtworkAsset>, AppError> {
    let instance = state
        .database
        .get_instance(InstanceId::from_uuid(request.id))
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let mime = match request.kind {
        InstanceArtworkKindDto::Icon => instance.settings.icon_mime.clone(),
        InstanceArtworkKindDto::Banner => instance.settings.banner_mime.clone(),
    };
    let Some(mime_type) = mime else {
        return Ok(None);
    };
    let instance_paths = paths_for_instance(state.inner(), &instance);
    let path = instance_artwork_path(&instance_paths, instance.id, request.kind);
    let bytes = tokio::fs::read(path).await.map_err(|_| artwork_error())?;
    Ok(Some(InstanceArtworkAsset {
        mime_type,
        data_base64: BASE64_STANDARD.encode(bytes),
    }))
}

#[tauri::command]
pub(super) async fn instance_game_options_get(
    state: tauri::State<'_, DesktopState>,
    request: GetInstanceGameOptionsRequest,
) -> Result<InstanceGameOptionsSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let instance_paths = paths_for_instance(state.inner(), &instance);
    let (file_exists, values) =
        read_recognized_options(&instance_options_path(&instance_paths, instance_id))
            .await
            .map_err(|_| game_options_error())?;
    Ok(InstanceGameOptionsSummary {
        file_exists,
        values,
    })
}

#[tauri::command]
pub(super) async fn instance_game_options_update(
    state: tauri::State<'_, DesktopState>,
    request: UpdateInstanceGameOptionsRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    if state
        .processes
        .active_for_instance(instance_id)
        .map_err(process_state_error)?
        .is_some()
    {
        return Err(AppError::new(
            "local.instance_running",
            "Stop Minecraft before changing its game configuration.",
        ));
    }
    let current = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if current.revision != request.expected_revision {
        return Err(AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ));
    }
    let instance_paths = paths_for_instance(state.inner(), &current);
    let pending = prepare_options_update(
        instance_options_path(&instance_paths, instance_id),
        &request.values,
    )
    .await
    .map_err(|_| game_options_error())?;
    if let Err(error) = state
        .database
        .advance_instance_revision(instance_id, request.expected_revision)
        .await
    {
        pending.rollback().await;
        return Err(map_storage_error(
            error,
            "slate could not record the game configuration change.",
        ));
    }
    pending.commit().await;
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not reload that instance."))
}

#[tauri::command]
pub(super) async fn instance_open_directory(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: OpenInstanceDirectoryRequest,
) -> Result<(), AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let game = instance.storage_path.join("game");
    let path = match request.kind {
        InstanceDirectoryKindDto::Game => game,
        InstanceDirectoryKindDto::Mods => game.join("mods"),
        InstanceDirectoryKindDto::Logs => game.join("logs"),
        InstanceDirectoryKindDto::Screenshots => game.join("screenshots"),
        InstanceDirectoryKindDto::Saves => game.join("saves"),
        InstanceDirectoryKindDto::CrashReports => game.join("crash-reports"),
    };
    tokio::fs::create_dir_all(&path)
        .await
        .map_err(|_| instance_files_error())?;
    let path = path.to_str().ok_or_else(instance_files_error)?.to_owned();
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|_| instance_files_error())
}
