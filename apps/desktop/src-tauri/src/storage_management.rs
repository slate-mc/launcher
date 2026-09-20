use slate_domain::InstanceId;
use slate_platform::AppPaths;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DiskUsage {
    pub bytes: u64,
    pub files: u64,
}

impl DiskUsage {
    pub(crate) fn include(&mut self, other: Self) {
        self.bytes = self.bytes.saturating_add(other.bytes);
        self.files = self.files.saturating_add(other.files);
    }
}

#[derive(Debug)]
pub(crate) struct StorageUsage {
    pub total: DiskUsage,
    pub instances: DiskUsage,
    pub temporary: DiskUsage,
    pub removed_content: DiskUsage,
    pub logs: DiskUsage,
    pub shared_game_files: DiskUsage,
    pub managed_java: DiskUsage,
}

pub(crate) fn scan_storage_usage(
    paths: &AppPaths,
    instance_roots: &[PathBuf],
    additional_roots: &[PathBuf],
) -> Result<StorageUsage, io::Error> {
    let temporary_targets = temporary_targets(paths, instance_roots)?;
    let mut total = measure_path(paths.app_data())?;
    let mut instances = DiskUsage::default();
    for root in instance_roots {
        let usage = measure_path(root)?;
        instances.include(usage);
        if !root.starts_with(paths.app_data()) {
            total.include(usage);
        }
    }
    for root in additional_roots {
        if !root.starts_with(paths.app_data()) {
            total.include(measure_path(root)?);
        }
    }
    Ok(StorageUsage {
        total,
        instances,
        temporary: measure_paths(&temporary_targets)?,
        removed_content: measure_path(&paths.trash().join("instance-content"))?,
        logs: measure_log_roots(paths, instance_roots)?,
        shared_game_files: measure_path(&paths.artifacts())?,
        managed_java: measure_path(&paths.java_runtimes())?,
    })
}

pub(crate) fn measure_path(path: &Path) -> Result<DiskUsage, io::Error> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(DiskUsage::default()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() {
        return Ok(DiskUsage::default());
    }
    if metadata.is_file() {
        return Ok(DiskUsage {
            bytes: metadata.len(),
            files: 1,
        });
    }
    if !metadata.is_dir() {
        return Ok(DiskUsage::default());
    }
    let mut usage = DiskUsage::default();
    for entry in std::fs::read_dir(path)? {
        usage.include(measure_path(&entry?.path())?);
    }
    Ok(usage)
}

pub(crate) fn clear_temporary_files(
    paths: &AppPaths,
    instance_roots: &[PathBuf],
) -> Result<DiskUsage, io::Error> {
    let targets = temporary_targets(paths, instance_roots)?;
    let usage = measure_paths(&targets)?;
    for target in targets {
        remove_managed_entry(&target)?;
    }
    Ok(usage)
}

pub(crate) fn clear_removed_content(paths: &AppPaths) -> Result<DiskUsage, io::Error> {
    clear_directory_contents(&paths.trash().join("instance-content"))
}

pub(crate) fn clear_logs(
    paths: &AppPaths,
    instance_roots: &[PathBuf],
) -> Result<DiskUsage, io::Error> {
    let mut usage = clear_directory_contents(&paths.logs())?;
    for root in instance_roots {
        usage.include(clear_directory_contents(&root.join("logs"))?);
    }
    Ok(usage)
}

pub(crate) fn clear_shared_game_files(paths: &AppPaths) -> Result<DiskUsage, io::Error> {
    clear_directory_contents(&paths.artifacts())
}

pub(crate) fn clear_managed_java(paths: &AppPaths) -> Result<DiskUsage, io::Error> {
    clear_directory_contents(&paths.java_runtimes())
}

#[derive(Debug)]
pub(crate) struct StagedInstanceDeletion {
    original: PathBuf,
    staged: PathBuf,
}

impl StagedInstanceDeletion {
    pub fn rollback(self) -> Result<(), io::Error> {
        if self.original.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the instance directory was recreated before rollback",
            ));
        }
        std::fs::rename(self.staged, self.original)
    }

    pub fn purge(self) -> Result<(), io::Error> {
        remove_managed_entry(&self.staged)
    }
}

pub(crate) fn stage_instance_deletion(
    root: &Path,
    instance_id: InstanceId,
) -> Result<Option<StagedInstanceDeletion>, io::Error> {
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the managed instance root is not a plain directory",
        ));
    }
    if root.file_name() != Some(OsStr::new(&instance_id.to_string())) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the managed instance root does not match its instance id",
        ));
    }
    let parent = root.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the managed instance root has no parent directory",
        )
    })?;
    let staged = parent.join(format!(
        ".slate-trash-purge-{}-{}",
        instance_id,
        Uuid::new_v4()
    ));
    std::fs::rename(root, &staged)?;
    Ok(Some(StagedInstanceDeletion {
        original: root.to_path_buf(),
        staged,
    }))
}

fn measure_log_roots(paths: &AppPaths, instance_roots: &[PathBuf]) -> Result<DiskUsage, io::Error> {
    let mut usage = measure_path(&paths.logs())?;
    for root in instance_roots {
        usage.include(measure_path(&root.join("logs"))?);
    }
    Ok(usage)
}

fn measure_paths(paths: &BTreeSet<PathBuf>) -> Result<DiskUsage, io::Error> {
    let mut usage = DiskUsage::default();
    for path in paths {
        usage.include(measure_path(path)?);
    }
    Ok(usage)
}

fn temporary_targets(
    paths: &AppPaths,
    instance_roots: &[PathBuf],
) -> Result<BTreeSet<PathBuf>, io::Error> {
    let mut targets = BTreeSet::new();
    collect_named_residue(&paths.artifacts(), &mut targets)?;
    collect_named_residue(&paths.runtimes(), &mut targets)?;
    collect_directory_children(&paths.jobs(), &mut targets)?;
    for root in instance_roots {
        collect_top_level_residue(root, &mut targets)?;
        collect_named_residue(&root.join("revisions"), &mut targets)?;
        collect_named_residue(&root.join("snapshots"), &mut targets)?;
    }
    Ok(targets)
}

fn collect_directory_children(
    root: &Path,
    targets: &mut BTreeSet<PathBuf>,
) -> Result<(), io::Error> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let path = entry?.path();
        if !std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            targets.insert(path);
        }
    }
    Ok(())
}

fn collect_top_level_residue(
    root: &Path,
    targets: &mut BTreeSet<PathBuf>,
) -> Result<(), io::Error> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if is_residue_name(&entry.file_name().to_string_lossy()) {
            targets.insert(entry.path());
        }
    }
    Ok(())
}

fn collect_named_residue(root: &Path, targets: &mut BTreeSet<PathBuf>) -> Result<(), io::Error> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if is_residue_name(&entry.file_name().to_string_lossy()) {
            targets.insert(path);
        } else if metadata.is_dir() {
            collect_named_residue(&path, targets)?;
        }
    }
    Ok(())
}

fn is_residue_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains(".partial-")
        || name.contains(".corrupt-")
        || name.starts_with(".staging-")
        || name.starts_with(".restore-staging-")
        || name.starts_with(".restore-backup-")
        || name.starts_with(".slate-clearing-")
        || name.starts_with(".slate-trash-purge-")
        || name.starts_with(".slate-move-staging-")
        || name.starts_with(".slate-import-staging-")
        || name.starts_with(".slate-export-") && name.ends_with(".tmp")
        || name.contains(".tmp-")
        || name.contains(".bak-")
        || name == "content-staging"
}

fn clear_directory_contents(root: &Path) -> Result<DiskUsage, io::Error> {
    let usage = measure_path(root)?;
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            std::fs::create_dir_all(root)?;
            return Ok(usage);
        }
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "a managed storage category is not a plain directory",
        ));
    }
    let parent = root.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a managed storage category has no parent directory",
        )
    })?;
    let name = root
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("storage");
    let staged = parent.join(format!(".slate-clearing-{name}-{}", Uuid::new_v4()));
    std::fs::rename(root, &staged)?;
    if let Err(error) = std::fs::create_dir(root) {
        let _ = std::fs::rename(&staged, root);
        return Err(error);
    }
    remove_managed_entry(&staged)?;
    Ok(usage)
}

fn remove_managed_entry(path: &Path) -> Result<(), io::Error> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        std::fs::remove_file(path)
    } else if metadata.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{clear_temporary_files, measure_path, stage_instance_deletion};
    use slate_domain::InstanceId;
    use slate_platform::AppPaths;

    #[test]
    fn measures_files_without_following_managed_scope() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path().join("data");
        std::fs::create_dir_all(root.join("nested"))?;
        std::fs::write(root.join("one.bin"), [1_u8, 2, 3])?;
        std::fs::write(root.join("nested/two.bin"), [4_u8, 5])?;
        assert_eq!(
            measure_path(&root)?,
            super::DiskUsage { bytes: 5, files: 2 }
        );
        Ok(())
    }

    #[test]
    fn clears_only_known_temporary_artifacts() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app"),
            temporary.path().join("storage"),
        );
        paths.ensure_base_directories()?;
        std::fs::write(paths.artifacts().join("keep.jar"), [1_u8])?;
        std::fs::write(paths.artifacts().join("index.json.corrupt-test"), [2_u8, 3])?;
        let cleared = clear_temporary_files(&paths, &[])?;
        assert_eq!(cleared.bytes, 2);
        assert!(paths.artifacts().join("keep.jar").is_file());
        assert!(!paths.artifacts().join("index.json.corrupt-test").exists());
        Ok(())
    }

    #[test]
    fn staged_instance_delete_can_rollback_or_purge() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let first_id = InstanceId::new();
        let first = temporary.path().join(first_id.to_string());
        std::fs::create_dir(&first)?;
        std::fs::write(first.join("world.dat"), [1_u8])?;
        stage_instance_deletion(&first, first_id)?
            .ok_or("expected staged deletion")?
            .rollback()?;
        assert!(first.join("world.dat").is_file());

        let second_id = InstanceId::new();
        let second = temporary.path().join(second_id.to_string());
        std::fs::create_dir(&second)?;
        stage_instance_deletion(&second, second_id)?
            .ok_or("expected staged deletion")?
            .purge()?;
        assert!(!second.exists());
        Ok(())
    }
}
