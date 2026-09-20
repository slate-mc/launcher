use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_OPTIONS_BYTES: usize = 1024 * 1024;

pub struct PendingGameOptionsWrite {
    destination: PathBuf,
    backup: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourcePackOrderChange {
    SetActive(bool),
    MoveHigher,
    MoveLower,
}

impl PendingGameOptionsWrite {
    pub async fn rollback(self) {
        let _ = tokio::fs::remove_file(&self.destination).await;
        if let Some(backup) = self.backup {
            let _ = tokio::fs::rename(backup, self.destination).await;
        }
    }

    pub async fn commit(self) {
        if let Some(backup) = self.backup {
            let _ = tokio::fs::remove_file(backup).await;
        }
    }
}

pub async fn read_recognized_options(
    path: &Path,
) -> Result<(bool, BTreeMap<String, String>), GameOptionsError> {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((false, BTreeMap::new()));
        }
        Err(error) => return Err(error.into()),
    };
    if bytes.len() > MAX_OPTIONS_BYTES {
        return Err(GameOptionsError::TooLarge);
    }
    let contents = String::from_utf8(bytes).map_err(|_| GameOptionsError::InvalidEncoding)?;
    let mut values = BTreeMap::new();
    for line in contents.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if option_rule(key).is_some() {
            values.insert(key.to_owned(), value.to_owned());
        }
    }
    Ok((true, values))
}

pub async fn prepare_options_update(
    path: PathBuf,
    values: &BTreeMap<String, String>,
) -> Result<PendingGameOptionsWrite, GameOptionsError> {
    validate_options(values)?;
    let existing = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    if existing.len() > MAX_OPTIONS_BYTES {
        return Err(GameOptionsError::TooLarge);
    }
    let contents = String::from_utf8(existing).map_err(|_| GameOptionsError::InvalidEncoding)?;
    let mut written = HashSet::new();
    let mut lines = contents
        .lines()
        .map(|line| {
            let Some((key, _)) = line.split_once(':') else {
                return line.to_owned();
            };
            if let Some(value) = values.get(key) {
                written.insert(key.to_owned());
                format!("{key}:{value}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>();
    for (key, value) in values {
        if !written.contains(key) {
            lines.push(format!("{key}:{value}"));
        }
    }
    let bytes = format!("{}\n", lines.join("\n")).into_bytes();
    if bytes.len() > MAX_OPTIONS_BYTES {
        return Err(GameOptionsError::TooLarge);
    }
    write_pending(path, bytes).await
}

pub async fn read_resource_packs(path: &Path) -> Result<Vec<String>, GameOptionsError> {
    let contents = read_options_contents(path).await?;
    let Some(value) = option_value(&contents, "resourcePacks") else {
        return Ok(Vec::new());
    };
    parse_resource_packs(value)
}

pub async fn prepare_resource_pack_update(
    path: PathBuf,
    file_name: &str,
    change: ResourcePackOrderChange,
) -> Result<PendingGameOptionsWrite, GameOptionsError> {
    validate_resource_pack_file_name(file_name)?;
    let existing = read_options_contents(&path).await?;
    let mut packs = option_value(&existing, "resourcePacks")
        .map(parse_resource_packs)
        .transpose()?
        .unwrap_or_default();
    let identity = format!("file/{file_name}");
    let current = packs.iter().position(|entry| entry == &identity);
    match change {
        ResourcePackOrderChange::SetActive(true) => {
            if current.is_none() {
                if packs.is_empty() {
                    packs.push("vanilla".to_owned());
                }
                packs.push(identity);
            }
        }
        ResourcePackOrderChange::SetActive(false) => {
            packs.retain(|entry| entry != &identity);
        }
        ResourcePackOrderChange::MoveHigher => {
            move_resource_pack(&mut packs, current, true)?;
        }
        ResourcePackOrderChange::MoveLower => {
            move_resource_pack(&mut packs, current, false)?;
        }
    }
    let value = serde_json::to_string(&packs).map_err(|_| GameOptionsError::InvalidPackList)?;
    let bytes = replace_option_value(&existing, "resourcePacks", &value)?;
    write_pending(path, bytes).await
}

async fn read_options_contents(path: &Path) -> Result<String, GameOptionsError> {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    if bytes.len() > MAX_OPTIONS_BYTES {
        return Err(GameOptionsError::TooLarge);
    }
    String::from_utf8(bytes).map_err(|_| GameOptionsError::InvalidEncoding)
}

fn option_value<'a>(contents: &'a str, requested: &str) -> Option<&'a str> {
    contents.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key == requested).then_some(value)
    })
}

fn parse_resource_packs(value: &str) -> Result<Vec<String>, GameOptionsError> {
    let packs = serde_json::from_str::<Vec<String>>(value)
        .map_err(|_| GameOptionsError::InvalidPackList)?;
    if packs.len() > 512
        || packs
            .iter()
            .any(|entry| entry.is_empty() || entry.len() > 512 || entry.contains(['\r', '\n']))
    {
        return Err(GameOptionsError::InvalidPackList);
    }
    Ok(packs)
}

fn replace_option_value(
    contents: &str,
    key: &str,
    value: &str,
) -> Result<Vec<u8>, GameOptionsError> {
    let mut replaced = false;
    let mut lines = contents
        .lines()
        .map(|line| {
            let Some((candidate, _)) = line.split_once(':') else {
                return line.to_owned();
            };
            if candidate == key {
                replaced = true;
                format!("{key}:{value}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>();
    if !replaced {
        lines.push(format!("{key}:{value}"));
    }
    let bytes = format!("{}\n", lines.join("\n")).into_bytes();
    if bytes.len() > MAX_OPTIONS_BYTES {
        return Err(GameOptionsError::TooLarge);
    }
    Ok(bytes)
}

fn validate_resource_pack_file_name(file_name: &str) -> Result<(), GameOptionsError> {
    if file_name.is_empty()
        || file_name.len() > 255
        || matches!(file_name, "." | "..")
        || file_name.contains(['/', '\\', '\r', '\n'])
    {
        return Err(GameOptionsError::InvalidPackFile);
    }
    Ok(())
}

fn move_resource_pack(
    packs: &mut [String],
    current: Option<usize>,
    higher: bool,
) -> Result<(), GameOptionsError> {
    let current = current.ok_or(GameOptionsError::InactivePack)?;
    let neighbor = if higher {
        ((current + 1)..packs.len()).find(|index| packs[*index].starts_with("file/"))
    } else {
        (0..current)
            .rev()
            .find(|index| packs[*index].starts_with("file/"))
    };
    let neighbor = neighbor.ok_or(GameOptionsError::NoPriorityChange)?;
    packs.swap(current, neighbor);
    Ok(())
}

fn validate_options(values: &BTreeMap<String, String>) -> Result<(), GameOptionsError> {
    for (key, value) in values {
        let rule = option_rule(key).ok_or_else(|| GameOptionsError::UnsupportedKey(key.clone()))?;
        if !rule.accepts(value) {
            return Err(GameOptionsError::InvalidValue(key.clone()));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum OptionRule {
    Boolean,
    Integer { minimum: i32, maximum: i32 },
    Decimal { minimum: f64, maximum: f64 },
    Language,
}

impl OptionRule {
    fn accepts(self, value: &str) -> bool {
        match self {
            Self::Boolean => matches!(value, "true" | "false"),
            Self::Integer { minimum, maximum } => value
                .parse::<i32>()
                .is_ok_and(|value| (minimum..=maximum).contains(&value)),
            Self::Decimal { minimum, maximum } => value
                .parse::<f64>()
                .is_ok_and(|value| value.is_finite() && value >= minimum && value <= maximum),
            Self::Language => {
                (2..=32).contains(&value.len())
                    && value
                        .chars()
                        .all(|character| character.is_ascii_lowercase() || character == '_')
            }
        }
    }
}

fn option_rule(key: &str) -> Option<OptionRule> {
    match key {
        "fullscreen"
        | "enableVsync"
        | "bobView"
        | "invertYMouse"
        | "autoJump"
        | "toggleCrouch"
        | "toggleSprint"
        | "chatLinks"
        | "chatLinksPrompt"
        | "realmsNotifications"
        | "allowServerListing"
        | "hideMatchedNames"
        | "showSubtitles"
        | "directionalAudio" => Some(OptionRule::Boolean),
        "graphicsMode" => Some(OptionRule::Integer {
            minimum: 0,
            maximum: 2,
        }),
        "renderDistance" => Some(OptionRule::Integer {
            minimum: 2,
            maximum: 64,
        }),
        "simulationDistance" => Some(OptionRule::Integer {
            minimum: 2,
            maximum: 32,
        }),
        "guiScale" => Some(OptionRule::Integer {
            minimum: 0,
            maximum: 8,
        }),
        "particles" | "chatVisibility" => Some(OptionRule::Integer {
            minimum: 0,
            maximum: 2,
        }),
        "mipmapLevels" | "narrator" => Some(OptionRule::Integer {
            minimum: 0,
            maximum: 4,
        }),
        "maxFps" => Some(OptionRule::Integer {
            minimum: 10,
            maximum: 260,
        }),
        "mouseSensitivity"
        | "chatOpacity"
        | "textBackgroundOpacity"
        | "darknessEffectScale"
        | "damageTiltStrength"
        | "soundCategory_master"
        | "soundCategory_music"
        | "soundCategory_record"
        | "soundCategory_weather"
        | "soundCategory_block"
        | "soundCategory_hostile"
        | "soundCategory_neutral"
        | "soundCategory_player"
        | "soundCategory_ambient"
        | "soundCategory_voice" => Some(OptionRule::Decimal {
            minimum: 0.0,
            maximum: 1.0,
        }),
        "entityDistanceScaling" => Some(OptionRule::Decimal {
            minimum: 0.5,
            maximum: 5.0,
        }),
        "lang" => Some(OptionRule::Language),
        _ => None,
    }
}

async fn write_pending(
    destination: PathBuf,
    bytes: Vec<u8>,
) -> Result<PendingGameOptionsWrite, GameOptionsError> {
    let parent = destination
        .parent()
        .ok_or(GameOptionsError::MissingParent)?;
    tokio::fs::create_dir_all(parent).await?;
    let temporary = destination.with_extension(format!("txt.tmp-{}", Uuid::new_v4()));
    let backup = destination.with_extension(format!("txt.bak-{}", Uuid::new_v4()));
    tokio::fs::write(&temporary, bytes).await?;
    let previous = if tokio::fs::try_exists(&destination).await? {
        if let Err(error) = tokio::fs::rename(&destination, &backup).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(error.into());
        }
        Some(backup)
    } else {
        None
    };
    if let Err(error) = tokio::fs::rename(&temporary, &destination).await {
        if let Some(backup) = previous.as_ref() {
            let _ = tokio::fs::rename(backup, &destination).await;
        }
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.into());
    }
    Ok(PendingGameOptionsWrite {
        destination,
        backup: previous,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum GameOptionsError {
    #[error("options.txt is too large")]
    TooLarge,
    #[error("options.txt is not UTF-8")]
    InvalidEncoding,
    #[error("resource pack list is invalid")]
    InvalidPackList,
    #[error("resource pack file name is invalid")]
    InvalidPackFile,
    #[error("resource pack is not active")]
    InactivePack,
    #[error("resource pack priority cannot move in that direction")]
    NoPriorityChange,
    #[error("unsupported game option {0}")]
    UnsupportedKey(String),
    #[error("invalid game option value for {0}")]
    InvalidValue(String),
    #[error("options.txt has no parent directory")]
    MissingParent,
    #[error("options.txt I/O failed")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::{
        ResourcePackOrderChange, prepare_options_update, prepare_resource_pack_update,
        read_recognized_options, read_resource_packs,
    };
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn updates_known_keys_without_discarding_unknown_mod_options()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("options.txt");
        tokio::fs::write(
            &path,
            "renderDistance:12\nmodded.option:keep-me\nfullscreen:false\n",
        )
        .await?;
        let pending = prepare_options_update(
            path.clone(),
            &BTreeMap::from([
                ("renderDistance".to_owned(), "24".to_owned()),
                ("enableVsync".to_owned(), "true".to_owned()),
            ]),
        )
        .await?;
        pending.commit().await;

        let contents = tokio::fs::read_to_string(&path).await?;
        assert!(contents.contains("renderDistance:24"));
        assert!(contents.contains("modded.option:keep-me"));
        assert!(contents.contains("fullscreen:false"));
        assert!(contents.contains("enableVsync:true"));
        let (exists, values) = read_recognized_options(&path).await?;
        assert!(exists);
        assert_eq!(values.get("renderDistance").map(String::as_str), Some("24"));
        Ok(())
    }

    #[tokio::test]
    async fn rejects_unknown_keys_from_the_renderer() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let result = prepare_options_update(
            directory.path().join("options.txt"),
            &BTreeMap::from([("javaArgs".to_owned(), "unsafe".to_owned())]),
        )
        .await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn activates_and_reorders_resource_packs_without_losing_unknown_options()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("options.txt");
        tokio::fs::write(
            &path,
            "resourcePacks:[\"vanilla\",\"mod_resources\",\"file/first.zip\"]\nmod.option:keep\n",
        )
        .await?;

        prepare_resource_pack_update(
            path.clone(),
            "second.zip",
            ResourcePackOrderChange::SetActive(true),
        )
        .await?
        .commit()
        .await;
        prepare_resource_pack_update(
            path.clone(),
            "first.zip",
            ResourcePackOrderChange::MoveHigher,
        )
        .await?
        .commit()
        .await;

        assert_eq!(
            read_resource_packs(&path).await?,
            vec![
                "vanilla",
                "mod_resources",
                "file/second.zip",
                "file/first.zip"
            ]
        );
        assert!(
            tokio::fs::read_to_string(&path)
                .await?
                .contains("mod.option:keep")
        );
        Ok(())
    }

    #[tokio::test]
    async fn deactivates_only_the_requested_resource_pack() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("options.txt");
        tokio::fs::write(
            &path,
            "resourcePacks:[\"vanilla\",\"file/first.zip\",\"file/second.zip\"]\n",
        )
        .await?;

        prepare_resource_pack_update(
            path.clone(),
            "first.zip",
            ResourcePackOrderChange::SetActive(false),
        )
        .await?
        .commit()
        .await;

        assert_eq!(
            read_resource_packs(&path).await?,
            vec!["vanilla", "file/second.zip"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn rejects_resource_pack_path_traversal() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let result = prepare_resource_pack_update(
            directory.path().join("options.txt"),
            "../outside.zip",
            ResourcePackOrderChange::SetActive(true),
        )
        .await;

        assert!(result.is_err());
        Ok(())
    }
}
