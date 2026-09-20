use crate::database::now_rfc3339;
use crate::{Database, InstanceModRecord, StorageError};
use slate_domain::InstanceId;
use sqlx::Row;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceSnapshotRecord {
    pub id: Uuid,
    pub instance_id: InstanceId,
    pub manifest_ref: String,
    pub size_bytes: u64,
    pub pinned: bool,
    pub created_at: String,
}

impl Database {
    pub async fn create_instance_snapshot(
        &self,
        instance_id: InstanceId,
        id: Uuid,
        manifest_ref: &str,
        size_bytes: u64,
        mods: &[InstanceModRecord],
    ) -> Result<InstanceSnapshotRecord, StorageError> {
        let size_bytes =
            i64::try_from(size_bytes).map_err(|_| StorageError::InvalidStoredValue {
                field: "instance_snapshots.size_bytes",
                value: size_bytes.to_string(),
            })?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO instance_snapshots \
             (id, instance_id, source_revision_id, kind, consistency, manifest_ref, size_bytes, \
              pinned, verified_at, created_at) \
             SELECT ?, i.id, i.active_revision_id, 'full', 'stopped', ?, ?, 0, ?, ? \
             FROM instances i WHERE i.id = ? AND i.trashed_at IS NULL",
        )
        .bind(id.to_string())
        .bind(manifest_ref)
        .bind(size_bytes)
        .bind(&now)
        .bind(&now)
        .bind(instance_id.to_string())
        .execute(&mut *transaction)
        .await?;
        if inserted.rows_affected() == 0 {
            transaction.rollback().await?;
            return Err(StorageError::InstanceNotFound);
        }
        for installed_mod in mods {
            sqlx::query(
                "INSERT INTO instance_snapshot_mods \
                 (snapshot_id, provider, project_id, version_id, display_name, file_path, \
                  hashes_json, enabled, pinned, installed_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id.to_string())
            .bind(installed_mod.provider.as_str())
            .bind(&installed_mod.project_id)
            .bind(&installed_mod.version_id)
            .bind(&installed_mod.display_name)
            .bind(&installed_mod.file_path)
            .bind(serde_json::to_string(&installed_mod.hashes)?)
            .bind(i64::from(installed_mod.enabled))
            .bind(i64::from(installed_mod.pinned))
            .bind(&installed_mod.installed_at)
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query(
            "INSERT INTO instance_snapshot_modpacks \
             (snapshot_id, provider, project_id, version_id, selected_optional_json, display_name, \
              icon_url, banner_url, created_at, updated_at) \
             SELECT ?, provider, project_id, version_id, selected_optional_json, display_name, \
              icon_url, banner_url, created_at, updated_at FROM instance_modpacks \
             WHERE instance_id = ?",
        )
        .bind(id.to_string())
        .bind(instance_id.to_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        self.get_instance_snapshot(instance_id, id).await
    }

    pub async fn restore_instance_snapshot_state(
        &self,
        instance_id: InstanceId,
        snapshot_id: Uuid,
        expected_revision: u64,
    ) -> Result<(), StorageError> {
        let stored_revision =
            i64::try_from(expected_revision).map_err(|_| StorageError::RevisionConflict {
                expected: expected_revision,
            })?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let source_revision: Option<String> = sqlx::query_scalar(
            "SELECT source_revision_id FROM instance_snapshots WHERE id = ? AND instance_id = ?",
        )
        .bind(snapshot_id.to_string())
        .bind(instance_id.to_string())
        .fetch_optional(&mut *transaction)
        .await?
        .flatten();
        let updated = sqlx::query(
            "UPDATE instances SET revision = revision + 1, updated_at = ?, \
             active_revision_id = COALESCE(?, active_revision_id) \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(&now)
        .bind(source_revision.as_deref())
        .bind(instance_id.to_string())
        .bind(stored_revision)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            transaction.rollback().await?;
            return self.revision_error(instance_id, stored_revision).await;
        }
        if let Some(source_revision) = source_revision.as_deref() {
            sqlx::query(
                "UPDATE instance_revisions SET status = CASE WHEN id = ? THEN 'installed' \
                 WHEN status = 'installed' THEN 'superseded' ELSE status END \
                 WHERE instance_id = ?",
            )
            .bind(source_revision)
            .bind(instance_id.to_string())
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE instance_configuration SET \
                 minecraft_version = (SELECT game_version FROM instance_revisions WHERE id = ?), \
                 loader_kind = COALESCE((SELECT loader_kind FROM instance_revisions WHERE id = ?), 'vanilla'), \
                 loader_version = (SELECT loader_version FROM instance_revisions WHERE id = ?), \
                 setup_state = 'ready' WHERE instance_id = ?",
            )
            .bind(source_revision)
            .bind(source_revision)
            .bind(source_revision)
            .bind(instance_id.to_string())
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query("DELETE FROM instance_mods WHERE instance_id = ?")
            .bind(instance_id.to_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO instance_mods \
             (instance_id, provider, project_id, version_id, display_name, file_path, \
              hashes_json, enabled, pinned, installed_at) \
             SELECT ?, provider, project_id, version_id, display_name, file_path, hashes_json, \
              enabled, pinned, installed_at FROM instance_snapshot_mods WHERE snapshot_id = ?",
        )
        .bind(instance_id.to_string())
        .bind(snapshot_id.to_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM instance_modpacks WHERE instance_id = ?")
            .bind(instance_id.to_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO instance_modpacks \
             (instance_id, provider, project_id, version_id, selected_optional_json, display_name, \
              icon_url, banner_url, created_at, updated_at) \
             SELECT ?, provider, project_id, version_id, selected_optional_json, display_name, \
              icon_url, banner_url, created_at, updated_at FROM instance_snapshot_modpacks \
             WHERE snapshot_id = ?",
        )
        .bind(instance_id.to_string())
        .bind(snapshot_id.to_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn get_instance_snapshot(
        &self,
        instance_id: InstanceId,
        snapshot_id: Uuid,
    ) -> Result<InstanceSnapshotRecord, StorageError> {
        let row = sqlx::query(
            "SELECT id, instance_id, manifest_ref, size_bytes, pinned, created_at \
             FROM instance_snapshots WHERE id = ? AND instance_id = ?",
        )
        .bind(snapshot_id.to_string())
        .bind(instance_id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::InstanceNotFound)?;
        snapshot_from_row(&row)
    }

    pub async fn list_instance_snapshots(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<InstanceSnapshotRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, instance_id, manifest_ref, size_bytes, pinned, created_at \
             FROM instance_snapshots WHERE instance_id = ? ORDER BY created_at DESC, id DESC",
        )
        .bind(instance_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(snapshot_from_row).collect()
    }

    pub async fn set_instance_snapshot_pinned(
        &self,
        instance_id: InstanceId,
        snapshot_id: Uuid,
        pinned: bool,
    ) -> Result<InstanceSnapshotRecord, StorageError> {
        let updated = sqlx::query(
            "UPDATE instance_snapshots SET pinned = ? WHERE id = ? AND instance_id = ?",
        )
        .bind(i64::from(pinned))
        .bind(snapshot_id.to_string())
        .bind(instance_id.to_string())
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StorageError::InstanceNotFound);
        }
        self.get_instance_snapshot(instance_id, snapshot_id).await
    }

    pub async fn delete_instance_snapshot(
        &self,
        instance_id: InstanceId,
        snapshot_id: Uuid,
    ) -> Result<InstanceSnapshotRecord, StorageError> {
        let record = self.get_instance_snapshot(instance_id, snapshot_id).await?;
        sqlx::query("DELETE FROM instance_snapshots WHERE id = ? AND instance_id = ?")
            .bind(snapshot_id.to_string())
            .bind(instance_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(record)
    }
}

fn snapshot_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<InstanceSnapshotRecord, StorageError> {
    let id = Uuid::parse_str(&row.try_get::<String, _>("id")?).map_err(|source| {
        StorageError::InvalidStoredId {
            field: "instance_snapshots.id",
            source,
        }
    })?;
    let instance_id = Uuid::parse_str(&row.try_get::<String, _>("instance_id")?)
        .map(InstanceId::from_uuid)
        .map_err(|source| StorageError::InvalidStoredId {
            field: "instance_snapshots.instance_id",
            source,
        })?;
    let size = row.try_get::<i64, _>("size_bytes")?;
    let size_bytes = u64::try_from(size).map_err(|_| StorageError::InvalidStoredValue {
        field: "instance_snapshots.size_bytes",
        value: size.to_string(),
    })?;
    Ok(InstanceSnapshotRecord {
        id,
        instance_id,
        manifest_ref: row.try_get("manifest_ref")?,
        size_bytes,
        pinned: row.try_get::<i64, _>("pinned")? != 0,
        created_at: row.try_get("created_at")?,
    })
}
