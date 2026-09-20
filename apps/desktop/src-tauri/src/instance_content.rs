use slate_domain::InstanceId;
use slate_platform::AppPaths;
use std::io;
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
    use super::scan_instance_mods;
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
}
