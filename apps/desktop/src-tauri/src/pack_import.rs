use serde_json::Value;
use slate_modpack_api_contracts::ImportPackFormat;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;
use zip::ZipArchive;

const SLATE_MANIFEST: &str = "slate-instance.json";
const CURSEFORGE_MANIFEST: &str = "manifest.json";
const MODRINTH_MANIFEST: &str = "modrinth.index.json";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 200_000;
const MAX_EXTRACTED_BYTES: u64 = 32 * 1024 * 1024 * 1024;

#[derive(Debug)]
pub(super) enum DetectedImportArchive {
    Slate(Vec<u8>),
    Pack {
        format: ImportPackFormat,
        manifest: Value,
    },
}

pub(super) fn detect_import_archive(
    source: &Path,
) -> Result<DetectedImportArchive, PackImportError> {
    let metadata = std::fs::metadata(source)?;
    if !metadata.is_file() || metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(PackImportError::ArchiveTooLarge);
    }
    let input = std::fs::File::open(source)?;
    let mut archive = ZipArchive::new(input)?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(PackImportError::ArchiveTooLarge);
    }
    if let Ok(bytes) = read_entry(&mut archive, SLATE_MANIFEST) {
        return Ok(DetectedImportArchive::Slate(bytes));
    }
    if let Ok(bytes) = read_entry(&mut archive, MODRINTH_MANIFEST) {
        return Ok(DetectedImportArchive::Pack {
            format: ImportPackFormat::Modrinth,
            manifest: serde_json::from_slice(&bytes)?,
        });
    }
    if let Ok(bytes) = read_entry(&mut archive, CURSEFORGE_MANIFEST) {
        return Ok(DetectedImportArchive::Pack {
            format: ImportPackFormat::CurseForge,
            manifest: serde_json::from_slice(&bytes)?,
        });
    }
    Err(PackImportError::ManifestMissing)
}

fn read_entry(
    archive: &mut ZipArchive<std::fs::File>,
    name: &str,
) -> Result<Vec<u8>, PackImportError> {
    let entry = archive
        .by_name(name)
        .map_err(|_| PackImportError::ManifestMissing)?;
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(PackImportError::ManifestTooLarge);
    }
    let mut bytes = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
    let mut limited = entry.take(MAX_MANIFEST_BYTES + 1);
    limited.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(PackImportError::ManifestTooLarge);
    }
    Ok(bytes)
}

pub(super) fn stage_import_archive(
    source: &Path,
    instance_root: &Path,
) -> Result<PathBuf, PackImportError> {
    let metadata = std::fs::metadata(source)?;
    if !metadata.is_file() || metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(PackImportError::ArchiveTooLarge);
    }
    let metadata_directory = instance_root.join("metadata");
    std::fs::create_dir_all(&metadata_directory)?;
    let destination = metadata_directory.join("import-source.zip");
    let temporary = metadata_directory.join(format!(".import-{}.tmp", Uuid::new_v4()));
    std::fs::copy(source, &temporary)?;
    if let Err(error) = std::fs::rename(&temporary, &destination) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(destination)
}

pub(super) fn extract_import_overrides(
    source: &Path,
    instance_root: &Path,
    prefixes: &[String],
) -> Result<(), PackImportError> {
    let game_directory = instance_root.join("game");
    std::fs::create_dir_all(&game_directory)?;
    let mut extracted = 0_u64;
    for raw_prefix in prefixes {
        let prefix = safe_prefix(raw_prefix)?;
        let input = std::fs::File::open(source)?;
        let mut archive = ZipArchive::new(input)?;
        if archive.len() > MAX_ARCHIVE_ENTRIES {
            return Err(PackImportError::ArchiveTooLarge);
        }
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                return Err(PackImportError::SymbolicLink);
            }
            let enclosed = entry
                .enclosed_name()
                .ok_or(PackImportError::UnsafeArchivePath)?;
            let Ok(relative) = enclosed.strip_prefix(&prefix) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            validate_relative(relative)?;
            let destination = game_directory.join(relative);
            if entry.is_dir() {
                std::fs::create_dir_all(destination)?;
                continue;
            }
            extracted = extracted
                .checked_add(entry.size())
                .ok_or(PackImportError::ArchiveTooLarge)?;
            if extracted > MAX_EXTRACTED_BYTES {
                return Err(PackImportError::ArchiveTooLarge);
            }
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut output = std::fs::File::create(destination)?;
            std::io::copy(&mut entry, &mut output)?;
        }
    }
    Ok(())
}

fn safe_prefix(value: &str) -> Result<PathBuf, PackImportError> {
    let value = value.replace('\\', "/");
    let path = PathBuf::from(value.trim_matches('/'));
    validate_relative(&path)?;
    Ok(path)
}

fn validate_relative(path: &Path) -> Result<(), PackImportError> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            !matches!(component, Component::Normal(_))
                || component.as_os_str().to_string_lossy().contains(':')
        })
    {
        return Err(PackImportError::UnsafeArchivePath);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub(super) enum PackImportError {
    #[error("the archive does not contain a supported manifest")]
    ManifestMissing,
    #[error("the archive manifest is too large")]
    ManifestTooLarge,
    #[error("the archive is too large")]
    ArchiveTooLarge,
    #[error("the archive contains an unsafe path")]
    UnsafeArchivePath,
    #[error("the archive contains a symbolic link")]
    SymbolicLink,
    #[error("the archive is invalid")]
    Archive(#[from] zip::result::ZipError),
    #[error("the archive manifest is invalid")]
    Json(#[from] serde_json::Error),
    #[error("the archive could not be read")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::{DetectedImportArchive, detect_import_archive, extract_import_overrides};
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn detects_modrinth_archives_and_extracts_safe_overrides()
    -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let archive_path = temporary.path().join("example.mrpack");
        let output = std::fs::File::create(&archive_path)?;
        let mut archive = zip::ZipWriter::new(output);
        archive.start_file("modrinth.index.json", SimpleFileOptions::default())?;
        archive.write_all(br#"{"formatVersion":1}"#)?;
        archive.start_file(
            "overrides/config/example.toml",
            SimpleFileOptions::default(),
        )?;
        archive.write_all(b"enabled=true")?;
        archive.finish()?;

        assert!(matches!(
            detect_import_archive(&archive_path)?,
            DetectedImportArchive::Pack { .. }
        ));
        let instance = temporary.path().join("instance");
        extract_import_overrides(&archive_path, &instance, &["overrides".to_owned()])?;
        assert_eq!(
            std::fs::read(instance.join("game/config/example.toml"))?,
            b"enabled=true"
        );
        Ok(())
    }
}
