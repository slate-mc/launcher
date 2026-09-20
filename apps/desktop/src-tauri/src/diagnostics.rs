use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing_appender::non_blocking::{NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::util::SubscriberInitExt;

pub(super) const LOG_PREFIX: &str = "slate-desktop.jsonl";
const MAX_LOG_FILES: usize = 15;
const MAX_LOG_BYTES: u64 = 32 * 1024 * 1024;
const MAX_LOG_AGE: Duration = Duration::from_secs(14 * 24 * 60 * 60);
const BUFFERED_LINES: usize = 4_096;

pub(super) struct DiagnosticsGuard {
    _worker: WorkerGuard,
}

pub(super) fn init_diagnostics(log_directory: &Path) -> Result<DiagnosticsGuard, DiagnosticsError> {
    fs::create_dir_all(log_directory)?;
    prune_diagnostic_logs(log_directory, SystemTime::now())?;
    let appender = tracing_appender::rolling::daily(log_directory, LOG_PREFIX);
    let (writer, worker) = NonBlockingBuilder::default()
        .buffered_lines_limit(BUFFERED_LINES)
        .lossy(true)
        .thread_name("slate-diagnostics")
        .finish(appender);
    tracing_subscriber::fmt()
        .json()
        .with_ansi(false)
        .with_target(true)
        .with_current_span(true)
        .with_span_list(true)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("slate_desktop=info,warn")),
        )
        .with_writer(writer)
        .finish()
        .try_init()
        .map_err(|_| DiagnosticsError::SubscriberUnavailable)?;
    Ok(DiagnosticsGuard { _worker: worker })
}

fn prune_diagnostic_logs(
    directory: &Path,
    now: SystemTime,
) -> Result<PruneSummary, DiagnosticsError> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let name = entry.file_name();
        if !metadata.is_file() || !name.to_string_lossy().starts_with(LOG_PREFIX) {
            continue;
        }
        files.push(LogFile {
            path: entry.path(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            size: metadata.len(),
        });
    }
    files.sort_by_key(|file| file.modified);
    let mut removed_files = 0_usize;
    let mut removed_bytes = 0_u64;
    let mut retained = Vec::with_capacity(files.len());
    for file in files {
        let expired = now
            .duration_since(file.modified)
            .is_ok_and(|age| age > MAX_LOG_AGE);
        if expired {
            remove_log_file(&file, &mut removed_files, &mut removed_bytes)?;
        } else {
            retained.push(file);
        }
    }
    let mut retained_bytes = retained.iter().map(|file| file.size).sum::<u64>();
    let mut excess_files = retained.len().saturating_sub(MAX_LOG_FILES);
    for file in retained {
        if excess_files == 0 && retained_bytes <= MAX_LOG_BYTES {
            break;
        }
        remove_log_file(&file, &mut removed_files, &mut removed_bytes)?;
        retained_bytes = retained_bytes.saturating_sub(file.size);
        excess_files = excess_files.saturating_sub(1);
    }
    Ok(PruneSummary {
        removed_files,
        removed_bytes,
    })
}

fn remove_log_file(
    file: &LogFile,
    removed_files: &mut usize,
    removed_bytes: &mut u64,
) -> Result<(), DiagnosticsError> {
    fs::remove_file(&file.path)?;
    *removed_files += 1;
    *removed_bytes = removed_bytes.saturating_add(file.size);
    Ok(())
}

struct LogFile {
    path: PathBuf,
    modified: SystemTime,
    size: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct PruneSummary {
    removed_files: usize,
    removed_bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum DiagnosticsError {
    #[error("diagnostic storage is unavailable")]
    Io(#[from] std::io::Error),
    #[error("diagnostic event subscriber is unavailable")]
    SubscriberUnavailable,
}

#[cfg(test)]
mod tests {
    use super::{LOG_PREFIX, MAX_LOG_FILES, prune_diagnostic_logs};
    use std::time::SystemTime;

    #[test]
    fn pruning_is_bounded_and_ignores_unrelated_files() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        for index in 0..(MAX_LOG_FILES + 3) {
            std::fs::write(
                directory.path().join(format!("{LOG_PREFIX}.{index:02}")),
                vec![b'x'; 16],
            )?;
        }
        let unrelated = directory.path().join("minecraft-session.log");
        std::fs::write(&unrelated, b"keep")?;

        let summary = prune_diagnostic_logs(directory.path(), SystemTime::now())?;

        assert_eq!(summary.removed_files, 3);
        assert!(unrelated.exists());
        assert_eq!(
            std::fs::read_dir(directory.path())?
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(LOG_PREFIX))
                .count(),
            MAX_LOG_FILES
        );
        Ok(())
    }
}
