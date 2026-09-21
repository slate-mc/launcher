use crate::diagnostics::LOG_PREFIX;
use serde::Serialize;
use serde_json::{Value, json};
use slate_contracts::{CreateSupportReportRequest, SupportReportPreview};
use slate_platform::AppPaths;
use slate_storage::{InstallJobRecord, InstanceRecord};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const MAX_LOG_LINE_BYTES: usize = 512 * 1024;

pub(super) fn support_report_preview(
    log_directory: &Path,
) -> Result<SupportReportPreview, SupportReportError> {
    let files = diagnostic_files(log_directory)?;
    Ok(SupportReportPreview {
        diagnostic_file_count: u32::try_from(files.len()).unwrap_or(u32::MAX),
        diagnostic_bytes: files.iter().map(|file| file.size).sum(),
    })
}

pub(super) struct SupportReportInput<'a> {
    pub report_id: Uuid,
    pub created_at: &'a str,
    pub request: CreateSupportReportRequest,
    pub paths: &'a AppPaths,
    pub instances: &'a [InstanceRecord],
    pub jobs: &'a [InstallJobRecord],
    pub private_values: &'a [String],
}

pub(super) fn export_support_report(
    destination: &Path,
    input: SupportReportInput<'_>,
) -> Result<u64, SupportReportError> {
    let parent = destination
        .parent()
        .ok_or(SupportReportError::DestinationUnavailable)?;
    let temporary = parent.join(format!(".slate-support-{}.partial", input.report_id));
    let result = write_support_archive(&temporary, &input);
    let bytes = match result {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    };
    if let Err(error) = fs::copy(&temporary, destination) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    let _ = fs::remove_file(&temporary);
    Ok(bytes)
}

fn write_support_archive(
    destination: &Path,
    input: &SupportReportInput<'_>,
) -> Result<u64, SupportReportError> {
    let output = File::create(destination)?;
    let mut archive = ZipWriter::new(output);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o600);
    let replacements = redaction_values(input.paths, input.instances, input.private_values);
    let manifest = SupportManifest::new(input);

    archive.start_file("report.json", options)?;
    serde_json::to_writer_pretty(&mut archive, &manifest)?;
    archive.write_all(b"\n")?;

    if input.request.include_launcher_logs {
        for (index, file) in diagnostic_files(&input.paths.logs())?.iter().enumerate() {
            archive.start_file(format!("launcher/diagnostics-{index:02}.jsonl"), options)?;
            write_sanitized_log(&mut archive, &file.path, &replacements)?;
        }
    }
    archive.finish()?;
    Ok(fs::metadata(destination)?.len())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportManifest<'a> {
    schema: u32,
    report_id: Uuid,
    created_at: &'a str,
    slate_version: &'static str,
    platform: SupportPlatform,
    included: SupportInclusions,
    privacy: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    instances: Option<Vec<SupportInstance<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    installation_activity: Option<Vec<SupportInstallActivity<'a>>>,
}

impl<'a> SupportManifest<'a> {
    fn new(input: &'a SupportReportInput<'a>) -> Self {
        Self {
            schema: 1,
            report_id: input.report_id,
            created_at: input.created_at,
            slate_version: env!("CARGO_PKG_VERSION"),
            platform: SupportPlatform {
                os: std::env::consts::OS,
                architecture: std::env::consts::ARCH,
            },
            included: SupportInclusions {
                launcher_logs: input.request.include_launcher_logs,
                install_activity: input.request.include_install_activity,
                instance_summary: input.request.include_instance_summary,
            },
            privacy: "Account credentials, player identity, absolute paths, worlds, screenshots, and Minecraft chat are not included.",
            instances: input
                .request
                .include_instance_summary
                .then(|| input.instances.iter().map(SupportInstance::from).collect()),
            installation_activity: input.request.include_install_activity.then(|| {
                input
                    .jobs
                    .iter()
                    .map(SupportInstallActivity::from)
                    .collect()
            }),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportPlatform {
    os: &'static str,
    architecture: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportInclusions {
    launcher_logs: bool,
    install_activity: bool,
    instance_summary: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportInstance<'a> {
    minecraft_version: &'a str,
    loader: &'static str,
    loader_version: Option<&'a str>,
    setup_state: &'static str,
    mod_count: u32,
}

impl<'a> From<&'a InstanceRecord> for SupportInstance<'a> {
    fn from(instance: &'a InstanceRecord) -> Self {
        Self {
            minecraft_version: &instance.minecraft_version,
            loader: instance.loader_kind.as_storage_value(),
            loader_version: instance.loader_version.as_deref(),
            setup_state: instance.setup_state.as_storage_value(),
            mod_count: instance.mod_count,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportInstallActivity<'a> {
    state: &'static str,
    phase: &'a str,
    completed_items: Option<u64>,
    total_items: Option<u64>,
    created_at: &'a str,
    updated_at: &'a str,
}

impl<'a> From<&'a InstallJobRecord> for SupportInstallActivity<'a> {
    fn from(job: &'a InstallJobRecord) -> Self {
        Self {
            state: job.state.as_storage_value(),
            phase: &job.phase,
            completed_items: job.completed_items,
            total_items: job.total_items,
            created_at: &job.created_at,
            updated_at: &job.updated_at,
        }
    }
}

fn write_sanitized_log(
    archive: &mut ZipWriter<File>,
    source: &Path,
    replacements: &[String],
) -> Result<(), SupportReportError> {
    let mut reader = BufReader::new(File::open(source)?);
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        if line.len() > MAX_LOG_LINE_BYTES {
            line.truncate(MAX_LOG_LINE_BYTES);
        }
        let raw = String::from_utf8_lossy(&line);
        let mut value = serde_json::from_str::<Value>(raw.trim()).unwrap_or_else(|_| {
            json!({
                "message": sanitize_text(raw.trim(), replacements),
                "unparsed": true
            })
        });
        sanitize_value(&mut value, replacements, None);
        serde_json::to_writer(&mut *archive, &value)?;
        archive.write_all(b"\n")?;
    }
    Ok(())
}

fn sanitize_value(value: &mut Value, replacements: &[String], key: Option<&str>) {
    if key.is_some_and(is_sensitive_key) {
        *value = Value::String("[redacted]".to_owned());
        return;
    }
    match value {
        Value::String(text) => *text = sanitize_text(text, replacements),
        Value::Array(values) => {
            for value in values {
                sanitize_value(value, replacements, None);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                sanitize_value(value, replacements, Some(key));
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "authorization",
        "password",
        "secret",
        "credential",
        "access_token",
        "refreshtoken",
        "refresh_token",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

pub(super) fn sanitize_text(text: &str, replacements: &[String]) -> String {
    let mut sanitized = text.to_owned();
    for replacement in replacements {
        if replacement.is_empty() {
            continue;
        }
        sanitized = sanitized.replace(replacement, "<private-path>");
        sanitized = sanitized.replace(&replacement.replace('\\', "/"), "<private-path>");
    }
    for marker in [
        "access_token=",
        "refresh_token=",
        "client_secret=",
        "authorization=",
        "Bearer ",
        "\"access_token\":\"",
        "\"refresh_token\":\"",
        "\"client_secret\":\"",
        "\"authorization\":\"",
    ] {
        sanitized = redact_marker_value(&sanitized, marker);
    }
    sanitized
}

fn redact_marker_value(text: &str, marker: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(index) = remaining.find(marker) {
        let value_start = index + marker.len();
        output.push_str(&remaining[..value_start]);
        output.push_str("[redacted]");
        let tail = &remaining[value_start..];
        let value_end = tail
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '&' | '"' | '\'')
            })
            .unwrap_or(tail.len());
        remaining = &tail[value_end..];
    }
    output.push_str(remaining);
    output
}

fn redaction_values(
    paths: &AppPaths,
    instances: &[InstanceRecord],
    private_values: &[String],
) -> Vec<String> {
    let mut values = vec![
        paths.app_data().to_string_lossy().into_owned(),
        paths.storage_root().to_string_lossy().into_owned(),
    ];
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        values.push(PathBuf::from(profile).to_string_lossy().into_owned());
    }
    values.extend(
        instances
            .iter()
            .map(|instance| instance.storage_path.to_string_lossy().into_owned()),
    );
    values.extend(private_values.iter().cloned());
    values.retain(|value| value.chars().count() >= 3);
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    values
}

fn diagnostic_files(directory: &Path) -> Result<Vec<DiagnosticFile>, SupportReportError> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() && entry.file_name().to_string_lossy().starts_with(LOG_PREFIX) {
            files.push(DiagnosticFile {
                path: entry.path(),
                size: metadata.len(),
                modified: metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            });
        }
    }
    files.sort_by_key(|file| file.modified);
    Ok(files)
}

struct DiagnosticFile {
    path: PathBuf,
    size: u64,
    modified: std::time::SystemTime,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum SupportReportError {
    #[error("support report destination is unavailable")]
    DestinationUnavailable,
    #[error("support report files are unavailable")]
    Io(#[from] std::io::Error),
    #[error("support report archive is invalid")]
    Archive(#[from] zip::result::ZipError),
    #[error("support report data is invalid")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::{SupportReportInput, export_support_report, sanitize_text};
    use crate::diagnostics::LOG_PREFIX;
    use slate_contracts::CreateSupportReportRequest;
    use slate_platform::AppPaths;
    use std::fs::File;
    use std::io::Read;
    use uuid::Uuid;
    use zip::ZipArchive;

    #[test]
    fn support_archive_redacts_paths_and_secret_fields() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let app_data = directory.path().join("private-user").join("slate");
        let storage = app_data.join("storage");
        let paths = AppPaths::from_roots(app_data.clone(), storage);
        paths.ensure_base_directories()?;
        std::fs::write(
            paths.logs().join(format!("{LOG_PREFIX}.2026-09-20")),
            format!(
                "{{\"message\":\"opened {} access_token=visible&next=yes\",\"client_secret\":\"visible\"}}\n",
                app_data.display()
            ),
        )?;
        let destination = directory.path().join("support.zip");
        export_support_report(
            &destination,
            SupportReportInput {
                report_id: Uuid::nil(),
                created_at: "2026-09-20T00:00:00Z",
                request: CreateSupportReportRequest {
                    include_launcher_logs: true,
                    include_install_activity: false,
                    include_instance_summary: false,
                },
                paths: &paths,
                instances: &[],
                jobs: &[],
                private_values: &[],
            },
        )?;

        let mut archive = ZipArchive::new(File::open(destination)?)?;
        let mut log = String::new();
        archive
            .by_name("launcher/diagnostics-00.jsonl")?
            .read_to_string(&mut log)?;
        assert!(!log.contains("private-user"));
        assert!(!log.contains("visible"));
        assert!(log.contains("<private-path>"));
        assert!(log.contains("[redacted]"));
        Ok(())
    }

    #[test]
    fn marker_redaction_preserves_the_surrounding_message() {
        assert_eq!(
            sanitize_text("before Bearer token-value after", &[]),
            "before Bearer [redacted] after"
        );
    }
}
