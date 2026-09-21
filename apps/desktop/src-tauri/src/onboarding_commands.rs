use super::*;

fn onboarding_summary(state: slate_storage::OnboardingStateRecord) -> OnboardingStateSummary {
    OnboardingStateSummary {
        completed: state.completed,
        custom_storage_selected: state.default_storage_root_id.is_some(),
    }
}

#[tauri::command]
pub(super) async fn onboarding_get(
    state: tauri::State<'_, DesktopState>,
) -> Result<OnboardingStateSummary, AppError> {
    state
        .database
        .get_onboarding_state()
        .await
        .map(onboarding_summary)
        .map_err(|error| map_storage_error(error, "slate could not load setup."))
}

#[tauri::command]
pub(super) async fn onboarding_select_storage(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<OnboardingStateSummary, AppError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Choose where slate stores games")
            .blocking_pick_folder()
    })
    .await
    .map_err(|_| storage_selection_error())?;
    let Some(parent) = picked.and_then(|path| path.into_path().ok()) else {
        return onboarding_get(state).await;
    };

    let app_data = state.paths.app_data().to_path_buf();
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let storage_root = parent.join("slate");
        let paths = AppPaths::from_roots(app_data, storage_root);
        paths.ensure_base_directories().map_err(|_| ())?;
        std::fs::canonicalize(paths.storage_root()).map_err(|_| ())
    })
    .await
    .map_err(|_| storage_selection_error())?
    .map_err(|_| storage_selection_error())?;
    let canonical_root = prepared.to_string_lossy().into_owned();
    let root_id = state
        .database
        .get_or_create_storage_root(&canonical_root)
        .await
        .map_err(|error| map_storage_error(error, "slate could not use that folder."))?;
    state
        .database
        .set_onboarding_storage_root(root_id)
        .await
        .map(onboarding_summary)
        .map_err(|error| map_storage_error(error, "slate could not save that choice."))
}

#[tauri::command]
pub(super) async fn onboarding_complete(
    state: tauri::State<'_, DesktopState>,
) -> Result<OnboardingStateSummary, AppError> {
    let has_account = state
        .database
        .has_configured_account()
        .await
        .map_err(|error| map_storage_error(error, "slate could not check your account."))?;
    if !has_account {
        return Err(AppError::new(
            "onboarding.account_required",
            "Connect a Minecraft account to continue.",
        ));
    }
    let has_instance = !state
        .database
        .list_instances(1)
        .await
        .map_err(|error| map_storage_error(error, "slate could not check your instances."))?
        .is_empty();
    if !has_instance {
        return Err(AppError::new(
            "onboarding.instance_required",
            "Create your first instance to finish setup.",
        ));
    }
    let completed = state
        .database
        .complete_onboarding()
        .await
        .map(onboarding_summary)
        .map_err(|error| map_storage_error(error, "slate could not finish setup."))?;
    let _ = state
        .telemetry
        .capture(
            slate_modpack_api_contracts::ProductEvent::OnboardingCompleted,
            None,
            None,
        )
        .await;
    Ok(completed)
}

fn storage_selection_error() -> AppError {
    AppError::new(
        "onboarding.storage_unavailable",
        "slate could not use that folder. Choose another writable location.",
    )
}
