use crate::database::now_rfc3339;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceModTarget<'a> {
    pub provider: Option<Provider>,
    pub project_id: Option<&'a str>,
    pub file_path: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceModEnabledChange<'a> {
    pub target: InstanceModTarget<'a>,
    pub updated_file_path: &'a str,
    pub enabled: bool,
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

    pub async fn set_instance_mod_enabled(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        change: InstanceModEnabledChange<'_>,
    ) -> Result<(), StorageError> {
        let mut transaction = self
            .begin_content_mutation(instance_id, expected_revision)
            .await?;
        if let (Some(provider), Some(project_id)) =
            (change.target.provider, change.target.project_id)
        {
            sqlx::query(
                "UPDATE instance_mods SET enabled = ?, file_path = ? \
                 WHERE instance_id = ? AND file_path = ? AND EXISTS(SELECT 1 FROM instance_mods \
                 WHERE instance_id = ? AND provider = ? AND project_id = ? AND file_path = ?)",
            )
            .bind(i64::from(change.enabled))
            .bind(change.updated_file_path)
            .bind(instance_id.to_string())
            .bind(change.target.file_path)
            .bind(instance_id.to_string())
            .bind(provider.as_str())
            .bind(project_id)
            .bind(change.target.file_path)
            .execute(&mut *transaction)
            .await?;
        }
        insert_content_audit(
            &mut transaction,
            instance_id,
            expected_revision,
            if change.enabled {
                "instance.mod.enabled"
            } else {
                "instance.mod.disabled"
            },
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn set_instance_mod_pinned(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        target: InstanceModTarget<'_>,
        pinned: bool,
    ) -> Result<(), StorageError> {
        let mut transaction = self
            .begin_content_mutation(instance_id, expected_revision)
            .await?;
        if let (Some(provider), Some(project_id)) = (target.provider, target.project_id) {
            sqlx::query(
                "UPDATE instance_mods SET pinned = ? WHERE instance_id = ? AND file_path = ? \
                 AND provider = ? AND project_id = ?",
            )
            .bind(i64::from(pinned))
            .bind(instance_id.to_string())
            .bind(target.file_path)
            .bind(provider.as_str())
            .bind(project_id)
            .execute(&mut *transaction)
            .await?;
        }
        insert_content_audit(
            &mut transaction,
            instance_id,
            expected_revision,
            if pinned {
                "instance.mod.pinned"
            } else {
                "instance.mod.unpinned"
            },
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn remove_instance_mod(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        target: InstanceModTarget<'_>,
    ) -> Result<(), StorageError> {
        let mut transaction = self
            .begin_content_mutation(instance_id, expected_revision)
            .await?;
        if let (Some(provider), Some(project_id)) = (target.provider, target.project_id) {
            sqlx::query(
                "DELETE FROM instance_mods WHERE instance_id = ? AND file_path = ? \
                 AND EXISTS(SELECT 1 FROM instance_mods WHERE instance_id = ? AND provider = ? \
                 AND project_id = ? AND file_path = ?)",
            )
            .bind(instance_id.to_string())
            .bind(target.file_path)
            .bind(instance_id.to_string())
            .bind(provider.as_str())
            .bind(project_id)
            .bind(target.file_path)
            .execute(&mut *transaction)
            .await?;
        }
        insert_content_audit(
            &mut transaction,
            instance_id,
            expected_revision,
            "instance.mod.removed",
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn begin_content_mutation(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
    ) -> Result<sqlx::Transaction<'_, sqlx::Sqlite>, StorageError> {
        let expected_revision =
            i64::try_from(expected_revision).map_err(|_| StorageError::NegativeRevision)?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE instances SET revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL \
             AND NOT EXISTS (SELECT 1 FROM jobs WHERE kind = 'instance_install' \
                 AND entity_id = ? AND state IN ('queued', 'running', 'paused'))",
        )
        .bind(now)
        .bind(instance_id.to_string())
        .bind(expected_revision)
        .bind(instance_id.to_string())
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            transaction.rollback().await?;
            return Err(self
                .content_mutation_error(instance_id, expected_revision)
                .await?);
        }
        Ok(transaction)
    }

    async fn content_mutation_error(
        &self,
        instance_id: InstanceId,
        expected_revision: i64,
    ) -> Result<StorageError, StorageError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE id = ? AND trashed_at IS NULL)",
        )
        .bind(instance_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Ok(StorageError::InstanceNotFound);
        }
        let busy: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM jobs WHERE kind = 'instance_install' \
             AND entity_id = ? AND state IN ('queued', 'running', 'paused'))",
        )
        .bind(instance_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        if busy {
            return Ok(StorageError::InstanceBusy);
        }
        Ok(StorageError::RevisionConflict {
            expected: u64::try_from(expected_revision)
                .map_err(|_| StorageError::NegativeRevision)?,
        })
    }
}

async fn insert_content_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    instance_id: InstanceId,
    before_revision: u64,
    action: &str,
) -> Result<(), StorageError> {
    let before_revision =
        i64::try_from(before_revision).map_err(|_| StorageError::NegativeRevision)?;
    sqlx::query(
        "INSERT INTO local_audit \
         (id, time, action, entity_ref, before_revision, after_revision, source, result) \
         VALUES (?, ?, ?, ?, ?, ?, 'launcher', 'success')",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(now_rfc3339()?)
    .bind(action)
    .bind(instance_id.to_string())
    .bind(before_revision)
    .bind(before_revision + 1)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        CompletedInstall, Database, InstalledRuntime, InstanceModEnabledChange, InstanceModTarget,
        NewInstance, NewInstanceMod, StorageError,
    };
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
        let preparing = database.get_instance(instance.id).await?;
        assert!(matches!(
            database
                .set_instance_mod_enabled(
                    instance.id,
                    preparing.revision,
                    InstanceModEnabledChange {
                        target: InstanceModTarget {
                            provider: None,
                            project_id: None,
                            file_path: "mods/local.jar",
                        },
                        updated_file_path: "mods/local.jar.disabled",
                        enabled: false,
                    },
                )
                .await,
            Err(StorageError::InstanceBusy)
        ));
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
                vec![
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
                    NewInstanceMod {
                        provider: Provider::CurseForge,
                        project_id: "394468".to_owned(),
                        version_id: "file".to_owned(),
                        display_name: "Sodium".to_owned(),
                        file_path: "mods/sodium.jar".to_owned(),
                        hashes: Hashes {
                            sha512: None,
                            sha256: Some("a".repeat(64)),
                            sha1: None,
                        },
                    },
                ],
            )
            .await?;

        let mods = database.list_instance_mods(instance.id).await?;
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].display_name, "Sodium");

        let installed = database.get_instance(instance.id).await?;
        database
            .set_instance_mod_enabled(
                instance.id,
                installed.revision,
                InstanceModEnabledChange {
                    target: InstanceModTarget {
                        provider: Some(Provider::Modrinth),
                        project_id: Some("AANobbMI"),
                        file_path: "mods/sodium.jar",
                    },
                    updated_file_path: "mods/sodium.jar.disabled",
                    enabled: false,
                },
            )
            .await?;
        let disabled = database.list_instance_mods(instance.id).await?;
        assert!(disabled.iter().all(|record| !record.enabled));
        assert!(
            disabled
                .iter()
                .all(|record| record.file_path == "mods/sodium.jar.disabled")
        );

        let revised = database.get_instance(instance.id).await?;
        assert_eq!(revised.revision, installed.revision + 1);
        database
            .remove_instance_mod(
                instance.id,
                revised.revision,
                InstanceModTarget {
                    provider: Some(Provider::Modrinth),
                    project_id: Some("AANobbMI"),
                    file_path: "mods/sodium.jar.disabled",
                },
            )
            .await?;
        assert!(database.list_instance_mods(instance.id).await?.is_empty());
        assert_eq!(
            database.get_instance(instance.id).await?.revision,
            revised.revision + 1
        );
        Ok(())
    }
}
