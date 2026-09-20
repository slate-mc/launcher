use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAX_TREE_ENTRIES: usize = 500_000;
const MAX_TREE_BYTES: u64 = 50 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const PORTABLE_MANIFEST: &str = "slate-instance.json";

pub fn copy_tree(source: &Path, destination: &Path) -> Result<u64, InstanceFileError> {
    if !source.exists() {
        std::fs::create_dir_all(destination)?;
        return Ok(0);
    }
    if !source.is_dir() {
        return Err(InstanceFileError::SourceNotDirectory);
    }
    std::fs::create_dir_all(destination)?;
    let mut queue = VecDeque::from([(source.to_path_buf(), destination.to_path_buf())]);
    let mut entries = 0_usize;
    let mut bytes = 0_u64;
    while let Some((current_source, current_destination)) = queue.pop_front() {
        for entry in std::fs::read_dir(&current_source)? {
            let entry = entry?;
            entries = entries.saturating_add(1);
            if entries > MAX_TREE_ENTRIES {
                return Err(InstanceFileError::EntryLimit);
            }
            let metadata = std::fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() {
                return Err(InstanceFileError::SymbolicLink);
            }
            let destination_entry = current_destination.join(entry.file_name());
            if metadata.is_dir() {
                std::fs::create_dir_all(&destination_entry)?;
                queue.push_back((entry.path(), destination_entry));
            } else if metadata.is_file() {
                bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or(InstanceFileError::SizeLimit)?;
                if bytes > MAX_TREE_BYTES {
                    return Err(InstanceFileError::SizeLimit);
                }
                std::fs::copy(entry.path(), destination_entry)?;
            }
        }
    }
    Ok(bytes)
}

pub fn create_snapshot(
    instance_root: &Path,
    snapshot_id: Uuid,
) -> Result<(PathBuf, u64), InstanceFileError> {
    let snapshots = instance_root.join("snapshots");
    std::fs::create_dir_all(&snapshots)?;
    let final_directory = snapshots.join(snapshot_id.to_string());
    let staging = snapshots.join(format!(".staging-{snapshot_id}"));
    if staging.exists() || final_directory.exists() {
        return Err(InstanceFileError::DestinationExists);
    }
    std::fs::create_dir(&staging)?;
    let result = copy_tree(&instance_root.join("game"), &staging.join("game"));
    let size = match result {
        Ok(size) => size,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    std::fs::rename(&staging, &final_directory)?;
    Ok((
        PathBuf::from("snapshots").join(snapshot_id.to_string()),
        size,
    ))
}

pub struct PendingGameRestore {
    game: PathBuf,
    backup: Option<PathBuf>,
}

impl PendingGameRestore {
    pub fn rollback(self) {
        let _ = std::fs::remove_dir_all(&self.game);
        if let Some(backup) = self.backup {
            let _ = std::fs::rename(backup, self.game);
        }
    }

    pub fn commit(self) {
        if let Some(backup) = self.backup {
            let _ = std::fs::remove_dir_all(backup);
        }
    }
}

pub fn prepare_snapshot_restore(
    instance_root: &Path,
    snapshot_id: Uuid,
) -> Result<PendingGameRestore, InstanceFileError> {
    let snapshot = instance_root
        .join("snapshots")
        .join(snapshot_id.to_string())
        .join("game");
    if !snapshot.is_dir() {
        return Err(InstanceFileError::SnapshotMissing);
    }
    let staging = instance_root.join(format!(".restore-staging-{snapshot_id}"));
    let backup = instance_root.join(format!(".restore-backup-{snapshot_id}"));
    if staging.exists() || backup.exists() {
        return Err(InstanceFileError::DestinationExists);
    }
    copy_tree(&snapshot, &staging)?;
    let game = instance_root.join("game");
    let previous = if game.exists() {
        if let Err(error) = std::fs::rename(&game, &backup) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error.into());
        }
        Some(backup)
    } else {
        None
    };
    if let Err(error) = std::fs::rename(&staging, &game) {
        if let Some(backup) = previous.as_ref() {
            let _ = std::fs::rename(backup, &game);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error.into());
    }
    Ok(PendingGameRestore {
        game,
        backup: previous,
    })
}

pub fn copy_duplicate_personal_data(
    source_instance: &Path,
    destination_instance: &Path,
    include_worlds: bool,
    include_screenshots: bool,
    include_settings: bool,
) -> Result<(), InstanceFileError> {
    let source_game = source_instance.join("game");
    let destination_game = destination_instance.join("game");
    std::fs::create_dir_all(&destination_game)?;
    if include_worlds {
        copy_tree(&source_game.join("saves"), &destination_game.join("saves"))?;
    }
    if include_screenshots {
        copy_tree(
            &source_game.join("screenshots"),
            &destination_game.join("screenshots"),
        )?;
    }
    if include_settings {
        for directory in ["config", "defaultconfigs", "resourcepacks", "shaderpacks"] {
            copy_tree(
                &source_game.join(directory),
                &destination_game.join(directory),
            )?;
        }
        for filename in ["options.txt", "servers.dat"] {
            let source = source_game.join(filename);
            if source.is_file() {
                std::fs::copy(source, destination_game.join(filename))?;
            }
        }
        let destination_metadata = destination_instance.join("metadata");
        std::fs::create_dir_all(&destination_metadata)?;
        for filename in ["profile-icon.bin", "profile-banner.bin"] {
            let source = source_instance.join("metadata").join(filename);
            if source.is_file() {
                std::fs::copy(source, destination_metadata.join(filename))?;
            }
        }
    }
    Ok(())
}

pub struct PendingInstanceRelocation {
    source: PathBuf,
    destination: PathBuf,
}

impl PendingInstanceRelocation {
    pub fn rollback(self) {
        let _ = std::fs::remove_dir_all(self.destination);
    }

    pub fn commit(self) -> Result<(), InstanceFileError> {
        std::fs::remove_dir_all(self.source)?;
        Ok(())
    }
}

pub fn prepare_instance_relocation(
    source: &Path,
    destination: &Path,
) -> Result<PendingInstanceRelocation, InstanceFileError> {
    let source = std::fs::canonicalize(source)?;
    let parent = destination
        .parent()
        .ok_or(InstanceFileError::InvalidDestination)?;
    std::fs::create_dir_all(parent)?;
    let parent = std::fs::canonicalize(parent)?;
    let filename = destination
        .file_name()
        .ok_or(InstanceFileError::InvalidDestination)?;
    let destination = parent.join(filename);
    if destination == source || destination.starts_with(&source) {
        return Err(InstanceFileError::InvalidDestination);
    }
    if destination.exists() {
        return Err(InstanceFileError::DestinationExists);
    }
    let staging = parent.join(format!(".slate-move-staging-{}", Uuid::new_v4()));
    if staging.exists() {
        return Err(InstanceFileError::DestinationExists);
    }
    if let Err(error) = copy_tree(&source, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&staging, &destination) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error.into());
    }
    Ok(PendingInstanceRelocation {
        source,
        destination,
    })
}

pub fn export_portable_archive(
    instance_root: &Path,
    destination: &Path,
    manifest: &[u8],
) -> Result<(), InstanceFileError> {
    if manifest.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(InstanceFileError::ManifestTooLarge);
    }
    let parent = destination
        .parent()
        .ok_or(InstanceFileError::InvalidDestination)?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".slate-export-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let output = std::fs::File::create(&temporary)?;
        let mut archive = ZipWriter::new(output);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o600);
        archive.start_file(PORTABLE_MANIFEST, options)?;
        archive.write_all(manifest)?;
        let mut entries = 0_usize;
        let mut bytes = manifest.len() as u64;
        for directory in ["game", "metadata"] {
            let source = instance_root.join(directory);
            if !source.exists() {
                continue;
            }
            append_directory_to_archive(
                &mut archive,
                instance_root,
                &source,
                &mut entries,
                &mut bytes,
            )?;
        }
        archive.finish()?;
        if destination.exists() {
            std::fs::remove_file(destination)?;
        }
        std::fs::rename(&temporary, destination)?;
        Ok::<_, InstanceFileError>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

fn append_directory_to_archive(
    archive: &mut ZipWriter<std::fs::File>,
    instance_root: &Path,
    source: &Path,
    entries: &mut usize,
    bytes: &mut u64,
) -> Result<(), InstanceFileError> {
    let mut queue = VecDeque::from([source.to_path_buf()]);
    while let Some(directory) = queue.pop_front() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            *entries = entries.saturating_add(1);
            if *entries > MAX_TREE_ENTRIES {
                return Err(InstanceFileError::EntryLimit);
            }
            let metadata = std::fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() {
                return Err(InstanceFileError::SymbolicLink);
            }
            if metadata.is_dir() {
                queue.push_back(entry.path());
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            *bytes = bytes
                .checked_add(metadata.len())
                .ok_or(InstanceFileError::SizeLimit)?;
            if *bytes > MAX_TREE_BYTES {
                return Err(InstanceFileError::SizeLimit);
            }
            let relative = entry
                .path()
                .strip_prefix(instance_root)
                .map_err(|_| InstanceFileError::InvalidDestination)?
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o600);
            archive.start_file(format!("payload/{relative}"), options)?;
            let mut input = std::fs::File::open(entry.path())?;
            std::io::copy(&mut input, archive)?;
        }
    }
    Ok(())
}

pub fn read_portable_manifest(source: &Path) -> Result<Vec<u8>, InstanceFileError> {
    let input = std::fs::File::open(source)?;
    let mut archive = ZipArchive::new(input)?;
    let manifest = archive
        .by_name(PORTABLE_MANIFEST)
        .map_err(|_| InstanceFileError::ManifestMissing)?;
    if manifest.size() > MAX_MANIFEST_BYTES {
        return Err(InstanceFileError::ManifestTooLarge);
    }
    let mut bytes = Vec::with_capacity(usize::try_from(manifest.size()).unwrap_or(0));
    let mut limited = manifest.take(MAX_MANIFEST_BYTES + 1);
    limited.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(InstanceFileError::ManifestTooLarge);
    }
    Ok(bytes)
}

pub fn extract_portable_archive(
    source: &Path,
    instance_root: &Path,
) -> Result<(), InstanceFileError> {
    let parent = instance_root
        .parent()
        .ok_or(InstanceFileError::InvalidDestination)?;
    std::fs::create_dir_all(parent)?;
    if instance_root.exists() {
        return Err(InstanceFileError::DestinationExists);
    }
    let staging = parent.join(format!(".slate-import-staging-{}", Uuid::new_v4()));
    let result = (|| {
        std::fs::create_dir(&staging)?;
        let input = std::fs::File::open(source)?;
        let mut archive = ZipArchive::new(input)?;
        if archive.len() > MAX_TREE_ENTRIES + 1 {
            return Err(InstanceFileError::EntryLimit);
        }
        let mut bytes = 0_u64;
        for index in 0..archive.len() {
            let entry = archive.by_index(index)?;
            if entry.name() == PORTABLE_MANIFEST {
                continue;
            }
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                return Err(InstanceFileError::SymbolicLink);
            }
            let enclosed = entry
                .enclosed_name()
                .ok_or(InstanceFileError::UnsafeArchivePath)?;
            let relative = enclosed
                .strip_prefix("payload")
                .map_err(|_| InstanceFileError::UnsafeArchivePath)?;
            let first = relative
                .components()
                .next()
                .ok_or(InstanceFileError::UnsafeArchivePath)?;
            if !matches!(first.as_os_str().to_str(), Some("game" | "metadata")) {
                return Err(InstanceFileError::UnsafeArchivePath);
            }
            let destination = staging.join(relative);
            if entry.is_dir() {
                std::fs::create_dir_all(destination)?;
                continue;
            }
            bytes = bytes
                .checked_add(entry.size())
                .ok_or(InstanceFileError::SizeLimit)?;
            if bytes > MAX_TREE_BYTES {
                return Err(InstanceFileError::SizeLimit);
            }
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut output = std::fs::File::create(destination)?;
            let entry_size = entry.size();
            let mut limited = entry.take(entry_size);
            std::io::copy(&mut limited, &mut output)?;
        }
        std::fs::rename(&staging, instance_root)?;
        Ok::<_, InstanceFileError>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(staging);
    }
    result
}

#[derive(Debug, thiserror::Error)]
pub enum InstanceFileError {
    #[error("source is not a directory")]
    SourceNotDirectory,
    #[error("instance data contains a symbolic link")]
    SymbolicLink,
    #[error("instance data contains too many entries")]
    EntryLimit,
    #[error("instance data exceeds the snapshot size limit")]
    SizeLimit,
    #[error("destination already exists")]
    DestinationExists,
    #[error("destination is not a safe instance location")]
    InvalidDestination,
    #[error("snapshot data is missing")]
    SnapshotMissing,
    #[error("portable instance manifest is missing")]
    ManifestMissing,
    #[error("portable instance manifest is too large")]
    ManifestTooLarge,
    #[error("portable archive contains an unsafe path")]
    UnsafeArchivePath,
    #[error("portable archive is invalid")]
    Archive(#[from] zip::result::ZipError),
    #[error("instance file I/O failed")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::{
        InstanceFileError, export_portable_archive, extract_portable_archive,
        prepare_instance_relocation, read_portable_manifest,
    };
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn portable_archive_roundtrips_owned_payload() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        std::fs::create_dir_all(source.join("game/saves/world"))?;
        std::fs::create_dir_all(source.join("metadata"))?;
        std::fs::write(source.join("game/saves/world/level.dat"), b"world")?;
        std::fs::write(source.join("metadata/profile-icon.bin"), b"image")?;
        let archive = temporary.path().join("instance.zip");
        let manifest = br#"{"schema":1}"#;

        export_portable_archive(&source, &archive, manifest)?;
        assert_eq!(read_portable_manifest(&archive)?, manifest);

        let imported = temporary.path().join("imported");
        extract_portable_archive(&archive, &imported)?;
        assert_eq!(
            std::fs::read(imported.join("game/saves/world/level.dat"))?,
            b"world"
        );
        assert_eq!(
            std::fs::read(imported.join("metadata/profile-icon.bin"))?,
            b"image"
        );
        Ok(())
    }

    #[test]
    fn portable_archive_rejects_traversal() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let archive_path = temporary.path().join("unsafe.zip");
        let output = std::fs::File::create(&archive_path)?;
        let mut archive = zip::ZipWriter::new(output);
        archive.start_file("slate-instance.json", SimpleFileOptions::default())?;
        archive.write_all(br#"{"schema":1}"#)?;
        archive.start_file(
            "payload/game/../../../outside.txt",
            SimpleFileOptions::default(),
        )?;
        archive.write_all(b"unsafe")?;
        archive.finish()?;

        let result = extract_portable_archive(&archive_path, &temporary.path().join("instance"));
        assert!(matches!(result, Err(InstanceFileError::UnsafeArchivePath)));
        assert!(!temporary.path().join("outside.txt").exists());
        Ok(())
    }

    #[test]
    fn relocation_keeps_source_until_commit() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let destination = temporary.path().join("other/instances/id");
        std::fs::create_dir_all(source.join("game"))?;
        std::fs::write(source.join("game/options.txt"), b"lang:en_us")?;

        let pending = prepare_instance_relocation(&source, &destination)?;
        assert!(source.exists());
        assert!(destination.join("game/options.txt").exists());
        pending.commit()?;
        assert!(!source.exists());
        assert!(destination.join("game/options.txt").exists());
        Ok(())
    }
}
