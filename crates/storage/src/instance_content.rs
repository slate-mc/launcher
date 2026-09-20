use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_domain::InstanceId;
use slate_modpack_api_contracts::{ContentKind, Hashes, Provider};
use sqlx::Row;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewInstanceProviderContent {
    pub kind: ContentKind,
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub display_name: String,
    pub icon_url: Option<String>,
    pub file_path: String,
    pub hashes: Hashes,
    pub pinned: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceProviderContentRecord {
    pub instance_id: InstanceId,
    pub kind: ContentKind,
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub display_name: String,
    pub icon_url: Option<String>,
    pub file_path: String,
    pub hashes: Hashes,
    pub pinned: bool,
    pub installed_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceProviderContentHistoryRecord {
    pub version_id: String,
    pub changed_at: String,
}

impl Database {
    pub async fn list_instance_provider_content(
        &self,
        instance_id: InstanceId,
        kind: ContentKind,
    ) -> Result<Vec<InstanceProviderContentRecord>, StorageError> {
        sqlx::query(
            "SELECT instance_id, kind, provider, project_id, version_id, display_name, \
             icon_url, file_path, hashes_json, pinned, installed_at FROM instance_provider_content \
             WHERE instance_id = ? AND kind = ? ORDER BY display_name COLLATE NOCASE, file_path",
        )
        .bind(instance_id.to_string())
        .bind(kind.as_str())
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            let stored_kind: String = row.try_get("kind")?;
            let provider: String = row.try_get("provider")?;
            let instance_id: String = row.try_get("instance_id")?;
            Ok(InstanceProviderContentRecord {
                instance_id: InstanceId::from_uuid(uuid::Uuid::parse_str(&instance_id).map_err(
                    |source| StorageError::InvalidStoredId {
                        field: "instance_provider_content.instance_id",
                        source,
                    },
                )?),
                kind: parse_content_kind(stored_kind)?,
                provider: Provider::from_str(&provider).map_err(|_| {
                    StorageError::InvalidStoredValue {
                        field: "instance_provider_content.provider",
                        value: provider,
                    }
                })?,
                project_id: row.try_get("project_id")?,
                version_id: row.try_get("version_id")?,
                display_name: row.try_get("display_name")?,
                icon_url: row.try_get("icon_url")?,
                file_path: row.try_get("file_path")?,
                hashes: serde_json::from_str(&row.try_get::<String, _>("hashes_json")?)?,
                pinned: row.try_get::<i64, _>("pinned")? != 0,
                installed_at: row.try_get("installed_at")?,
            })
        })
        .collect()
    }

    pub async fn move_instance_provider_content(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        previous_path: &str,
        updated_path: &str,
    ) -> Result<(), StorageError> {
        let mut transaction = self
            .begin_content_mutation(instance_id, expected_revision)
            .await?;
        sqlx::query(
            "UPDATE instance_provider_content SET file_path = ? \
             WHERE instance_id = ? AND file_path = ?",
        )
        .bind(updated_path.trim())
        .bind(instance_id.to_string())
        .bind(previous_path.trim())
        .execute(&mut *transaction)
        .await?;
        insert_content_audit(
            &mut transaction,
            instance_id,
            expected_revision,
            "instance.content.toggled",
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn remove_instance_provider_content(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        file_path: &str,
    ) -> Result<(), StorageError> {
        let mut transaction = self
            .begin_content_mutation(instance_id, expected_revision)
            .await?;
        sqlx::query(
            "DELETE FROM instance_provider_content WHERE instance_id = ? AND file_path = ?",
        )
        .bind(instance_id.to_string())
        .bind(file_path.trim())
        .execute(&mut *transaction)
        .await?;
        insert_content_audit(
            &mut transaction,
            instance_id,
            expected_revision,
            "instance.content.removed",
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn set_instance_provider_content_pinned(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        kind: ContentKind,
        provider: Provider,
        project_id: &str,
        pinned: bool,
    ) -> Result<(), StorageError> {
        let mut transaction = self
            .begin_content_mutation(instance_id, expected_revision)
            .await?;
        sqlx::query(
            "UPDATE instance_provider_content SET pinned = ? WHERE instance_id = ? AND kind = ? \
             AND provider = ? AND project_id = ?",
        )
        .bind(i64::from(pinned))
        .bind(instance_id.to_string())
        .bind(kind.as_str())
        .bind(provider.as_str())
        .bind(project_id.trim())
        .execute(&mut *transaction)
        .await?;
        insert_content_audit(
            &mut transaction,
            instance_id,
            expected_revision,
            if pinned {
                "instance.content.pinned"
            } else {
                "instance.content.unpinned"
            },
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn list_instance_provider_content_history(
        &self,
        instance_id: InstanceId,
        kind: ContentKind,
        provider: Provider,
        project_id: &str,
    ) -> Result<Vec<InstanceProviderContentHistoryRecord>, StorageError> {
        sqlx::query(
            "SELECT version_id, changed_at FROM instance_provider_content_history \
             WHERE instance_id = ? AND kind = ? AND provider = ? AND project_id = ? \
             ORDER BY changed_at DESC, id DESC LIMIT 20",
        )
        .bind(instance_id.to_string())
        .bind(kind.as_str())
        .bind(provider.as_str())
        .bind(project_id.trim())
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(InstanceProviderContentHistoryRecord {
                version_id: row.try_get("version_id")?,
                changed_at: row.try_get("changed_at")?,
            })
        })
        .collect()
    }
}

fn parse_content_kind(value: String) -> Result<ContentKind, StorageError> {
    match value.as_str() {
        "resource_pack" => Ok(ContentKind::ResourcePack),
        "shader_pack" => Ok(ContentKind::ShaderPack),
        "data_pack" => Ok(ContentKind::DataPack),
        _ => Err(StorageError::InvalidStoredValue {
            field: "instance_provider_content.kind",
            value,
        }),
    }
}

pub(crate) async fn insert_provider_content(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    instance_id: &str,
    items: Vec<NewInstanceProviderContent>,
    replaced_paths: Vec<String>,
) -> Result<(), StorageError> {
    for path in replaced_paths {
        let replaced = sqlx::query(
            "SELECT kind, provider, project_id, version_id FROM instance_provider_content \
             WHERE instance_id = ? AND file_path = ?",
        )
        .bind(instance_id)
        .bind(path.trim())
        .fetch_all(&mut **transaction)
        .await?;
        for item in replaced {
            sqlx::query(
                "INSERT INTO instance_provider_content_history \
                 (id, instance_id, kind, provider, project_id, version_id, changed_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(instance_id)
            .bind(item.try_get::<String, _>("kind")?)
            .bind(item.try_get::<String, _>("provider")?)
            .bind(item.try_get::<String, _>("project_id")?)
            .bind(item.try_get::<String, _>("version_id")?)
            .bind(now_rfc3339()?)
            .execute(&mut **transaction)
            .await?;
        }
        sqlx::query(
            "DELETE FROM instance_provider_content WHERE instance_id = ? AND file_path = ?",
        )
        .bind(instance_id)
        .bind(path.trim())
        .execute(&mut **transaction)
        .await?;
    }
    let now = now_rfc3339()?;
    for item in items {
        sqlx::query(
            "INSERT INTO instance_provider_content \
             (instance_id, kind, provider, project_id, version_id, display_name, icon_url, \
              file_path, hashes_json, pinned, installed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(instance_id, kind, provider, project_id) DO UPDATE SET \
              version_id = excluded.version_id, display_name = excluded.display_name, \
              icon_url = excluded.icon_url, file_path = excluded.file_path, \
              hashes_json = excluded.hashes_json, pinned = excluded.pinned, \
              installed_at = excluded.installed_at",
        )
        .bind(instance_id)
        .bind(item.kind.as_str())
        .bind(item.provider.as_str())
        .bind(item.project_id.trim())
        .bind(item.version_id.trim())
        .bind(item.display_name.trim())
        .bind(item.icon_url.as_deref())
        .bind(item.file_path.trim())
        .bind(serde_json::to_string(&item.hashes)?)
        .bind(i64::from(item.pinned))
        .bind(&now)
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "DELETE FROM instance_provider_content_history WHERE id IN (SELECT id FROM \
             instance_provider_content_history WHERE instance_id = ? AND kind = ? AND provider = ? \
             AND project_id = ? ORDER BY changed_at DESC, id DESC LIMIT -1 OFFSET 20)",
        )
        .bind(instance_id)
        .bind(item.kind.as_str())
        .bind(item.provider.as_str())
        .bind(item.project_id.trim())
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
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
        CompletedInstall, Database, InstalledRuntime, NewInstance, NewInstanceProviderContent,
    };
    use slate_domain::{InstanceMode, InstanceName, LoaderFamily, ManagementMode, RequestId};
    use slate_modpack_api_contracts::{ContentKind, Hashes, Provider};

    #[tokio::test]
    async fn provider_content_preserves_pins_and_records_version_history()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let root = database.create_storage_root("C:/slate", None).await?;
        let instance = database
            .create_instance(NewInstance {
                name: InstanceName::parse("Content")?,
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
        let first = database
            .begin_instance_install(instance.id, instance.revision, RequestId::new())
            .await?;
        database
            .complete_instance_provider_content_install(
                completed(first.job.id, first.revision_id),
                vec![content("version-1", "resourcepacks/content-v1.zip", false)],
                Vec::new(),
            )
            .await?;
        let ready = database.get_instance(instance.id).await?;
        database
            .set_instance_provider_content_pinned(
                instance.id,
                ready.revision,
                ContentKind::ResourcePack,
                Provider::Modrinth,
                "project",
                true,
            )
            .await?;
        let pinned = database.get_instance(instance.id).await?;
        let second = database
            .begin_instance_install(instance.id, pinned.revision, RequestId::new())
            .await?;
        database
            .complete_instance_provider_content_install(
                completed(second.job.id, second.revision_id),
                vec![content("version-2", "resourcepacks/content-v2.zip", true)],
                vec!["resourcepacks/content-v1.zip".to_owned()],
            )
            .await?;

        let installed = database
            .list_instance_provider_content(instance.id, ContentKind::ResourcePack)
            .await?;
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version_id, "version-2");
        assert!(installed[0].pinned);
        let history = database
            .list_instance_provider_content_history(
                instance.id,
                ContentKind::ResourcePack,
                Provider::Modrinth,
                "project",
            )
            .await?;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].version_id, "version-1");
        Ok(())
    }

    fn content(version_id: &str, file_path: &str, pinned: bool) -> NewInstanceProviderContent {
        NewInstanceProviderContent {
            kind: ContentKind::ResourcePack,
            provider: Provider::Modrinth,
            project_id: "project".to_owned(),
            version_id: version_id.to_owned(),
            display_name: "Content".to_owned(),
            icon_url: None,
            file_path: file_path.to_owned(),
            hashes: Hashes {
                sha512: Some("hash".to_owned()),
                sha256: None,
                sha1: None,
            },
            pinned,
        }
    }

    fn completed(
        job_id: slate_domain::JobId,
        revision_id: slate_domain::RevisionId,
    ) -> CompletedInstall {
        CompletedInstall {
            job_id,
            revision_id,
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
        }
    }
}
