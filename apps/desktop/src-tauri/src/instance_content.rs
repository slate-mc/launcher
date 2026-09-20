use slate_domain::{InstanceId, LoaderFamily};
use slate_platform::AppPaths;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const MAXIMUM_LOCAL_MOD_SIZE: u64 = 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub(crate) enum LocalModImportError {
    #[error("that mod file is already installed")]
    AlreadyInstalled,
    #[error("the selected file is not a valid mod JAR")]
    InvalidArchive,
    #[error("the selected mod does not support this instance loader")]
    WrongLoader,
    #[error("the selected mod is too large")]
    TooLarge,
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportedModFile {
    pub destination: PathBuf,
    pub file_path: String,
}

impl ImportedModFile {
    pub fn rollback(&self) -> Result<(), io::Error> {
        match std::fs::remove_file(&self.destination) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstanceModFile {
    pub display_name: String,
    pub file_path: String,
    pub enabled: bool,
    pub size: u64,
    pub modified_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InstanceContentKind {
    Resource,
    Shader,
    Data,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstanceContentFile {
    pub display_name: String,
    pub file_path: String,
    pub enabled: bool,
    pub can_toggle: bool,
    pub size: u64,
    pub modified_at: Option<String>,
    pub world_name: Option<String>,
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

pub(crate) fn validate_local_mod_source(
    source: &Path,
    loader: LoaderFamily,
) -> Result<(), LocalModImportError> {
    let metadata = std::fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(LocalModImportError::InvalidArchive);
    }
    if metadata.len() == 0 {
        return Err(LocalModImportError::InvalidArchive);
    }
    if metadata.len() > MAXIMUM_LOCAL_MOD_SIZE {
        return Err(LocalModImportError::TooLarge);
    }
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| safe_component(name) && name.to_ascii_lowercase().ends_with(".jar"))
        .ok_or(LocalModImportError::InvalidArchive)?;
    if file_name.len() > 240 {
        return Err(LocalModImportError::InvalidArchive);
    }
    validate_mod_archive(source, loader)
}

pub(crate) fn import_local_mod(
    paths: &AppPaths,
    instance_id: InstanceId,
    loader: LoaderFamily,
    source: &Path,
) -> Result<ImportedModFile, LocalModImportError> {
    validate_local_mod_source(source, loader)?;
    ensure_local_mod_not_installed(paths, instance_id, source)?;
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(LocalModImportError::InvalidArchive)?;
    let instance_root = paths.instance(instance_id);
    let game_directory = instance_root.join("game");
    ensure_existing_plain_directory(&instance_root)?;
    ensure_existing_plain_directory(&game_directory)?;
    let mods_directory = game_directory.join("mods");
    ensure_plain_directory(&mods_directory)?;
    let destination = mods_directory.join(file_name);
    if destination.exists() {
        return Err(LocalModImportError::AlreadyInstalled);
    }

    let mut input = File::open(source)?;
    let mut pending = NamedTempFile::new_in(&mods_directory)?;
    let copied = io::copy(
        &mut std::io::Read::take(&mut input, MAXIMUM_LOCAL_MOD_SIZE + 1),
        &mut pending,
    )?;
    if copied == 0 {
        return Err(LocalModImportError::InvalidArchive);
    }
    if copied > MAXIMUM_LOCAL_MOD_SIZE {
        return Err(LocalModImportError::TooLarge);
    }
    pending.as_file_mut().sync_all()?;
    validate_mod_archive(pending.path(), loader)?;
    let persisted = pending.persist_noclobber(&destination).map_err(|error| {
        if error.error.kind() == io::ErrorKind::AlreadyExists {
            LocalModImportError::AlreadyInstalled
        } else {
            LocalModImportError::Io(error.error)
        }
    })?;
    persisted.sync_all()?;
    Ok(ImportedModFile {
        destination,
        file_path: format!("mods/{file_name}"),
    })
}

pub(crate) fn ensure_local_mod_not_installed(
    paths: &AppPaths,
    instance_id: InstanceId,
    source: &Path,
) -> Result<(), LocalModImportError> {
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| safe_component(name))
        .ok_or(LocalModImportError::InvalidArchive)?;
    if paths
        .instance(instance_id)
        .join("game")
        .join("mods")
        .join(file_name)
        .try_exists()?
    {
        return Err(LocalModImportError::AlreadyInstalled);
    }
    Ok(())
}

fn validate_mod_archive(path: &Path, loader: LoaderFamily) -> Result<(), LocalModImportError> {
    let file = File::open(path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|_| LocalModImportError::InvalidArchive)?;
    let has_loader_descriptor = match loader {
        LoaderFamily::Fabric => archive.by_name("fabric.mod.json").is_ok(),
        LoaderFamily::NeoForge => {
            archive.by_name("META-INF/neoforge.mods.toml").is_ok()
                || archive.by_name("META-INF/mods.toml").is_ok()
        }
        LoaderFamily::Vanilla => false,
    };
    if !has_loader_descriptor {
        return Err(LocalModImportError::WrongLoader);
    }
    Ok(())
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

pub(crate) fn scan_instance_content(
    paths: &AppPaths,
    instance_id: InstanceId,
    kind: InstanceContentKind,
) -> Result<Vec<InstanceContentFile>, io::Error> {
    let game = paths.instance(instance_id).join("game");
    let mut roots = Vec::new();
    match kind {
        InstanceContentKind::Resource => {
            roots.push((game.join("resourcepacks"), "resourcepacks".to_owned(), None));
        }
        InstanceContentKind::Shader => {
            roots.push((game.join("shaderpacks"), "shaderpacks".to_owned(), None));
        }
        InstanceContentKind::Data => {
            let saves = game.join("saves");
            let worlds = match std::fs::read_dir(saves) {
                Ok(worlds) => worlds,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
                Err(error) => return Err(error),
            };
            for world in worlds {
                let world = world?;
                let metadata = std::fs::symlink_metadata(world.path())?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    continue;
                }
                let world_name = world.file_name().to_string_lossy().into_owned();
                if !safe_component(&world_name) {
                    continue;
                }
                roots.push((
                    world.path().join("datapacks"),
                    format!("saves/{world_name}/datapacks"),
                    Some(world_name),
                ));
            }
        }
    }
    let mut files = Vec::new();
    for (root, prefix, world_name) in roots {
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let metadata = std::fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if !safe_component(&file_name) {
                continue;
            }
            let lower_name = file_name.to_ascii_lowercase();
            let is_archive = lower_name.ends_with(".zip") || lower_name.ends_with(".zip.disabled");
            let is_pack_directory = metadata.is_dir() && entry.path().join("pack.mcmeta").is_file();
            if !(metadata.is_file() && is_archive || is_pack_directory) {
                continue;
            }
            let enabled = !lower_name.ends_with(".disabled");
            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|modified| OffsetDateTime::from(modified).format(&Rfc3339).ok());
            files.push(InstanceContentFile {
                display_name: display_name_from_pack(&file_name),
                file_path: format!("{prefix}/{file_name}"),
                enabled,
                can_toggle: metadata.is_file(),
                size: if metadata.is_file() {
                    metadata.len()
                } else {
                    0
                },
                modified_at,
                world_name: world_name.clone(),
            });
        }
    }
    files.sort_by(|left, right| {
        left.world_name
            .cmp(&right.world_name)
            .then_with(|| {
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase())
            })
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
    Ok(files)
}

pub(crate) fn set_instance_content_enabled(
    paths: &AppPaths,
    instance_id: InstanceId,
    kind: InstanceContentKind,
    file_path: &str,
    enabled: bool,
) -> Result<FileMove, io::Error> {
    let (source, file_name) = instance_content_path(paths, instance_id, kind, file_path)?;
    let metadata = std::fs::symlink_metadata(&source)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_mod_path());
    }
    let lower_name = file_name.to_ascii_lowercase();
    let target_name = if enabled {
        if !lower_name.ends_with(".zip.disabled") {
            return Err(invalid_mod_path());
        }
        file_name[..file_name.len() - ".disabled".len()].to_owned()
    } else {
        if !lower_name.ends_with(".zip") {
            return Err(invalid_mod_path());
        }
        format!("{file_name}.disabled")
    };
    let target = source.with_file_name(&target_name);
    ensure_move_source(&source, &target)?;
    std::fs::rename(&source, &target)?;
    let prefix = file_path.rsplit_once('/').map_or("", |(prefix, _)| prefix);
    Ok(FileMove {
        from: source,
        to: target,
        updated_file_path: Some(format!("{prefix}/{target_name}")),
    })
}

pub(crate) fn trash_instance_content(
    paths: &AppPaths,
    instance_id: InstanceId,
    kind: InstanceContentKind,
    file_path: &str,
) -> Result<FileMove, io::Error> {
    let (source, file_name) = instance_content_path(paths, instance_id, kind, file_path)?;
    let metadata = std::fs::symlink_metadata(&source)?;
    if metadata.file_type().is_symlink() || !(metadata.is_file() || metadata.is_dir()) {
        return Err(invalid_mod_path());
    }
    let trash_root = paths.trash();
    let content_trash = trash_root.join("instance-content");
    let instance_trash = content_trash.join(instance_id.to_string());
    for directory in [&trash_root, &content_trash, &instance_trash] {
        ensure_plain_directory(directory)?;
    }
    let target = instance_trash.join(format!("{}-{file_name}", uuid::Uuid::new_v4()));
    if target.exists() {
        return Err(invalid_mod_path());
    }
    std::fs::rename(&source, &target)?;
    Ok(FileMove {
        from: source,
        to: target,
        updated_file_path: None,
    })
}

fn instance_content_path(
    paths: &AppPaths,
    instance_id: InstanceId,
    kind: InstanceContentKind,
    file_path: &str,
) -> Result<(PathBuf, String), io::Error> {
    let components = file_path.split('/').collect::<Vec<_>>();
    let valid_shape = match kind {
        InstanceContentKind::Resource => {
            components.len() == 2 && components.first() == Some(&"resourcepacks")
        }
        InstanceContentKind::Shader => {
            components.len() == 2 && components.first() == Some(&"shaderpacks")
        }
        InstanceContentKind::Data => {
            components.len() == 4
                && components.first() == Some(&"saves")
                && components.get(2) == Some(&"datapacks")
        }
    };
    if !valid_shape
        || components
            .iter()
            .any(|component| !safe_component(component))
    {
        return Err(invalid_mod_path());
    }
    let file_name = components.last().ok_or_else(invalid_mod_path)?.to_string();
    let instance_root = paths.instance(instance_id);
    let game = instance_root.join("game");
    let source = components
        .iter()
        .fold(game.clone(), |path, component| path.join(component));
    let parent = source.parent().ok_or_else(invalid_mod_path)?;
    for directory in [&instance_root, &game, parent] {
        let metadata = std::fs::symlink_metadata(directory)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(invalid_mod_path());
        }
    }
    Ok((source, file_name))
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value.contains(['/', '\\'])
        && !value.chars().any(char::is_control)
        && !value.contains(':')
}

fn display_name_from_pack(file_name: &str) -> String {
    let without_disabled = file_name.strip_suffix(".disabled").unwrap_or(file_name);
    let without_extension = without_disabled
        .strip_suffix(".zip")
        .unwrap_or(without_disabled);
    without_extension.replace(['_', '-'], " ")
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

fn ensure_existing_plain_directory(path: &Path) -> Result<(), io::Error> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_mod_path());
    }
    Ok(())
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
    use super::{
        InstanceContentKind, LocalModImportError, import_local_mod, scan_instance_content,
        scan_instance_mods, set_instance_content_enabled, set_instance_mod_enabled,
        trash_instance_content, trash_instance_mod,
    };
    use slate_domain::{InstanceId, LoaderFamily};
    use slate_platform::AppPaths;
    use std::io::Write;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn write_mod_jar(
        path: &std::path::Path,
        descriptor: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output = std::fs::File::create(path)?;
        let mut archive = ZipWriter::new(output);
        archive.start_file(descriptor, SimpleFileOptions::default())?;
        archive.write_all(b"{}")?;
        archive.finish()?;
        Ok(())
    }

    #[test]
    fn imports_a_compatible_local_mod_without_moving_the_source()
    -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app-data"),
            temporary.path().join("storage"),
        );
        paths.ensure_base_directories()?;
        let instance_id = InstanceId::new();
        std::fs::create_dir_all(paths.instance(instance_id).join("game"))?;
        let source = temporary.path().join("sodium.jar");
        write_mod_jar(&source, "fabric.mod.json")?;

        let imported = import_local_mod(&paths, instance_id, LoaderFamily::Fabric, &source)?;

        assert_eq!(imported.file_path, "mods/sodium.jar");
        assert!(source.is_file());
        assert!(imported.destination.is_file());
        imported.rollback()?;
        assert!(!imported.destination.exists());
        Ok(())
    }

    #[test]
    fn rejects_wrong_loader_and_duplicate_local_mods() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app-data"),
            temporary.path().join("storage"),
        );
        paths.ensure_base_directories()?;
        let instance_id = InstanceId::new();
        std::fs::create_dir_all(paths.instance(instance_id).join("game"))?;
        let source = temporary.path().join("example.jar");
        write_mod_jar(&source, "fabric.mod.json")?;

        assert!(matches!(
            import_local_mod(&paths, instance_id, LoaderFamily::NeoForge, &source),
            Err(LocalModImportError::WrongLoader)
        ));
        import_local_mod(&paths, instance_id, LoaderFamily::Fabric, &source)?;
        assert!(matches!(
            import_local_mod(&paths, instance_id, LoaderFamily::Fabric, &source),
            Err(LocalModImportError::AlreadyInstalled)
        ));
        Ok(())
    }

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

    #[test]
    fn inventories_pack_files_and_world_datapacks() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app-data"),
            temporary.path().join("storage"),
        );
        let instance_id = InstanceId::new();
        let game = paths.instance(instance_id).join("game");
        std::fs::create_dir_all(game.join("resourcepacks/folder-pack"))?;
        std::fs::write(game.join("resourcepacks/folder-pack/pack.mcmeta"), b"{}")?;
        std::fs::write(game.join("resourcepacks/slate.zip"), b"zip")?;
        std::fs::write(game.join("resourcepacks/readme.txt"), b"ignored")?;
        std::fs::create_dir_all(game.join("shaderpacks"))?;
        std::fs::write(game.join("shaderpacks/complementary.zip.disabled"), b"zip")?;
        std::fs::create_dir_all(game.join("saves/Survival/datapacks"))?;
        std::fs::write(game.join("saves/Survival/datapacks/recipes.zip"), b"zip")?;

        let resource_packs =
            scan_instance_content(&paths, instance_id, InstanceContentKind::Resource)?;
        assert_eq!(resource_packs.len(), 2);
        assert!(resource_packs.iter().any(|pack| !pack.can_toggle));
        let shaders = scan_instance_content(&paths, instance_id, InstanceContentKind::Shader)?;
        assert_eq!(shaders.len(), 1);
        assert!(!shaders[0].enabled);
        let data_packs = scan_instance_content(&paths, instance_id, InstanceContentKind::Data)?;
        assert_eq!(data_packs.len(), 1);
        assert_eq!(data_packs[0].world_name.as_deref(), Some("Survival"));
        Ok(())
    }

    #[test]
    fn toggles_and_trashes_pack_archives_safely() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app-data"),
            temporary.path().join("storage"),
        );
        paths.ensure_base_directories()?;
        let instance_id = InstanceId::new();
        let packs = paths.instance(instance_id).join("game/resourcepacks");
        std::fs::create_dir_all(&packs)?;
        std::fs::write(packs.join("slate.zip"), b"zip")?;

        let disabled = set_instance_content_enabled(
            &paths,
            instance_id,
            InstanceContentKind::Resource,
            "resourcepacks/slate.zip",
            false,
        )?;
        assert!(packs.join("slate.zip.disabled").exists());
        disabled.rollback()?;
        let removed = trash_instance_content(
            &paths,
            instance_id,
            InstanceContentKind::Resource,
            "resourcepacks/slate.zip",
        )?;
        assert!(!packs.join("slate.zip").exists());
        removed.rollback()?;
        assert!(packs.join("slate.zip").exists());
        assert!(
            trash_instance_content(
                &paths,
                instance_id,
                InstanceContentKind::Resource,
                "resourcepacks/../outside.zip",
            )
            .is_err()
        );
        Ok(())
    }
}
