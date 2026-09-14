use crate::{Database, StorageError};
use slate_domain::InstanceId;
use slate_modpack_api_contracts::{Hashes, Provider};
use sqlx::Row;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewInstanceMod {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub display_name: String,
    pub file_path: String,
    pub hashes: Hashes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceModRecord {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub display_name: String,
    pub file_path: String,
    pub hashes: Hashes,
    pub enabled: bool,
    pub pinned: bool,
    pub installed_at: String,
}

impl Database {
    pub async fn list_instance_mods(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<InstanceModRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT provider, project_id, version_id, display_name, file_path, hashes_json, \
             enabled, pinned, installed_at FROM instance_mods WHERE instance_id = ? \
             ORDER BY display_name COLLATE NOCASE, provider, project_id",
        )
        .bind(instance_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                let provider: String = row.try_get("provider")?;
                Ok(InstanceModRecord {
                    provider: Provider::from_str(&provider).map_err(|_| {
                        StorageError::InvalidStoredValue {
                            field: "instance_mods.provider",
                            value: provider,
                        }
                    })?,
                    project_id: row.try_get("project_id")?,
                    version_id: row.try_get("version_id")?,
                    display_name: row.try_get("display_name")?,
                    file_path: row.try_get("file_path")?,
                    hashes: serde_json::from_str(&row.try_get::<String, _>("hashes_json")?)?,
                    enabled: row.try_get::<i64, _>("enabled")? != 0,
                    pinned: row.try_get::<i64, _>("pinned")? != 0,
                    installed_at: row.try_get("installed_at")?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{CompletedInstall, Database, InstalledRuntime, NewInstance, NewInstanceMod};
    use slate_domain::{InstanceMode, InstanceName, LoaderFamily, ManagementMode, RequestId};
    use slate_modpack_api_contracts::{Hashes, Provider};

    #[tokio::test]
    async fn installed_mods_are_recorded_with_completed_revisions()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let root = database.create_storage_root("C:/slate", None).await?;
        let instance = database
            .create_instance(NewInstance {
                name: InstanceName::parse("Fabric")?,
                mode: InstanceMode::Modded,
                management_mode: ManagementMode::Local,
                root_id: root,
                minecraft_version: "1.21.1".to_owned(),
                loader_kind: LoaderFamily::Fabric,
                loader_version: Some("0.16.10".to_owned()),
                memory_mb: 4096,
                modpack_source: None,
            })
            .await?;
        let pending = database
            .begin_instance_install(instance.id, instance.revision, RequestId::new())
            .await?;
        database
            .complete_instance_mod_install(
                CompletedInstall {
                    job_id: pending.job.id,
                    revision_id: pending.revision_id,
                    manifest_digest: "manifest".to_owned(),
                    client_version: "fabric-loader-0.16.10-1.21.1".to_owned(),
                    runtime: InstalledRuntime {
                        vendor: "Eclipse Adoptium".to_owned(),
                        release_name: "21".to_owned(),
                        java_version: "21".to_owned(),
                        major: 21,
                        os: "windows".to_owned(),
                        arch: "x86_64".to_owned(),
                        executable_ref: "C:/slate/java.exe".to_owned(),
                        source_digest: "digest".to_owned(),
                    },
                    message: "Installed".to_owned(),
                },
                NewInstanceMod {
                    provider: Provider::Modrinth,
                    project_id: "AANobbMI".to_owned(),
                    version_id: "version".to_owned(),
                    display_name: "Sodium".to_owned(),
                    file_path: "mods/sodium.jar".to_owned(),
                    hashes: Hashes {
                        sha512: None,
                        sha256: Some("a".repeat(64)),
                        sha1: None,
                    },
                },
            )
            .await?;

        let mods = database.list_instance_mods(instance.id).await?;
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].display_name, "Sodium");
        Ok(())
    }
}
