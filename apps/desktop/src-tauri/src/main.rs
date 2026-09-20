#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod instance_content;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use instance_content::{
    FileMove, InstanceModFile, scan_instance_mods, set_instance_mod_enabled, trash_instance_mod,
};
use slate_auth::{
    AuthError, CredentialVault, MICROSOFT_CONSUMER_TENANT, MinecraftAuthClient,
    MinecraftAuthConfig, MinecraftSession, SLATE_MICROSOFT_CLIENT_ID,
};
use slate_contracts::{
    AccountIdRequest, AppError, AppPreferencesDto, AuthCancelRequest, AuthFlowStateDto,
    AuthFlowStatus, AuthStartResponse, BootstrapResponse, CapabilitySummary, CreateInstanceRequest,
    GameSessionStateDto, GameSessionSummary, GetInstanceArtworkRequest, InstallInstanceRequest,
    InstallJobStateDto, InstallJobSummary, InstallModRequest, InstallModpackRequest,
    InstanceArtworkAsset, InstanceArtworkKindDto, InstanceModOriginDto, InstanceModResolution,
    InstanceModSummary, InstanceModeDto, InstanceModsRequest, InstanceSettingsSummary,
    InstanceSummary, InstanceWindowModeDto, JavaRuntimeSummary, JavaSelectionModeDto,
    LaunchInstanceRequest, LauncherBehaviorDto, LoaderKindDto, LoaderVersionCatalog,
    LoaderVersionsRequest, MemoryModeDto, MinecraftAccountStatusDto, MinecraftAccountSummary,
    MinecraftReleaseKindDto, MinecraftVersionCatalog, MinecraftVersionOption, ModSearchRequest,
    ModpackInstallStarted, ModpackProjectRequest, ModpackSearchRequest, ModpackSortDto,
    ModpackSourceSummary, ModpackVersionRequest, ModpackVersionsRequest, PerformancePresetDto,
    PreflightSummary, ProcessPriorityDto, ReduceMotionPreferenceDto, RemoveInstanceModRequest,
    RenameInstanceRequest, SelectInstanceArtworkRequest, SelectInstanceJavaRequest,
    SessionLogEvent, SessionLogEventKindDto, SessionLogSubscription, SetDefaultAccountRequest,
    SetFavoriteRequest, SetInstanceModEnabledRequest, StopGameSessionRequest,
    SubscribeSessionLogRequest, ThemePreferenceDto, TrashInstanceRequest,
    UnsubscribeSessionLogRequest, UpdateAppPreferencesRequest, UpdateInstanceConfigurationRequest,
    UpdateInstanceSettingsRequest,
};
use slate_domain::{
    AccountId, InstanceId, InstanceName, InstanceNameError, LoaderFamily, ManagementMode,
    RequestId, RevisionId, SessionId, StorageRootId,
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
    SearchResponse, VersionPage,
};
use slate_modpack_client::{ModpackApiClient, SearchOptions, SearchSort, VersionOptions};
use slate_platform::{
    AppPaths, JavaArchitecture, JavaRuntimeProbe, detect_java_runtime, probe_java_executable,
    restricted_child_environment,
};
use slate_process::{
    ActiveProcess, ChildProcessPriority, LogChunk, LogChunkKind, ProcessState, ProcessSupervisor,
    SessionLogTail,
};
use slate_storage::{
    AccountRecord, AccountStatus, AppPreferences, AuthenticatedAccount, CompletedInstall, Database,
    InstallJobRecord, InstalledRuntime, InstanceModEnabledChange, InstanceModRecord,
    InstanceModTarget, InstanceRecord, InstanceWindowMode, JavaSelectionMode, JobState,
    LauncherBehavior, MemoryMode, NewInstance, NewInstanceMod, NewModpackSource, PerformancePreset,
    ProcessPriority, ReduceMotionPreference, StorageError, ThemePreference, UpdateInstanceSettings,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

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
                "The operating-system credential vault is unavailable.",
            )
        },
    ])
}

#[tauri::command]
async fn modpack_providers(
    state: tauri::State<'_, DesktopState>,
) -> Result<ProvidersResponse, AppError> {
    state.modpacks.providers().await.map_err(modpack_api_error)
}

#[tauri::command]
async fn modpacks_search(
    state: tauri::State<'_, DesktopState>,
    request: ModpackSearchRequest,
) -> Result<SearchResponse, AppError> {
    state
        .modpacks
        .search(&SearchOptions {
            query: request.query,
            provider: request.provider,
            minecraft_version: request.minecraft_version,
            loader: request.loader,
            category: request.category,
            sort: match request.sort {
                ModpackSortDto::Relevance => SearchSort::Relevance,
                ModpackSortDto::Downloads => SearchSort::Downloads,
                ModpackSortDto::Updated => SearchSort::Updated,
                ModpackSortDto::Newest => SearchSort::Newest,
            },
            cursor: request.cursor,
            page: request.page,
            limit: request.limit.unwrap_or(20),
        })
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
async fn mods_search(
    state: tauri::State<'_, DesktopState>,
    request: ModSearchRequest,
) -> Result<SearchResponse, AppError> {
    let instance = state
        .database
        .get_instance(InstanceId::from_uuid(request.instance_id))
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let (loader, _) = instance_mod_target(&instance)?;
    state
        .modpacks
        .search_mods(&SearchOptions {
            query: request.query,
            provider: request.provider,
            minecraft_version: Some(instance.minecraft_version),
            loader: Some(loader),
            category: None,
            sort: match request.sort {
                ModpackSortDto::Relevance => SearchSort::Relevance,
                ModpackSortDto::Downloads => SearchSort::Downloads,
                ModpackSortDto::Updated => SearchSort::Updated,
                ModpackSortDto::Newest => SearchSort::Newest,
            },
            cursor: request.cursor,
            page: request.page,
            limit: request.limit.unwrap_or(20),
        })
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
async fn instance_mods_list(
    state: tauri::State<'_, DesktopState>,
    request: InstanceModsRequest,
) -> Result<Vec<InstanceModSummary>, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let stored_mods = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load installed mods."))?;
    let paths = state.paths.clone();
    let files = tokio::task::spawn_blocking(move || scan_instance_mods(&paths, instance_id))
        .await
        .map_err(|_| {
            AppError::new(
                "local.mod_inventory_failed",
                "slate could not inspect the instance mod directory.",
            )
        })?
        .map_err(|_| {
            AppError::new(
                "local.mod_inventory_failed",
                "slate could not inspect the instance mod directory.",
            )
        })?;
    let mut stored_by_path = stored_mods
        .into_iter()
        .map(|record| (normalized_content_path(&record.file_path), record))
        .collect::<HashMap<_, _>>();
    let untracked_origin = if instance.modpack_source.is_some() {
        InstanceModOriginDto::Modpack
    } else {
        InstanceModOriginDto::Local
    };
    Ok(files
        .into_iter()
        .map(|file| {
            let record = stored_by_path.remove(&normalized_content_path(&file.file_path));
            instance_mod_summary(record, file, untracked_origin.clone())
        })
        .collect())
}

#[tauri::command]
async fn instance_mods_resolve(
    state: tauri::State<'_, DesktopState>,
    request: InstanceModsRequest,
) -> Result<Vec<InstanceModResolution>, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let stored_mods = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load installed mods."))?;
    let mut references_by_path = stored_mods
        .into_iter()
        .map(|record| {
            (
                normalized_content_path(&record.file_path),
                ProviderReference {
                    provider: record.provider,
                    project_id: record.project_id,
                    version_id: Some(record.version_id),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    if let Some(source) = instance.modpack_source {
        match state
            .modpacks
            .version(source.provider, &source.project_id, &source.version_id)
            .await
        {
            Ok(version) => {
                for file in version.files {
                    if file.kind == PackFileType::Mod
                        && let Some(reference) = file.source
                        && reference.provider != slate_modpack_api_contracts::Provider::Ftb
                    {
                        references_by_path
                            .entry(normalized_content_path(&file.path))
                            .or_insert(reference);
                    }
                }
            }
            Err(error) => tracing::warn!(
                instance_id = %request.instance_id,
                error = %error,
                "could not load modpack sources for installed mod resolution"
            ),
        }
    }

    let unique_projects = references_by_path
        .values()
        .map(|reference| {
            (
                (reference.provider, reference.project_id.clone()),
                ModProjectReference {
                    provider: reference.provider,
                    project_id: reference.project_id.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    let resolved = if unique_projects.is_empty() {
        Vec::new()
    } else {
        match state
            .modpacks
            .resolve_mods(&ResolveModsRequest {
                items: unique_projects,
            })
            .await
        {
            Ok(response) => response.items,
            Err(error) => {
                tracing::warn!(
                    instance_id = %request.instance_id,
                    error = %error,
                    "provider mod metadata resolution failed"
                );
                Vec::new()
            }
        }
    };
    let resolved_by_project = resolved
        .into_iter()
        .map(|project| ((project.provider, project.project_id.clone()), project))
        .collect::<BTreeMap<_, _>>();

    Ok(references_by_path
        .into_iter()
        .map(|(file_path, reference)| {
            let project =
                resolved_by_project.get(&(reference.provider, reference.project_id.clone()));
            InstanceModResolution {
                file_path,
                provider: reference.provider,
                project_id: reference.project_id,
                version_id: reference.version_id,
                display_name: project.map(|project| project.name.clone()),
                icon_url: project.and_then(|project| project.icon_url.clone()),
            }
        })
        .collect())
}

#[tauri::command]
async fn instance_mod_set_enabled(
    state: tauri::State<'_, DesktopState>,
    request: SetInstanceModEnabledRequest,
) -> Result<InstanceSummary, AppError> {
    let reference =
        validate_instance_mod_reference(request.provider, request.project_id.as_deref())?;
    let instance_id = InstanceId::from_uuid(request.instance_id);
    prepare_instance_content_change(state.inner(), instance_id, request.expected_revision).await?;
    let paths = state.paths.clone();
    let file_path = request.file_path.clone();
    let enabled = request.enabled;
    let file_move = tokio::task::spawn_blocking(move || {
        set_instance_mod_enabled(&paths, instance_id, &file_path, enabled)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let updated_file_path = file_move
        .updated_file_path
        .as_deref()
        .ok_or_else(content_file_error)?;
    let database_result = state
        .database
        .set_instance_mod_enabled(
            instance_id,
            request.expected_revision,
            InstanceModEnabledChange {
                target: InstanceModTarget {
                    provider: reference.map(|(provider, _)| provider),
                    project_id: reference.map(|(_, project_id)| project_id),
                    file_path: &request.file_path,
                },
                updated_file_path,
                enabled: request.enabled,
            },
        )
        .await;
    finish_instance_content_change(
        state.inner(),
        instance_id,
        file_move,
        database_result,
        "slate could not update that mod.",
    )
    .await
}

#[tauri::command]
async fn instance_mod_remove(
    state: tauri::State<'_, DesktopState>,
    request: RemoveInstanceModRequest,
) -> Result<InstanceSummary, AppError> {
    let reference =
        validate_instance_mod_reference(request.provider, request.project_id.as_deref())?;
    let instance_id = InstanceId::from_uuid(request.instance_id);
    prepare_instance_content_change(state.inner(), instance_id, request.expected_revision).await?;
    let paths = state.paths.clone();
    let file_path = request.file_path.clone();
    let file_move =
        tokio::task::spawn_blocking(move || trash_instance_mod(&paths, instance_id, &file_path))
            .await
            .map_err(|_| content_file_error())?
            .map_err(|_| content_file_error())?;
    let database_result = state
        .database
        .remove_instance_mod(
            instance_id,
            request.expected_revision,
            InstanceModTarget {
                provider: reference.map(|(provider, _)| provider),
                project_id: reference.map(|(_, project_id)| project_id),
                file_path: &request.file_path,
            },
        )
        .await;
    finish_instance_content_change(
        state.inner(),
        instance_id,
        file_move,
        database_result,
        "slate could not remove that mod.",
    )
    .await
}

fn validate_instance_mod_reference(
    provider: Option<slate_modpack_api_contracts::Provider>,
    project_id: Option<&str>,
) -> Result<Option<(slate_modpack_api_contracts::Provider, &str)>, AppError> {
    match (provider, project_id) {
        (None, None) => Ok(None),
        (Some(provider), Some(project_id))
            if provider != slate_modpack_api_contracts::Provider::Ftb
                && !project_id.is_empty()
                && project_id.len() <= 128
                && project_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) =>
        {
            Ok(Some((provider, project_id)))
        }
        _ => Err(AppError::new(
            "mod.invalid_reference",
            "The installed mod reference is invalid.",
        )),
    }
}

async fn prepare_instance_content_change(
    state: &DesktopState,
    instance_id: InstanceId,
    expected_revision: u64,
) -> Result<(), AppError> {
    refresh_exited_sessions(state).await;
    if state
        .processes
        .active_for_instance(instance_id)
        .map_err(process_state_error)?
        .is_some()
    {
        return Err(AppError::new(
            "local.instance_running",
            "Stop Minecraft before changing this instance's content.",
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
    Ok(())
}

async fn finish_instance_content_change(
    state: &DesktopState,
    instance_id: InstanceId,
    file_move: FileMove,
    database_result: Result<(), StorageError>,
    fallback: &'static str,
) -> Result<InstanceSummary, AppError> {
    if let Err(error) = database_result {
        let rollback = tokio::task::spawn_blocking(move || file_move.rollback()).await;
        if !matches!(rollback, Ok(Ok(()))) {
            return Err(AppError::new(
                "local.content_rollback_failed",
                "slate could not restore the mod after the change failed. The instance needs attention.",
            ));
        }
        return Err(map_storage_error(error, fallback));
    }
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| {
            map_storage_error(
                error,
                "slate changed the mod but could not refresh the instance.",
            )
        })
}

fn content_file_error() -> AppError {
    AppError::new(
        "local.mod_file_change_failed",
        "slate could not safely change that mod file.",
    )
}

#[tauri::command]
async fn modpack_get(
    state: tauri::State<'_, DesktopState>,
    request: ModpackProjectRequest,
) -> Result<Modpack, AppError> {
    state
        .modpacks
        .project(request.provider, &request.project_id)
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
async fn modpack_versions_list(
    state: tauri::State<'_, DesktopState>,
    request: ModpackVersionsRequest,
) -> Result<VersionPage, AppError> {
    state
        .modpacks
        .versions(
            request.provider,
            &request.project_id,
            &VersionOptions {
                minecraft_version: request.minecraft_version,
                loader: request.loader,
                release_type: request.release_type,
                cursor: request.cursor,
                page: request.page,
                limit: request.limit.unwrap_or(20),
            },
        )
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
async fn modpack_version_get(
    state: tauri::State<'_, DesktopState>,
    request: ModpackVersionRequest,
) -> Result<ModpackVersion, AppError> {
    state
        .modpacks
        .version(request.provider, &request.project_id, &request.version_id)
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
async fn accounts_list(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<MinecraftAccountSummary>, AppError> {
    state
        .database
        .list_accounts()
        .await
        .map(|accounts| accounts.into_iter().map(account_summary).collect())
        .map_err(|error| map_storage_error(error, "slate could not load Minecraft accounts."))
}

#[tauri::command]
async fn auth_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<AuthStartResponse, AppError> {
    if let Some((start, authorization_url)) = state.auth_flows.active_start()? {
        app.opener()
            .open_url(authorization_url, None::<&str>)
            .map_err(|_| {
                AppError::new(
                    "auth.browser_unavailable",
                    "slate could not reopen the system browser. Check your default browser and try again.",
                )
            })?;
        return Ok(start);
    }
    state.credential_vault.check_available().map_err(|_| {
        AppError::new(
            "auth.credential_vault_unavailable",
            "The operating-system credential vault is unavailable. Unlock or configure it before signing in.",
        )
    })?;
    let (start, pending) = state
        .auth_client
        .begin_login()
        .await
        .map_err(map_auth_error)?;
    let flow_id = start.flow_id;
    let expires_at = (OffsetDateTime::now_utc()
        + time::Duration::seconds(i64::try_from(start.expires_in.as_secs()).unwrap_or(5 * 60)))
    .format(&Rfc3339)
    .map_err(|_| auth_state_error())?;
    state.auth_flows.insert_waiting(
        flow_id,
        expires_at.clone(),
        start.authorization_url.to_string(),
    )?;
    if app
        .opener()
        .open_url(start.authorization_url.as_str(), None::<&str>)
        .is_err()
    {
        state.auth_flows.fail(
            flow_id,
            "slate could not open the system browser. Check your default browser and try again."
                .to_owned(),
        );
        return Err(AppError::new(
            "auth.browser_unavailable",
            "slate could not open the system browser. Check your default browser and try again.",
        ));
    }
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
    state.auth_flows.attach(flow_id, cancel_tx)?;
    let task_state = state.inner().clone();
    let verifying = task_state.auth_flows.clone();
    tauri::async_runtime::spawn(async move {
        let result = tokio::select! {
            result = pending.complete_with_callback(move || verifying.set_verifying(flow_id)) => result,
            _ = cancel_rx => return,
        };
        let completed = match result {
            Ok(completed) => completed,
            Err(error) => {
                eprintln!("[slate-auth] flow {flow_id} failed: {error}");
                task_state
                    .auth_flows
                    .fail(flow_id, auth_error_message(&error));
                return;
            }
        };
        let credential_ref = CredentialVault::credential_ref(completed.profile.id);
        let vault = task_state.credential_vault.clone();
        let stored_ref = credential_ref.clone();
        let refresh_token = completed.refresh_token;
        let stored =
            tauri::async_runtime::spawn_blocking(move || vault.store(&stored_ref, &refresh_token))
                .await;
        if !matches!(stored, Ok(Ok(()))) {
            task_state.auth_flows.fail(
                flow_id,
                "Minecraft was verified, but the refresh credential could not be saved in the operating-system vault."
                    .to_owned(),
            );
            return;
        }
        let account = task_state
            .database
            .upsert_authenticated_account(AuthenticatedAccount {
                profile_id: completed.profile.id,
                display_name: completed.profile.name,
                credential_ref: credential_ref.clone(),
                skin_url: completed.profile.skin_url,
            })
            .await;
        match account {
            Ok(account) => task_state
                .auth_flows
                .succeed(flow_id, account_summary(account)),
            Err(_) => {
                let vault = task_state.credential_vault.clone();
                let _ = tauri::async_runtime::spawn_blocking(move || vault.remove(&credential_ref))
                    .await;
                task_state.auth_flows.fail(
                    flow_id,
                    "Minecraft was verified, but slate could not save the local account record."
                        .to_owned(),
                );
            }
        }
    });
    Ok(AuthStartResponse {
        flow_id,
        expires_at,
    })
}

#[tauri::command]
fn auth_get_status(
    state: tauri::State<'_, DesktopState>,
    flow_id: Uuid,
) -> Result<AuthFlowStatus, AppError> {
    state.auth_flows.status(flow_id)
}

#[tauri::command]
fn auth_cancel(
    state: tauri::State<'_, DesktopState>,
    request: AuthCancelRequest,
) -> Result<AuthFlowStatus, AppError> {
    state.auth_flows.cancel(request.flow_id)
}

#[tauri::command]
async fn account_refresh(
    state: tauri::State<'_, DesktopState>,
    request: AccountIdRequest,
) -> Result<MinecraftAccountSummary, AppError> {
    refresh_account(state.inner(), AccountId::from_uuid(request.id)).await
}

#[tauri::command]
async fn account_set_default(
    state: tauri::State<'_, DesktopState>,
    request: SetDefaultAccountRequest,
) -> Result<MinecraftAccountSummary, AppError> {
    state
        .database
        .set_default_account(AccountId::from_uuid(request.id))
        .await
        .map(account_summary)
        .map_err(|error| map_storage_error(error, "slate could not change the default account."))
}

#[tauri::command]
async fn account_remove(
    state: tauri::State<'_, DesktopState>,
    request: AccountIdRequest,
) -> Result<(), AppError> {
    let credential_ref = state
        .database
        .remove_account(AccountId::from_uuid(request.id))
        .await
        .map_err(|error| map_storage_error(error, "slate could not remove that account."))?;
    let vault = state.credential_vault.clone();
    tauri::async_runtime::spawn_blocking(move || vault.remove(&credential_ref))
        .await
        .map_err(|_| auth_state_error())?
        .map_err(|_| {
            AppError::new(
                "auth.credential_cleanup_failed",
                "The account was removed from slate, but its operating-system credential could not be deleted.",
            )
        })
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
            modpack_source: None,
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
async fn instance_update_settings(
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
async fn instance_select_java(
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
async fn instance_select_artwork(
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
    let destination = instance_artwork_path(&state.paths, instance_id, kind);
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
async fn instance_reset_artwork(
    state: tauri::State<'_, DesktopState>,
    request: SelectInstanceArtworkRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.id);
    let destination = instance_artwork_path(&state.paths, instance_id, request.kind);
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
async fn instance_get_artwork(
    state: tauri::State<'_, DesktopState>,
    request: GetInstanceArtworkRequest,
) -> Result<Option<InstanceArtworkAsset>, AppError> {
    let instance = state
        .database
        .get_instance(InstanceId::from_uuid(request.id))
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let mime = match request.kind {
        InstanceArtworkKindDto::Icon => instance.settings.icon_mime,
        InstanceArtworkKindDto::Banner => instance.settings.banner_mime,
    };
    let Some(mime_type) = mime else {
        return Ok(None);
    };
    let path = instance_artwork_path(&state.paths, instance.id, request.kind);
    let bytes = tokio::fs::read(path).await.map_err(|_| artwork_error())?;
    Ok(Some(InstanceArtworkAsset {
        mime_type,
        data_base64: BASE64_STANDARD.encode(bytes),
    }))
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

#[tauri::command]
async fn instance_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallInstanceRequest,
) -> Result<InstallJobSummary, AppError> {
    refresh_exited_sessions(state.inner()).await;
    let instance_id = InstanceId::from_uuid(request.id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let plan = if let Some(source) = &instance.modpack_source {
        Some(fetch_instance_modpack_plan(state.inner(), source).await?)
    } else {
        None
    };
    queue_instance_install(
        state.inner(),
        instance,
        request.expected_revision,
        plan,
        Vec::new(),
    )
    .await
}

async fn queue_instance_install(
    state: &DesktopState,
    instance: InstanceRecord,
    expected_revision: u64,
    mut modpack_plan: Option<InstallPlan>,
    pending_mods: Vec<NewInstanceMod>,
) -> Result<InstallJobSummary, AppError> {
    let instance_id = instance.id;
    if state
        .processes
        .active_for_instance(instance_id)
        .map_err(process_state_error)?
        .is_some()
    {
        return Err(AppError::new(
            "local.instance_running",
            "Minecraft is already running for this instance.",
        ));
    }
    if instance.setup_state == slate_domain::InstanceSetupState::Preparing {
        return Err(AppError::new(
            "local.install_already_running",
            "That instance already has an installation in progress.",
        ));
    }
    let content_update = if pending_mods.is_empty() {
        None
    } else {
        let plan = modpack_plan.take().ok_or_else(|| {
            AppError::new(
                "mod.invalid_plan",
                "The mod service returned an empty content plan.",
            )
        })?;
        let parent = state
            .database
            .get_installed_revision(instance_id)
            .await
            .map_err(|error| {
                map_storage_error(
                    error,
                    "Install the base instance before adding individual mods.",
                )
            })?;
        Some((parent, plan))
    };
    let preferences = state
        .database
        .get_app_preferences()
        .await
        .map_err(|error| map_storage_error(error, "slate could not load download settings."))?;
    let pending = state
        .database
        .begin_instance_install(instance_id, expected_revision, RequestId::new())
        .await
        .map_err(|error| map_storage_error(error, "slate could not queue the installation."))?;
    let response = install_job_summary(pending.job.clone());
    let task_state = state.clone();
    tauri::async_runtime::spawn(async move {
        let (progress_tx, mut progress_rx) =
            tokio::sync::mpsc::unbounded_channel::<InstallProgress>();
        let progress_gate = Arc::new(Mutex::new((None, Instant::now() - Duration::from_secs(1))));
        let progress_database = task_state.database.clone();
        let progress_job_id = pending.job.id;
        let progress_task = tauri::async_runtime::spawn(async move {
            while let Some(progress) = progress_rx.recv().await {
                let _ = progress_database
                    .update_install_progress(
                        progress_job_id,
                        progress.phase.as_str(),
                        &progress.message,
                        progress.completed_items,
                        progress.total_items,
                    )
                    .await;
            }
        });
        let progress_callback = move |progress: InstallProgress| {
            let now = Instant::now();
            let finished_stage =
                progress.completed_items == progress.total_items && progress.total_items.is_some();
            let should_send = progress_gate.lock().is_ok_and(|mut gate| {
                let phase_changed = gate.0 != Some(progress.phase);
                if phase_changed
                    || finished_stage
                    || now.duration_since(gate.1) >= Duration::from_millis(125)
                {
                    *gate = (Some(progress.phase), now);
                    true
                } else {
                    false
                }
            });
            if should_send {
                let _ = progress_tx.send(progress);
            }
        };
        let result = if let Some((parent, plan)) = content_update {
            update_content_with_progress(
                ContentUpdateRequest {
                    instance_id,
                    revision_id: pending.revision_id,
                    parent_revision_id: parent.id,
                    parent_manifest_digest: parent.manifest_digest,
                    minecraft_version: instance.minecraft_version,
                    loader_kind: instance.loader_kind,
                    loader_version: instance.loader_version,
                    plan,
                    download_concurrency: preferences.download_concurrency,
                    paths: task_state.paths.clone(),
                },
                progress_callback,
            )
            .await
        } else {
            install_with_progress(
                NativeInstallRequest {
                    instance_id,
                    revision_id: pending.revision_id,
                    minecraft_version: instance.minecraft_version,
                    loader_kind: instance.loader_kind,
                    loader_version: instance.loader_version,
                    modpack_plan,
                    download_concurrency: preferences.download_concurrency,
                    paths: task_state.paths.clone(),
                },
                progress_callback,
            )
            .await
        };
        let _ = progress_task.await;
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
                let message = if !pending_mods.is_empty() {
                    format!(
                        "Installed {} verified mod files without reinstalling the base instance",
                        outcome.installed_content_files
                    )
                } else if outcome.installed_content_files > 0 {
                    format!(
                        "Ready with {} verified pack files; downloaded {} game files and reused {} cached files",
                        outcome.installed_content_files,
                        outcome.downloaded_artifacts,
                        outcome.reused_artifacts
                    )
                } else {
                    format!(
                        "Installed and verified {} files; reused {} cached files",
                        outcome.downloaded_artifacts, outcome.reused_artifacts
                    )
                };
                let completion = if pending_mods.is_empty() {
                    task_state
                        .database
                        .complete_instance_install(
                            pending.job.id,
                            pending.revision_id,
                            &outcome.manifest_digest,
                            &outcome.resolved_version_id,
                            runtime,
                            &message,
                        )
                        .await
                } else {
                    task_state
                        .database
                        .complete_instance_mod_install(
                            CompletedInstall {
                                job_id: pending.job.id,
                                revision_id: pending.revision_id,
                                manifest_digest: outcome.manifest_digest,
                                client_version: outcome.resolved_version_id,
                                runtime,
                                message,
                            },
                            pending_mods,
                        )
                        .await
                };
                if let Err(error) = completion {
                    tracing::error!(
                        job_id = %pending.job.id,
                        error = %error,
                        "could not commit a completed instance installation"
                    );
                }
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

async fn fetch_instance_modpack_plan(
    state: &DesktopState,
    source: &slate_storage::ModpackSourceRecord,
) -> Result<InstallPlan, AppError> {
    state
        .modpacks
        .install_plan(
            source.provider,
            &source.project_id,
            &source.version_id,
            &InstallPlanRequest {
                platform: current_modpack_platform(),
                arch: current_modpack_architecture()?,
                include_optional: source.selected_optional.clone(),
            },
        )
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
async fn modpack_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallModpackRequest,
) -> Result<ModpackInstallStarted, AppError> {
    refresh_exited_sessions(state.inner()).await;
    if request.include_optional.len() > 1_000 {
        return Err(AppError::new(
            "modpack.too_many_options",
            "Too many optional files were selected.",
        ));
    }
    let name = parse_instance_name(&request.instance_name).map_err(instance_name_app_error)?;
    let project = state
        .modpacks
        .project(request.provider, &request.project_id)
        .await
        .map_err(modpack_api_error)?;
    let version = state
        .modpacks
        .version(request.provider, &request.project_id, &request.version_id)
        .await
        .map_err(modpack_api_error)?;
    let plan = state
        .modpacks
        .install_plan(
            request.provider,
            &request.project_id,
            &request.version_id,
            &InstallPlanRequest {
                platform: current_modpack_platform(),
                arch: current_modpack_architecture()?,
                include_optional: request.include_optional.clone(),
            },
        )
        .await
        .map_err(modpack_api_error)?;
    validate_resolved_modpack(&request, &version, &plan)?;

    let loader_kind = launcher_loader_kind(plan.runtime.loader.kind)?;
    let loader_version = validate_selected_loader_version(
        &plan.runtime.minecraft,
        loader_kind,
        plan.runtime.loader.version.as_deref(),
    )
    .await?;
    let memory_mb = plan.runtime.memory.recommended_mb.clamp(1_024, 32_768);
    validate_instance_configuration(
        InstanceModeDto::Modded,
        &plan.runtime.minecraft,
        loader_kind,
        loader_version.as_deref(),
        memory_mb,
    )
    .map_err(configuration_app_error)?;

    let record = state
        .database
        .create_instance(NewInstance {
            name,
            mode: slate_domain::InstanceMode::Modded,
            management_mode: ManagementMode::Local,
            root_id: state.storage_root_id,
            minecraft_version: plan.runtime.minecraft.clone(),
            loader_kind: loader_kind.into(),
            loader_version,
            memory_mb,
            modpack_source: Some(NewModpackSource {
                provider: request.provider,
                project_id: request.project_id,
                version_id: request.version_id,
                selected_optional: request.include_optional,
                display_name: project.name,
                icon_url: project.icon_url,
                banner_url: project.banner_url,
            }),
        })
        .await
        .map_err(|error| {
            map_storage_error(error, "slate could not create the modpack instance.")
        })?;
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
    let job = match queue_instance_install(
        state.inner(),
        record.clone(),
        record.revision,
        Some(plan),
        Vec::new(),
    )
    .await
    {
        Ok(job) => job,
        Err(error) => {
            let _ = state
                .database
                .trash_instance(record.id, record.revision)
                .await;
            return Err(error);
        }
    };
    let installed_record = state
        .database
        .get_instance(record.id)
        .await
        .map_err(|error| {
            map_storage_error(error, "slate could not reload the modpack instance.")
        })?;
    Ok(ModpackInstallStarted {
        instance: instance_summary(installed_record),
        job,
    })
}

#[tauri::command]
async fn instance_mod_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallModRequest,
) -> Result<InstallJobSummary, AppError> {
    refresh_exited_sessions(state.inner()).await;
    if request.mods.is_empty() || request.mods.len() > 50 {
        return Err(AppError::new(
            "mod.invalid_selection",
            "Select between 1 and 50 mods to install at once.",
        ));
    }
    let mut requested = HashSet::new();
    for selection in &request.mods {
        let display_name = selection.display_name.trim();
        let project_id = selection.project_id.trim();
        if selection.provider == slate_modpack_api_contracts::Provider::Ftb
            || display_name.is_empty()
            || display_name.chars().count() > 160
            || display_name.chars().any(char::is_control)
            || project_id.is_empty()
            || project_id.len() > 128
            || !requested.insert(format!("{}:{project_id}", selection.provider))
        {
            return Err(AppError::new(
                "mod.invalid_selection",
                "The selected mod list contains an invalid or duplicate project.",
            ));
        }
    }
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let installed = installed_mod_index(state.inner(), &instance).await?;
    let (loader, loader_version) = instance_mod_target(&instance)?;
    let mut merged_plan: Option<InstallPlan> = None;
    let mut pending_by_identity = BTreeMap::<String, NewInstanceMod>::new();
    let mut planned_destinations = BTreeMap::<String, PlannedModDestination>::new();
    for selection in &request.mods {
        let root_identity = format!("{}:{}", selection.provider, selection.project_id.trim());
        if installed.identities.contains(&root_identity) {
            return Err(AppError::new(
                "mod.already_installed",
                format!(
                    "{} is already installed in this instance.",
                    selection.display_name
                ),
            ));
        }
        let mut plan = state
            .modpacks
            .mod_install_plan(
                selection.provider,
                selection.project_id.trim(),
                &ModInstallPlanRequest {
                    minecraft_version: instance.minecraft_version.clone(),
                    loader,
                    loader_version: Some(loader_version.clone()),
                },
            )
            .await
            .map_err(modpack_api_error)?;
        validate_instance_mod_plan(
            &instance,
            selection.provider,
            selection.project_id.trim(),
            &plan,
        )?;
        let mut accepted_downloads = Vec::new();
        for download in plan.downloads {
            let (provider, project_id, version_id) = parse_mod_download_id(&download.id)
                .ok_or_else(|| {
                    AppError::new(
                        "mod.invalid_plan",
                        "The mod service returned an invalid dependency identity.",
                    )
                })?;
            let dependency_identity = format!("{provider}:{project_id}");
            if installed.identities.contains(&dependency_identity) {
                continue;
            }
            if let Some(existing) = pending_by_identity.get_mut(&dependency_identity) {
                if existing.version_id != version_id {
                    return Err(AppError::new(
                        "mod.dependency_conflict",
                        "The selected mods require conflicting versions of the same dependency.",
                    ));
                }
                if dependency_identity == root_identity {
                    existing.display_name = selection.display_name.trim().to_owned();
                }
                continue;
            }
            let display_name = if dependency_identity == root_identity {
                selection.display_name.trim().to_owned()
            } else {
                let file_name = download
                    .destination
                    .rsplit('/')
                    .next()
                    .unwrap_or(project_id.as_str());
                file_name
                    .strip_suffix(".jar")
                    .unwrap_or(file_name)
                    .to_owned()
            };
            let normalized_destination = normalized_content_path(&download.destination);
            if let Some(existing) = installed.artifacts.get(&normalized_destination) {
                if !installed_artifact_matches(existing, download.size, &download.hashes) {
                    return Err(AppError::new(
                        "mod.file_conflict",
                        format!(
                            "{} conflicts with an existing mod file named {}.",
                            selection.display_name, download.destination
                        ),
                    ));
                }
                pending_by_identity.insert(
                    dependency_identity,
                    NewInstanceMod {
                        provider,
                        project_id,
                        version_id,
                        display_name,
                        file_path: download.destination,
                        hashes: download.hashes,
                    },
                );
                continue;
            }
            if let Some(existing) = planned_destinations.get(&normalized_destination) {
                if !same_mod_artifact(
                    existing.size,
                    &existing.hashes,
                    download.size,
                    &download.hashes,
                ) {
                    return Err(AppError::new(
                        "mod.file_conflict",
                        format!(
                            "{} and {} resolve to different files named {}. Deselect one and try again.",
                            existing.requested_name, selection.display_name, download.destination
                        ),
                    ));
                }
                pending_by_identity.insert(
                    dependency_identity,
                    NewInstanceMod {
                        provider,
                        project_id,
                        version_id,
                        display_name,
                        file_path: download.destination,
                        hashes: download.hashes,
                    },
                );
                continue;
            }
            planned_destinations.insert(
                normalized_destination,
                PlannedModDestination {
                    size: download.size,
                    hashes: download.hashes.clone(),
                    requested_name: selection.display_name.trim().to_owned(),
                },
            );
            pending_by_identity.insert(
                dependency_identity,
                NewInstanceMod {
                    provider,
                    project_id,
                    version_id,
                    display_name,
                    file_path: download.destination.clone(),
                    hashes: download.hashes.clone(),
                },
            );
            accepted_downloads.push(download);
        }
        plan.downloads = accepted_downloads;
        if let Some(merged) = &mut merged_plan {
            merged.downloads.extend(plan.downloads);
        } else {
            merged_plan = Some(plan);
        }
    }
    let mut plan = merged_plan.ok_or_else(|| {
        AppError::new(
            "mod.already_installed",
            "Every selected mod and dependency is already installed.",
        )
    })?;
    if plan.downloads.is_empty() || pending_by_identity.is_empty() {
        return Err(AppError::new(
            "mod.already_installed",
            "Every selected mod and dependency is already installed.",
        ));
    }
    plan.total_download_size = plan.downloads.iter().try_fold(0_u64, |total, download| {
        total.checked_add(download.size).ok_or_else(|| {
            AppError::new(
                "mod.invalid_plan",
                "The selected mod download size is invalid.",
            )
        })
    })?;
    queue_instance_install(
        state.inner(),
        instance,
        request.expected_revision,
        Some(plan),
        pending_by_identity.into_values().collect(),
    )
    .await
}

struct InstalledModIndex {
    identities: HashSet<String>,
    artifacts: HashMap<String, InstalledModArtifact>,
}

struct InstalledModArtifact {
    size: u64,
    known_hashes: Vec<Hashes>,
}

struct PlannedModDestination {
    size: u64,
    hashes: Hashes,
    requested_name: String,
}

fn same_mod_artifact(left_size: u64, left: &Hashes, right_size: u64, right: &Hashes) -> bool {
    if left_size != right_size {
        return false;
    }
    let comparisons = [
        (left.sha512.as_deref(), right.sha512.as_deref()),
        (left.sha256.as_deref(), right.sha256.as_deref()),
        (left.sha1.as_deref(), right.sha1.as_deref()),
    ];
    let mut matched = false;
    for (left, right) in comparisons {
        if let (Some(left), Some(right)) = (left, right) {
            if !left.eq_ignore_ascii_case(right) {
                return false;
            }
            matched = true;
        }
    }
    matched
}

fn installed_artifact_matches(
    installed: &InstalledModArtifact,
    planned_size: u64,
    planned_hashes: &Hashes,
) -> bool {
    installed.known_hashes.iter().any(|known_hashes| {
        same_mod_artifact(installed.size, known_hashes, planned_size, planned_hashes)
    })
}

async fn installed_mod_index(
    state: &DesktopState,
    instance: &InstanceRecord,
) -> Result<InstalledModIndex, AppError> {
    let paths = state.paths.clone();
    let instance_id = instance.id;
    let files = tokio::task::spawn_blocking(move || scan_instance_mods(&paths, instance_id))
        .await
        .map_err(|_| content_file_error())?
        .map_err(|_| content_file_error())?;
    let mut artifacts = files
        .into_iter()
        .map(|file| {
            (
                normalized_content_path(&file.file_path),
                InstalledModArtifact {
                    size: file.size,
                    known_hashes: Vec::new(),
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let mut identities = HashSet::new();
    for installed in state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect installed mods."))?
    {
        if let Some(artifact) = artifacts.get_mut(&normalized_content_path(&installed.file_path)) {
            identities.insert(format!("{}:{}", installed.provider, installed.project_id));
            artifact.known_hashes.push(installed.hashes);
        }
    }
    if let Some(source) = &instance.modpack_source {
        let version = state
            .modpacks
            .version(source.provider, &source.project_id, &source.version_id)
            .await
            .map_err(modpack_api_error)?;
        for file in version.files {
            if file.kind == PackFileType::Mod
                && let Some(artifact) = artifacts.get_mut(&normalized_content_path(&file.path))
            {
                artifact.known_hashes.push(file.hashes);
                if let Some(reference) = file.source {
                    identities.insert(format!("{}:{}", reference.provider, reference.project_id));
                }
            }
        }
    }
    Ok(InstalledModIndex {
        identities,
        artifacts,
    })
}

fn parse_mod_download_id(
    value: &str,
) -> Option<(slate_modpack_api_contracts::Provider, String, String)> {
    let mut parts = value.splitn(3, ':');
    let provider = slate_modpack_api_contracts::Provider::from_str(parts.next()?).ok()?;
    let project_id = parts.next()?.trim();
    let version_id = parts.next()?.trim();
    if provider == slate_modpack_api_contracts::Provider::Ftb
        || project_id.is_empty()
        || version_id.is_empty()
    {
        return None;
    }
    Some((provider, project_id.to_owned(), version_id.to_owned()))
}

fn instance_mod_target(instance: &InstanceRecord) -> Result<(LoaderKind, String), AppError> {
    let loader = match instance.loader_kind {
        LoaderFamily::Vanilla => {
            return Err(AppError::new(
                "mod.vanilla_instance",
                "Vanilla instances cannot install loader mods.",
            ));
        }
        LoaderFamily::Fabric => LoaderKind::Fabric,
        LoaderFamily::NeoForge => LoaderKind::NeoForge,
    };
    let version = instance
        .loader_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "mod.loader_version_missing",
                "This instance does not have an exact loader version.",
            )
        })?;
    Ok((loader, version.to_owned()))
}

fn validate_instance_mod_plan(
    instance: &InstanceRecord,
    provider: slate_modpack_api_contracts::Provider,
    project_id: &str,
    plan: &InstallPlan,
) -> Result<(), AppError> {
    let (loader, loader_version) = instance_mod_target(instance)?;
    let valid_downloads = !plan.downloads.is_empty()
        && plan.downloads.len() <= 257
        && plan.extract.is_empty()
        && plan.delete.is_empty()
        && plan.downloads.iter().all(|download| {
            download.destination.starts_with("mods/")
                && download.hashes.has_cryptographic_hash()
                && parse_mod_download_id(&download.id).is_some()
        });
    if plan.schema != 1
        || plan.instance.provider != provider
        || plan.instance.project_id != project_id
        || plan.runtime.minecraft != instance.minecraft_version
        || plan.runtime.loader.kind != loader
        || plan.runtime.loader.version.as_deref() != Some(loader_version.as_str())
        || !valid_downloads
    {
        return Err(AppError::new(
            "mod.resolution_mismatch",
            "The provider could not prove an exact match for this instance. Nothing was installed.",
        ));
    }
    Ok(())
}

fn validate_resolved_modpack(
    request: &InstallModpackRequest,
    version: &ModpackVersion,
    plan: &InstallPlan,
) -> Result<(), AppError> {
    if version.provider != request.provider
        || version.project_id != request.project_id
        || version.id != request.version_id
        || plan.instance.provider != request.provider
        || plan.instance.project_id != request.project_id
        || plan.instance.version_id != request.version_id
        || version.minecraft.version != plan.runtime.minecraft
        || version.loader != plan.runtime.loader
    {
        return Err(AppError::new(
            "modpack.resolution_mismatch",
            "The provider returned inconsistent modpack metadata. Nothing was installed.",
        ));
    }
    Ok(())
}

fn launcher_loader_kind(loader: LoaderKind) -> Result<LoaderKindDto, AppError> {
    match loader {
        LoaderKind::Vanilla => Ok(LoaderKindDto::Vanilla),
        LoaderKind::Fabric => Ok(LoaderKindDto::Fabric),
        LoaderKind::NeoForge => Ok(LoaderKindDto::NeoForge),
        LoaderKind::Forge | LoaderKind::Quilt => Err(AppError::new(
            "modpack.loader_not_supported",
            "This pack uses a loader that slate cannot install yet.",
        )),
    }
}

const fn current_modpack_platform() -> ModpackPlatform {
    if cfg!(target_os = "windows") {
        ModpackPlatform::Windows
    } else if cfg!(target_os = "macos") {
        ModpackPlatform::Macos
    } else {
        ModpackPlatform::Linux
    }
}

fn current_modpack_architecture() -> Result<ModpackArchitecture, AppError> {
    if cfg!(target_arch = "x86_64") {
        Ok(ModpackArchitecture::X86_64)
    } else if cfg!(target_arch = "aarch64") {
        Ok(ModpackArchitecture::Aarch64)
    } else {
        Err(AppError::new(
            "modpack.unsupported_architecture",
            "Modpack installation currently supports x86-64 and ARM64 systems.",
        ))
    }
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
async fn instance_launch(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, DesktopState>,
    request: LaunchInstanceRequest,
) -> Result<GameSessionSummary, AppError> {
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
            "Minecraft is already running for this instance.",
        ));
    }
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
    let launch_account = state
        .database
        .account_for_launch(instance_id, request.account_id.map(AccountId::from_uuid))
        .await
        .map_err(|error| {
            map_storage_error(error, "Sign in to a Minecraft account before playing.")
        })?;
    if launch_account.status != AccountStatus::Ready {
        return Err(AppError::new(
            "auth.reauthentication_required",
            "This Minecraft account needs to sign in again before playing.",
        ));
    }
    let minecraft_session = refresh_minecraft_session(
        state.inner(),
        launch_account.id,
        launch_account.profile_id,
        &launch_account.credential_ref,
    )
    .await?;
    let revision = state
        .database
        .get_installed_revision(instance_id)
        .await
        .map_err(|error| {
            map_storage_error(error, "slate could not load the installed revision.")
        })?;
    let manifest_path = installed_manifest_path(&state.paths, instance_id, revision.id);
    let installed = load_installed_revision(
        &manifest_path,
        instance_id,
        revision.id,
        &revision.manifest_digest,
    )
    .await
    .map_err(|_| {
        AppError::new(
            "local.install_manifest_invalid",
            "The installed revision manifest is missing, changed, or invalid. Reinstall the instance.",
        )
    })?;
    let layers = installed.metadata_layers;
    let runtime = installed.runtime;
    let installed_artifacts = installed.launch_artifacts;
    if runtime.executable != revision.runtime_executable
        || runtime.major_version != revision.runtime_major
    {
        return Err(AppError::new(
            "local.runtime_binding_changed",
            "The runtime binding changed after installation. Reinstall the instance.",
        ));
    }
    let managed_probe_path = runtime.executable.clone();
    let managed_probe =
        tauri::async_runtime::spawn_blocking(move || probe_java_executable(&managed_probe_path))
            .await
            .map_err(|_| {
                AppError::new(
                    "local.runtime_probe_unavailable",
                    "slate could not validate the managed Java runtime.",
                )
            })?;
    if !managed_probe.available || managed_probe.major_version != Some(runtime.major_version) {
        return Err(AppError::new(
            "local.runtime_invalid",
            "The managed Java runtime is missing or no longer matches this instance.",
        ));
    }

    let resolved = ResolvedVersion::resolve(layers).map_err(|_| {
        AppError::new(
            "local.install_manifest_invalid",
            "The installed metadata chain is invalid. Reinstall the instance.",
        )
    })?;
    let architecture = minecraft_architecture(&runtime.architecture)?;
    let selected_executable = match instance.settings.java_mode {
        JavaSelectionMode::Managed => runtime.executable.clone(),
        JavaSelectionMode::Detected | JavaSelectionMode::Custom => instance
            .settings
            .custom_java_path
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(java_selection_error)?,
    };
    let selected_probe_path = selected_executable.clone();
    let selected_probe =
        tauri::async_runtime::spawn_blocking(move || probe_java_executable(&selected_probe_path))
            .await
            .map_err(|_| java_selection_error())?;
    if !selected_probe.available
        || selected_probe.major_version != Some(resolved.java_version().major_version)
        || selected_probe.architecture.and_then(platform_architecture) != Some(architecture)
    {
        return Err(AppError::new(
            "local.runtime_invalid",
            "The selected Java runtime no longer matches this instance's required version and architecture.",
        ));
    }
    let layout = launch_layout(&state.paths, instance_id, revision.id);
    let runtime_for_plan = JavaRuntime::new(
        selected_executable.clone(),
        resolved.java_version().major_version,
        architecture,
    )
    .map_err(|_| AppError::new("local.runtime_invalid", "The selected runtime is invalid."))?;
    let mut environment: BTreeMap<String, EnvironmentValue> =
        restricted_child_environment(Some(&selected_executable))
            .into_iter()
            .map(|(name, value)| (name, EnvironmentValue::public(value)))
            .collect();
    environment.extend(
        instance
            .settings
            .environment
            .iter()
            .map(|(name, value)| (name.clone(), EnvironmentValue::public(value.clone()))),
    );
    let custom_resolution = instance
        .settings
        .resolution_width
        .zip(instance.settings.resolution_height);
    let quick_play = instance
        .settings
        .quick_play_server
        .as_ref()
        .map(|server| QuickPlay::Multiplayer(server.clone()));
    let additional_jvm_arguments = launch_jvm_arguments(&instance.settings);
    apply_managed_game_options(
        &layout.game_directory,
        &instance.settings.game_language,
        instance.settings.window_mode == InstanceWindowMode::Fullscreen,
    )
    .await?;
    let preparation = LaunchPlanner::prepare(
        &resolved,
        LaunchRequest {
            runtime: runtime_for_plan,
            layout,
            identity: LaunchIdentity::new(
                minecraft_session.profile.name.clone(),
                minecraft_session.profile.id,
                minecraft_session.access_token(),
                minecraft_session.client_id(),
                minecraft_session.xuid(),
            )
            .map_err(|_| {
                AppError::new(
                    "auth.minecraft_identity_invalid",
                    "Minecraft returned an identity that slate could not launch safely.",
                )
            })?,
            rules: current_rule_context(architecture),
            options: LaunchOptions {
                initial_memory_mib: instance.settings.initial_memory_mb,
                maximum_memory_mib: instance.memory_mb,
                additional_jvm_arguments,
                custom_resolution,
                demo_user: false,
                quick_play,
                ..LaunchOptions::default()
            },
            environment,
        },
    )
    .map_err(|_| {
        AppError::new(
            "local.launch_plan_invalid",
            "slate could not construct a valid launch plan for this revision.",
        )
    })?;
    verify_installed_launch_artifacts(
        state.paths.storage_root(),
        &preparation.required_artifacts,
        &installed_artifacts,
    )
    .await
    .map_err(|_| {
        AppError::new(
            "local.install_corrupt",
            "A required game file is missing, changed, or corrupt. Reinstall the instance.",
        )
    })?;

    let session_id = SessionId::new();
    state
        .database
        .create_session_starting(instance_id, revision.id, session_id, launch_account.id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not create the game session."))?;
    let log_path = state
        .paths
        .instance(instance_id)
        .join("logs")
        .join(format!("{session_id}.log"));
    let started = match state.processes.start(
        instance_id,
        session_id,
        &preparation.plan,
        log_path,
        child_process_priority(instance.settings.process_priority),
    ) {
        Ok(started) => started,
        Err(error) => {
            let _ = state.database.fail_session_start(session_id).await;
            return Err(process_start_error(error));
        }
    };
    if let Err(error) = state
        .database
        .mark_session_running(session_id, started.pid)
        .await
    {
        let _ = state
            .processes
            .terminate_after_tracking_failure(instance_id);
        let _ = state.database.fail_session_start(session_id).await;
        return Err(map_storage_error(
            error,
            "The game was stopped because slate could not track its session.",
        ));
    }
    match instance.settings.launcher_behavior {
        LauncherBehavior::KeepOpen => {}
        LauncherBehavior::Minimize => {
            let _ = window.minimize();
        }
        LauncherBehavior::Hide => {
            let _ = window.hide();
            let state = state.inner().clone();
            tauri::async_runtime::spawn(async move {
                restore_window_after_session(state, window, session_id).await;
            });
        }
    }
    Ok(GameSessionSummary {
        id: session_id.as_uuid(),
        instance_id: instance_id.as_uuid(),
        state: GameSessionStateDto::Running,
        mode: "authenticated".to_owned(),
        pid: started.pid,
        log_name: started.log_path.file_name().map_or_else(
            || "session.log".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
    })
}

#[tauri::command]
async fn sessions_list(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<GameSessionSummary>, AppError> {
    refresh_exited_sessions(state.inner()).await;
    state
        .processes
        .active()
        .map(|processes| processes.into_iter().map(game_session_summary).collect())
        .map_err(process_state_error)
}

#[tauri::command]
async fn session_force_stop(
    state: tauri::State<'_, DesktopState>,
    request: StopGameSessionRequest,
) -> Result<GameSessionSummary, AppError> {
    refresh_exited_sessions(state.inner()).await;
    state
        .processes
        .force_stop(SessionId::from_uuid(request.id))
        .map(game_session_summary)
        .map_err(process_stop_error)
}

#[tauri::command]
async fn session_log_subscribe(
    state: tauri::State<'_, DesktopState>,
    request: SubscribeSessionLogRequest,
    on_event: tauri::ipc::Channel<SessionLogEvent>,
) -> Result<SessionLogSubscription, AppError> {
    refresh_exited_sessions(state.inner()).await;
    let session_id = SessionId::from_uuid(request.session_id);
    let process = state
        .processes
        .active_for_session(session_id)
        .map_err(process_state_error)?
        .ok_or_else(|| {
            AppError::new(
                "local.session_not_running",
                "That Minecraft session is no longer available for live log streaming.",
            )
        })?;
    let subscription_id = Uuid::new_v4();
    let mut tail = SessionLogTail::new(process.log_path);
    let snapshot = tail.snapshot().await.map_err(|_| {
        AppError::new(
            "local.session_log_unavailable",
            "slate could not read this Minecraft session log.",
        )
        .retryable(true)
    })?;
    send_session_log_chunk(&on_event, subscription_id, session_id, snapshot)
        .map_err(|_| log_channel_error())?;

    let (cancel_sender, cancel_receiver) = tokio::sync::oneshot::channel();
    state.log_streams.insert(subscription_id, cancel_sender)?;
    let coordinator = state.log_streams.clone();
    let processes = state.processes.clone();
    tauri::async_runtime::spawn(async move {
        stream_session_log(
            processes,
            session_id,
            subscription_id,
            tail,
            on_event,
            cancel_receiver,
        )
        .await;
        coordinator.finish(subscription_id);
    });

    Ok(SessionLogSubscription {
        id: subscription_id,
        session_id: request.session_id,
    })
}

#[tauri::command]
fn session_log_unsubscribe(
    state: tauri::State<'_, DesktopState>,
    request: UnsubscribeSessionLogRequest,
) -> Result<(), AppError> {
    state.log_streams.cancel(request.subscription_id)
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
    let settings = record.settings.clone();
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

const fn window_mode_dto(value: InstanceWindowMode) -> InstanceWindowModeDto {
    match value {
        InstanceWindowMode::Windowed => InstanceWindowModeDto::Windowed,
        InstanceWindowMode::Maximized => InstanceWindowModeDto::Maximized,
        InstanceWindowMode::Fullscreen => InstanceWindowModeDto::Fullscreen,
    }
}

const fn launcher_behavior_dto(value: LauncherBehavior) -> LauncherBehaviorDto {
    match value {
        LauncherBehavior::KeepOpen => LauncherBehaviorDto::KeepOpen,
        LauncherBehavior::Minimize => LauncherBehaviorDto::Minimize,
        LauncherBehavior::Hide => LauncherBehaviorDto::Hide,
    }
}

const fn process_priority_dto(value: ProcessPriority) -> ProcessPriorityDto {
    match value {
        ProcessPriority::Low => ProcessPriorityDto::Low,
        ProcessPriority::BelowNormal => ProcessPriorityDto::BelowNormal,
        ProcessPriority::Normal => ProcessPriorityDto::Normal,
        ProcessPriority::AboveNormal => ProcessPriorityDto::AboveNormal,
        ProcessPriority::High => ProcessPriorityDto::High,
    }
}

const fn memory_mode_dto(value: MemoryMode) -> MemoryModeDto {
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

const fn performance_preset_dto(value: PerformancePreset) -> PerformancePresetDto {
    match value {
        PerformancePreset::Balanced => PerformancePresetDto::Balanced,
        PerformancePreset::Throughput => PerformancePresetDto::Throughput,
        PerformancePreset::LowLatency => PerformancePresetDto::LowLatency,
        PerformancePreset::Custom => PerformancePresetDto::Custom,
    }
}

fn instance_mod_summary(
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

fn normalized_content_path(value: &str) -> String {
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    normalized
        .strip_suffix(".disabled")
        .unwrap_or(&normalized)
        .to_owned()
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

async fn refresh_account(
    state: &DesktopState,
    account_id: AccountId,
) -> Result<MinecraftAccountSummary, AppError> {
    let account = state
        .database
        .get_account(account_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that account."))?;
    refresh_minecraft_session(
        state,
        account.id,
        account.profile_id,
        &account.credential_ref,
    )
    .await?;
    state
        .database
        .get_account(account_id)
        .await
        .map(account_summary)
        .map_err(|error| map_storage_error(error, "slate could not reload that account."))
}

async fn refresh_minecraft_session(
    state: &DesktopState,
    account_id: AccountId,
    expected_profile_id: Uuid,
    credential_ref: &str,
) -> Result<MinecraftSession, AppError> {
    let vault = state.credential_vault.clone();
    let credential_ref_owned = credential_ref.to_owned();
    let refresh_token =
        tauri::async_runtime::spawn_blocking(move || vault.load(&credential_ref_owned))
            .await
            .map_err(|_| auth_state_error())?
            .map_err(|_| {
                AppError::new(
                    "auth.credential_unavailable",
                    "The saved Microsoft credential is unavailable. Sign in again to continue.",
                )
            })?;
    let refreshed = match state.auth_client.refresh_session(&refresh_token).await {
        Ok(refreshed) => refreshed,
        Err(error) => {
            if auth_requires_reauthentication(&error) {
                let _ = state
                    .database
                    .mark_account_reauthentication_required(account_id)
                    .await;
            }
            return Err(map_auth_error(error));
        }
    };
    if refreshed.session.profile.id != expected_profile_id {
        let _ = state
            .database
            .mark_account_reauthentication_required(account_id)
            .await;
        return Err(AppError::new(
            "auth.profile_changed",
            "Microsoft returned a different Minecraft profile. Sign in again instead of launching.",
        ));
    }
    if let Some(replacement) = refreshed.replacement_refresh_token {
        let vault = state.credential_vault.clone();
        let credential_ref = credential_ref.to_owned();
        tauri::async_runtime::spawn_blocking(move || vault.store(&credential_ref, &replacement))
            .await
            .map_err(|_| auth_state_error())?
            .map_err(|_| {
                AppError::new(
                    "auth.credential_update_failed",
                    "Microsoft refreshed the account, but slate could not safely update its saved credential.",
                )
            })?;
    }
    state
        .database
        .update_account_validation(
            account_id,
            &refreshed.session.profile.name,
            refreshed.session.profile.skin_url.as_deref(),
        )
        .await
        .map_err(|error| map_storage_error(error, "slate could not update the account record."))?;
    Ok(refreshed.session)
}

fn account_summary(record: AccountRecord) -> MinecraftAccountSummary {
    MinecraftAccountSummary {
        id: record.id.as_uuid(),
        profile_id: record.profile_id,
        display_name: record.display_name,
        skin_url: record.skin_url,
        status: match record.status {
            AccountStatus::Ready => MinecraftAccountStatusDto::Ready,
            AccountStatus::ReauthenticationRequired => {
                MinecraftAccountStatusDto::ReauthenticationRequired
            }
        },
        is_default: record.is_default,
        last_validated_at: record.last_validated_at,
    }
}

fn auth_state_error() -> AppError {
    AppError::new(
        "auth.local_state_unavailable",
        "slate could not access the local sign-in state. Restart slate and try again.",
    )
}

fn auth_flow_not_found() -> AppError {
    AppError::new(
        "auth.flow_not_found",
        "That sign-in attempt is no longer available. Start a new sign-in.",
    )
}

fn auth_requires_reauthentication(error: &AuthError) -> bool {
    matches!(
        error,
        AuthError::MicrosoftRejected(_)
            | AuthError::StageRejected {
                stage: slate_auth::AuthStage::MicrosoftToken,
                status: 400 | 401,
            }
    )
}

fn map_auth_error(error: AuthError) -> AppError {
    let retryable = matches!(
        error,
        AuthError::Http(_)
            | AuthError::StageRejected {
                status: 429 | 500..=599,
                ..
            }
    );
    let code = match error {
        AuthError::AuthorizationDenied => "auth.authorization_denied",
        AuthError::CallbackPortUnavailable { .. } => "auth.callback_port_unavailable",
        AuthError::CallbackExpired => "auth.flow_expired",
        AuthError::MinecraftApplicationNotAuthorized => "auth.application_not_authorized",
        AuthError::MinecraftNotOwned => "auth.minecraft_not_owned",
        AuthError::MinecraftProfileMissing => "auth.minecraft_profile_missing",
        AuthError::XboxPolicy(_) => "auth.xbox_policy",
        AuthError::MicrosoftRejected(_) => "auth.microsoft_rejected",
        AuthError::Http(_) => "auth.network_unavailable",
        _ => "auth.provider_failure",
    };
    AppError::new(code, auth_error_message(&error)).retryable(retryable)
}

fn auth_error_message(error: &AuthError) -> String {
    match error {
        AuthError::AuthorizationDenied => "Microsoft sign-in was cancelled.".to_owned(),
        AuthError::CallbackPortUnavailable { port, .. } => format!(
            "slate could not open its Microsoft sign-in callback on port {port}. Close the app using that port and try again."
        ),
        AuthError::CallbackExpired => {
            "The sign-in window expired. Start a new Microsoft sign-in.".to_owned()
        }
        AuthError::MicrosoftRejected(code) if code == "invalid_grant" => {
            "The saved Microsoft session expired. Sign in again to continue.".to_owned()
        }
        AuthError::MicrosoftRejected(code) if code == "invalid_client" => {
            "Microsoft rejected slate as a public client. Configure the callback under Mobile and desktop applications; slate does not use a client secret."
                .to_owned()
        }
        AuthError::MicrosoftRejected(code) => {
            format!("Microsoft rejected the authentication request ({code}).")
        }
        AuthError::RefreshTokenMissing => {
            "Microsoft completed sign-in but did not issue an offline refresh credential. Remove slate from your Microsoft app permissions, then sign in again."
                .to_owned()
        }
        AuthError::XboxIdentityMissing => {
            "Xbox sign-in completed without the identity claim required by Minecraft. Confirm this Microsoft account has an Xbox profile."
                .to_owned()
        }
        AuthError::XboxPolicy(2_148_916_233) => {
            "This Microsoft account does not have an Xbox profile. Create one, then try again."
                .to_owned()
        }
        AuthError::XboxPolicy(2_148_916_238) => {
            "This child account must be added to a Microsoft family before it can sign in."
                .to_owned()
        }
        AuthError::XboxPolicy(_) => {
            "Xbox account policy prevented sign-in. Review the account's Xbox privacy and family settings."
                .to_owned()
        }
        AuthError::MinecraftApplicationNotAuthorized => {
            "Minecraft Services rejected slate's application registration.".to_owned()
        }
        AuthError::MinecraftNotOwned => {
            "This Microsoft account does not own Minecraft: Java Edition.".to_owned()
        }
        AuthError::MinecraftProfileMissing => {
            "This account owns Minecraft but does not have a Java profile yet.".to_owned()
        }
        AuthError::MinecraftProfileInvalid => {
            "Minecraft returned an invalid Java profile. Try signing in again shortly.".to_owned()
        }
        AuthError::Http(_) | AuthError::StageRejected { status: 429 | 500..=599, .. } => {
            "Microsoft or Minecraft authentication is temporarily unavailable. Try again shortly."
                .to_owned()
        }
        AuthError::StageRejected { stage, status } => {
            format!("The {stage:?} authentication step was rejected (HTTP {status}).")
        }
        _ => "Minecraft account verification did not complete. Try signing in again.".to_owned(),
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

fn validated_instance_settings(
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

fn validated_free_text(
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

fn validated_optional_text(
    field: &'static str,
    value: Option<String>,
    maximum: usize,
) -> Result<Option<String>, AppError> {
    value
        .map(|value| validated_free_text(field, value, maximum))
        .transpose()
        .map(|value| value.filter(|value| !value.is_empty()))
}

fn validated_optional_server(value: Option<String>) -> Result<Option<String>, AppError> {
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

fn recommended_memory_mb(instance: &InstanceRecord) -> u32 {
    match instance.mode {
        slate_domain::InstanceMode::Vanilla | slate_domain::InstanceMode::Pvp => 4_096,
        slate_domain::InstanceMode::Modded => match instance.mod_count {
            0..=50 => 4_096,
            51..=150 => 6_144,
            151..=300 => 8_192,
            _ => 12_288,
        },
    }
}

fn validated_jvm_arguments(arguments: Vec<String>) -> Result<Vec<String>, AppError> {
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
                "One or more JVM arguments would override slate-managed launch settings.",
            ));
        }
        validated.push(argument.to_owned());
    }
    Ok(validated)
}

fn validated_environment(
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

fn settings_validation_error(field: &'static str, message: impl Into<String>) -> AppError {
    let message = message.into();
    AppError::new("validation.instance_settings", message.clone()).with_field_error(field, message)
}

fn validated_java_selection(
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
                "Minecraft {} requires Java {expected_major}. Choose a matching runtime.",
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

fn required_java_major(minecraft_version: &str) -> u32 {
    let mut components = minecraft_version
        .split(['.', '-'])
        .take(3)
        .map(|component| component.parse::<u32>().unwrap_or_default());
    let major = components.next().unwrap_or_default();
    let minor = components.next().unwrap_or_default();
    let patch = components.next().unwrap_or_default();
    match (major, minor, patch) {
        (1, 0..=16, _) => 8,
        (1, 17, _) => 16,
        (1, 18..=19, _) | (1, 20, 0..=4) => 17,
        _ => 21,
    }
}

fn java_selection_error() -> AppError {
    AppError::new(
        "runtime.java_unavailable",
        "slate could not validate that Java executable. Choose another runtime.",
    )
}

fn artwork_error() -> AppError {
    AppError::new(
        "local.artwork_unavailable",
        "Choose a PNG, JPEG, or WebP image within the size limit.",
    )
}

fn instance_artwork_path(
    paths: &AppPaths,
    instance_id: InstanceId,
    kind: InstanceArtworkKindDto,
) -> PathBuf {
    let filename = match kind {
        InstanceArtworkKindDto::Icon => "profile-icon.bin",
        InstanceArtworkKindDto::Banner => "profile-banner.bin",
    };
    paths.instance(instance_id).join("metadata").join(filename)
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

struct ManagedFileReplacement {
    destination: PathBuf,
    backup: Option<PathBuf>,
}

impl ManagedFileReplacement {
    async fn rollback(self) {
        let _ = tokio::fs::remove_file(&self.destination).await;
        if let Some(backup) = self.backup {
            let _ = tokio::fs::rename(backup, self.destination).await;
        }
    }

    async fn commit(self) {
        if let Some(backup) = self.backup {
            let _ = tokio::fs::remove_file(backup).await;
        }
    }
}

async fn replace_managed_file(
    destination: PathBuf,
    bytes: Vec<u8>,
) -> Result<ManagedFileReplacement, AppError> {
    replace_file(destination, bytes, artwork_error).await
}

async fn replace_file(
    destination: PathBuf,
    bytes: Vec<u8>,
    error: fn() -> AppError,
) -> Result<ManagedFileReplacement, AppError> {
    let parent = destination.parent().ok_or_else(error)?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|_| error())?;
    let temporary = destination.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let backup = destination.with_extension(format!("bak-{}", Uuid::new_v4()));
    tokio::fs::write(&temporary, bytes)
        .await
        .map_err(|_| error())?;
    let previous = if tokio::fs::try_exists(&destination)
        .await
        .map_err(|_| error())?
    {
        if let Err(io_error) = tokio::fs::rename(&destination, &backup).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(if io_error.kind() == std::io::ErrorKind::PermissionDenied {
                AppError::new(
                    "local.artwork_in_use",
                    "Close applications using that managed file and try again.",
                )
            } else {
                error()
            });
        }
        Some(backup)
    } else {
        None
    };
    if tokio::fs::rename(&temporary, &destination).await.is_err() {
        if let Some(backup) = previous.as_ref() {
            let _ = tokio::fs::rename(backup, &destination).await;
        }
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error());
    }
    Ok(ManagedFileReplacement {
        destination,
        backup: previous,
    })
}

async fn remove_managed_file(destination: PathBuf) -> Result<ManagedFileReplacement, AppError> {
    let backup = destination.with_extension(format!("bak-{}", Uuid::new_v4()));
    let previous = if tokio::fs::try_exists(&destination)
        .await
        .map_err(|_| artwork_error())?
    {
        tokio::fs::rename(&destination, &backup)
            .await
            .map_err(|_| artwork_error())?;
        Some(backup)
    } else {
        None
    };
    Ok(ManagedFileReplacement {
        destination,
        backup: previous,
    })
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
        "slate could not refresh the official version catalog.",
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
        "slate could not read the current Minecraft process state.",
    )
    .retryable(true)
}

fn modpack_api_error(error: slate_modpack_client::ClientError) -> AppError {
    AppError::new("modpack.service_unavailable", error.user_message()).retryable(true)
}

fn process_start_error(error: slate_process::ProcessError) -> AppError {
    if matches!(error, slate_process::ProcessError::InstanceAlreadyRunning) {
        AppError::new(
            "local.instance_running",
            "Minecraft is already running for this instance.",
        )
    } else {
        AppError::new(
            "local.launch_failed",
            "slate could not start the Minecraft process. Check the instance log and try again.",
        )
    }
}

fn process_stop_error(error: slate_process::ProcessError) -> AppError {
    if matches!(error, slate_process::ProcessError::SessionNotFound) {
        AppError::new(
            "local.session_not_running",
            "That Minecraft session has already stopped.",
        )
    } else {
        AppError::new(
            "local.stop_failed",
            "slate could not force-close the Minecraft process.",
        )
        .retryable(true)
    }
}

fn log_stream_state_error() -> AppError {
    AppError::new(
        "local.session_log_state_unavailable",
        "slate could not update the live log subscription.",
    )
    .retryable(true)
}

fn log_channel_error() -> AppError {
    AppError::new(
        "local.session_log_channel_unavailable",
        "slate could not connect the live Minecraft log to this window.",
    )
    .retryable(true)
}

fn send_session_log_chunk(
    channel: &tauri::ipc::Channel<SessionLogEvent>,
    subscription_id: Uuid,
    session_id: SessionId,
    chunk: LogChunk,
) -> tauri::Result<()> {
    channel.send(SessionLogEvent {
        subscription_id,
        session_id: session_id.as_uuid(),
        kind: match chunk.kind {
            LogChunkKind::Snapshot => SessionLogEventKindDto::Snapshot,
            LogChunkKind::Append => SessionLogEventKindDto::Append,
            LogChunkKind::Reset => SessionLogEventKindDto::Reset,
        },
        offset: chunk.offset.to_string(),
        truncated: chunk.truncated,
        text: chunk.text,
    })
}

async fn stream_session_log(
    processes: ProcessSupervisor,
    session_id: SessionId,
    subscription_id: Uuid,
    mut tail: SessionLogTail,
    channel: tauri::ipc::Channel<SessionLogEvent>,
    mut cancel: tokio::sync::oneshot::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(200));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = &mut cancel => return,
            _ = interval.tick() => {
                let mut caught_up = false;
                for _ in 0..4 {
                    match tail.next_chunk().await {
                        Ok(Some(chunk)) => {
                            if send_session_log_chunk(
                                &channel,
                                subscription_id,
                                session_id,
                                chunk,
                            ).is_err() {
                                return;
                            }
                        }
                        Ok(None) => {
                            caught_up = true;
                            break;
                        }
                        Err(_) => {
                            let _ = channel.send(SessionLogEvent {
                                subscription_id,
                                session_id: session_id.as_uuid(),
                                kind: SessionLogEventKindDto::Error,
                                offset: tail.offset().to_string(),
                                truncated: false,
                                text: "slate could not continue reading this session log.".to_owned(),
                            });
                            return;
                        }
                    }
                }

                match processes.active_for_session(session_id) {
                    Ok(Some(_)) => {}
                    Ok(None) if caught_up => {
                        let _ = channel.send(SessionLogEvent {
                            subscription_id,
                            session_id: session_id.as_uuid(),
                            kind: SessionLogEventKindDto::Closed,
                            offset: tail.offset().to_string(),
                            truncated: false,
                            text: String::new(),
                        });
                        return;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        let _ = channel.send(SessionLogEvent {
                            subscription_id,
                            session_id: session_id.as_uuid(),
                            kind: SessionLogEventKindDto::Error,
                            offset: tail.offset().to_string(),
                            truncated: false,
                            text: "slate could not verify the game process state.".to_owned(),
                        });
                        return;
                    }
                }
            }
        }
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

const fn platform_architecture(value: JavaArchitecture) -> Option<Architecture> {
    match value {
        JavaArchitecture::X86 => Some(Architecture::X86),
        JavaArchitecture::X86_64 => Some(Architecture::X86_64),
        JavaArchitecture::Arm64 => Some(Architecture::Arm64),
    }
}

const fn child_process_priority(value: ProcessPriority) -> ChildProcessPriority {
    match value {
        ProcessPriority::Low => ChildProcessPriority::Low,
        ProcessPriority::BelowNormal => ChildProcessPriority::BelowNormal,
        ProcessPriority::Normal => ChildProcessPriority::Normal,
        ProcessPriority::AboveNormal => ChildProcessPriority::AboveNormal,
        ProcessPriority::High => ChildProcessPriority::High,
    }
}

fn launch_jvm_arguments(settings: &slate_storage::InstanceSettingsRecord) -> Vec<String> {
    let mut arguments = match settings.performance_preset {
        PerformancePreset::Balanced | PerformancePreset::Custom => Vec::new(),
        PerformancePreset::Throughput => vec![
            "-XX:+UseG1GC".to_owned(),
            "-XX:MaxGCPauseMillis=100".to_owned(),
            "-XX:+ParallelRefProcEnabled".to_owned(),
        ],
        PerformancePreset::LowLatency => vec![
            "-XX:+UseG1GC".to_owned(),
            "-XX:MaxGCPauseMillis=50".to_owned(),
            "-XX:+ParallelRefProcEnabled".to_owned(),
        ],
    };
    for argument in &settings.jvm_arguments {
        if !arguments.contains(argument) {
            arguments.push(argument.clone());
        }
    }
    arguments
}

async fn apply_managed_game_options(
    game_directory: &std::path::Path,
    language: &str,
    fullscreen: bool,
) -> Result<(), AppError> {
    tokio::fs::create_dir_all(game_directory)
        .await
        .map_err(|_| game_options_error())?;
    let path = game_directory.join("options.txt");
    let existing = match tokio::fs::read_to_string(&path).await {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err(game_options_error()),
    };
    let mut language_written = false;
    let mut fullscreen_written = false;
    let mut lines = existing
        .lines()
        .map(|line| {
            let Some((key, _)) = line.split_once(':') else {
                return line.to_owned();
            };
            match key {
                "lang" => {
                    language_written = true;
                    format!("lang:{language}")
                }
                "fullscreen" => {
                    fullscreen_written = true;
                    format!("fullscreen:{fullscreen}")
                }
                _ => line.to_owned(),
            }
        })
        .collect::<Vec<_>>();
    if !language_written {
        lines.push(format!("lang:{language}"));
    }
    if !fullscreen_written {
        lines.push(format!("fullscreen:{fullscreen}"));
    }
    let bytes = format!("{}\n", lines.join("\n")).into_bytes();
    let replacement = replace_file(path, bytes, game_options_error).await?;
    replacement.commit().await;
    Ok(())
}

fn game_options_error() -> AppError {
    AppError::new(
        "local.game_options_unavailable",
        "slate could not apply this instance's game language and window mode.",
    )
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

async fn refresh_exited_sessions(state: &DesktopState) {
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

async fn restore_window_after_session(
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

fn main() {
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let paths = AppPaths::discover()?;
            paths.ensure_base_directories()?;
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
            app.manage(DesktopState {
                database,
                paths,
                storage_root_id,
                processes: ProcessSupervisor::default(),
                auth_client,
                credential_vault: CredentialVault,
                auth_flows: AuthCoordinator::default(),
                log_streams: SessionLogCoordinator::default(),
                modpacks,
            });
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
            instance_set_favorite,
            instance_trash,
            instance_install,
            install_jobs_list,
            instance_launch,
            sessions_list,
            session_force_stop,
            session_log_subscribe,
            session_log_unsubscribe,
            preferences_get,
            preferences_update,
            preflight_get,
            modpack_providers,
            modpacks_search,
            mods_search,
            modpack_get,
            modpack_versions_list,
            modpack_version_get,
            modpack_install,
            instance_mods_list,
            instance_mods_resolve,
            instance_mod_set_enabled,
            instance_mod_remove,
            instance_mod_install,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = application {
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
