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
    pub enabled: bool,
    pub pinned: bool,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewInstanceModDependencySet {
    pub root_provider: Provider,
    pub root_project_id: String,
    pub dependencies: Vec<InstanceModDependency>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceModDependency {
    pub provider: Provider,
    pub project_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceModDependencyRecord {
    pub root_provider: Provider,
    pub root_project_id: String,
    pub root_display_name: Option<String>,
    pub dependency_provider: Provider,
    pub dependency_project_id: String,
    pub dependency_display_name: Option<String>,
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

    pub async fn list_instance_mod_dependencies(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<InstanceModDependencyRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT d.root_provider, d.root_project_id, rm.display_name AS root_display_name, \
             d.dependency_provider, d.dependency_project_id, \
             dm.display_name AS dependency_display_name \
             FROM instance_mod_dependencies d LEFT JOIN instance_mods rm \
             ON rm.instance_id = d.instance_id AND rm.provider = d.root_provider \
             AND rm.project_id = d.root_project_id LEFT JOIN instance_mods dm \
             ON dm.instance_id = d.instance_id AND dm.provider = d.dependency_provider \
             AND dm.project_id = d.dependency_project_id WHERE d.instance_id = ? \
             ORDER BY d.root_provider, d.root_project_id, \
             COALESCE(dm.display_name, d.dependency_project_id) COLLATE NOCASE",
        )
        .bind(instance_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                let root_provider: String = row.try_get("root_provider")?;
                let dependency_provider: String = row.try_get("dependency_provider")?;
                Ok(InstanceModDependencyRecord {
                    root_provider: parse_stored_provider(
                        "instance_mod_dependencies.root_provider",
                        root_provider,
                    )?,
                    root_project_id: row.try_get("root_project_id")?,
                    root_display_name: row.try_get("root_display_name")?,
                    dependency_provider: parse_stored_provider(
                        "instance_mod_dependencies.dependency_provider",
                        dependency_provider,
                    )?,
                    dependency_project_id: row.try_get("dependency_project_id")?,
                    dependency_display_name: row.try_get("dependency_display_name")?,
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
                "DELETE FROM instance_mod_dependencies WHERE instance_id = ? AND \
                 ((root_provider = ? AND root_project_id = ?) OR \
                  (dependency_provider = ? AND dependency_project_id = ?))",
            )
            .bind(instance_id.to_string())
            .bind(provider.as_str())
            .bind(project_id)
            .bind(provider.as_str())
            .bind(project_id)
            .execute(&mut *transaction)
            .await?;
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

fn parse_stored_provider(field: &'static str, value: String) -> Result<Provider, StorageError> {
    Provider::from_str(&value).map_err(|_| StorageError::InvalidStoredValue { field, value })
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
        CompletedInstall, Database, InstalledRuntime, InstanceModDependency,
        InstanceModEnabledChange, InstanceModTarget, NewInstance, NewInstanceMod,
        NewInstanceModDependencySet, StorageError,
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
                        enabled: true,
                        pinned: false,
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
                        enabled: true,
                        pinned: false,
                    },
                ],
                vec![NewInstanceModDependencySet {
                    root_provider: Provider::Modrinth,
                    root_project_id: "AANobbMI".to_owned(),
                    dependencies: vec![InstanceModDependency {
                        provider: Provider::CurseForge,
                        project_id: "394468".to_owned(),
                    }],
                }],
            )
            .await?;

        let mods = database.list_instance_mods(instance.id).await?;
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].display_name, "Sodium");
        let dependencies = database.list_instance_mod_dependencies(instance.id).await?;
        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0].root_project_id, "AANobbMI");
        assert_eq!(dependencies[0].dependency_project_id, "394468");

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
        let pending_update = database
            .begin_instance_install(instance.id, revised.revision, RequestId::new())
            .await?;
        database
            .complete_instance_mod_update(
                CompletedInstall {
                    job_id: pending_update.job.id,
                    revision_id: pending_update.revision_id,
                    manifest_digest: "updated-manifest".to_owned(),
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
                    message: "Updated Sodium".to_owned(),
                },
                vec![NewInstanceMod {
                    provider: Provider::Modrinth,
                    project_id: "AANobbMI".to_owned(),
                    version_id: "version-2".to_owned(),
                    display_name: "Sodium".to_owned(),
                    file_path: "mods/sodium-2.jar.disabled".to_owned(),
                    hashes: Hashes {
                        sha512: None,
                        sha256: Some("b".repeat(64)),
                        sha1: None,
                    },
                    enabled: false,
                    pinned: true,
                }],
                vec!["mods/sodium.jar.disabled".to_owned()],
                Vec::new(),
            )
            .await?;
        let updated = database.list_instance_mods(instance.id).await?;
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].version_id, "version-2");
        assert_eq!(updated[0].file_path, "mods/sodium-2.jar.disabled");
        assert!(!updated[0].enabled);
        assert!(updated[0].pinned);
        assert!(
            database
                .list_instance_mod_dependencies(instance.id)
                .await?
                .is_empty()
        );

        let updated_instance = database.get_instance(instance.id).await?;
        database
            .remove_instance_mod(
                instance.id,
                updated_instance.revision,
                InstanceModTarget {
                    provider: Some(Provider::Modrinth),
                    project_id: Some("AANobbMI"),
                    file_path: "mods/sodium-2.jar.disabled",
                },
            )
            .await?;
        assert!(database.list_instance_mods(instance.id).await?.is_empty());
        assert_eq!(
            database.get_instance(instance.id).await?.revision,
            updated_instance.revision + 1
        );
        Ok(())
    }
}
