use super::*;

pub(super) fn instance_summary(record: slate_storage::InstanceRecord) -> InstanceSummary {
    let settings = record.settings.clone();
    InstanceSummary {
        id: record.id.as_uuid(),
        name: record.name.to_string(),
        storage_path: record.storage_path.to_string_lossy().into_owned(),
        mode: record.mode.into(),
        management_mode: record.management_mode.into(),
        favorite: record.favorite,
        revision: record.revision,
        minecraft_version: record.minecraft_version,
        loader_kind: record.loader_kind.into(),
        loader_version: record.loader_version,
        memory_mb: record.memory_mb,
        mod_count: record.mod_count,
        setup_state: record.setup_state.into(),
        settings: InstanceSettingsSummary {
            description: settings.description,
            notes: settings.notes,
            group_name: settings.group_name,
            tags: settings.tags,
            has_custom_icon: settings.icon_mime.is_some(),
            has_custom_banner: settings.banner_mime.is_some(),
            banner_position_x: settings.banner_position_x,
            banner_position_y: settings.banner_position_y,
            preferred_account_id: settings.preferred_account_id.map(|id| id.as_uuid()),
            window_mode: window_mode_dto(settings.window_mode),
            resolution_width: settings.resolution_width,
            resolution_height: settings.resolution_height,
            launcher_behavior: launcher_behavior_dto(settings.launcher_behavior),
            game_language: settings.game_language,
            quick_play_server: settings.quick_play_server,
            process_priority: process_priority_dto(settings.process_priority),
            cpu_affinity: settings.cpu_affinity,
            memory_mode: memory_mode_dto(settings.memory_mode),
            initial_memory_mb: settings.initial_memory_mb,
            effective_memory_mb: record.memory_mb,
            java_mode: java_mode_dto(settings.java_mode),
            custom_java_label: settings.custom_java_label,
            performance_preset: performance_preset_dto(settings.performance_preset),
            jvm_arguments: settings.jvm_arguments,
            environment: settings.environment,
            backup_before_changes: settings.backup_before_changes,
            backup_retention: settings.backup_retention,
            log_retention_days: settings.log_retention_days,
        },
        modpack_source: record.modpack_source.map(|source| ModpackSourceSummary {
            provider: source.provider,
            project_id: source.project_id,
            version_id: source.version_id,
            display_name: source.display_name,
            icon_url: source.icon_url,
            banner_url: source.banner_url,
        }),
        created_at: record.created_at,
        updated_at: record.updated_at,
        last_played: record.last_played,
    }
}

pub(super) const fn window_mode_dto(value: InstanceWindowMode) -> InstanceWindowModeDto {
    match value {
        InstanceWindowMode::Windowed => InstanceWindowModeDto::Windowed,
        InstanceWindowMode::Maximized => InstanceWindowModeDto::Maximized,
        InstanceWindowMode::Fullscreen => InstanceWindowModeDto::Fullscreen,
    }
}

pub(super) const fn launcher_behavior_dto(value: LauncherBehavior) -> LauncherBehaviorDto {
    match value {
        LauncherBehavior::KeepOpen => LauncherBehaviorDto::KeepOpen,
        LauncherBehavior::Minimize => LauncherBehaviorDto::Minimize,
        LauncherBehavior::Hide => LauncherBehaviorDto::Hide,
    }
}

pub(super) const fn process_priority_dto(value: ProcessPriority) -> ProcessPriorityDto {
    match value {
        ProcessPriority::Low => ProcessPriorityDto::Low,
        ProcessPriority::BelowNormal => ProcessPriorityDto::BelowNormal,
        ProcessPriority::Normal => ProcessPriorityDto::Normal,
        ProcessPriority::AboveNormal => ProcessPriorityDto::AboveNormal,
        ProcessPriority::High => ProcessPriorityDto::High,
    }
}

pub(super) const fn memory_mode_dto(value: MemoryMode) -> MemoryModeDto {
    match value {
        MemoryMode::Auto => MemoryModeDto::Auto,
        MemoryMode::Custom => MemoryModeDto::Custom,
    }
}

const fn java_mode_dto(value: JavaSelectionMode) -> JavaSelectionModeDto {
    match value {
        JavaSelectionMode::Managed => JavaSelectionModeDto::Managed,
        JavaSelectionMode::Detected => JavaSelectionModeDto::Detected,
        JavaSelectionMode::Custom => JavaSelectionModeDto::Custom,
    }
}

pub(super) const fn performance_preset_dto(value: PerformancePreset) -> PerformancePresetDto {
    match value {
        PerformancePreset::Balanced => PerformancePresetDto::Balanced,
        PerformancePreset::Throughput => PerformancePresetDto::Throughput,
        PerformancePreset::LowLatency => PerformancePresetDto::LowLatency,
        PerformancePreset::Custom => PerformancePresetDto::Custom,
    }
}

pub(super) fn instance_mod_summary(
    record: Option<InstanceModRecord>,
    file: InstanceModFile,
    untracked_origin: InstanceModOriginDto,
) -> InstanceModSummary {
    match record {
        Some(record) => InstanceModSummary {
            provider: Some(record.provider),
            project_id: Some(record.project_id),
            version_id: Some(record.version_id),
            display_name: record.display_name,
            file_path: file.file_path,
            enabled: file.enabled,
            pinned: record.pinned,
            installed_at: Some(record.installed_at),
            origin: InstanceModOriginDto::Added,
            file_size: file.size,
            icon_url: None,
        },
        None => InstanceModSummary {
            provider: None,
            project_id: None,
            version_id: None,
            display_name: file.display_name,
            file_path: file.file_path,
            enabled: file.enabled,
            pinned: false,
            installed_at: file.modified_at,
            origin: untracked_origin,
            file_size: file.size,
            icon_url: None,
        },
    }
}

pub(super) const fn content_kind(kind: InstanceContentKindDto) -> InstanceContentKind {
    match kind {
        InstanceContentKindDto::ResourcePack => InstanceContentKind::Resource,
        InstanceContentKindDto::ShaderPack => InstanceContentKind::Shader,
        InstanceContentKindDto::DataPack => InstanceContentKind::Data,
    }
}

pub(super) const fn pack_file_matches_content(
    file_kind: PackFileType,
    content_kind: InstanceContentKindDto,
) -> bool {
    matches!(
        (file_kind, content_kind),
        (
            PackFileType::ResourcePack,
            InstanceContentKindDto::ResourcePack
        ) | (PackFileType::ShaderPack, InstanceContentKindDto::ShaderPack)
            | (PackFileType::DataPack, InstanceContentKindDto::DataPack)
    )
}

pub(super) fn content_file_summary(
    file: InstanceContentFile,
    kind: InstanceContentKindDto,
    managed_paths: &HashSet<String>,
) -> InstanceContentFileSummary {
    let origin = if managed_paths.contains(&normalized_content_path(&file.file_path)) {
        InstanceModOriginDto::Modpack
    } else {
        InstanceModOriginDto::Local
    };
    InstanceContentFileSummary {
        display_name: file.display_name,
        file_path: file.file_path,
        kind,
        enabled: file.enabled,
        can_toggle: file.can_toggle,
        origin,
        file_size: file.size,
        modified_at: file.modified_at,
        world_name: file.world_name,
    }
}

pub(super) fn normalized_content_path(value: &str) -> String {
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    normalized
        .strip_suffix(".disabled")
        .unwrap_or(&normalized)
        .to_owned()
}

pub(super) fn preferences_dto(value: AppPreferences) -> AppPreferencesDto {
    AppPreferencesDto {
        theme: match value.theme {
            ThemePreference::Dark => ThemePreferenceDto::Dark,
            ThemePreference::Light => ThemePreferenceDto::Light,
            ThemePreference::System => ThemePreferenceDto::System,
        },
        download_concurrency: value.download_concurrency,
        telemetry_enabled: value.telemetry_enabled,
        reduce_motion: match value.reduce_motion {
            ReduceMotionPreference::System => ReduceMotionPreferenceDto::System,
            ReduceMotionPreference::On => ReduceMotionPreferenceDto::On,
            ReduceMotionPreference::Off => ReduceMotionPreferenceDto::Off,
        },
        trash_retention_days: value.trash_retention_days,
    }
}
