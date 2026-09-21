use super::*;
use crate::support_report::{SupportReportInput, export_support_report, support_report_preview};

#[tauri::command]
pub(super) async fn support_report_preview_get(
    state: tauri::State<'_, DesktopState>,
) -> Result<SupportReportPreview, AppError> {
    let log_directory = state.paths.logs();
    tauri::async_runtime::spawn_blocking(move || support_report_preview(&log_directory))
        .await
        .map_err(|_| support_report_error())?
        .map_err(|_| support_report_error())
}

#[tauri::command]
pub(super) async fn support_report_export(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: CreateSupportReportRequest,
) -> Result<Option<SupportReportExport>, AppError> {
    state.features.require(FeatureAccess::SupportReport)?;
    validate_report_request(&request)?;
    let prepared = prepare_support_report(&state, request).await?;
    let today = OffsetDateTime::now_utc().date();
    let suggested_name = format!(
        "slate-support-{:04}{:02}{:02}-{}.zip",
        today.year(),
        u8::from(today.month()),
        today.day(),
        &prepared.report_id.to_string()[..8]
    );
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Save slate support report")
            .set_file_name(suggested_name)
            .add_filter("slate support report", &["zip"])
            .blocking_save_file()
    })
    .await
    .map_err(|_| support_report_error())?;
    let Some(destination) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(None);
    };
    let destination_for_export = destination.clone();
    let paths = state.paths.clone();
    let report_id = prepared.report_id;
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        write_prepared_report(&destination_for_export, &paths, prepared)
    })
    .await
    .map_err(|_| support_report_error())??;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("slate-support-report.zip")
        .to_owned();
    tracing::info!(report_id = %report_id, bytes, "support report exported");
    Ok(Some(SupportReportExport {
        report_id,
        file_name,
        bytes,
    }))
}

#[tauri::command]
pub(super) async fn support_report_submit(
    state: tauri::State<'_, DesktopState>,
    request: CreateSupportReportRequest,
) -> Result<SupportReportSubmission, AppError> {
    state.features.require(FeatureAccess::SupportReport)?;
    validate_report_request(&request)?;
    let prepared = prepare_support_report(&state, request).await?;
    let report_id = prepared.report_id;
    let paths = state.paths.clone();
    let (_, archive) = tauri::async_runtime::spawn_blocking(move || {
        let temporary = tempfile::tempdir().map_err(|_| support_report_error())?;
        let destination = temporary.path().join("report.zip");
        let bytes = write_prepared_report(&destination, &paths, prepared)?;
        let archive = std::fs::read(destination).map_err(|_| support_report_error())?;
        Ok::<_, AppError>((bytes, archive))
    })
    .await
    .map_err(|_| support_report_error())??;
    let receipt = state
        .modpacks
        .upload_support_report(report_id, archive)
        .await
        .map_err(support_upload_error)?;
    tracing::info!(report_id = %receipt.report_id, bytes = receipt.bytes, "support report submitted");
    Ok(SupportReportSubmission {
        report_id: receipt.report_id,
        bytes: receipt.bytes,
    })
}

struct PreparedSupportReport {
    report_id: Uuid,
    created_at: String,
    request: CreateSupportReportRequest,
    instances: Vec<slate_storage::InstanceRecord>,
    jobs: Vec<slate_storage::InstallJobRecord>,
    private_values: Vec<String>,
}

async fn prepare_support_report(
    state: &DesktopState,
    request: CreateSupportReportRequest,
) -> Result<PreparedSupportReport, AppError> {
    let instances = state
        .database
        .list_instances(1_000)
        .await
        .map_err(|error| map_storage_error(error, "slate could not prepare the support report."))?;
    let jobs = if request.include_install_activity {
        state
            .database
            .list_install_jobs(100)
            .await
            .map_err(|error| {
                map_storage_error(error, "slate could not prepare the support report.")
            })?
    } else {
        Vec::new()
    };
    let private_values = state
        .database
        .list_accounts()
        .await
        .map_err(|error| map_storage_error(error, "slate could not prepare the support report."))?
        .into_iter()
        .flat_map(|account| {
            [
                Some(account.id.to_string()),
                Some(account.profile_id.to_string()),
                Some(account.display_name),
                Some(account.credential_ref),
                account.skin_url,
            ]
            .into_iter()
            .flatten()
        })
        .collect();
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| support_report_error())?;
    Ok(PreparedSupportReport {
        report_id: Uuid::new_v4(),
        created_at,
        request,
        instances,
        jobs,
        private_values,
    })
}

fn write_prepared_report(
    destination: &std::path::Path,
    paths: &AppPaths,
    prepared: PreparedSupportReport,
) -> Result<u64, AppError> {
    export_support_report(
        destination,
        SupportReportInput {
            report_id: prepared.report_id,
            created_at: &prepared.created_at,
            request: prepared.request,
            paths,
            instances: &prepared.instances,
            jobs: &prepared.jobs,
            private_values: &prepared.private_values,
        },
    )
    .map_err(|_| support_report_error())
}

fn validate_report_request(request: &CreateSupportReportRequest) -> Result<(), AppError> {
    if request.include_launcher_logs
        || request.include_install_activity
        || request.include_instance_summary
    {
        Ok(())
    } else {
        Err(AppError::new(
            "support.empty_report",
            "Choose at least one item to include in the report.",
        ))
    }
}

fn support_report_error() -> AppError {
    AppError::new(
        "support.report_unavailable",
        "slate could not create the support report. Try again.",
    )
    .retryable(true)
}

fn support_upload_error(error: slate_modpack_client::ClientError) -> AppError {
    tracing::warn!(error = %error, "support report submission failed");
    AppError::new(
        "support.delivery_unavailable",
        "slate could not send the support report. Check your connection and try again.",
    )
    .retryable(true)
}
