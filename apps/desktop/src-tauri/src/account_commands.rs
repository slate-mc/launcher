use super::*;

#[tauri::command]
pub(super) async fn accounts_list(
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
pub(super) async fn auth_start(
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
            "slate could not securely save sign-in details. Restart your computer and try again.",
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
                "Minecraft was verified, but slate could not save the account securely.".to_owned(),
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
            Ok(account) => {
                let _ = task_state
                    .telemetry
                    .capture(
                        slate_modpack_api_contracts::ProductEvent::AccountConnected,
                        None,
                        None,
                    )
                    .await;
                task_state
                    .auth_flows
                    .succeed(flow_id, account_summary(account));
            }
            Err(_) => {
                let vault = task_state.credential_vault.clone();
                let _ = tauri::async_runtime::spawn_blocking(move || vault.remove(&credential_ref))
                    .await;
                task_state.auth_flows.fail(
                    flow_id,
                    "Minecraft was verified, but slate could not finish adding the account."
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
pub(super) fn auth_get_status(
    state: tauri::State<'_, DesktopState>,
    flow_id: Uuid,
) -> Result<AuthFlowStatus, AppError> {
    state.auth_flows.status(flow_id)
}

#[tauri::command]
pub(super) fn auth_cancel(
    state: tauri::State<'_, DesktopState>,
    request: AuthCancelRequest,
) -> Result<AuthFlowStatus, AppError> {
    state.auth_flows.cancel(request.flow_id)
}

#[tauri::command]
pub(super) async fn account_refresh(
    state: tauri::State<'_, DesktopState>,
    request: AccountIdRequest,
) -> Result<MinecraftAccountSummary, AppError> {
    refresh_account(state.inner(), AccountId::from_uuid(request.id)).await
}

#[tauri::command]
pub(super) async fn account_set_default(
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
pub(super) async fn account_remove(
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
                "The account was removed from slate, but some saved sign-in data could not be deleted.",
            )
        })
}
