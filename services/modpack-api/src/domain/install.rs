use slate_modpack_api_contracts::{
    ExtractAction, InstallPlan, InstallPlanDownload, InstallPlanInstance, InstallPlanRequest,
    JavaPlan, ModpackVersion, PackFileType, RuntimePlan, Side,
};
use std::collections::BTreeSet;

pub fn generate_install_plan(
    version: &ModpackVersion,
    request: &InstallPlanRequest,
) -> Result<InstallPlan, InstallPlanError> {
    let known_options = version
        .files
        .iter()
        .filter_map(|file| file.option.as_ref().map(|option| option.id.as_str()))
        .collect::<BTreeSet<_>>();
    if request
        .include_optional
        .iter()
        .any(|option| !known_options.contains(option.as_str()))
    {
        return Err(InstallPlanError::UnknownOptionalFile);
    }

    let included_options = request
        .include_optional
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut destinations = BTreeSet::new();
    let mut downloads = Vec::new();
    let mut extract = Vec::new();
    let mut total_download_size = 0_u64;

    for file in &version.files {
        if file.side == Side::Server {
            continue;
        }
        if file.optional
            && !file.option.as_ref().is_some_and(|option| {
                option.default || included_options.contains(option.id.as_str())
            })
        {
            continue;
        }
        if !file.hashes.has_cryptographic_hash() {
            return Err(InstallPlanError::MissingHash);
        }
        let mut destination = normalize_install_path(&file.path)?;
        let download_id = format!("{}:{}:{}", version.provider, version.project_id, file.id);
        if file.kind == PackFileType::Archive {
            destination = format!(".slate/archives/{}.archive", safe_identifier(&file.id));
            extract.push(ExtractAction {
                download_id: download_id.clone(),
                destination: ".".to_owned(),
                source_prefix: Some("overrides/".to_owned()),
            });
            extract.push(ExtractAction {
                download_id: download_id.clone(),
                destination: ".".to_owned(),
                source_prefix: Some("client-overrides/".to_owned()),
            });
        }
        if !destinations.insert(destination.clone()) {
            return Err(InstallPlanError::DuplicateDestination);
        }
        total_download_size = total_download_size
            .checked_add(file.size)
            .ok_or(InstallPlanError::DownloadSizeOverflow)?;
        downloads.push(InstallPlanDownload {
            id: download_id,
            destination,
            size: file.size,
            hashes: file.hashes.clone(),
            sources: vec![file.download.clone()],
            required: !file.optional,
        });
    }

    Ok(InstallPlan {
        schema: 1,
        instance: InstallPlanInstance {
            provider: version.provider,
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            name: version.name.clone(),
        },
        runtime: RuntimePlan {
            minecraft: version.minecraft.version.clone(),
            loader: version.loader.clone(),
            java: JavaPlan {
                major: java_major_for_minecraft(&version.minecraft.version),
            },
            memory: version.memory.clone(),
        },
        downloads,
        extract,
        delete: Vec::new(),
        total_download_size,
    })
}

pub fn normalize_install_path(value: &str) -> Result<String, InstallPlanError> {
    let normalized = value.replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.starts_with("~/")
        || normalized.starts_with("//")
    {
        return Err(InstallPlanError::InvalidPath);
    }
    let mut segments = Vec::new();
    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." || segment.contains(':') || segment.contains('\0') {
            return Err(InstallPlanError::InvalidPath);
        }
        segments.push(segment);
    }
    if segments.is_empty() {
        return Err(InstallPlanError::InvalidPath);
    }
    Ok(segments.join("/"))
}

fn safe_identifier(value: &str) -> String {
    let safe = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .take(100)
        .collect::<String>();
    if safe.is_empty() {
        "archive".to_owned()
    } else {
        safe
    }
}

fn java_major_for_minecraft(version: &str) -> u32 {
    let parts = version
        .split('.')
        .filter_map(|part| part.parse::<u32>().ok())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [year, ..] if *year >= 26 => 25,
        [1, minor, patch, ..] if *minor < 17 || (*minor == 16 && *patch <= 5) => 8,
        [1, 17, ..] => 16,
        [1, minor, patch, ..] if *minor < 20 || (*minor == 20 && *patch <= 4) => 17,
        [1, minor, ..] if *minor < 20 => 17,
        _ => 21,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum InstallPlanError {
    #[error("installation path is not a safe relative path")]
    InvalidPath,
    #[error("two files resolve to the same installation path")]
    DuplicateDestination,
    #[error("a required file has no cryptographic hash")]
    MissingHash,
    #[error("an unknown optional file was selected")]
    UnknownOptionalFile,
    #[error("total download size overflowed")]
    DownloadSizeOverflow,
}

#[cfg(test)]
mod tests {
    use super::{java_major_for_minecraft, normalize_install_path};

    #[test]
    fn install_paths_are_normalized_and_cannot_escape() {
        assert_eq!(
            normalize_install_path("./config\\example.toml"),
            Ok("config/example.toml".to_owned())
        );
        for invalid in [
            "../mods/a.jar",
            "C:\\mods\\a.jar",
            "//server/share/a.jar",
            "/etc/passwd",
            "~/a.jar",
            "mods/a.jar:stream",
        ] {
            assert!(normalize_install_path(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn java_compatibility_covers_legacy_and_current_versions() {
        assert_eq!(java_major_for_minecraft("1.16.5"), 8);
        assert_eq!(java_major_for_minecraft("1.17.1"), 16);
        assert_eq!(java_major_for_minecraft("1.20.4"), 17);
        assert_eq!(java_major_for_minecraft("1.20.5"), 21);
        assert_eq!(java_major_for_minecraft("26.2"), 25);
    }
}
