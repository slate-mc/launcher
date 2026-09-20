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
              file_path, hashes_json, installed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(instance_id, kind, provider, project_id) DO UPDATE SET \
              version_id = excluded.version_id, display_name = excluded.display_name, \
              icon_url = excluded.icon_url, file_path = excluded.file_path, \
              hashes_json = excluded.hashes_json, installed_at = excluded.installed_at",
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
        .bind(&now)
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
