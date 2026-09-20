use slate_domain::InstanceId;
use slate_platform::AppPaths;
use std::io;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstanceModFile {
    pub display_name: String,
    pub file_path: String,
    pub enabled: bool,
    pub size: u64,
    pub modified_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileMove {
    pub from: PathBuf,
    pub to: PathBuf,
    pub updated_file_path: Option<String>,
}

impl FileMove {
    pub fn rollback(&self) -> Result<(), io::Error> {
        if self.from.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the original mod path already exists",
            ));
        }
        std::fs::rename(&self.to, &self.from)
    }
}

pub(crate) fn scan_instance_mods(
    paths: &AppPaths,
    instance_id: InstanceId,
) -> Result<Vec<InstanceModFile>, io::Error> {
    let mods_directory = paths.instance(instance_id).join("game").join("mods");
    let entries = match std::fs::read_dir(mods_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut mods = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_file() || file_type.is_symlink() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let lower_name = file_name.to_ascii_lowercase();
        let enabled = lower_name.ends_with(".jar");
        if !enabled && !lower_name.ends_with(".jar.disabled") {
            continue;
        }
        let metadata = entry.metadata()?;
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|modified| OffsetDateTime::from(modified).format(&Rfc3339).ok());
        mods.push(InstanceModFile {
            display_name: display_name_from_file(&file_name),
            file_path: format!("mods/{file_name}"),
            enabled,
            size: metadata.len(),
            modified_at,
        });
    }
    mods.sort_by(|left, right| {
        left.display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase())
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
    Ok(mods)
}

pub(crate) fn set_instance_mod_enabled(
    paths: &AppPaths,
    instance_id: InstanceId,
    file_path: &str,
    enabled: bool,
) -> Result<FileMove, io::Error> {
    let (source, file_name) = instance_mod_path(paths, instance_id, file_path)?;
    let lower_name = file_name.to_ascii_lowercase();
    let target_name = if enabled {
        if !lower_name.ends_with(".jar.disabled") {
            return Err(invalid_mod_path());
        }
        file_name[..file_name.len() - ".disabled".len()].to_owned()
    } else {
        if !lower_name.ends_with(".jar") {
            return Err(invalid_mod_path());
        }
        format!("{file_name}.disabled")
    };
    let target = source.with_file_name(&target_name);
    ensure_move_source(&source, &target)?;
    std::fs::rename(&source, &target)?;
    Ok(FileMove {
        from: source,
        to: target,
        updated_file_path: Some(format!("mods/{target_name}")),
    })
}

pub(crate) fn trash_instance_mod(
    paths: &AppPaths,
    instance_id: InstanceId,
    file_path: &str,
) -> Result<FileMove, io::Error> {
    let (source, file_name) = instance_mod_path(paths, instance_id, file_path)?;
    let trash_root = paths.trash();
    let content_trash = trash_root.join("instance-content");
    let instance_trash = content_trash.join(instance_id.to_string());
    for directory in [&trash_root, &content_trash, &instance_trash] {
        ensure_plain_directory(directory)?;
    }
    let target = instance_trash.join(format!("{}-{file_name}", uuid::Uuid::new_v4()));
    ensure_move_source(&source, &target)?;
    std::fs::rename(&source, &target)?;
    Ok(FileMove {
        from: source,
        to: target,
        updated_file_path: None,
    })
}

fn instance_mod_path(
    paths: &AppPaths,
    instance_id: InstanceId,
    file_path: &str,
) -> Result<(PathBuf, String), io::Error> {
    let Some((directory, file_name)) = file_path.split_once('/') else {
        return Err(invalid_mod_path());
    };
    let lower_name = file_name.to_ascii_lowercase();
    if directory != "mods"
        || file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name == "."
        || file_name == ".."
        || file_name.chars().any(char::is_control)
        || (!lower_name.ends_with(".jar") && !lower_name.ends_with(".jar.disabled"))
    {
        return Err(invalid_mod_path());
    }
    let instance_root = paths.instance(instance_id);
    let game_directory = instance_root.join("game");
    let mods_directory = game_directory.join("mods");
    for directory in [&instance_root, &game_directory, &mods_directory] {
        let metadata = std::fs::symlink_metadata(directory)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(invalid_mod_path());
        }
    }
    Ok((mods_directory.join(file_name), file_name.to_owned()))
}

fn ensure_move_source(source: &Path, target: &Path) -> Result<(), io::Error> {
    let metadata = std::fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_mod_path());
    }
    if target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "the destination mod path already exists",
        ));
    }
    Ok(())
}

fn ensure_plain_directory(path: &Path) -> Result<(), io::Error> {
    match std::fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                Err(invalid_mod_path())
            } else {
                Ok(())
            }
        }
        Err(error) => Err(error),
    }
}

fn invalid_mod_path() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "invalid instance mod path")
}

fn display_name_from_file(file_name: &str) -> String {
    file_name
        .strip_suffix(".disabled")
        .unwrap_or(file_name)
        .strip_suffix(".jar")
        .unwrap_or(file_name)
        .chars()
        .take(240)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{scan_instance_mods, set_instance_mod_enabled, trash_instance_mod};
    use slate_domain::InstanceId;
    use slate_platform::AppPaths;

    #[test]
    fn inventories_enabled_and_disabled_mod_jars_only() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app-data"),
            temporary.path().join("storage"),
        );
        let instance_id = InstanceId::new();
        let mods = paths.instance(instance_id).join("game").join("mods");
        std::fs::create_dir_all(mods.join("folder.jar"))?;
        std::fs::write(mods.join("sodium-1.0.jar"), b"jar")?;
        std::fs::write(mods.join("disabled-mod.jar.disabled"), b"jar")?;
        std::fs::write(mods.join("shader.zip"), b"zip")?;

        let inventory = scan_instance_mods(&paths, instance_id)?;

        assert_eq!(inventory.len(), 2);
        assert_eq!(inventory[0].display_name, "disabled-mod");
        assert!(!inventory[0].enabled);
        assert_eq!(inventory[1].display_name, "sodium-1.0");
        assert!(inventory[1].enabled);
        Ok(())
    }

    #[test]
    fn toggles_mods_with_reversible_renames() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app-data"),
            temporary.path().join("storage"),
        );
        paths.ensure_base_directories()?;
        let instance_id = InstanceId::new();
        let mods = paths.instance(instance_id).join("game").join("mods");
        std::fs::create_dir_all(&mods)?;
        std::fs::write(mods.join("sodium.jar"), b"jar")?;

        let disabled = set_instance_mod_enabled(&paths, instance_id, "mods/sodium.jar", false)?;
        assert_eq!(
            disabled.updated_file_path.as_deref(),
            Some("mods/sodium.jar.disabled")
        );
        assert!(mods.join("sodium.jar.disabled").is_file());
        disabled.rollback()?;
        assert!(mods.join("sodium.jar").is_file());

        let disabled = set_instance_mod_enabled(&paths, instance_id, "mods/sodium.jar", false)?;
        let enabled = set_instance_mod_enabled(
            &paths,
            instance_id,
            disabled.updated_file_path.as_deref().unwrap_or_default(),
            true,
        )?;
        assert_eq!(
            enabled.updated_file_path.as_deref(),
            Some("mods/sodium.jar")
        );
        assert!(mods.join("sodium.jar").is_file());
        Ok(())
    }

    #[test]
    fn moves_removed_mods_to_trash_and_can_restore_them() -> Result<(), Box<dyn std::error::Error>>
    {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app-data"),
            temporary.path().join("storage"),
        );
        paths.ensure_base_directories()?;
        let instance_id = InstanceId::new();
        let mods = paths.instance(instance_id).join("game").join("mods");
        std::fs::create_dir_all(&mods)?;
        std::fs::write(mods.join("sodium.jar"), b"jar")?;

        let removed = trash_instance_mod(&paths, instance_id, "mods/sodium.jar")?;
        assert!(!mods.join("sodium.jar").exists());
        assert!(removed.to.is_file());
        removed.rollback()?;
        assert!(mods.join("sodium.jar").is_file());
        Ok(())
    }

    #[test]
    fn rejects_paths_outside_the_instance_mod_directory() -> Result<(), Box<dyn std::error::Error>>
    {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app-data"),
            temporary.path().join("storage"),
        );
        paths.ensure_base_directories()?;
        let instance_id = InstanceId::new();
        std::fs::create_dir_all(paths.instance(instance_id).join("game").join("mods"))?;

        for unsafe_path in [
            "../sodium.jar",
            "mods/../sodium.jar",
            "mods/nested/sodium.jar",
            "C:/sodium.jar",
            "mods/sodium.zip",
        ] {
            assert!(trash_instance_mod(&paths, instance_id, unsafe_path).is_err());
        }
        Ok(())
    }
}
