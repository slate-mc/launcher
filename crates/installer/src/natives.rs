use super::InstallError;
use slate_minecraft::NativeExtraction;
use std::io::{Read, Write};
use std::path::{Component, Path};
use uuid::Uuid;
use zip::ZipArchive;

const MAX_NATIVE_ENTRIES: usize = 100_000;
const MAX_NATIVE_BYTES: u64 = 1024 * 1024 * 1024;

pub(super) async fn extract_natives(
    extractions: Vec<NativeExtraction>,
) -> Result<(), InstallError> {
    if extractions.is_empty() {
        return Ok(());
    }
    let destination = extractions[0].destination.clone();
    let staging = destination.with_file_name(format!(".natives-{}.staging", Uuid::new_v4()));
    tokio::fs::create_dir_all(&staging).await?;
    let staging_for_worker = staging.clone();
    let extraction = tokio::task::spawn_blocking(move || {
        extract_native_archives(&extractions, &staging_for_worker)
    })
    .await?;
    if let Err(error) = extraction {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }
    if destination.exists() {
        tokio::fs::remove_dir_all(&destination).await?;
    }
    tokio::fs::rename(staging, destination).await?;
    Ok(())
}

fn extract_native_archives(
    extractions: &[NativeExtraction],
    staging: &Path,
) -> Result<(), InstallError> {
    let mut total = 0_u64;
    for extraction in extractions {
        let file = std::fs::File::open(&extraction.archive_path)?;
        let mut archive = ZipArchive::new(file)?;
        if archive.len() > MAX_NATIVE_ENTRIES {
            return Err(InstallError::TooManyNativeEntries(archive.len()));
        }
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let enclosed = entry
                .enclosed_name()
                .ok_or(InstallError::UnsafeArchivePath)?;
            let path = enclosed.to_path_buf();
            if entry.is_dir()
                || extraction
                    .excludes
                    .iter()
                    .any(|prefix| path.starts_with(prefix))
                || path.starts_with("META-INF")
            {
                continue;
            }
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
                || path
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err(InstallError::UnsafeArchivePath);
            }
            total = total
                .checked_add(entry.size())
                .ok_or(InstallError::NativeSetTooLarge)?;
            if total > MAX_NATIVE_BYTES {
                return Err(InstallError::NativeSetTooLarge);
            }
            let target = staging.join(path);
            let parent = target.parent().ok_or(InstallError::UnsafeArchivePath)?;
            std::fs::create_dir_all(parent)?;
            let mut output = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(target)?;
            let size = entry.size();
            std::io::copy(&mut entry.by_ref().take(size), &mut output)?;
            output.flush()?;
        }
    }
    Ok(())
}
