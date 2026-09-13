#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slate_contracts::{
    AppError, AppPreferencesDto, BootstrapResponse, CapabilitySummary, CreateInstanceRequest,
    GameSessionSummary, InstallInstanceRequest, InstallJobStateDto, InstallJobSummary,
    InstanceModeDto, InstanceSummary, JavaRuntimeSummary, LaunchDemoRequest, LoaderKindDto,
    LoaderVersionCatalog, LoaderVersionsRequest, MinecraftReleaseKindDto, MinecraftVersionCatalog,
    MinecraftVersionOption, PreflightSummary, ReduceMotionPreferenceDto, RenameInstanceRequest,
    SetFavoriteRequest, ThemePreferenceDto, TrashInstanceRequest, UpdateAppPreferencesRequest,
    UpdateInstanceConfigurationRequest,
};
use slate_domain::{
    InstanceId, InstanceName, InstanceNameError, ManagementMode, RequestId, RevisionId, SessionId,
    StorageRootId,
};
use slate_installer::{
    InstallRequest as NativeInstallRequest, install, load_installed_revision,
};
use slate_loaders::{FabricAdapter, NeoForgeAdapter};
use slate_minecraft::{
    Architecture, ArtifactRequirement, EnvironmentValue, JavaRuntime, LaunchIdentity, LaunchLayout,
    LaunchOptions, LaunchPlanner, LaunchRequest, MojangMetadataClient, OperatingSystem,
    ResolvedVersion, RuleContext, verify_artifact,
};
use slate_platform::{AppPaths, detect_java_runtime, probe_java_executable};
use slate_process::ProcessSupervisor;
use slate_storage::{
    AppPreferences, Database, InstallJobRecord, InstalledRuntime, JobState, NewInstance,
    ReduceMotionPreference, StorageError, ThemePreference,
};
use std::collections::BTreeMap;
use std::path::Path;
use tauri::Manager;
use uuid::Uuid;

#[derive(Clone, Debug)]
struct DesktopState {
    database: Database,
    paths: AppPaths,
    storage_root_id: StorageRootId,
    processes: ProcessSupervisor,
}

#[tauri::command]
fn app_bootstrap() -> BootstrapResponse {
    BootstrapResponse::new(vec![
        CapabilitySummary::available("instance.library"),
        CapabilitySummary::available("instance.create"),
        CapabilitySummary::available("instance.configure"),
        CapabilitySummary::available("settings.local"),
        CapabilitySummary::available("metadata.minecraft"),
        CapabilitySummary::available("metadata.fabric"),
        CapabilitySummary::available("metadata.neoforge"),
        CapabilitySummary::available("minecraft.install"),
        CapabilitySummary::available("minecraft.launch.demo"),
        CapabilitySummary::unavailable(
            "minecraft.launch.authenticated",
            "Microsoft account sign-in requires the product app registration.",
        ),
        CapabilitySummary::unavailable(
            "minecraft.account",
            "Microsoft account sign-in requires the product app registration.",
        ),
    ])
}

#[tauri::command]
async fn minecraft_versions_list() -> Result<MinecraftVersionCatalog, AppError> {
    let client = MojangMetadataClient::new().map_err(|_| metadata_error())?;
    let manifest = client
        .fetch_manifest()
        .await
        .map_err(|_| metadata_error())?;
    let versions = manifest
        .versions
        .into_iter()
        .filter(|entry| entry.version_type == "release")
        .take(160)
        .map(|entry| MinecraftVersionOption {
            id: entry.id,
            kind: MinecraftReleaseKindDto::Release,
        })
        .collect();
    Ok(MinecraftVersionCatalog {
        latest_release: manifest.latest.release,
        versions,
    })
}

#[tauri::command]
async fn loader_versions_list(
    request: LoaderVersionsRequest,
) -> Result<LoaderVersionCatalog, AppError> {
    let versions = fetch_loader_versions(&request.minecraft_version, request.loader_kind).await?;
    let recommended_version = versions.first().cloned();
    let unavailable_reason = (request.loader_kind != LoaderKindDto::Vanilla && versions.is_empty())
        .then(|| {
            "No compatible loader release was published for this Minecraft version.".to_owned()
        });
    Ok(LoaderVersionCatalog {
        loader_kind: request.loader_kind,
        minecraft_version: request.minecraft_version,
        recommended_version,
        versions,
        unavailable_reason,
    })
}

async fn fetch_loader_versions(
    minecraft_version: &str,
    loader_kind: LoaderKindDto,
) -> Result<Vec<String>, AppError> {
    let client = MojangMetadataClient::new().map_err(|_| metadata_error())?;
    let manifest = client
        .fetch_manifest()
        .await
        .map_err(|_| metadata_error())?;
    let known_release = manifest
        .find(minecraft_version)
        .is_some_and(|entry| entry.version_type == "release");
    if !known_release {
        return Err(AppError::new(
            "metadata.minecraft_version_unknown",
            "Choose a Minecraft release from the current catalog.",
        )
        .with_field_error(
            "minecraftVersion",
            "The selected release is not in the catalog.",
        ));
    }

    let versions = match loader_kind {
        LoaderKindDto::Vanilla => Vec::new(),
        LoaderKindDto::Fabric => FabricAdapter::new()
            .map_err(|_| metadata_error())?
            .fetch_loader_versions(minecraft_version)
            .await
            .map_err(|_| metadata_error())?,
        LoaderKindDto::NeoForge => NeoForgeAdapter::new()
            .map_err(|_| metadata_error())?
            .fetch_loader_versions(minecraft_version)
            .await
            .map_err(|_| metadata_error())?,
    };
    Ok(versions)
}

async fn validate_selected_loader_version(
    minecraft_version: &str,
    loader_kind: LoaderKindDto,
    selected: Option<&str>,
) -> Result<Option<String>, AppError> {
    if loader_kind == LoaderKindDto::Vanilla {
        return Ok(None);
    }
    let versions = fetch_loader_versions(minecraft_version, loader_kind).await?;
    let selected = selected.map(str::trim).filter(|value| !value.is_empty());
    let Some(selected) = selected else {
        return Err(configuration_app_error(
            ConfigurationValidationError::LoaderVersionRequired,
        ));
    };
    if !versions.iter().any(|version| version == selected) {
        return Err(AppError::new(
            "metadata.loader_version_unknown",
            "Choose a compatible loader version from the current catalog.",
        )
        .with_field_error(
            "loaderVersion",
            "The selected loader version is not compatible with this Minecraft release.",
        ));
    }
    Ok(Some(selected.to_owned()))
}

#[tauri::command]
async fn instances_list(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<InstanceSummary>, AppError> {
    state
        .database
        .list_instances(250)
        .await
        .map(|records| records.into_iter().map(instance_summary).collect())
        .map_err(|error| map_storage_error(error, "slate could not load your local instances."))
}

#[tauri::command]
async fn instance_get(
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
async fn instance_create(
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
    let record = state
        .database
        .create_instance(NewInstance {
            name,
            mode: request.mode.into(),
            management_mode: ManagementMode::Local,
            root_id: state.storage_root_id,
            minecraft_version: request.minecraft_version.trim().to_owned(),
            loader_kind: request.loader_kind.into(),
            loader_version,
            memory_mb: request.memory_mb,
        })
        .await
        .map_err(|error| map_storage_error(error, "slate could not create the instance."))?;

    if std::fs::create_dir_all(state.paths.instance(record.id)).is_err() {
        let _ = state
            .database
            .trash_instance(record.id, record.revision)
            .await;
        return Err(AppError::new(
            "local.instance_directory_unavailable",
            "slate could not create the managed instance directory. The incomplete record was moved to trash.",
        ));
    }

    Ok(instance_summary(record))
}

#[tauri::command]
async fn instance_rename(
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
async fn instance_update_configuration(
    state: tauri::State<'_, DesktopState>,
    request: UpdateInstanceConfigurationRequest,
) -> Result<InstanceSummary, AppError> {
    let loader_version = validate_selected_loader_version(
        &request.minecraft_version,
        request.loader_kind,
        request.loader_version.as_deref(),
    )
    .await?;
    let current = state
        .database
        .get_instance(InstanceId::from_uuid(request.id))
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    validate_instance_configuration(
        current.mode.into(),
        &request.minecraft_version,
        request.loader_kind,
        loader_version.as_deref(),
        request.memory_mb,
    )
    .map_err(configuration_app_error)?;

    state
        .database
        .update_instance_configuration(
            InstanceId::from_uuid(request.id),
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
async fn instance_set_favorite(
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
async fn instance_trash(
    state: tauri::State<'_, DesktopState>,
    request: TrashInstanceRequest,
) -> Result<(), AppError> {
    state
        .database
        .trash_instance(InstanceId::from_uuid(request.id), request.expected_revision)
        .await
        .map_err(|error| map_storage_error(error, "slate could not move the instance to trash."))
}

#[tauri::command]
async fn instance_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallInstanceRequest,
) -> Result<InstallJobSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if instance.setup_state == slate_domain::InstanceSetupState::Preparing {
        return Err(AppError::new(
            "local.install_already_running",
            "That instance already has an installation in progress.",
        ));
    }
    let preferences = state
        .database
        .get_app_preferences()
        .await
        .map_err(|error| map_storage_error(error, "slate could not load download settings."))?;
    let pending = state
        .database
        .begin_instance_install(instance_id, request.expected_revision, RequestId::new())
        .await
        .map_err(|error| map_storage_error(error, "slate could not queue the installation."))?;
    let response = install_job_summary(pending.job.clone());
    let task_state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _ = task_state
            .database
            .mark_install_running(pending.job.id, "metadata", "Resolving official metadata")
            .await;
        let result = install(NativeInstallRequest {
            instance_id,
            revision_id: pending.revision_id,
            minecraft_version: instance.minecraft_version,
            loader_kind: instance.loader_kind,
            loader_version: instance.loader_version,
            download_concurrency: preferences.download_concurrency,
            paths: task_state.paths.clone(),
        })
        .await;
        match result {
            Ok(outcome) => {
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
                let message = format!(
                    "Installed and verified {} files; reused {} cached files",
                    outcome.downloaded_artifacts, outcome.reused_artifacts
                );
                let _ = task_state
                    .database
                    .complete_instance_install(
                        pending.job.id,
                        pending.revision_id,
                        &outcome.manifest_digest,
                        &outcome.resolved_version_id,
                        runtime,
                        &message,
                    )
                    .await;
            }
            Err(error) => {
                let message = bounded_error_message(&error.to_string());
                let _ = task_state
                    .database
                    .fail_instance_install(pending.job.id, pending.revision_id, &message)
                    .await;
            }
        }
    });
    Ok(response)
}

#[tauri::command]
async fn install_jobs_list(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<InstallJobSummary>, AppError> {
    refresh_exited_sessions(state.inner()).await;
    state
        .database
        .list_install_jobs(100)
        .await
        .map(|jobs| jobs.into_iter().map(install_job_summary).collect())
        .map_err(|error| map_storage_error(error, "slate could not load installation activity."))
}

#[tauri::command]
async fn instance_launch_demo(
    state: tauri::State<'_, DesktopState>,
    request: LaunchDemoRequest,
) -> Result<GameSessionSummary, AppError> {
    refresh_exited_sessions(state.inner()).await;
    let instance_id = InstanceId::from_uuid(request.id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if instance.setup_state != slate_domain::InstanceSetupState::Ready {
        return Err(AppError::new(
            "local.instance_not_ready",
            "Install and verify this instance before launching it.",
        ));
    }
    let revision = state
        .database
        .get_installed_revision(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load the installed revision."))?;
    let manifest_path = installed_manifest_path(&state.paths, instance_id, revision.id);
    let (layers, runtime) = load_installed_revision(&manifest_path)
        .await
        .map_err(|_| AppError::new(
            "local.install_manifest_invalid",
            "The installed revision manifest is missing or invalid. Reinstall the instance.",
        ))?;
    if runtime.executable != revision.runtime_executable
        || runtime.major_version != revision.runtime_major
    {
        return Err(AppError::new(
            "local.runtime_binding_changed",
            "The runtime binding changed after installation. Reinstall the instance.",
        ));
    }
    let probe_path = runtime.executable.clone();
    let probe = tauri::async_runtime::spawn_blocking(move || probe_java_executable(&probe_path))
        .await
        .map_err(|_| AppError::new(
            "local.runtime_probe_unavailable",
            "slate could not validate the managed Java runtime.",
        ))?;
    if !probe.available || probe.major_version != Some(runtime.major_version) {
        return Err(AppError::new(
            "local.runtime_invalid",
            "The managed Java runtime is missing or no longer matches this instance.",
        ));
    }

    let resolved = ResolvedVersion::resolve(layers).map_err(|_| AppError::new(
        "local.install_manifest_invalid",
        "The installed metadata chain is invalid. Reinstall the instance.",
    ))?;
    let architecture = minecraft_architecture(&runtime.architecture)?;
    let layout = launch_layout(&state.paths, instance_id, revision.id);
    let runtime_for_plan = JavaRuntime::new(
        runtime.executable.clone(),
        runtime.major_version,
        architecture,
    )
    .map_err(|_| AppError::new("local.runtime_invalid", "The managed runtime is invalid."))?;
    let mut environment = BTreeMap::new();
    for name in ["SystemRoot", "WINDIR", "TEMP", "TMP", "LANG"] {
        if let Some(value) = std::env::var_os(name) {
            environment.insert(
                name.to_owned(),
                EnvironmentValue::public(value.to_string_lossy().into_owned()),
            );
        }
    }
    let preparation = LaunchPlanner::prepare(
        &resolved,
        LaunchRequest {
            runtime: runtime_for_plan,
            layout,
            identity: LaunchIdentity::new("Player", Uuid::nil(), "0", "0", "0")
                .map_err(|_| AppError::new("local.demo_identity_invalid", "Demo launch could not be prepared."))?,
            rules: current_rule_context(architecture),
            options: LaunchOptions {
                maximum_memory_mib: instance.memory_mb,
                demo_user: true,
                ..LaunchOptions::default()
            },
            environment,
        },
    )
    .map_err(|_| AppError::new(
        "local.launch_plan_invalid",
        "slate could not construct a valid launch plan for this revision.",
    ))?;
    verify_core_launch_artifacts(&preparation.required_artifacts).await?;

    let session_id = SessionId::new();
    state
        .database
        .create_session_starting(instance_id, revision.id, session_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not create the game session."))?;
    let log_path = state
        .paths
        .instance(instance_id)
        .join("logs")
        .join(format!("{session_id}.log"));
    let started = state
        .processes
        .start(instance_id, session_id, &preparation.plan, log_path)
        .map_err(|error| AppError::new("local.launch_failed", error.to_string()))?;
    if let Err(error) = state.database.mark_session_running(session_id, started.pid).await {
        let _ = state.processes.terminate_after_tracking_failure(instance_id);
        return Err(map_storage_error(
            error,
            "The game was stopped because slate could not track its session.",
        ));
    }
    Ok(GameSessionSummary {
        id: session_id.as_uuid(),
        instance_id: instance_id.as_uuid(),
        state: "running".to_owned(),
        mode: "demo".to_owned(),
        pid: started.pid,
        log_name: started
            .log_path
            .file_name()
            .map_or_else(|| "session.log".to_owned(), |name| name.to_string_lossy().into_owned()),
    })
}

#[tauri::command]
async fn preferences_get(
    state: tauri::State<'_, DesktopState>,
) -> Result<AppPreferencesDto, AppError> {
    state
        .database
        .get_app_preferences()
        .await
        .map(preferences_dto)
        .map_err(|error| map_storage_error(error, "slate could not load your preferences."))
}

#[tauri::command]
async fn preferences_update(
    state: tauri::State<'_, DesktopState>,
    request: UpdateAppPreferencesRequest,
) -> Result<AppPreferencesDto, AppError> {
    if !(1..=8).contains(&request.download_concurrency) {
        return Err(AppError::new(
            "validation.download_concurrency",
            "Choose between 1 and 8 simultaneous downloads.",
        )
        .with_field_error("downloadConcurrency", "Choose a value from 1 through 8."));
    }

    state
        .database
        .update_app_preferences(AppPreferences {
            theme: match request.theme {
                ThemePreferenceDto::Dark => ThemePreference::Dark,
                ThemePreferenceDto::Light => ThemePreference::Light,
                ThemePreferenceDto::System => ThemePreference::System,
            },
            download_concurrency: request.download_concurrency,
            telemetry_enabled: request.telemetry_enabled,
            reduce_motion: match request.reduce_motion {
                ReduceMotionPreferenceDto::System => ReduceMotionPreference::System,
                ReduceMotionPreferenceDto::On => ReduceMotionPreference::On,
                ReduceMotionPreferenceDto::Off => ReduceMotionPreference::Off,
            },
        })
        .await
        .map(preferences_dto)
        .map_err(|error| map_storage_error(error, "slate could not save your preferences."))
}

#[tauri::command]
async fn preflight_get(
    state: tauri::State<'_, DesktopState>,
) -> Result<PreflightSummary, AppError> {
    let database_ready = state.database.health_check().await.is_ok();
    let account_configured = state
        .database
        .has_configured_account()
        .await
        .unwrap_or(false);
    let storage_ready = state.paths.storage_root().is_dir();
    let probe = tauri::async_runtime::spawn_blocking(detect_java_runtime)
        .await
        .map_err(|_| {
            AppError::new(
                "local.java_probe_unavailable",
                "slate could not inspect the local Java runtime.",
            )
            .retryable(true)
        })?;

    Ok(PreflightSummary {
        database_ready,
        storage_ready,
        account_configured,
        java: JavaRuntimeSummary {
            available: probe.available,
            version: probe.version,
            unavailable_reason: probe.unavailable_reason,
        },
        launch_implemented: true,
    })
}

fn instance_summary(record: slate_storage::InstanceRecord) -> InstanceSummary {
    InstanceSummary {
        id: record.id.as_uuid(),
        name: record.name.to_string(),
        mode: record.mode.into(),
        management_mode: record.management_mode.into(),
        favorite: record.favorite,
        revision: record.revision,
        minecraft_version: record.minecraft_version,
        loader_kind: record.loader_kind.into(),
        loader_version: record.loader_version,
        memory_mb: record.memory_mb,
        setup_state: record.setup_state.into(),
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn preferences_dto(value: AppPreferences) -> AppPreferencesDto {
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
    }
}

fn parse_instance_name(value: &str) -> Result<InstanceName, InstanceNameError> {
    InstanceName::parse(value)
}

fn validate_instance_configuration(
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
    if mode == InstanceModeDto::Modded && loader_kind == LoaderKindDto::Vanilla {
        return Err(ConfigurationValidationError::ModdedLoader);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigurationValidationError {
    MinecraftVersion,
    VanillaLoader,
    ModdedLoader,
    LoaderVersionRequired,
    VanillaLoaderVersion,
    Memory,
}

fn instance_name_app_error(_: InstanceNameError) -> AppError {
    AppError::new(
        "validation.instance_name",
        "Enter an instance name between 1 and 80 characters.",
    )
    .with_field_error("name", "Use between 1 and 80 characters.")
}

fn configuration_app_error(error: ConfigurationValidationError) -> AppError {
    match error {
        ConfigurationValidationError::MinecraftVersion => AppError::new(
            "validation.minecraft_version",
            "Enter a valid Minecraft version identifier.",
        )
        .with_field_error(
            "minecraftVersion",
            "Use letters, numbers, dots, dashes, underscores, or plus signs.",
        ),
        ConfigurationValidationError::VanillaLoader => AppError::new(
            "validation.loader_mode",
            "Vanilla instances cannot use a mod loader.",
        )
        .with_field_error("loaderKind", "Choose Vanilla for this experience."),
        ConfigurationValidationError::ModdedLoader => AppError::new(
            "validation.loader_mode",
            "Modded instances require Fabric or NeoForge.",
        )
        .with_field_error("loaderKind", "Choose Fabric or NeoForge."),
        ConfigurationValidationError::LoaderVersionRequired => AppError::new(
            "validation.loader_version",
            "Fabric and NeoForge require a loader version.",
        )
        .with_field_error("loaderVersion", "Enter the exact loader version."),
        ConfigurationValidationError::VanillaLoaderVersion => AppError::new(
            "validation.loader_version",
            "Vanilla does not use a loader version.",
        )
        .with_field_error("loaderVersion", "Remove the loader version."),
        ConfigurationValidationError::Memory => AppError::new(
            "validation.memory",
            "Choose a memory value between 1024 MB and 32768 MB.",
        )
        .with_field_error("memoryMb", "Choose 1024 through 32768 MB."),
    }
}

fn map_storage_error(error: StorageError, fallback: &'static str) -> AppError {
    match error {
        StorageError::InstanceNotFound => AppError::new(
            "local.instance_not_found",
            "That instance no longer exists.",
        ),
        StorageError::RevisionConflict { .. } => AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ),
        _ => AppError::new("local.storage_unavailable", fallback).retryable(true),
    }
}

fn metadata_error() -> AppError {
    AppError::new(
        "metadata.catalog_unavailable",
        "slate could not refresh the official version catalog.",
    )
    .retryable(true)
}

fn install_job_summary(record: InstallJobRecord) -> InstallJobSummary {
    let message = serde_json::from_str::<serde_json::Value>(&record.progress_json)
        .ok()
        .and_then(|value| value.get("message").and_then(|message| message.as_str()).map(str::to_owned))
        .unwrap_or_else(|| record.phase.clone());
    InstallJobSummary {
        id: record.id.as_uuid(),
        instance_id: record.instance_id.as_uuid(),
        revision_id: record.revision_id.as_uuid(),
        state: match record.state {
            JobState::Queued => InstallJobStateDto::Queued,
            JobState::Running => InstallJobStateDto::Running,
            JobState::Succeeded => InstallJobStateDto::Succeeded,
            JobState::Failed => InstallJobStateDto::Failed,
            JobState::Cancelled => InstallJobStateDto::Cancelled,
        },
        phase: record.phase,
        message,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn bounded_error_message(message: &str) -> String {
    const MAX_CHARACTERS: usize = 500;
    let mut bounded = message.chars().take(MAX_CHARACTERS).collect::<String>();
    if message.chars().count() > MAX_CHARACTERS {
        bounded.push('…');
    }
    bounded
}

fn installed_manifest_path(
    paths: &AppPaths,
    instance_id: InstanceId,
    revision_id: RevisionId,
) -> std::path::PathBuf {
    paths
        .instance(instance_id)
        .join("revisions")
        .join(revision_id.to_string())
        .join("metadata")
        .join("installed-revision.json")
}

fn launch_layout(
    paths: &AppPaths,
    instance_id: InstanceId,
    revision_id: RevisionId,
) -> LaunchLayout {
    let minecraft_root = paths.artifacts().join("minecraft");
    LaunchLayout {
        game_directory: paths.instance(instance_id).join("game"),
        libraries_directory: minecraft_root.join("libraries"),
        versions_directory: minecraft_root.join("versions"),
        assets_directory: minecraft_root.join("assets"),
        natives_directory: paths
            .instance(instance_id)
            .join("revisions")
            .join(revision_id.to_string())
            .join("natives"),
    }
}

fn minecraft_architecture(value: &str) -> Result<Architecture, AppError> {
    match value {
        "x64" | "amd64" | "x86_64" => Ok(Architecture::X86_64),
        "x86" | "x32" => Ok(Architecture::X86),
        "aarch64" | "arm64" => Ok(Architecture::Arm64),
        _ => Err(AppError::new(
            "local.runtime_architecture_unknown",
            "The installed Java runtime reported an unsupported architecture.",
        )),
    }
}

fn current_rule_context(architecture: Architecture) -> RuleContext {
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

async fn verify_core_launch_artifacts(
    requirements: &[ArtifactRequirement],
) -> Result<(), AppError> {
    for requirement in requirements {
        if !requirement.target_path().is_file() {
            return Err(AppError::new(
                "local.install_incomplete",
                "A required game file is missing. Reinstall the instance.",
            ));
        }
        if requirement.expected_hashes().is_empty() {
            continue;
        }
        let requirement = requirement.clone();
        let result =
            tauri::async_runtime::spawn_blocking(move || verify_artifact(&requirement)).await;
        if !matches!(result, Ok(Ok(()))) {
            return Err(AppError::new(
                "local.install_corrupt",
                "A required game file failed verification. Reinstall the instance.",
            ));
        }
    }
    Ok(())
}

async fn refresh_exited_sessions(state: &DesktopState) {
    let Ok(exited) = state.processes.refresh() else {
        return;
    };
    for process in exited {
        let _ = state
            .database
            .finish_session(process.session_id, process.exit_code)
            .await;
    }
}

fn main() {
    let application = tauri::Builder::default()
        .setup(|app| {
            let paths = AppPaths::discover()?;
            paths.ensure_base_directories()?;
            let database =
                tauri::async_runtime::block_on(Database::connect(&paths.state_database()))?;
            let canonical_storage = std::fs::canonicalize(paths.storage_root())?;
            let canonical_storage = canonical_storage.to_string_lossy().into_owned();
            let storage_root_id = tauri::async_runtime::block_on(
                database.get_or_create_storage_root(&canonical_storage),
            )?;
            app.manage(DesktopState {
                database,
                paths,
                storage_root_id,
                processes: ProcessSupervisor::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_bootstrap,
            minecraft_versions_list,
            loader_versions_list,
            instances_list,
            instance_get,
            instance_create,
            instance_rename,
            instance_update_configuration,
            instance_set_favorite,
            instance_trash,
            instance_install,
            install_jobs_list,
            instance_launch_demo,
            preferences_get,
            preferences_update,
            preflight_get,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = application {
        eprintln!("slate failed to start: {error}");
        std::process::exit(1);
    }
}
