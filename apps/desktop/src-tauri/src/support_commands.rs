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
    if !request.include_launcher_logs
        && !request.include_install_activity
        && !request.include_instance_summary
    {
        return Err(AppError::new(
            "support.empty_report",
            "Choose at least one item to include in the report.",
        ));
    }

    let report_id = Uuid::new_v4();
    let today = OffsetDateTime::now_utc().date();
    let suggested_name = format!(
        "slate-support-{:04}{:02}{:02}-{}.zip",
        today.year(),
        u8::from(today.month()),
        today.day(),
        &report_id.to_string()[..8]
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
        .collect::<Vec<_>>();
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| support_report_error())?;
    let paths = state.paths.clone();
    let destination_for_export = destination.clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        export_support_report(
            &destination_for_export,
            SupportReportInput {
                report_id,
                created_at: &created_at,
                request,
                paths: &paths,
                instances: &instances,
                jobs: &jobs,
                private_values: &private_values,
            },
        )
    })
    .await
    .map_err(|_| support_report_error())?
    .map_err(|_| support_report_error())?;
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

fn support_report_error() -> AppError {
    AppError::new(
        "support.report_unavailable",
        "slate could not create the support report. Choose another location and try again.",
    )
    .retryable(true)
}
