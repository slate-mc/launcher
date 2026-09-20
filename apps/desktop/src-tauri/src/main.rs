#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slate_auth::{
    AuthError, CredentialVault, MICROSOFT_CONSUMER_TENANT, MinecraftAuthClient,
    MinecraftAuthConfig, MinecraftSession, SLATE_MICROSOFT_CLIENT_ID,
};
use slate_contracts::{
    AccountIdRequest, AppError, AppPreferencesDto, AuthCancelRequest, AuthFlowStateDto,
    AuthFlowStatus, AuthStartResponse, BootstrapResponse, CapabilitySummary, CreateInstanceRequest,
    GameSessionStateDto, GameSessionSummary, InstallInstanceRequest, InstallJobStateDto,
    InstallJobSummary, InstallModRequest, InstallModpackRequest, InstanceModSummary,
    InstanceModeDto, InstanceModsRequest, InstanceSummary, JavaRuntimeSummary,
    LaunchInstanceRequest, LoaderKindDto, LoaderVersionCatalog, LoaderVersionsRequest,
    MinecraftAccountStatusDto, MinecraftAccountSummary, MinecraftReleaseKindDto,
    MinecraftVersionCatalog, MinecraftVersionOption, ModSearchRequest, ModpackInstallStarted,
    ModpackProjectRequest, ModpackSearchRequest, ModpackSortDto, ModpackSourceSummary,
    ModpackVersionRequest, ModpackVersionsRequest, PreflightSummary, ReduceMotionPreferenceDto,
    RenameInstanceRequest, SessionLogEvent, SessionLogEventKindDto, SessionLogSubscription,
    SetDefaultAccountRequest, SetFavoriteRequest, StopGameSessionRequest,
    SubscribeSessionLogRequest, ThemePreferenceDto, TrashInstanceRequest,
    UnsubscribeSessionLogRequest, UpdateAppPreferencesRequest, UpdateInstanceConfigurationRequest,
};
use slate_domain::{
    AccountId, InstanceId, InstanceName, InstanceNameError, LoaderFamily, ManagementMode,
    RequestId, RevisionId, SessionId, StorageRootId,
};
use slate_installer::{
    InstallProgress, InstallRequest as NativeInstallRequest, install_with_progress,
    load_installed_revision, verify_installed_launch_artifacts,
};
use slate_loaders::{FabricAdapter, NeoForgeAdapter};
use slate_minecraft::{
    Architecture, EnvironmentValue, JavaRuntime, LaunchIdentity, LaunchLayout, LaunchOptions,
    LaunchPlanner, LaunchRequest, MojangMetadataClient, OperatingSystem, ResolvedVersion,
    RuleContext,
};
use slate_modpack_api_contracts::{
    Architecture as ModpackArchitecture, InstallPlan, InstallPlanRequest, LoaderKind,
    ModInstallPlanRequest, Modpack, ModpackVersion, Platform as ModpackPlatform, ProvidersResponse,
    SearchResponse, VersionPage,
};
use slate_modpack_client::{ModpackApiClient, SearchOptions, SearchSort, VersionOptions};
use slate_platform::{
    AppPaths, detect_java_runtime, probe_java_executable, restricted_child_environment,
};
use slate_process::{
    ActiveProcess, LogChunk, LogChunkKind, ProcessState, ProcessSupervisor, SessionLogTail,
};
use slate_storage::{
    AccountRecord, AccountStatus, AppPreferences, AuthenticatedAccount, CompletedInstall, Database,
    InstallJobRecord, InstalledRuntime, InstanceModRecord, InstanceRecord, JobState, NewInstance,
    NewInstanceMod, NewModpackSource, ReduceMotionPreference, StorageError, ThemePreference,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;
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
    state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    state
        .database
        .list_instance_mods(instance_id)
        .await
        .map(|mods| mods.into_iter().map(instance_mod_summary).collect())
        .map_err(|error| map_storage_error(error, "slate could not load installed mods."))
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
        None,
    )
    .await
}

async fn queue_instance_install(
    state: &DesktopState,
    instance: InstanceRecord,
    expected_revision: u64,
    modpack_plan: Option<InstallPlan>,
    pending_mod: Option<NewInstanceMod>,
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
        let result = install_with_progress(
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
            move |progress| {
                let now = Instant::now();
                let finished_stage = progress.completed_items == progress.total_items
                    && progress.total_items.is_some();
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
            },
        )
        .await;
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
                let message = format!(
                    "Installed and verified {} files; reused {} cached files",
                    outcome.downloaded_artifacts, outcome.reused_artifacts
                );
                let completion = if let Some(installed_mod) = pending_mod {
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
                            installed_mod,
                        )
                        .await
                } else {
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
        None,
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
    let display_name = request.display_name.trim();
    if display_name.is_empty()
        || display_name.chars().count() > 160
        || display_name.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "mod.invalid_name",
            "The mod name returned by the provider is invalid.",
        ));
    }
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let (loader, loader_version) = instance_mod_target(&instance)?;
    let mut plan = state
        .modpacks
        .mod_install_plan(
            request.provider,
            &request.project_id,
            &ModInstallPlanRequest {
                minecraft_version: instance.minecraft_version.clone(),
                loader,
                loader_version: Some(loader_version.clone()),
            },
        )
        .await
        .map_err(modpack_api_error)?;
    validate_instance_mod_plan(&instance, &request, &plan)?;
    let download = plan.downloads.first().cloned().ok_or_else(|| {
        AppError::new(
            "mod.invalid_plan",
            "The mod service returned an empty install plan.",
        )
    })?;
    let existing = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect installed mods."))?
        .into_iter()
        .find(|installed| {
            installed.provider == request.provider && installed.project_id == request.project_id
        });
    if let Some(existing) = existing
        && existing.file_path != download.destination
    {
        plan.delete.push(existing.file_path);
    }
    let pending_mod = NewInstanceMod {
        provider: request.provider,
        project_id: request.project_id,
        version_id: plan.instance.version_id.clone(),
        display_name: display_name.to_owned(),
        file_path: download.destination,
        hashes: download.hashes,
    };
    queue_instance_install(
        state.inner(),
        instance,
        request.expected_revision,
        Some(plan),
        Some(pending_mod),
    )
    .await
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
    request: &InstallModRequest,
    plan: &InstallPlan,
) -> Result<(), AppError> {
    let (loader, loader_version) = instance_mod_target(instance)?;
    let valid_download = plan.downloads.len() == 1
        && plan.extract.is_empty()
        && plan.downloads[0].destination.starts_with("mods/")
        && plan.downloads[0].hashes.has_cryptographic_hash();
    if plan.schema != 1
        || plan.instance.provider != request.provider
        || plan.instance.project_id != request.project_id
        || plan.runtime.minecraft != instance.minecraft_version
        || plan.runtime.loader.kind != loader
        || plan.runtime.loader.version.as_deref() != Some(loader_version.as_str())
        || !valid_download
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
    let probe_path = runtime.executable.clone();
    let probe = tauri::async_runtime::spawn_blocking(move || probe_java_executable(&probe_path))
        .await
        .map_err(|_| {
            AppError::new(
                "local.runtime_probe_unavailable",
                "slate could not validate the managed Java runtime.",
            )
        })?;
    if !probe.available || probe.major_version != Some(runtime.major_version) {
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
    let layout = launch_layout(&state.paths, instance_id, revision.id);
    let runtime_for_plan = JavaRuntime::new(
        runtime.executable.clone(),
        runtime.major_version,
        architecture,
    )
    .map_err(|_| AppError::new("local.runtime_invalid", "The managed runtime is invalid."))?;
    let environment = restricted_child_environment(Some(&runtime.executable))
        .into_iter()
        .map(|(name, value)| (name, EnvironmentValue::public(value)))
        .collect();
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
                maximum_memory_mib: instance.memory_mb,
                demo_user: false,
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
    let started = match state
        .processes
        .start(instance_id, session_id, &preparation.plan, log_path)
    {
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
        modpack_source: record.modpack_source.map(|source| ModpackSourceSummary {
            provider: source.provider,
            project_id: source.project_id,
            version_id: source.version_id,
            display_name: source.display_name,
            icon_url: source.icon_url,
        }),
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn instance_mod_summary(record: InstanceModRecord) -> InstanceModSummary {
    InstanceModSummary {
        provider: record.provider,
        project_id: record.project_id,
        version_id: record.version_id,
        display_name: record.display_name,
        file_path: record.file_path,
        enabled: record.enabled,
        pinned: record.pinned,
        installed_at: record.installed_at,
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

fn main() {
    let application = tauri::Builder::default()
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
            instance_mod_install,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = application {
        eprintln!("slate failed to start: {error}");
        std::process::exit(1);
    }
}
