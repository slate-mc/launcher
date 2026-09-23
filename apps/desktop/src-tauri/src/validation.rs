use super::*;

pub(super) fn parse_instance_name(value: &str) -> Result<InstanceName, InstanceNameError> {
    InstanceName::parse(value)
}

pub(super) fn validate_instance_configuration(
    mode: InstanceModeDto,
    minecraft_version: &str,
    loader_kind: LoaderKindDto,
    loader_version: Option<&str>,
    memory_mb: u32,
) -> Result<(), ConfigurationValidationError> {
    let version = minecraft_version.trim();
    if version.is_empty()
        || version.len() > 64
        || version
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || ".-_+".contains(character)))
    {
        return Err(ConfigurationValidationError::MinecraftVersion);
    }
    if mode == InstanceModeDto::Vanilla && loader_kind != LoaderKindDto::Vanilla {
        return Err(ConfigurationValidationError::VanillaLoader);
    }
    if matches!(mode, InstanceModeDto::Modded | InstanceModeDto::SlateClient)
        && loader_kind == LoaderKindDto::Vanilla
    {
        return Err(ConfigurationValidationError::ModdedLoader);
    }
    if mode == InstanceModeDto::SlateClient
        && (version != "1.21.1"
            || loader_kind != LoaderKindDto::Fabric
            || loader_version.map(str::trim) != Some("0.19.5"))
    {
        return Err(ConfigurationValidationError::UnsupportedSlateClientTarget);
    }
    let loader_version = loader_version
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if loader_kind != LoaderKindDto::Vanilla && loader_version.is_none_or(|value| value.len() > 64)
    {
        return Err(ConfigurationValidationError::LoaderVersionRequired);
    }
    if loader_kind == LoaderKindDto::Vanilla && loader_version.is_some() {
        return Err(ConfigurationValidationError::VanillaLoaderVersion);
    }
    if !(1024..=32768).contains(&memory_mb) {
        return Err(ConfigurationValidationError::Memory);
    }
    Ok(())
}

pub(super) fn validated_instance_settings(
    instance: &InstanceRecord,
    request: UpdateInstanceSettingsRequest,
) -> Result<UpdateInstanceSettings, AppError> {
    let description = validated_free_text("description", request.description, 500)?;
    let notes = validated_free_text("notes", request.notes, 4_000)?;
    let group_name = validated_optional_text("groupName", request.group_name, 80)?;
    let mut tags = Vec::with_capacity(request.tags.len());
    let mut seen_tags = HashSet::new();
    if request.tags.len() > 20 {
        return Err(settings_validation_error(
            "tags",
            "Use no more than 20 tags.",
        ));
    }
    for raw_tag in request.tags {
        let tag = raw_tag.trim();
        if tag.is_empty()
            || tag.chars().count() > 32
            || tag.chars().any(|character| character.is_control())
        {
            return Err(settings_validation_error(
                "tags",
                "Each tag must contain 1 through 32 visible characters.",
            ));
        }
        let identity = tag.to_lowercase();
        if seen_tags.insert(identity) {
            tags.push(tag.to_owned());
        }
    }

    let (resolution_width, resolution_height) = match (
        request.resolution_width,
        request.resolution_height,
    ) {
        (None, None) => (None, None),
        (Some(width), Some(height))
            if (320..=16_384).contains(&width) && (240..=16_384).contains(&height) =>
        {
            (Some(width), Some(height))
        }
        _ => {
            return Err(settings_validation_error(
                "resolutionWidth",
                "Set both dimensions using a width from 320 through 16384 and a height from 240 through 16384.",
            ));
        }
    };
    let game_language = request.game_language.trim().to_ascii_lowercase();
    if game_language.len() < 2
        || game_language.len() > 32
        || game_language.chars().any(|character| {
            !(character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_')
        })
    {
        return Err(settings_validation_error(
            "gameLanguage",
            "Use a Minecraft language code such as en_us.",
        ));
    }
    let quick_play_server = validated_optional_server(request.quick_play_server)?;
    let mut cpu_affinity = request.cpu_affinity;
    cpu_affinity.sort_unstable();
    cpu_affinity.dedup();
    let logical_cpu_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(64);
    if cpu_affinity.len() > 64
        || cpu_affinity
            .iter()
            .any(|cpu| usize::from(*cpu) >= logical_cpu_count)
    {
        return Err(settings_validation_error(
            "cpuAffinity",
            "Choose logical CPU indices available on this computer, or leave the field empty to use every CPU.",
        ));
    }

    if !(256..=32_768).contains(&request.initial_memory_mb) {
        return Err(settings_validation_error(
            "initialMemoryMb",
            "Choose 256 through 32768 MB.",
        ));
    }
    let maximum_memory_mb = match request.memory_mode {
        MemoryModeDto::Auto => recommended_memory_mb(instance),
        MemoryModeDto::Custom if (1_024..=32_768).contains(&request.maximum_memory_mb) => {
            request.maximum_memory_mb
        }
        MemoryModeDto::Custom => {
            return Err(settings_validation_error(
                "maximumMemoryMb",
                "Choose 1024 through 32768 MB.",
            ));
        }
    };
    if request.initial_memory_mb > maximum_memory_mb {
        return Err(settings_validation_error(
            "initialMemoryMb",
            "Initial memory cannot exceed maximum memory.",
        ));
    }

    if request.java_mode != JavaSelectionModeDto::Managed
        && instance.settings.custom_java_path.is_none()
    {
        return Err(settings_validation_error(
            "javaMode",
            "Choose and validate a Java executable first.",
        ));
    }
    let jvm_arguments = validated_jvm_arguments(request.jvm_arguments)?;
    let environment = validated_environment(request.environment)?;
    if !(1..=50).contains(&request.backup_retention) {
        return Err(settings_validation_error(
            "backupRetention",
            "Keep between 1 and 50 snapshots.",
        ));
    }
    if !(1..=365).contains(&request.log_retention_days) {
        return Err(settings_validation_error(
            "logRetentionDays",
            "Keep logs for between 1 and 365 days.",
        ));
    }

    Ok(UpdateInstanceSettings {
        description,
        notes,
        group_name,
        tags,
        preferred_account_id: request.preferred_account_id.map(AccountId::from_uuid),
        banner_position_x: request.banner_position_x,
        banner_position_y: request.banner_position_y,
        window_mode: match request.window_mode {
            InstanceWindowModeDto::Windowed => InstanceWindowMode::Windowed,
            InstanceWindowModeDto::Maximized => InstanceWindowMode::Maximized,
            InstanceWindowModeDto::Fullscreen => InstanceWindowMode::Fullscreen,
        },
        resolution_width,
        resolution_height,
        launcher_behavior: match request.launcher_behavior {
            LauncherBehaviorDto::KeepOpen => LauncherBehavior::KeepOpen,
            LauncherBehaviorDto::Minimize => LauncherBehavior::Minimize,
            LauncherBehaviorDto::Hide => LauncherBehavior::Hide,
        },
        game_language,
        quick_play_server,
        process_priority: match request.process_priority {
            ProcessPriorityDto::Low => ProcessPriority::Low,
            ProcessPriorityDto::BelowNormal => ProcessPriority::BelowNormal,
            ProcessPriorityDto::Normal => ProcessPriority::Normal,
            ProcessPriorityDto::AboveNormal => ProcessPriority::AboveNormal,
            ProcessPriorityDto::High => ProcessPriority::High,
        },
        cpu_affinity,
        memory_mode: match request.memory_mode {
            MemoryModeDto::Auto => MemoryMode::Auto,
            MemoryModeDto::Custom => MemoryMode::Custom,
        },
        initial_memory_mb: request.initial_memory_mb,
        maximum_memory_mb,
        java_mode: match request.java_mode {
            JavaSelectionModeDto::Managed => JavaSelectionMode::Managed,
            JavaSelectionModeDto::Detected => JavaSelectionMode::Detected,
            JavaSelectionModeDto::Custom => JavaSelectionMode::Custom,
        },
        performance_preset: match request.performance_preset {
            PerformancePresetDto::Balanced => PerformancePreset::Balanced,
            PerformancePresetDto::Throughput => PerformancePreset::Throughput,
            PerformancePresetDto::LowLatency => PerformancePreset::LowLatency,
            PerformancePresetDto::Custom => PerformancePreset::Custom,
        },
        jvm_arguments,
        environment,
        backup_before_changes: request.backup_before_changes,
        backup_retention: request.backup_retention,
        log_retention_days: request.log_retention_days,
    })
}

pub(super) fn validated_free_text(
    field: &'static str,
    value: String,
    maximum: usize,
) -> Result<String, AppError> {
    let value = value.trim().to_owned();
    if value.chars().count() > maximum
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(settings_validation_error(
            field,
            format!("Use no more than {maximum} characters."),
        ));
    }
    Ok(value)
}

pub(super) fn validated_optional_text(
    field: &'static str,
    value: Option<String>,
    maximum: usize,
) -> Result<Option<String>, AppError> {
    value
        .map(|value| validated_free_text(field, value, maximum))
        .transpose()
        .map(|value| value.filter(|value| !value.is_empty()))
}

pub(super) fn validated_optional_server(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 255
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(settings_validation_error(
            "quickPlayServer",
            "Enter a host or host:port without spaces.",
        ));
    }
    Ok(Some(value.to_owned()))
}

pub(super) fn recommended_memory_mb(instance: &InstanceRecord) -> u32 {
    let baseline = match instance.mode {
        slate_domain::InstanceMode::Vanilla | slate_domain::InstanceMode::SlateClient => 4_096,
        slate_domain::InstanceMode::Modded => match instance.mod_count {
            0..=50 => 4_096,
            51..=150 => 6_144,
            151..=300 => 8_192,
            _ => 12_288,
        },
    };
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let total_mb = u32::try_from(system.total_memory() / (1024 * 1024)).unwrap_or(u32::MAX);
    if total_mb == 0 {
        return baseline;
    }
    let safe_maximum = total_mb
        .saturating_mul(3)
        .checked_div(4)
        .unwrap_or(total_mb)
        .saturating_sub(1_024)
        .clamp(2_048, 32_768);
    baseline.min(safe_maximum).max(1_024)
}

pub(super) fn validated_jvm_arguments(arguments: Vec<String>) -> Result<Vec<String>, AppError> {
    if arguments.len() > 64 {
        return Err(settings_validation_error(
            "jvmArguments",
            "Use no more than 64 JVM arguments.",
        ));
    }
    const BLOCKED_PREFIXES: &[&str] = &[
        "-xms",
        "-xmx",
        "-cp",
        "-classpath",
        "--class-path",
        "--module-path",
        "-jar",
        "-javaagent",
        "-agentlib",
        "-agentpath",
        "-djava.library.path",
    ];
    let mut validated = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let argument = argument.trim();
        let normalized = argument.to_ascii_lowercase();
        if argument.is_empty()
            || argument.len() > 512
            || argument.chars().any(|character| character.is_control())
            || argument.starts_with('@')
            || BLOCKED_PREFIXES.iter().any(|prefix| {
                normalized == *prefix || normalized.starts_with(&format!("{prefix}="))
            })
        {
            return Err(settings_validation_error(
                "jvmArguments",
                "One or more JVM arguments would override settings required to launch Minecraft.",
            ));
        }
        validated.push(argument.to_owned());
    }
    Ok(validated)
}

pub(super) fn validated_environment(
    environment: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, AppError> {
    if environment.len() > 16 {
        return Err(settings_validation_error(
            "environment",
            "Use no more than 16 environment overrides.",
        ));
    }
    const BLOCKED: &[&str] = &[
        "PATH",
        "JAVA_HOME",
        "JAVA_TOOL_OPTIONS",
        "_JAVA_OPTIONS",
        "JDK_JAVA_OPTIONS",
        "CLASSPATH",
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "TEMP",
        "TMP",
    ];
    let mut validated = BTreeMap::new();
    for (raw_key, value) in environment {
        let key = raw_key.trim().to_ascii_uppercase();
        if key.is_empty()
            || key.len() > 64
            || !key.chars().all(|character| {
                character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
            })
            || key.as_bytes()[0].is_ascii_digit()
            || BLOCKED.contains(&key.as_str())
            || key.starts_with("SLATE_")
            || value.len() > 1_024
            || value.contains('\0')
        {
            return Err(settings_validation_error(
                "environment",
                "Use safe variable names and values that do not override Java, paths, or slate internals.",
            ));
        }
        validated.insert(key, value);
    }
    Ok(validated)
}

pub(super) fn settings_validation_error(
    field: &'static str,
    message: impl Into<String>,
) -> AppError {
    let message = message.into();
    AppError::new("validation.instance_settings", message.clone()).with_field_error(field, message)
}

pub(super) fn validated_java_selection(
    instance: &InstanceRecord,
    probe: JavaRuntimeProbe,
    mode: JavaSelectionMode,
) -> Result<(JavaSelectionMode, Option<String>, Option<String>), AppError> {
    if !probe.available {
        return Err(java_selection_error());
    }
    let expected_major = required_java_major(&instance.minecraft_version);
    if probe.major_version != Some(expected_major) {
        return Err(AppError::new(
            "validation.java_version",
            format!(
                "Minecraft {} requires Java {expected_major}. Choose a matching Java version.",
                instance.minecraft_version
            ),
        ));
    }
    let executable = probe
        .executable
        .filter(|path| path.is_absolute() && path.is_file())
        .ok_or_else(java_selection_error)?;
    let label = probe
        .version
        .map(|value| value.chars().take(160).collect::<String>())
        .or_else(|| Some(format!("Java {expected_major}")));
    Ok((mode, Some(executable.to_string_lossy().into_owned()), label))
}

pub(super) fn required_java_major(minecraft_version: &str) -> u32 {
    let mut components = minecraft_version
        .split(['.', '-'])
        .take(3)
        .map(|component| component.parse::<u32>().unwrap_or_default());
    let major = components.next().unwrap_or_default();
    let minor = components.next().unwrap_or_default();
    let patch = components.next().unwrap_or_default();
    match (major, minor, patch) {
        (26.., _, _) => 25,
        (1, 0..=16, _) => 8,
        (1, 17, _) => 16,
        (1, 18..=19, _) | (1, 20, 0..=4) => 17,
        _ => 21,
    }
}

pub(super) fn java_selection_error() -> AppError {
    AppError::new(
        "runtime.java_unavailable",
        "slate could not use that Java application. Choose another one.",
    )
}

#[cfg(test)]
mod tests {
    use super::required_java_major;

    #[test]
    fn java_compatibility_covers_legacy_modern_and_year_versions() {
        assert_eq!(required_java_major("1.16.5"), 8);
        assert_eq!(required_java_major("1.17.1"), 16);
        assert_eq!(required_java_major("1.20.4"), 17);
        assert_eq!(required_java_major("1.20.5"), 21);
        assert_eq!(required_java_major("1.21.1"), 21);
        assert_eq!(required_java_major("26.1.3"), 25);
    }
}
