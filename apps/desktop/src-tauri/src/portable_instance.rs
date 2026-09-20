use super::*;

pub(super) fn paths_for_instance(state: &DesktopState, instance: &InstanceRecord) -> AppPaths {
    state
        .paths
        .with_instance_root(instance.id, instance.storage_path.clone())
}

pub(super) fn portable_manifest(instance: &InstanceRecord) -> PortableInstanceManifest {
    let settings = &instance.settings;
    PortableInstanceManifest {
        schema: 1,
        exported_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "unknown".to_owned()),
        name: instance.name.to_string(),
        mode: instance.mode.into(),
        minecraft_version: instance.minecraft_version.clone(),
        loader_kind: instance.loader_kind.into(),
        loader_version: instance.loader_version.clone(),
        memory_mb: instance.memory_mb,
        settings: PortableInstanceSettings {
            description: settings.description.clone(),
            notes: settings.notes.clone(),
            group_name: settings.group_name.clone(),
            tags: settings.tags.clone(),
            icon_mime: settings.icon_mime.clone(),
            banner_mime: settings.banner_mime.clone(),
            banner_position_x: settings.banner_position_x,
            banner_position_y: settings.banner_position_y,
            window_mode: window_mode_dto(settings.window_mode),
            resolution_width: settings.resolution_width,
            resolution_height: settings.resolution_height,
            launcher_behavior: launcher_behavior_dto(settings.launcher_behavior),
            game_language: settings.game_language.clone(),
            quick_play_server: settings.quick_play_server.clone(),
            process_priority: process_priority_dto(settings.process_priority),
            cpu_affinity: settings.cpu_affinity.clone(),
            memory_mode: memory_mode_dto(settings.memory_mode),
            initial_memory_mb: settings.initial_memory_mb,
            performance_preset: performance_preset_dto(settings.performance_preset),
            jvm_arguments: settings.jvm_arguments.clone(),
            environment: settings.environment.clone(),
            backup_before_changes: settings.backup_before_changes,
            backup_retention: settings.backup_retention,
            log_retention_days: settings.log_retention_days,
        },
        modpack_source: instance
            .modpack_source
            .as_ref()
            .map(|source| PortableModpackSource {
                provider: source.provider,
                project_id: source.project_id.clone(),
                version_id: source.version_id.clone(),
                selected_optional: source.selected_optional.clone(),
                display_name: source.display_name.clone(),
                icon_url: source.icon_url.clone(),
                banner_url: source.banner_url.clone(),
            }),
    }
}

pub(super) fn portable_settings_request(
    instance: &InstanceRecord,
    settings: PortableInstanceSettings,
) -> UpdateInstanceSettingsRequest {
    UpdateInstanceSettingsRequest {
        id: instance.id.as_uuid(),
        description: settings.description,
        notes: settings.notes,
        group_name: settings.group_name,
        tags: settings.tags,
        preferred_account_id: None,
        banner_position_x: settings.banner_position_x,
        banner_position_y: settings.banner_position_y,
        window_mode: settings.window_mode,
        resolution_width: settings.resolution_width,
        resolution_height: settings.resolution_height,
        launcher_behavior: settings.launcher_behavior,
        game_language: settings.game_language,
        quick_play_server: settings.quick_play_server,
        process_priority: settings.process_priority,
        cpu_affinity: settings.cpu_affinity,
        memory_mode: settings.memory_mode,
        initial_memory_mb: settings.initial_memory_mb,
        maximum_memory_mb: instance.memory_mb,
        java_mode: JavaSelectionModeDto::Managed,
        performance_preset: settings.performance_preset,
        jvm_arguments: settings.jvm_arguments,
        environment: settings.environment,
        backup_before_changes: settings.backup_before_changes,
        backup_retention: settings.backup_retention,
        log_retention_days: settings.log_retention_days,
        expected_revision: instance.revision,
    }
}

pub(super) fn validated_portable_modpack_source(
    source: Option<PortableModpackSource>,
) -> Result<Option<NewModpackSource>, AppError> {
    let Some(source) = source else {
        return Ok(None);
    };
    if source.project_id.trim().is_empty()
        || source.project_id.len() > 256
        || source.version_id.trim().is_empty()
        || source.version_id.len() > 256
        || source.display_name.trim().is_empty()
        || source.display_name.len() > 256
        || source.selected_optional.len() > 1_000
        || source
            .icon_url
            .as_ref()
            .is_some_and(|value| value.len() > 2_048)
        || source
            .banner_url
            .as_ref()
            .is_some_and(|value| value.len() > 2_048)
    {
        return Err(portable_instance_error());
    }
    Ok(Some(NewModpackSource {
        provider: source.provider,
        project_id: source.project_id.trim().to_owned(),
        version_id: source.version_id.trim().to_owned(),
        selected_optional: source.selected_optional,
        display_name: source.display_name.trim().to_owned(),
        icon_url: source.icon_url,
        banner_url: source.banner_url,
    }))
}

pub(super) async fn restore_imported_artwork(
    state: &DesktopState,
    mut instance: InstanceRecord,
    settings: &PortableInstanceSettings,
) -> Result<InstanceRecord, AppError> {
    for (filename, column, declared_mime, maximum) in [
        (
            "profile-icon.bin",
            "icon_mime",
            settings.icon_mime.as_deref(),
            2 * 1024 * 1024,
        ),
        (
            "profile-banner.bin",
            "banner_mime",
            settings.banner_mime.as_deref(),
            8 * 1024 * 1024,
        ),
    ] {
        let Some(declared_mime) = declared_mime else {
            continue;
        };
        let path = instance.storage_path.join("metadata").join(filename);
        let Ok(bytes) = tokio::fs::read(&path).await else {
            continue;
        };
        if bytes.len() > maximum || image_mime(&bytes) != Some(declared_mime) {
            let _ = tokio::fs::remove_file(path).await;
            continue;
        }
        state
            .database
            .set_instance_artwork_mime(instance.id, instance.revision, column, Some(declared_mime))
            .await
            .map_err(|error| {
                map_storage_error(error, "slate could not restore imported artwork.")
            })?;
        instance = state
            .database
            .get_instance(instance.id)
            .await
            .map_err(|error| {
                map_storage_error(error, "slate could not reload imported artwork.")
            })?;
    }
    Ok(instance)
}

pub(super) async fn cleanup_failed_import(state: &DesktopState, instance: &InstanceRecord) {
    if let Ok(current) = state.database.get_instance(instance.id).await {
        let _ = state
            .database
            .trash_instance(current.id, current.revision)
            .await;
    }
    let _ = tokio::fs::remove_dir_all(&instance.storage_path).await;
}

pub(super) fn portable_filename(name: &str) -> String {
    let value = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value.trim().trim_end_matches(['.', ' ']);
    if value.is_empty() {
        "instance".to_owned()
    } else {
        value.to_owned()
    }
}
