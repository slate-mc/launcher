use super::*;

pub(super) async fn refresh_account(
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

pub(super) async fn refresh_minecraft_session(
    state: &DesktopState,
    account_id: AccountId,
    expected_profile_id: Uuid,
    credential_ref: &str,
) -> Result<MinecraftSession, AppError> {
    let vault = state.credential_vault.clone();
    let credential_ref_owned = credential_ref.to_owned();
    let refresh_token = tauri::async_runtime::spawn_blocking(move || {
        vault.load(&credential_ref_owned)
    })
    .await
    .map_err(|_| auth_state_error())?
    .map_err(|_| {
        AppError::new(
            "auth.credential_unavailable",
            "Your saved Microsoft sign-in is no longer available. Sign in again to continue.",
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
                    "slate could not save the refreshed account. Sign in again to continue.",
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

pub(super) fn account_summary(record: AccountRecord) -> MinecraftAccountSummary {
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

pub(super) fn auth_state_error() -> AppError {
    AppError::new(
        "auth.local_state_unavailable",
        "That sign-in could not be resumed. Restart slate and try again.",
    )
}

pub(super) fn auth_flow_not_found() -> AppError {
    AppError::new(
        "auth.flow_not_found",
        "That sign-in attempt is no longer available. Start a new sign-in.",
    )
}

pub(super) fn auth_requires_reauthentication(error: &AuthError) -> bool {
    matches!(
        error,
        AuthError::MicrosoftRejected(_)
            | AuthError::StageRejected {
                stage: slate_auth::AuthStage::MicrosoftToken,
                status: 400 | 401,
            }
    )
}

pub(super) fn map_auth_error(error: AuthError) -> AppError {
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

pub(super) fn auth_error_message(error: &AuthError) -> String {
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
            "Microsoft did not finish preparing the account for slate. Remove slate from your Microsoft app permissions, then sign in again."
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
