use super::*;

#[tauri::command]
pub(super) async fn preferences_get(
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
pub(super) async fn preferences_update(
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
    if request.download_bandwidth_limit_mib > 1024 {
        return Err(AppError::new(
            "validation.download_bandwidth",
            "Choose a download speed limit up to 1024 MB/s.",
        )
        .with_field_error(
            "downloadBandwidthLimitMib",
            "Choose Unlimited or a value up to 1024 MB/s.",
        ));
    }
    if request.trash_retention_days != 0 && !(7..=365).contains(&request.trash_retention_days) {
        return Err(AppError::new(
            "validation.trash_retention",
            "Choose a trash retention period from 7 through 365 days, or keep trashed instances until you delete them.",
        )
        .with_field_error(
            "trashRetentionDays",
            "Choose 7 through 365 days, or Never.",
        ));
    }

    let preferences = state
        .database
        .update_app_preferences(AppPreferences {
            theme: match request.theme {
                ThemePreferenceDto::Dark => ThemePreference::Dark,
                ThemePreferenceDto::Light => ThemePreference::Light,
                ThemePreferenceDto::System => ThemePreference::System,
            },
            download_concurrency: request.download_concurrency,
            download_bandwidth_limit_mib: request.download_bandwidth_limit_mib,
            telemetry_enabled: request.telemetry_enabled,
            reduce_motion: match request.reduce_motion {
                ReduceMotionPreferenceDto::System => ReduceMotionPreference::System,
                ReduceMotionPreferenceDto::On => ReduceMotionPreference::On,
                ReduceMotionPreferenceDto::Off => ReduceMotionPreference::Off,
            },
            trash_retention_days: request.trash_retention_days,
        })
        .await
        .map_err(|error| map_storage_error(error, "slate could not save your preferences."))?;
    let response = preferences_dto(preferences);
    state.telemetry.set_enabled(request.telemetry_enabled).await;
    tauri::async_runtime::spawn(purge_expired_instance_trash(state.inner().clone()));
    Ok(response)
}

#[tauri::command]
pub(super) async fn preflight_get(
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
                "slate could not check the installed Java version.",
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
