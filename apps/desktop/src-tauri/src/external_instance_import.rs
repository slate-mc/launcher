use serde_json::Value;
use slate_contracts::LoaderKindDto;
use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};

const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_IMPORT_ENTRIES: usize = 500_000;
const MAX_IMPORT_BYTES: u64 = 50 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExternalLauncher {
    Prism,
    CurseForge,
    AtLauncher,
}

impl ExternalLauncher {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Prism => "Prism Launcher or MultiMC",
            Self::CurseForge => "CurseForge",
            Self::AtLauncher => "ATLauncher",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExternalInstance {
    pub(super) launcher: ExternalLauncher,
    pub(super) name: String,
    pub(super) minecraft_version: String,
    pub(super) loader_kind: LoaderKindDto,
    pub(super) loader_version: Option<String>,
    pub(super) memory_mb: u32,
    pub(super) game_directory: PathBuf,
    pub(super) icon_path: Option<PathBuf>,
}

pub(super) fn inspect_external_instance(
    root: &Path,
) -> Result<ExternalInstance, ExternalImportError> {
    let metadata = std::fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ExternalImportError::InvalidInstance);
    }
    if root.join("instance.cfg").is_file() && root.join("mmc-pack.json").is_file() {
        return inspect_prism_instance(root);
    }
    if root.join("minecraftinstance.json").is_file() {
        return inspect_curseforge_instance(root);
    }
    if root.join("instance.json").is_file() {
        return inspect_atlauncher_instance(root);
    }
    Err(ExternalImportError::InvalidInstance)
}

fn inspect_prism_instance(root: &Path) -> Result<ExternalInstance, ExternalImportError> {
    let config = read_text(&root.join("instance.cfg"))?;
    let pack = read_json(&root.join("mmc-pack.json"))?;
    let components = pack
        .get("components")
        .and_then(Value::as_array)
        .ok_or(ExternalImportError::InvalidInstance)?;
    let minecraft_version = component_version(components, "net.minecraft")
        .ok_or(ExternalImportError::InvalidInstance)?;
    let loader = [
        ("net.fabricmc.fabric-loader", LoaderKindDto::Fabric),
        ("net.neoforged", LoaderKindDto::NeoForge),
    ]
    .into_iter()
    .filter_map(|(uid, kind)| component_version(components, uid).map(|version| (kind, version)))
    .collect::<Vec<_>>();
    if components.iter().any(|component| {
        component
            .get("uid")
            .and_then(Value::as_str)
            .is_some_and(|uid| matches!(uid, "net.minecraftforge" | "org.quiltmc.quilt-loader"))
    }) {
        return Err(ExternalImportError::UnsupportedLoader);
    }
    let (loader_kind, loader_version) = match loader.as_slice() {
        [] => (LoaderKindDto::Vanilla, None),
        [(kind, version)] => (*kind, Some(version.clone())),
        _ => return Err(ExternalImportError::InvalidInstance),
    };
    let game_directory = [root.join("minecraft"), root.join(".minecraft")]
        .into_iter()
        .find(|directory| directory.is_dir())
        .ok_or(ExternalImportError::InvalidInstance)?;
    let name = ini_value(&config, "name")
        .or_else(|| path_name(root))
        .ok_or(ExternalImportError::InvalidInstance)?;
    let memory_mb = ini_value(&config, "MaxMemAlloc")
        .and_then(|value| value.parse::<u32>().ok())
        .map_or(8_192, bounded_memory);
    let icon_path = ini_value(&config, "iconKey")
        .filter(|value| safe_component(value))
        .map(|value| root.join(format!("{value}.png")))
        .filter(|path| path.is_file())
        .or_else(|| {
            root.join("icon.png")
                .is_file()
                .then(|| root.join("icon.png"))
        });
    Ok(ExternalInstance {
        launcher: ExternalLauncher::Prism,
        name,
        minecraft_version,
        loader_kind,
        loader_version,
        memory_mb,
        game_directory,
        icon_path,
    })
}

fn inspect_curseforge_instance(root: &Path) -> Result<ExternalInstance, ExternalImportError> {
    let manifest = read_json(&root.join("minecraftinstance.json"))?;
    let name = string_field(&manifest, "name")
        .or_else(|| path_name(root))
        .ok_or(ExternalImportError::InvalidInstance)?;
    let minecraft_version =
        string_field(&manifest, "gameVersion").ok_or(ExternalImportError::InvalidInstance)?;
    let loader = manifest.get("baseModLoader");
    let (loader_kind, loader_version) = match loader {
        None | Some(Value::Null) => (LoaderKindDto::Vanilla, None),
        Some(loader) => {
            let name = string_field(loader, "name").ok_or(ExternalImportError::InvalidInstance)?;
            let lower = name.to_ascii_lowercase();
            let kind = if lower.starts_with("fabric-") || lower.starts_with("fabricloader-") {
                LoaderKindDto::Fabric
            } else if lower.starts_with("neoforge-") {
                LoaderKindDto::NeoForge
            } else {
                return Err(ExternalImportError::UnsupportedLoader);
            };
            let version = string_field(loader, "forgeVersion")
                .or_else(|| name.split_once('-').map(|(_, version)| version.to_owned()))
                .ok_or(ExternalImportError::InvalidInstance)?;
            (kind, Some(version))
        }
    };
    let memory_mb = manifest
        .get("allocatedMemory")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .map_or(8_192, bounded_memory);
    Ok(ExternalInstance {
        launcher: ExternalLauncher::CurseForge,
        name,
        minecraft_version,
        loader_kind,
        loader_version,
        memory_mb,
        game_directory: root.to_path_buf(),
        icon_path: None,
    })
}

fn inspect_atlauncher_instance(root: &Path) -> Result<ExternalInstance, ExternalImportError> {
    let manifest = read_json(&root.join("instance.json"))?;
    let launcher = manifest
        .get("launcher")
        .and_then(Value::as_object)
        .ok_or(ExternalImportError::InvalidInstance)?;
    let name = launcher
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| path_name(root))
        .ok_or(ExternalImportError::InvalidInstance)?;
    let minecraft_version =
        string_field(&manifest, "id").ok_or(ExternalImportError::InvalidInstance)?;
    let (loader_kind, loader_version) = match launcher.get("loaderVersion") {
        None | Some(Value::Null) => (LoaderKindDto::Vanilla, None),
        Some(loader) => {
            let kind = match string_field(loader, "type")
                .ok_or(ExternalImportError::InvalidInstance)?
                .to_ascii_lowercase()
                .as_str()
            {
                "fabric" => LoaderKindDto::Fabric,
                "neoforge" => LoaderKindDto::NeoForge,
                _ => return Err(ExternalImportError::UnsupportedLoader),
            };
            let version =
                string_field(loader, "version").ok_or(ExternalImportError::InvalidInstance)?;
            (kind, Some(version))
        }
    };
    let memory_mb = launcher
        .get("maximumMemory")
        .or_else(|| launcher.get("requiredMemory"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .map_or(8_192, bounded_memory);
    let icon_path = root
        .join("instance.png")
        .is_file()
        .then(|| root.join("instance.png"));
    Ok(ExternalInstance {
        launcher: ExternalLauncher::AtLauncher,
        name,
        minecraft_version,
        loader_kind,
        loader_version,
        memory_mb,
        game_directory: root.to_path_buf(),
        icon_path,
    })
}

pub(super) fn copy_external_game_directory(
    source: &Path,
    destination: &Path,
) -> Result<u64, ExternalImportError> {
    let source = std::fs::canonicalize(source)?;
    let metadata = std::fs::symlink_metadata(&source)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ExternalImportError::InvalidInstance);
    }
    std::fs::create_dir_all(destination)?;
    let destination = std::fs::canonicalize(destination)?;
    if source == destination || source.starts_with(&destination) || destination.starts_with(&source)
    {
        return Err(ExternalImportError::UnsafePath);
    }
    let mut queue = VecDeque::from([(source.clone(), destination)]);
    let mut entries = 0_usize;
    let mut bytes = 0_u64;
    while let Some((current_source, current_destination)) = queue.pop_front() {
        for entry in std::fs::read_dir(&current_source)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path
                .strip_prefix(&source)
                .map_err(|_| ExternalImportError::UnsafePath)?;
            validate_relative(relative)?;
            if skipped_root_entry(relative) {
                continue;
            }
            entries = entries.saturating_add(1);
            if entries > MAX_IMPORT_ENTRIES {
                return Err(ExternalImportError::TooLarge);
            }
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(ExternalImportError::UnsafePath);
            }
            let target = current_destination.join(entry.file_name());
            if metadata.is_dir() {
                std::fs::create_dir_all(&target)?;
                queue.push_back((path, target));
            } else if metadata.is_file() {
                bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or(ExternalImportError::TooLarge)?;
                if bytes > MAX_IMPORT_BYTES {
                    return Err(ExternalImportError::TooLarge);
                }
                std::fs::copy(path, target)?;
            }
        }
    }
    Ok(bytes)
}

fn component_version(components: &[Value], uid: &str) -> Option<String> {
    components.iter().find_map(|component| {
        (component.get("uid").and_then(Value::as_str) == Some(uid))
            .then(|| {
                component
                    .get("version")
                    .or_else(|| component.get("cachedVersion"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            })
            .flatten()
    })
}

fn read_json(path: &Path) -> Result<Value, ExternalImportError> {
    let bytes = read_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(ExternalImportError::Json)
}

fn read_text(path: &Path) -> Result<String, ExternalImportError> {
    String::from_utf8(read_bytes(path)?).map_err(|_| ExternalImportError::InvalidInstance)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, ExternalImportError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_METADATA_BYTES
    {
        return Err(ExternalImportError::InvalidInstance);
    }
    std::fs::read(path).map_err(ExternalImportError::Io)
}

fn ini_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let (candidate, value) = line.split_once('=')?;
        candidate
            .trim()
            .eq_ignore_ascii_case(key)
            .then(|| value.trim().trim_matches('"').to_owned())
            .filter(|value| !value.is_empty())
    })
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn path_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn bounded_memory(value: u32) -> u32 {
    value.clamp(1_024, 32_768)
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value.contains(['/', '\\', ':'])
        && !value.chars().any(char::is_control)
}

fn validate_relative(path: &Path) -> Result<(), ExternalImportError> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            !matches!(component, Component::Normal(_))
                || component.as_os_str().to_string_lossy().contains(':')
        })
    {
        return Err(ExternalImportError::UnsafePath);
    }
    Ok(())
}

fn skipped_root_entry(path: &Path) -> bool {
    if path.components().count() != 1 {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        "instance.cfg"
            | "mmc-pack.json"
            | "manifest.json"
            | "modrinth.index.json"
            | "minecraftinstance.json"
            | "instance.json"
            | "instance.png"
            | ".curseclient"
            | "assets"
            | "libraries"
            | "versions"
            | "natives"
            | "runtime"
            | "runtimes"
            | "bin"
            | "jarmods"
            | "logs"
            | "crash-reports"
    )
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ExternalImportError {
    #[error("the selected folder is not a supported launcher instance")]
    InvalidInstance,
    #[error("the selected instance uses an unsupported loader")]
    UnsupportedLoader,
    #[error("the selected instance contains an unsafe path")]
    UnsafePath,
    #[error("the selected instance is too large")]
    TooLarge,
    #[error("the selected instance metadata is invalid")]
    Json(#[from] serde_json::Error),
    #[error("the selected instance could not be read")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::{
        ExternalImportError, ExternalLauncher, copy_external_game_directory,
        inspect_external_instance,
    };
    use slate_contracts::LoaderKindDto;

    #[test]
    fn reads_prism_neoforge_instances_and_copies_only_game_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("prism-instance");
        std::fs::create_dir_all(source.join("minecraft/mods"))?;
        std::fs::write(
            source.join("instance.cfg"),
            b"name=Imported pack\nMaxMemAlloc=6144\n",
        )?;
        std::fs::write(
            source.join("mmc-pack.json"),
            br#"{"formatVersion":1,"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.neoforged","version":"21.1.172"}]}"#,
        )?;
        std::fs::write(source.join("minecraft/mods/example.jar"), b"mod")?;

        let instance = inspect_external_instance(&source)?;
        assert_eq!(instance.launcher, ExternalLauncher::Prism);
        assert_eq!(instance.loader_kind, LoaderKindDto::NeoForge);
        assert_eq!(instance.loader_version.as_deref(), Some("21.1.172"));
        assert_eq!(instance.memory_mb, 6_144);

        let destination = temporary.path().join("destination");
        copy_external_game_directory(&instance.game_directory, &destination)?;
        assert_eq!(std::fs::read(destination.join("mods/example.jar"))?, b"mod");
        Ok(())
    }

    #[test]
    fn reads_curseforge_and_atlauncher_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let curseforge = temporary.path().join("curseforge");
        std::fs::create_dir_all(&curseforge)?;
        std::fs::write(
            curseforge.join("minecraftinstance.json"),
            br#"{"name":"Curse pack","gameVersion":"1.21.1","allocatedMemory":7000,"baseModLoader":{"name":"neoforge-21.1.172","forgeVersion":"21.1.172"}}"#,
        )?;
        let curseforge = inspect_external_instance(&curseforge)?;
        assert_eq!(curseforge.launcher, ExternalLauncher::CurseForge);
        assert_eq!(curseforge.loader_kind, LoaderKindDto::NeoForge);

        let atlauncher = temporary.path().join("atlauncher");
        std::fs::create_dir_all(&atlauncher)?;
        std::fs::write(
            atlauncher.join("instance.json"),
            br#"{"id":"1.20.1","launcher":{"name":"AT pack","maximumMemory":5000,"loaderVersion":{"type":"Fabric","version":"0.16.10"}}}"#,
        )?;
        std::fs::create_dir_all(atlauncher.join("mods"))?;
        std::fs::write(atlauncher.join("mods/example.jar"), b"mod")?;
        let atlauncher = inspect_external_instance(&atlauncher)?;
        assert_eq!(atlauncher.launcher, ExternalLauncher::AtLauncher);
        assert_eq!(atlauncher.loader_kind, LoaderKindDto::Fabric);
        assert_eq!(atlauncher.memory_mb, 5_000);

        let destination = temporary.path().join("copied-atlauncher");
        copy_external_game_directory(&atlauncher.game_directory, &destination)?;
        assert_eq!(std::fs::read(destination.join("mods/example.jar"))?, b"mod");
        assert!(!destination.join("instance.json").exists());
        Ok(())
    }

    #[test]
    fn rejects_unsupported_forge_instances() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        std::fs::write(
            temporary.path().join("minecraftinstance.json"),
            br#"{"name":"Forge pack","gameVersion":"1.20.1","baseModLoader":{"name":"forge-47.3.0","forgeVersion":"47.3.0"}}"#,
        )?;
        assert!(matches!(
            inspect_external_instance(temporary.path()),
            Err(ExternalImportError::UnsupportedLoader)
        ));
        Ok(())
    }
}
