#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account_commands;
mod artwork_support;
mod auth_support;
mod catalog_commands;
mod content_commands;
mod diagnostics;
mod game_options;
mod install_commands;
mod instance_commands;
mod instance_content;
mod instance_files;
mod instance_lifecycle_commands;
mod instance_lifecycle_support;
mod launch_commands;
mod launch_support;
mod mapping;
mod mod_install_support;
mod onboarding_commands;
mod portable_instance;
mod server_commands;
mod server_support;
mod session_commands;
mod session_support;
mod settings_commands;
mod storage_commands;
mod storage_management;
mod storage_support;
mod validation;

use account_commands::*;
use artwork_support::*;
use auth_support::*;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use catalog_commands::*;
use content_commands::*;
use diagnostics::init_diagnostics;
use game_options::{prepare_options_update, read_recognized_options};
use install_commands::*;
use instance_commands::*;
use instance_content::{
    FileMove, InstanceContentFile, InstanceContentKind, InstanceModFile, scan_instance_content,
    scan_instance_mods, set_instance_content_enabled, set_instance_mod_enabled,
    trash_instance_content, trash_instance_mod,
};
use instance_files::{
    copy_duplicate_personal_data, create_snapshot, export_portable_archive,
    extract_portable_archive, prepare_instance_relocation, prepare_snapshot_restore,
    read_portable_manifest,
};
use instance_lifecycle_commands::*;
use instance_lifecycle_support::*;
use launch_commands::*;
use launch_support::*;
use mapping::*;
use mod_install_support::*;
use onboarding_commands::*;
use portable_instance::*;
use serde::{Deserialize, Serialize};
use server_commands::*;
use server_support::*;
use session_commands::*;
use session_support::*;
use settings_commands::*;
use slate_auth::{
    AuthError, CredentialVault, MICROSOFT_CONSUMER_TENANT, MinecraftAuthClient,
    MinecraftAuthConfig, MinecraftSession, SLATE_MICROSOFT_CLIENT_ID,
};
use slate_contracts::{
    AccountIdRequest, AppError, AppPreferencesDto, ApplyModpackUpdateRequest, AuthCancelRequest,
    AuthFlowStateDto, AuthFlowStatus, AuthStartResponse, BootstrapResponse, CapabilitySummary,
    CheckModpackUpdateRequest, ClearStorageCategoryRequest, CreateInstanceRequest,
    CreateInstanceSnapshotRequest, CreateSavedServerRequest, DeleteInstanceSnapshotRequest,
    DeleteTrashedInstanceRequest, DuplicateInstanceRequest, EmptyInstanceTrashRequest,
    ExportInstanceRequest, GameSessionStateDto, GameSessionSummary, GetInstanceArtworkRequest,
    GetInstanceGameOptionsRequest, ImportInstanceRequest, InstallInstanceRequest,
    InstallJobStateDto, InstallJobSummary, InstallModRequest, InstallModpackRequest,
    InstanceArtworkAsset, InstanceArtworkKindDto, InstanceContentFileSummary,
    InstanceContentFilesRequest, InstanceContentKindDto, InstanceDirectoryKindDto,
    InstanceGameOptionsSummary, InstanceModOriginDto, InstanceModResolution, InstanceModSummary,
    InstanceModeDto, InstanceModsRequest, InstanceSettingsSummary, InstanceSnapshotSummary,
    InstanceSnapshotsRequest, InstanceSummary, InstanceWindowModeDto, JavaRuntimeSummary,
    JavaSelectionModeDto, LaunchInstanceRequest, LauncherBehaviorDto, LoaderKindDto,
    LoaderVersionCatalog, LoaderVersionsRequest, MemoryModeDto, MinecraftAccountStatusDto,
    MinecraftAccountSummary, MinecraftReleaseKindDto, MinecraftVersionCatalog,
    MinecraftVersionOption, ModSearchRequest, ModpackInstallStarted, ModpackProjectRequest,
    ModpackSearchRequest, ModpackSortDto, ModpackSourceSummary, ModpackUpdateSummary,
    ModpackVersionRequest, ModpackVersionsRequest, MoveInstanceStorageRequest,
    OnboardingStateSummary, OpenInstanceDirectoryRequest, PerformancePresetDto, PingServerRequest,
    PreflightSummary, ProcessPriorityDto, ReduceMotionPreferenceDto,
    RemoveInstanceContentFileRequest, RemoveInstanceModRequest, RemoveSavedServerRequest,
    RenameInstanceRequest, RestoreInstanceSnapshotRequest, RestoreTrashedInstanceRequest,
    SavedServerSummary, SelectInstanceArtworkRequest, SelectInstanceJavaRequest,
    ServerStatusSummary, SessionLogEvent, SessionLogEventKindDto, SessionLogSubscription,
    SetDefaultAccountRequest, SetFavoriteRequest, SetInstanceContentFileEnabledRequest,
    SetInstanceModEnabledRequest, SetInstanceModPinnedRequest, SetInstanceSnapshotPinnedRequest,
    StopGameSessionRequest, StorageCategoryDto, StorageCategorySummary, StorageCleanupResult,
    StorageOverview, SubscribeSessionLogRequest, ThemePreferenceDto, TrashInstanceRequest,
    TrashedInstanceSummary, UnsubscribeSessionLogRequest, UpdateAppPreferencesRequest,
    UpdateInstanceConfigurationRequest, UpdateInstanceGameOptionsRequest,
    UpdateInstanceSettingsRequest, UpdateSavedServerRequest,
};
use slate_domain::{
    AccountId, InstanceId, InstanceName, InstanceNameError, LoaderFamily, ManagementMode,
    RequestId, RevisionId, ServerId, SessionId, StorageRootId,
};
use slate_installer::{
    ContentUpdateRequest, InstallProgress, InstallRequest as NativeInstallRequest,
    install_with_progress, load_installed_revision, update_content_with_progress,
    verify_installed_launch_artifacts,
};
use slate_loaders::{FabricAdapter, NeoForgeAdapter};
use slate_minecraft::{
    Architecture, EnvironmentValue, JavaRuntime, LaunchIdentity, LaunchLayout, LaunchOptions,
    LaunchPlanner, LaunchRequest, MojangMetadataClient, OperatingSystem, QuickPlay,
    ResolvedVersion, RuleContext,
};
use slate_modpack_api_contracts::{
    Architecture as ModpackArchitecture, Hashes, InstallPlan, InstallPlanRequest, LoaderKind,
    ModInstallPlanRequest, ModProjectReference, Modpack, ModpackVersion, PackFileType,
    Platform as ModpackPlatform, ProviderReference, ProvidersResponse, ResolveModsRequest,
    SearchResponse, Side, VersionPage,
};
use slate_modpack_client::{ModpackApiClient, SearchOptions, SearchSort, VersionOptions};
use slate_platform::{
    AppPaths, JavaArchitecture, JavaRuntimeProbe, ManagedRelativePath, detect_java_runtime,
    probe_java_executable, restricted_child_environment,
};
use slate_process::{
    ActiveProcess, ChildProcessPriority, LogChunk, LogChunkKind, ProcessState, ProcessSupervisor,
    SessionLogTail,
};
use slate_storage::{
    AccountRecord, AccountStatus, AppPreferences, AuthenticatedAccount, CompletedInstall,
    CompletedModpackUpdate, Database, InstallJobRecord, InstalledRuntime, InstanceModEnabledChange,
    InstanceModRecord, InstanceModTarget, InstanceRecord, InstanceSnapshotRecord,
    InstanceWindowMode, JavaSelectionMode, JobState, LauncherBehavior, MemoryMode, NewInstance,
    NewInstanceMod, NewModpackSource, NewSavedServer, PerformancePreset, ProcessPriority,
    ReduceMotionPreference, SavedServerRecord, StorageError, ThemePreference,
    TrashedInstanceRecord, UpdateInstanceSettings,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use storage_commands::*;
use storage_management::{
    DiskUsage, StagedInstanceDeletion, clear_logs, clear_managed_java, clear_removed_content,
    clear_shared_game_files, clear_temporary_files, measure_path, scan_storage_usage,
    stage_instance_deletion,
};
use storage_support::*;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;
use validation::*;

#[derive(Clone, Debug)]
struct DesktopState {
    database: Database,
    paths: AppPaths,
    storage_root_id: StorageRootId,
    processes: ProcessSupervisor,
    auth_client: MinecraftAuthClient,
    credential_vault: CredentialVault,
    auth_flows: AuthCoordinator,
    log_streams: SessionLogCoordinator,
    modpacks: ModpackApiClient,
}

async fn preferred_storage_root_id(state: &DesktopState) -> Result<StorageRootId, AppError> {
    state
        .database
        .get_onboarding_state()
        .await
        .map(|onboarding| {
            onboarding
                .default_storage_root_id
                .unwrap_or(state.storage_root_id)
        })
        .map_err(|error| map_storage_error(error, "slate could not prepare storage."))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableInstanceManifest {
    schema: u32,
    exported_at: String,
    name: String,
    mode: InstanceModeDto,
    minecraft_version: String,
    loader_kind: LoaderKindDto,
    loader_version: Option<String>,
    memory_mb: u32,
    settings: PortableInstanceSettings,
    modpack_source: Option<PortableModpackSource>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableInstanceSettings {
    description: String,
    notes: String,
    group_name: Option<String>,
    tags: Vec<String>,
    icon_mime: Option<String>,
    banner_mime: Option<String>,
    banner_position_x: u8,
    banner_position_y: u8,
    window_mode: InstanceWindowModeDto,
    resolution_width: Option<u32>,
    resolution_height: Option<u32>,
    launcher_behavior: LauncherBehaviorDto,
    game_language: String,
    quick_play_server: Option<String>,
    process_priority: ProcessPriorityDto,
    #[serde(default)]
    cpu_affinity: Vec<u16>,
    memory_mode: MemoryModeDto,
    initial_memory_mb: u32,
    performance_preset: PerformancePresetDto,
    jvm_arguments: Vec<String>,
    environment: BTreeMap<String, String>,
    backup_before_changes: bool,
    backup_retention: u8,
    log_retention_days: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableModpackSource {
    provider: slate_modpack_api_contracts::Provider,
    project_id: String,
    version_id: String,
    selected_optional: Vec<String>,
    display_name: String,
    icon_url: Option<String>,
    banner_url: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct SessionLogCoordinator {
    inner: Arc<Mutex<HashMap<Uuid, tokio::sync::oneshot::Sender<()>>>>,
}

impl SessionLogCoordinator {
    fn insert(
        &self,
        subscription_id: Uuid,
        cancel_signal: tokio::sync::oneshot::Sender<()>,
    ) -> Result<(), AppError> {
        self.inner
            .lock()
            .map_err(|_| log_stream_state_error())?
            .insert(subscription_id, cancel_signal);
        Ok(())
    }

    fn cancel(&self, subscription_id: Uuid) -> Result<(), AppError> {
        if let Some(cancel_signal) = self
            .inner
            .lock()
            .map_err(|_| log_stream_state_error())?
            .remove(&subscription_id)
        {
            let _ = cancel_signal.send(());
        }
        Ok(())
    }

    fn finish(&self, subscription_id: Uuid) {
        if let Ok(mut streams) = self.inner.lock() {
            streams.remove(&subscription_id);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct AuthCoordinator {
    inner: Arc<Mutex<HashMap<Uuid, AuthFlowEntry>>>,
}

#[derive(Debug)]
struct AuthFlowEntry {
    status: AuthFlowStatus,
    expires_at: String,
    authorization_url: Option<String>,
    cancel_signal: Option<tokio::sync::oneshot::Sender<()>>,
}

impl AuthCoordinator {
    fn active_start(&self) -> Result<Option<(AuthStartResponse, String)>, AppError> {
        let flows = self.inner.lock().map_err(|_| auth_state_error())?;
        Ok(flows.values().find_map(|entry| {
            matches!(
                entry.status.state,
                AuthFlowStateDto::WaitingForBrowser | AuthFlowStateDto::Verifying
            )
            .then(|| {
                entry.authorization_url.as_ref().map(|authorization_url| {
                    (
                        AuthStartResponse {
                            flow_id: entry.status.flow_id,
                            expires_at: entry.expires_at.clone(),
                        },
                        authorization_url.clone(),
                    )
                })
            })
            .flatten()
        }))
    }

    fn insert_waiting(
        &self,
        flow_id: Uuid,
        expires_at: String,
        authorization_url: String,
    ) -> Result<(), AppError> {
        let mut flows = self.inner.lock().map_err(|_| auth_state_error())?;
        flows.retain(|_, entry| {
            !matches!(
                entry.status.state,
                AuthFlowStateDto::Succeeded
                    | AuthFlowStateDto::Failed
                    | AuthFlowStateDto::Cancelled
            )
        });
        flows.insert(
            flow_id,
            AuthFlowEntry {
                status: AuthFlowStatus {
                    flow_id,
                    state: AuthFlowStateDto::WaitingForBrowser,
                    account: None,
                    user_message: None,
                },
                expires_at,
                authorization_url: Some(authorization_url),
                cancel_signal: None,
            },
        );
        Ok(())
    }

    fn attach(
        &self,
        flow_id: Uuid,
        cancel_signal: tokio::sync::oneshot::Sender<()>,
    ) -> Result<(), AppError> {
        let mut flows = self.inner.lock().map_err(|_| auth_state_error())?;
        let entry = flows.get_mut(&flow_id).ok_or_else(auth_flow_not_found)?;
        entry.cancel_signal = Some(cancel_signal);
        Ok(())
    }

    fn status(&self, flow_id: Uuid) -> Result<AuthFlowStatus, AppError> {
        self.inner
            .lock()
            .map_err(|_| auth_state_error())?
            .get(&flow_id)
            .map(|entry| entry.status.clone())
            .ok_or_else(auth_flow_not_found)
    }

    fn set_verifying(&self, flow_id: Uuid) {
        if let Ok(mut flows) = self.inner.lock()
            && let Some(entry) = flows.get_mut(&flow_id)
            && entry.status.state == AuthFlowStateDto::WaitingForBrowser
        {
            entry.status.state = AuthFlowStateDto::Verifying;
        }
    }

    fn succeed(&self, flow_id: Uuid, account: MinecraftAccountSummary) {
        if let Ok(mut flows) = self.inner.lock()
            && let Some(entry) = flows.get_mut(&flow_id)
            && entry.status.state != AuthFlowStateDto::Cancelled
        {
            entry.status.state = AuthFlowStateDto::Succeeded;
            entry.status.account = Some(account);
            entry.status.user_message = None;
            entry.authorization_url = None;
            entry.cancel_signal = None;
        }
    }

    fn fail(&self, flow_id: Uuid, message: String) {
        if let Ok(mut flows) = self.inner.lock()
            && let Some(entry) = flows.get_mut(&flow_id)
            && entry.status.state != AuthFlowStateDto::Cancelled
        {
            entry.status.state = AuthFlowStateDto::Failed;
            entry.status.user_message = Some(message);
            entry.authorization_url = None;
            entry.cancel_signal = None;
        }
    }

    fn cancel(&self, flow_id: Uuid) -> Result<AuthFlowStatus, AppError> {
        let mut flows = self.inner.lock().map_err(|_| auth_state_error())?;
        let entry = flows.get_mut(&flow_id).ok_or_else(auth_flow_not_found)?;
        if matches!(
            entry.status.state,
            AuthFlowStateDto::WaitingForBrowser | AuthFlowStateDto::Verifying
        ) {
            if let Some(cancel_signal) = entry.cancel_signal.take() {
                let _ = cancel_signal.send(());
            }
            entry.status.state = AuthFlowStateDto::Cancelled;
            entry.status.user_message = Some("Microsoft sign-in was cancelled.".to_owned());
            entry.authorization_url = None;
        }
        Ok(entry.status.clone())
    }
}

#[tauri::command]
fn app_bootstrap(state: tauri::State<'_, DesktopState>) -> BootstrapResponse {
    let credential_vault_ready = state.credential_vault.check_available().is_ok();
    BootstrapResponse::new(vec![
        CapabilitySummary::available("instance.library"),
        CapabilitySummary::available("instance.create"),
        CapabilitySummary::available("instance.configure"),
        CapabilitySummary::available("settings.local"),
        CapabilitySummary::available("metadata.minecraft"),
        CapabilitySummary::available("metadata.fabric"),
        CapabilitySummary::available("metadata.neoforge"),
        CapabilitySummary::available("minecraft.install"),
        CapabilitySummary::available("minecraft.launch"),
        CapabilitySummary::available("minecraft.session_logs"),
        CapabilitySummary::available("content.modpacks"),
        CapabilitySummary::available("content.modpacks.install"),
        CapabilitySummary::available("content.mods"),
        CapabilitySummary::available("content.mods.install"),
        if credential_vault_ready {
            CapabilitySummary::available("minecraft.account")
        } else {
            CapabilitySummary::unavailable(
                "minecraft.account",
                "Secure account storage needs attention.",
            )
        },
    ])
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
        StorageError::AccountNotFound => AppError::new(
            "auth.account_not_found",
            "Sign in to a Minecraft account before playing.",
        ),
        StorageError::InstanceNotFound => AppError::new(
            "local.instance_not_found",
            "That instance no longer exists.",
        ),
        StorageError::RevisionConflict { .. } => AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ),
        StorageError::InstanceBusy => AppError::new(
            "local.instance_busy",
            "Wait for the current installation or stop Minecraft before changing instance content.",
        ),
        _ => AppError::new("local.storage_unavailable", fallback).retryable(true),
    }
}

fn metadata_error() -> AppError {
    AppError::new(
        "metadata.catalog_unavailable",
        "slate could not load the latest Minecraft versions.",
    )
    .retryable(true)
}

fn install_job_summary(record: InstallJobRecord) -> InstallJobSummary {
    let message = serde_json::from_str::<serde_json::Value>(&record.progress_json)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(|message| message.as_str())
                .map(str::to_owned)
        })
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
        completed_items: record.completed_items,
        total_items: record.total_items,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn game_session_summary(process: ActiveProcess) -> GameSessionSummary {
    GameSessionSummary {
        id: process.session_id.as_uuid(),
        instance_id: process.instance_id.as_uuid(),
        state: match process.state {
            ProcessState::Running => GameSessionStateDto::Running,
            ProcessState::Stopping => GameSessionStateDto::Stopping,
        },
        mode: "authenticated".to_owned(),
        pid: process.pid,
        log_name: process.log_path.file_name().map_or_else(
            || "session.log".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
    }
}

fn process_state_error(_: slate_process::ProcessError) -> AppError {
    AppError::new(
        "local.process_state_unavailable",
        "slate could not check whether Minecraft is already running.",
    )
    .retryable(true)
}

fn main() {
    let boot_paths = match AppPaths::discover().and_then(|paths| {
        paths.ensure_base_directories()?;
        Ok(paths)
    }) {
        Ok(paths) => paths,
        Err(_) => {
            eprintln!("slate could not prepare its local data folders.");
            std::process::exit(1);
        }
    };
    let diagnostic_guard = match init_diagnostics(&boot_paths.logs()) {
        Ok(guard) => Some(guard),
        Err(_) => {
            eprintln!("slate could not start local diagnostics; the launcher will continue.");
            None
        }
    };
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        "desktop starting"
    );
    let setup_paths = boot_paths.clone();
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let paths = setup_paths.clone();
            let database =
                tauri::async_runtime::block_on(Database::connect(&paths.state_database()))?;
            tauri::async_runtime::block_on(database.recover_interrupted_installs())?;
            let canonical_storage = std::fs::canonicalize(paths.storage_root())?;
            let canonical_storage = canonical_storage.to_string_lossy().into_owned();
            let storage_root_id = tauri::async_runtime::block_on(
                database.get_or_create_storage_root(&canonical_storage),
            )?;
            let auth_client = MinecraftAuthClient::new(MinecraftAuthConfig::new(
                SLATE_MICROSOFT_CLIENT_ID,
                MICROSOFT_CONSUMER_TENANT,
            )?)?;
            let modpacks = ModpackApiClient::for_current_build()?;
            let state = DesktopState {
                database,
                paths,
                storage_root_id,
                processes: ProcessSupervisor::default(),
                auth_client,
                credential_vault: CredentialVault,
                auth_flows: AuthCoordinator::default(),
                log_streams: SessionLogCoordinator::default(),
                modpacks,
            };
            app.manage(state.clone());
            tauri::async_runtime::spawn(purge_expired_instance_trash(state));
            tracing::info!("desktop ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_bootstrap,
            accounts_list,
            auth_start,
            auth_get_status,
            auth_cancel,
            account_refresh,
            account_set_default,
            account_remove,
            minecraft_versions_list,
            loader_versions_list,
            instances_list,
            instance_get,
            instance_create,
            instance_rename,
            instance_update_configuration,
            instance_update_settings,
            instance_select_java,
            instance_select_artwork,
            instance_reset_artwork,
            instance_get_artwork,
            instance_game_options_get,
            instance_game_options_update,
            instance_open_directory,
            instance_duplicate,
            instance_move_storage,
            instance_export,
            instance_import,
            instance_snapshots_list,
            instance_snapshot_create,
            instance_snapshot_restore,
            instance_snapshot_delete,
            instance_snapshot_set_pinned,
            instance_set_favorite,
            instance_trash,
            storage_overview,
            storage_clear_category,
            trashed_instance_restore,
            trashed_instance_delete,
            trashed_instances_empty,
            instance_install,
            install_jobs_list,
            instance_launch,
            sessions_list,
            session_force_stop,
            session_log_subscribe,
            session_log_unsubscribe,
            servers_list,
            server_create,
            server_update,
            server_remove,
            server_ping,
            preferences_get,
            preferences_update,
            preflight_get,
            onboarding_get,
            onboarding_select_storage,
            onboarding_complete,
            modpack_providers,
            modpacks_search,
            mods_search,
            modpack_get,
            modpack_versions_list,
            modpack_version_get,
            modpack_install,
            modpack_update_check,
            modpack_update_apply,
            instance_mods_list,
            instance_mods_resolve,
            instance_content_files_list,
            instance_content_file_set_enabled,
            instance_content_file_remove,
            instance_mod_set_enabled,
            instance_mod_set_pinned,
            instance_mod_remove,
            instance_mod_install,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = application {
        tracing::error!("desktop stopped because startup or runtime setup failed");
        drop(diagnostic_guard);
        eprintln!("slate failed to start: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InstalledModArtifact, installed_artifact_matches, parse_mod_download_id, same_mod_artifact,
    };
    use slate_modpack_api_contracts::{Hashes, Provider};

    #[test]
    fn mod_install_plan_ids_preserve_dependency_identity() {
        assert_eq!(
            parse_mod_download_id("modrinth:AANobbMI:version-id"),
            Some((
                Provider::Modrinth,
                "AANobbMI".to_owned(),
                "version-id".to_owned()
            ))
        );
        assert_eq!(
            parse_mod_download_id("curseforge:1689768:8928631"),
            Some((
                Provider::CurseForge,
                "1689768".to_owned(),
                "8928631".to_owned()
            ))
        );
        assert_eq!(parse_mod_download_id("ftb:1:2"), None);
        assert_eq!(parse_mod_download_id("modrinth:missing-version"), None);
    }

    #[test]
    fn equivalent_cross_provider_files_share_one_download() {
        let modrinth = Hashes {
            sha512: Some("a".repeat(128)),
            sha256: None,
            sha1: Some("b".repeat(40)),
        };
        let curseforge = Hashes {
            sha512: None,
            sha256: None,
            sha1: Some("b".repeat(40)),
        };
        assert!(same_mod_artifact(42, &modrinth, 42, &curseforge));
        assert!(!same_mod_artifact(42, &modrinth, 43, &curseforge));
        assert!(!same_mod_artifact(
            42,
            &modrinth,
            42,
            &Hashes {
                sha512: None,
                sha256: None,
                sha1: Some("c".repeat(40)),
            }
        ));
    }

    #[test]
    fn installed_cross_provider_dependency_is_reused_when_hashes_match() {
        let installed = InstalledModArtifact {
            size: 2_521_340,
            known_hashes: vec![Hashes {
                sha512: Some("a".repeat(128)),
                sha256: None,
                sha1: Some("16c30dfcceaed1ac3b6ac84cbd348a906dc5093e".to_owned()),
            }],
        };
        let curseforge_dependency = Hashes {
            sha512: None,
            sha256: None,
            sha1: Some("16c30dfcceaed1ac3b6ac84cbd348a906dc5093e".to_owned()),
        };

        assert!(installed_artifact_matches(
            &installed,
            2_521_340,
            &curseforge_dependency
        ));
        assert!(!installed_artifact_matches(
            &installed,
            2_521_341,
            &curseforge_dependency
        ));
    }
}
