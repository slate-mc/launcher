use crate::instance_content::InstanceContentKind;
use slate_domain::{InstanceId, LoaderFamily};
use slate_platform::AppPaths;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const MAXIMUM_LOCAL_CONTENT_SIZE: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocalContentImportKind {
    Mod(LoaderFamily),
    Pack(InstanceContentKind),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum LocalContentImportError {
    #[error("that content file is already installed")]
    AlreadyInstalled,
    #[error("the selected file is not a valid content archive")]
    InvalidArchive,
    #[error("the selected mod does not support this instance loader")]
    WrongLoader,
    #[error("the selected content file is too large")]
    TooLarge,
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportedLocalFile {
    pub destination: PathBuf,
    pub file_path: String,
}

impl ImportedLocalFile {
    pub fn rollback(&self) -> Result<(), io::Error> {
        match std::fs::remove_file(&self.destination) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

pub(crate) fn validate_local_content_source(
    source: &Path,
    kind: LocalContentImportKind,
) -> Result<(), LocalContentImportError> {
    let metadata = std::fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(LocalContentImportError::InvalidArchive);
    }
    if metadata.len() > MAXIMUM_LOCAL_CONTENT_SIZE {
        return Err(LocalContentImportError::TooLarge);
    }
    let expected_extension = match kind {
        LocalContentImportKind::Mod(_) => ".jar",
        LocalContentImportKind::Pack(_) => ".zip",
    };
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| {
            safe_component(name) && name.to_ascii_lowercase().ends_with(expected_extension)
        })
        .ok_or(LocalContentImportError::InvalidArchive)?;
    if file_name.len() > 240 {
        return Err(LocalContentImportError::InvalidArchive);
    }
    validate_archive(source, kind)
}

pub(crate) fn ensure_local_content_not_installed(
    paths: &AppPaths,
    instance_id: InstanceId,
    source: &Path,
    kind: LocalContentImportKind,
) -> Result<(), LocalContentImportError> {
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| safe_component(name))
        .ok_or(LocalContentImportError::InvalidArchive)?;
    let (directory, _) = destination(kind)?;
    if paths
        .instance(instance_id)
        .join("game")
        .join(directory)
        .join(file_name)
        .try_exists()?
    {
        return Err(LocalContentImportError::AlreadyInstalled);
    }
    Ok(())
}

pub(crate) fn import_local_content(
    paths: &AppPaths,
    instance_id: InstanceId,
    source: &Path,
    kind: LocalContentImportKind,
) -> Result<ImportedLocalFile, LocalContentImportError> {
    validate_local_content_source(source, kind)?;
    ensure_local_content_not_installed(paths, instance_id, source, kind)?;
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(LocalContentImportError::InvalidArchive)?;
    let instance_root = paths.instance(instance_id);
    let game_directory = instance_root.join("game");
    ensure_existing_plain_directory(&instance_root)?;
    ensure_existing_plain_directory(&game_directory)?;
    let (directory, prefix) = destination(kind)?;
    let content_directory = game_directory.join(directory);
    ensure_plain_directory(&content_directory)?;
    let destination = content_directory.join(file_name);

    let mut input = File::open(source)?;
    let mut pending = NamedTempFile::new_in(&content_directory)?;
    let copied = io::copy(
        &mut std::io::Read::take(&mut input, MAXIMUM_LOCAL_CONTENT_SIZE + 1),
        &mut pending,
    )?;
    if copied == 0 {
        return Err(LocalContentImportError::InvalidArchive);
    }
    if copied > MAXIMUM_LOCAL_CONTENT_SIZE {
        return Err(LocalContentImportError::TooLarge);
    }
    pending.as_file_mut().sync_all()?;
    validate_archive(pending.path(), kind)?;
    let persisted = pending.persist_noclobber(&destination).map_err(|error| {
        if error.error.kind() == io::ErrorKind::AlreadyExists {
            LocalContentImportError::AlreadyInstalled
        } else {
            LocalContentImportError::Io(error.error)
        }
    })?;
    persisted.sync_all()?;
    Ok(ImportedLocalFile {
        destination,
        file_path: format!("{prefix}/{file_name}"),
    })
}

fn destination(
    kind: LocalContentImportKind,
) -> Result<(&'static str, &'static str), LocalContentImportError> {
    match kind {
        LocalContentImportKind::Mod(_) => Ok(("mods", "mods")),
        LocalContentImportKind::Pack(InstanceContentKind::Resource) => {
            Ok(("resourcepacks", "resourcepacks"))
        }
        LocalContentImportKind::Pack(InstanceContentKind::Shader) => {
            Ok(("shaderpacks", "shaderpacks"))
        }
        LocalContentImportKind::Pack(InstanceContentKind::Data) => {
            Err(LocalContentImportError::InvalidArchive)
        }
    }
}

fn validate_archive(
    path: &Path,
    kind: LocalContentImportKind,
) -> Result<(), LocalContentImportError> {
    let file = File::open(path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|_| LocalContentImportError::InvalidArchive)?;
    let valid = match kind {
        LocalContentImportKind::Mod(LoaderFamily::Fabric) => {
            archive.by_name("fabric.mod.json").is_ok()
        }
        LocalContentImportKind::Mod(LoaderFamily::NeoForge) => {
            archive.by_name("META-INF/neoforge.mods.toml").is_ok()
                || archive.by_name("META-INF/mods.toml").is_ok()
        }
        LocalContentImportKind::Mod(LoaderFamily::Vanilla) => false,
        LocalContentImportKind::Pack(InstanceContentKind::Resource) => {
            archive.by_name("pack.mcmeta").is_ok()
        }
        LocalContentImportKind::Pack(InstanceContentKind::Shader) => archive
            .file_names()
            .any(|name| name == "shaders/" || name.starts_with("shaders/")),
        LocalContentImportKind::Pack(InstanceContentKind::Data) => false,
    };
    if valid {
        Ok(())
    } else if matches!(kind, LocalContentImportKind::Mod(_)) {
        Err(LocalContentImportError::WrongLoader)
    } else {
        Err(LocalContentImportError::InvalidArchive)
    }
}

fn ensure_plain_directory(path: &Path) -> Result<(), io::Error> {
    match std::fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            ensure_existing_plain_directory(path)
        }
        Err(error) => Err(error),
    }
}

fn ensure_existing_plain_directory(path: &Path) -> Result<(), io::Error> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid instance content directory",
        ));
    }
    Ok(())
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value.contains(['/', '\\'])
        && !value.chars().any(char::is_control)
        && !value.contains(':')
}

#[cfg(test)]
mod tests {
    use super::{LocalContentImportError, LocalContentImportKind, import_local_content};
    use crate::instance_content::InstanceContentKind;
    use slate_domain::{InstanceId, LoaderFamily};
    use slate_platform::AppPaths;
    use std::io::Write;
    use std::path::Path;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn write_archive(path: &Path, descriptor: &str) -> Result<(), Box<dyn std::error::Error>> {
        let output = std::fs::File::create(path)?;
        let mut archive = ZipWriter::new(output);
        archive.start_file(descriptor, SimpleFileOptions::default())?;
        archive.write_all(b"{}")?;
        archive.finish()?;
        Ok(())
    }

    fn test_paths() -> Result<(tempfile::TempDir, AppPaths, InstanceId), Box<dyn std::error::Error>>
    {
        let temporary = tempfile::tempdir()?;
        let paths = AppPaths::from_roots(
            temporary.path().join("app-data"),
            temporary.path().join("storage"),
        );
        paths.ensure_base_directories()?;
        let instance_id = InstanceId::new();
        std::fs::create_dir_all(paths.instance(instance_id).join("game"))?;
        Ok((temporary, paths, instance_id))
    }

    #[test]
    fn imports_a_compatible_mod_without_moving_the_source() -> Result<(), Box<dyn std::error::Error>>
    {
        let (temporary, paths, instance_id) = test_paths()?;
        let source = temporary.path().join("sodium.jar");
        write_archive(&source, "fabric.mod.json")?;

        let imported = import_local_content(
            &paths,
            instance_id,
            &source,
            LocalContentImportKind::Mod(LoaderFamily::Fabric),
        )?;

        assert_eq!(imported.file_path, "mods/sodium.jar");
        assert!(source.is_file());
        assert!(imported.destination.is_file());
        imported.rollback()?;
        assert!(!imported.destination.exists());
        Ok(())
    }

    #[test]
    fn rejects_wrong_loader_and_duplicate_mods() -> Result<(), Box<dyn std::error::Error>> {
        let (temporary, paths, instance_id) = test_paths()?;
        let source = temporary.path().join("example.jar");
        write_archive(&source, "fabric.mod.json")?;

        assert!(matches!(
            import_local_content(
                &paths,
                instance_id,
                &source,
                LocalContentImportKind::Mod(LoaderFamily::NeoForge),
            ),
            Err(LocalContentImportError::WrongLoader)
        ));
        import_local_content(
            &paths,
            instance_id,
            &source,
            LocalContentImportKind::Mod(LoaderFamily::Fabric),
        )?;
        assert!(matches!(
            import_local_content(
                &paths,
                instance_id,
                &source,
                LocalContentImportKind::Mod(LoaderFamily::Fabric),
            ),
            Err(LocalContentImportError::AlreadyInstalled)
        ));
        Ok(())
    }

    #[test]
    fn imports_resource_and_shader_pack_archives() -> Result<(), Box<dyn std::error::Error>> {
        let (temporary, paths, instance_id) = test_paths()?;
        let resource = temporary.path().join("resources.zip");
        let shader = temporary.path().join("shaders.zip");
        write_archive(&resource, "pack.mcmeta")?;
        write_archive(&shader, "shaders/program.fsh")?;

        let resource = import_local_content(
            &paths,
            instance_id,
            &resource,
            LocalContentImportKind::Pack(InstanceContentKind::Resource),
        )?;
        let shader = import_local_content(
            &paths,
            instance_id,
            &shader,
            LocalContentImportKind::Pack(InstanceContentKind::Shader),
        )?;

        assert_eq!(resource.file_path, "resourcepacks/resources.zip");
        assert_eq!(shader.file_path, "shaderpacks/shaders.zip");
        Ok(())
    }
}
