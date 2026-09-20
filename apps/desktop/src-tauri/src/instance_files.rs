use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_TREE_ENTRIES: usize = 500_000;
const MAX_TREE_BYTES: u64 = 50 * 1024 * 1024 * 1024;

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
    #[error("snapshot data is missing")]
    SnapshotMissing,
    #[error("instance file I/O failed")]
    Io(#[from] std::io::Error),
}
