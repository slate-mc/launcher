use crate::database::now_rfc3339;
use crate::instance_settings::settings_from_row;
use crate::{Database, StorageError};
use slate_domain::{
    InstanceId, InstanceMode, InstanceName, InstanceSetupState, LoaderFamily, ManagementMode,
    StorageRootId,
};
use slate_modpack_api_contracts::Provider;
use slate_platform::ManagedRelativePath;
use sqlx::Row;
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewInstance {
    pub name: InstanceName,
    pub mode: InstanceMode,
    pub management_mode: ManagementMode,
    pub root_id: StorageRootId,
    pub minecraft_version: String,
    pub loader_kind: LoaderFamily,
    pub loader_version: Option<String>,
    pub memory_mb: u32,
    pub modpack_source: Option<NewModpackSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewModpackSource {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub selected_optional: Vec<String>,
    pub display_name: String,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModpackSourceRecord {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub selected_optional: Vec<String>,
    pub display_name: String,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceRecord {
    pub id: InstanceId,
    pub name: InstanceName,
    pub mode: InstanceMode,
    pub management_mode: ManagementMode,
    pub root_id: StorageRootId,
    pub relative_path: ManagedRelativePath,
    pub storage_path: PathBuf,
    pub favorite: bool,
    pub revision: u64,
    pub minecraft_version: String,
    pub loader_kind: LoaderFamily,
    pub loader_version: Option<String>,
    pub memory_mb: u32,
    pub mod_count: u32,
    pub setup_state: InstanceSetupState,
    pub settings: crate::InstanceSettingsRecord,
    pub modpack_source: Option<ModpackSourceRecord>,
    pub created_at: String,
    pub updated_at: String,
    pub last_played: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrashedInstanceRecord {
    pub id: InstanceId,
    pub name: InstanceName,
    pub revision: u64,
    pub storage_path: PathBuf,
    pub minecraft_version: String,
    pub loader_kind: LoaderFamily,
    pub loader_version: Option<String>,
    pub source_name: Option<String>,
    pub icon_url: Option<String>,
    pub trashed_at: String,
}

impl Database {
    pub async fn create_storage_root(
        &self,
        canonical_path: &str,
        device_fingerprint: Option<&str>,
    ) -> Result<StorageRootId, StorageError> {
        let id = StorageRootId::new();
        let now = now_rfc3339()?;
        sqlx::query(
            "INSERT INTO storage_roots \
             (id, canonical_path, device_fingerprint, state, created_at, updated_at) \
             VALUES (?, ?, ?, 'available', ?, ?)",
        )
        .bind(id.to_string())
        .bind(canonical_path)
        .bind(device_fingerprint)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn get_or_create_storage_root(
        &self,
        canonical_path: &str,
    ) -> Result<StorageRootId, StorageError> {
        if let Some(value) =
            sqlx::query_scalar::<_, String>("SELECT id FROM storage_roots WHERE canonical_path = ?")
                .bind(canonical_path)
                .fetch_optional(&self.pool)
                .await?
        {
            return parse_uuid(value, "storage_roots.id").map(StorageRootId::from_uuid);
        }

        match self.create_storage_root(canonical_path, None).await {
            Ok(id) => Ok(id),
            Err(StorageError::Database(error))
                if error
                    .as_database_error()
                    .is_some_and(|database_error| database_error.is_unique_violation()) =>
            {
                let value: String =
                    sqlx::query_scalar("SELECT id FROM storage_roots WHERE canonical_path = ?")
                        .bind(canonical_path)
                        .fetch_one(&self.pool)
                        .await?;
                parse_uuid(value, "storage_roots.id").map(StorageRootId::from_uuid)
            }
            Err(error) => Err(error),
        }
    }

    pub async fn create_instance(
        &self,
        instance: NewInstance,
    ) -> Result<InstanceRecord, StorageError> {
        let id = InstanceId::new();
        let relative_path_value = format!("instances/{id}");
        let relative_path = ManagedRelativePath::parse(&relative_path_value)?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO instances \
             (id, name, mode, management_mode, root_id, relative_path, favorite, revision, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, 0, ?, ?)",
        )
        .bind(id.to_string())
        .bind(instance.name.as_str())
        .bind(instance.mode.as_storage_value())
        .bind(instance.management_mode.as_storage_value())
        .bind(instance.root_id.to_string())
        .bind(relative_path.as_str())
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "INSERT INTO instance_settings (instance_id, created_at, updated_at) VALUES (?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "INSERT INTO instance_configuration \
             (instance_id, minecraft_version, loader_kind, loader_version, memory_mb, setup_state) \
             VALUES (?, ?, ?, ?, ?, 'configured')",
        )
        .bind(id.to_string())
        .bind(instance.minecraft_version.trim())
        .bind(instance.loader_kind.as_storage_value())
        .bind(instance.loader_version.as_deref().map(str::trim))
        .bind(i64::from(instance.memory_mb))
        .execute(&mut *transaction)
        .await?;

        if let Some(source) = &instance.modpack_source {
            sqlx::query(
                "INSERT INTO instance_modpacks \
                 (instance_id, provider, project_id, version_id, selected_optional_json, \
                  display_name, icon_url, banner_url, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id.to_string())
            .bind(source.provider.as_str())
            .bind(source.project_id.trim())
            .bind(source.version_id.trim())
            .bind(serde_json::to_string(&source.selected_optional)?)
            .bind(source.display_name.trim())
            .bind(source.icon_url.as_deref())
            .bind(source.banner_url.as_deref())
            .bind(&now)
            .bind(&now)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        self.get_instance(id).await
    }

    pub async fn get_instance(&self, id: InstanceId) -> Result<InstanceRecord, StorageError> {
        let row = sqlx::query(
            "SELECT i.id, i.name, i.mode, i.management_mode, i.root_id, i.relative_path, \
             r.canonical_path AS root_path, \
             i.favorite, i.revision, i.created_at, i.updated_at, i.preferred_account_id, \
             c.minecraft_version, \
             c.loader_kind, c.loader_version, c.memory_mb, c.setup_state, \
             g.name AS group_name, s.description AS settings_description, \
             s.notes AS settings_notes, s.tags_json AS settings_tags_json, \
             s.icon_mime AS settings_icon_mime, s.banner_mime AS settings_banner_mime, \
             s.banner_position_x AS settings_banner_position_x, \
             s.banner_position_y AS settings_banner_position_y, \
             s.window_mode AS settings_window_mode, \
             s.resolution_width AS settings_resolution_width, \
             s.resolution_height AS settings_resolution_height, \
             s.launcher_behavior AS settings_launcher_behavior, \
             s.game_language AS settings_game_language, \
             s.quick_play_server AS settings_quick_play_server, \
             s.process_priority AS settings_process_priority, \
             s.cpu_affinity_json AS settings_cpu_affinity_json, \
             s.memory_mode AS settings_memory_mode, \
             s.initial_memory_mb AS settings_initial_memory_mb, \
             s.java_mode AS settings_java_mode, s.custom_java_path AS settings_custom_java_path, \
             s.custom_java_label AS settings_custom_java_label, \
             s.performance_preset AS settings_performance_preset, \
             s.jvm_arguments_json AS settings_jvm_arguments_json, \
             s.environment_json AS settings_environment_json, \
             s.backup_before_changes AS settings_backup_before_changes, \
             s.backup_retention AS settings_backup_retention, \
             s.log_retention_days AS settings_log_retention_days, \
             (SELECT COUNT(DISTINCT im.file_path) FROM instance_mods im \
              WHERE im.instance_id = i.id AND im.enabled = 1) AS mod_count, \
             m.provider AS modpack_provider, m.project_id AS modpack_project_id, \
             m.version_id AS modpack_version_id, m.selected_optional_json, \
             m.display_name AS modpack_display_name, m.icon_url AS modpack_icon_url, \
             m.banner_url AS modpack_banner_url, \
             (SELECT MAX(s.started_at) FROM sessions s WHERE s.instance_id = i.id \
              AND s.state IN ('running', 'exited', 'crashed', 'cancelled')) AS last_played \
             FROM instances i INNER JOIN storage_roots r ON r.id = i.root_id \
             INNER JOIN instance_configuration c ON c.instance_id = i.id \
             INNER JOIN instance_settings s ON s.instance_id = i.id \
             LEFT JOIN instance_groups g ON g.id = i.group_id \
             LEFT JOIN instance_modpacks m ON m.instance_id = i.id \
             WHERE i.id = ? AND i.trashed_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::InstanceNotFound)?;

        row_to_instance(&row)
    }

    pub async fn list_instances(&self, limit: u32) -> Result<Vec<InstanceRecord>, StorageError> {
        if !(1..=1_000).contains(&limit) {
            return Err(StorageError::InvalidPageLimit);
        }

        let rows = sqlx::query(
            "SELECT i.id, i.name, i.mode, i.management_mode, i.root_id, i.relative_path, \
             r.canonical_path AS root_path, \
             i.favorite, i.revision, i.created_at, i.updated_at, i.preferred_account_id, \
             c.minecraft_version, \
             c.loader_kind, c.loader_version, c.memory_mb, c.setup_state, \
             g.name AS group_name, s.description AS settings_description, \
             s.notes AS settings_notes, s.tags_json AS settings_tags_json, \
             s.icon_mime AS settings_icon_mime, s.banner_mime AS settings_banner_mime, \
             s.banner_position_x AS settings_banner_position_x, \
             s.banner_position_y AS settings_banner_position_y, \
             s.window_mode AS settings_window_mode, \
             s.resolution_width AS settings_resolution_width, \
             s.resolution_height AS settings_resolution_height, \
             s.launcher_behavior AS settings_launcher_behavior, \
             s.game_language AS settings_game_language, \
             s.quick_play_server AS settings_quick_play_server, \
             s.process_priority AS settings_process_priority, \
             s.cpu_affinity_json AS settings_cpu_affinity_json, \
             s.memory_mode AS settings_memory_mode, \
             s.initial_memory_mb AS settings_initial_memory_mb, \
             s.java_mode AS settings_java_mode, s.custom_java_path AS settings_custom_java_path, \
             s.custom_java_label AS settings_custom_java_label, \
             s.performance_preset AS settings_performance_preset, \
             s.jvm_arguments_json AS settings_jvm_arguments_json, \
             s.environment_json AS settings_environment_json, \
             s.backup_before_changes AS settings_backup_before_changes, \
             s.backup_retention AS settings_backup_retention, \
             s.log_retention_days AS settings_log_retention_days, \
             (SELECT COUNT(DISTINCT im.file_path) FROM instance_mods im \
              WHERE im.instance_id = i.id AND im.enabled = 1) AS mod_count, \
             m.provider AS modpack_provider, m.project_id AS modpack_project_id, \
             m.version_id AS modpack_version_id, m.selected_optional_json, \
             m.display_name AS modpack_display_name, m.icon_url AS modpack_icon_url, \
             m.banner_url AS modpack_banner_url, \
             (SELECT MAX(s.started_at) FROM sessions s WHERE s.instance_id = i.id \
              AND s.state IN ('running', 'exited', 'crashed', 'cancelled')) AS last_played \
             FROM instances i INNER JOIN storage_roots r ON r.id = i.root_id \
             INNER JOIN instance_configuration c ON c.instance_id = i.id \
             INNER JOIN instance_settings s ON s.instance_id = i.id \
             LEFT JOIN instance_groups g ON g.id = i.group_id \
             LEFT JOIN instance_modpacks m ON m.instance_id = i.id \
             WHERE i.trashed_at IS NULL \
             ORDER BY i.favorite DESC, i.name COLLATE NOCASE, i.id LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_instance).collect()
    }

    pub async fn rename_instance(
        &self,
        id: InstanceId,
        name: &InstanceName,
        expected_revision: u64,
    ) -> Result<InstanceRecord, StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let result = sqlx::query(
            "UPDATE instances SET name = ?, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(name.as_str())
        .bind(now_rfc3339()?)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&self.pool)
        .await?;

        self.ensure_updated(id, expected_revision, result.rows_affected())
            .await?;
        self.get_instance(id).await
    }

    pub async fn update_instance_configuration(
        &self,
        id: InstanceId,
        minecraft_version: &str,
        loader_kind: LoaderFamily,
        loader_version: Option<&str>,
        memory_mb: u32,
        expected_revision: u64,
    ) -> Result<InstanceRecord, StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE instances SET revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(&now)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&mut *transaction)
        .await?;

        if result.rows_affected() == 0 {
            transaction.rollback().await?;
            return self.revision_error(id, expected_revision).await;
        }

        let current = sqlx::query(
            "SELECT minecraft_version, loader_kind, loader_version \
             FROM instance_configuration WHERE instance_id = ?",
        )
        .bind(id.to_string())
        .fetch_one(&mut *transaction)
        .await?;
        let current_minecraft_version: String = current.try_get("minecraft_version")?;
        let current_loader_kind: String = current.try_get("loader_kind")?;
        let current_loader_version: Option<String> = current.try_get("loader_version")?;
        let minecraft_version = minecraft_version.trim();
        let loader_version = loader_version.map(str::trim);
        let runtime_changed = current_minecraft_version != minecraft_version
            || current_loader_kind != loader_kind.as_storage_value()
            || current_loader_version.as_deref() != loader_version;

        if runtime_changed {
            sqlx::query(
                "UPDATE instance_configuration SET minecraft_version = ?, loader_kind = ?, \
                 loader_version = ?, memory_mb = ?, setup_state = 'configured' \
                 WHERE instance_id = ?",
            )
            .bind(minecraft_version)
            .bind(loader_kind.as_storage_value())
            .bind(loader_version)
            .bind(i64::from(memory_mb))
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await?;
            // A manually changed runtime can no longer claim the source pack's exact compatibility.
            sqlx::query("DELETE FROM instance_modpacks WHERE instance_id = ?")
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM instance_mods WHERE instance_id = ?")
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await?;
        } else {
            sqlx::query("UPDATE instance_configuration SET memory_mb = ? WHERE instance_id = ?")
                .bind(i64::from(memory_mb))
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        self.get_instance(id).await
    }

    pub async fn set_instance_favorite(
        &self,
        id: InstanceId,
        favorite: bool,
        expected_revision: u64,
    ) -> Result<InstanceRecord, StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let result = sqlx::query(
            "UPDATE instances SET favorite = ?, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(i64::from(favorite))
        .bind(now_rfc3339()?)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&self.pool)
        .await?;

        self.ensure_updated(id, expected_revision, result.rows_affected())
            .await?;
        self.get_instance(id).await
    }

    pub async fn relocate_instance(
        &self,
        id: InstanceId,
        root_id: StorageRootId,
        relative_path: &ManagedRelativePath,
        expected_revision: u64,
    ) -> Result<InstanceRecord, StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let result = sqlx::query(
            "UPDATE instances SET root_id = ?, relative_path = ?, revision = revision + 1, \
             updated_at = ? WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(root_id.to_string())
        .bind(relative_path.as_str())
        .bind(now_rfc3339()?)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&self.pool)
        .await?;

        self.ensure_updated(id, expected_revision, result.rows_affected())
            .await?;
        self.get_instance(id).await
    }

    pub async fn trash_instance(
        &self,
        id: InstanceId,
        expected_revision: u64,
    ) -> Result<(), StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE instances SET trashed_at = ?, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(&now)
        .bind(&now)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            transaction.rollback().await?;
            return self.revision_error(id, expected_revision).await;
        }
        sqlx::query(
            "UPDATE jobs SET state = 'cancelled', phase = 'cancelled', \
             progress_json = json_set(progress_json, '$.message', \
                 'Installation stopped because the instance was moved to trash.', \
                 '$.completedItems', NULL, '$.totalItems', NULL), updated_at = ? \
             WHERE kind = 'instance_install' AND entity_id = ? \
                 AND state IN ('queued', 'running')",
        )
        .bind(&now)
        .bind(id.to_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE instance_revisions SET status = 'failed' WHERE instance_id = ? \
             AND status IN ('proposed', 'staged')",
        )
        .bind(id.to_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn list_trashed_instances(
        &self,
        limit: u32,
    ) -> Result<Vec<TrashedInstanceRecord>, StorageError> {
        if !(1..=1_000).contains(&limit) {
            return Err(StorageError::InvalidPageLimit);
        }
        let rows = sqlx::query(
            "SELECT i.id, i.name, i.revision, i.trashed_at, i.relative_path, \
             r.canonical_path AS root_path, c.minecraft_version, c.loader_kind, \
             c.loader_version, m.display_name AS source_name, m.icon_url \
             FROM instances i INNER JOIN storage_roots r ON r.id = i.root_id \
             INNER JOIN instance_configuration c ON c.instance_id = i.id \
             LEFT JOIN instance_modpacks m ON m.instance_id = i.id \
             WHERE i.trashed_at IS NOT NULL \
             ORDER BY i.trashed_at DESC, i.id LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(trashed_instance_from_row).collect()
    }

    pub async fn get_trashed_instance(
        &self,
        id: InstanceId,
    ) -> Result<TrashedInstanceRecord, StorageError> {
        let row = sqlx::query(
            "SELECT i.id, i.name, i.revision, i.trashed_at, i.relative_path, \
             r.canonical_path AS root_path, c.minecraft_version, c.loader_kind, \
             c.loader_version, m.display_name AS source_name, m.icon_url \
             FROM instances i INNER JOIN storage_roots r ON r.id = i.root_id \
             INNER JOIN instance_configuration c ON c.instance_id = i.id \
             LEFT JOIN instance_modpacks m ON m.instance_id = i.id \
             WHERE i.id = ? AND i.trashed_at IS NOT NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::InstanceNotFound)?;
        trashed_instance_from_row(&row)
    }

    pub async fn restore_trashed_instance(
        &self,
        id: InstanceId,
        expected_revision: u64,
    ) -> Result<InstanceRecord, StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let result = sqlx::query(
            "UPDATE instances SET trashed_at = NULL, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NOT NULL",
        )
        .bind(now_rfc3339()?)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return self.trashed_revision_error(id, expected_revision).await;
        }
        self.get_instance(id).await
    }

    pub async fn permanently_delete_trashed_instance_record(
        &self,
        id: InstanceId,
        expected_revision: u64,
    ) -> Result<(), StorageError> {
        self.permanently_delete_trashed_instance_records(&[(id, expected_revision)])
            .await
    }

    pub async fn permanently_delete_trashed_instance_records(
        &self,
        records: &[(InstanceId, u64)],
    ) -> Result<(), StorageError> {
        let mut transaction = self.pool.begin().await?;
        let mut validated = Vec::with_capacity(records.len());
        for (id, expected_revision) in records {
            let requested_revision = *expected_revision;
            let expected_revision = revision_to_i64(requested_revision)?;
            let current: Option<i64> = sqlx::query_scalar(
                "SELECT revision FROM instances WHERE id = ? AND trashed_at IS NOT NULL",
            )
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await?;
            let Some(current) = current else {
                transaction.rollback().await?;
                return Err(StorageError::InstanceNotFound);
            };
            if current != expected_revision {
                transaction.rollback().await?;
                return Err(StorageError::RevisionConflict {
                    expected: requested_revision,
                });
            }
            validated.push((*id, expected_revision));
        }

        for (id, expected_revision) in validated {
            sqlx::query(
                "DELETE FROM session_events WHERE session_id IN \
                 (SELECT id FROM sessions WHERE instance_id = ?)",
            )
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await?;
            sqlx::query("DELETE FROM sessions WHERE instance_id = ?")
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await?;
            sqlx::query(
                "DELETE FROM job_steps WHERE job_id IN \
                 (SELECT id FROM jobs WHERE entity_id = ?)",
            )
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await?;
            sqlx::query("DELETE FROM jobs WHERE entity_id = ?")
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await?;
            sqlx::query("UPDATE instances SET active_revision_id = NULL WHERE id = ?")
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await?;
            let deleted = sqlx::query(
                "DELETE FROM instances WHERE id = ? AND revision = ? AND trashed_at IS NOT NULL",
            )
            .bind(id.to_string())
            .bind(expected_revision)
            .execute(&mut *transaction)
            .await?;
            if deleted.rows_affected() != 1 {
                transaction.rollback().await?;
                return Err(StorageError::RevisionConflict {
                    expected: u64::try_from(expected_revision)
                        .map_err(|_| StorageError::NegativeRevision)?,
                });
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn mark_active_instances_for_repair(&self) -> Result<u64, StorageError> {
        Ok(sqlx::query(
            "UPDATE instance_configuration SET setup_state = 'configured' \
             WHERE instance_id IN (SELECT id FROM instances WHERE trashed_at IS NULL)",
        )
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    async fn trashed_revision_error<T>(
        &self,
        id: InstanceId,
        expected_revision: i64,
    ) -> Result<T, StorageError> {
        let current: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM instances WHERE id = ? AND trashed_at IS NOT NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        match current {
            Some(_) => Err(StorageError::RevisionConflict {
                expected: u64::try_from(expected_revision)
                    .map_err(|_| StorageError::NegativeRevision)?,
            }),
            None => Err(StorageError::InstanceNotFound),
        }
    }

    async fn ensure_updated(
        &self,
        id: InstanceId,
        expected_revision: i64,
        rows_affected: u64,
    ) -> Result<(), StorageError> {
        if rows_affected == 0 {
            return self.revision_error(id, expected_revision).await;
        }
        Ok(())
    }

    pub(crate) async fn revision_error<T>(
        &self,
        id: InstanceId,
        expected_revision: i64,
    ) -> Result<T, StorageError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE id = ? AND trashed_at IS NULL)",
        )
        .bind(id.to_string())
        .fetch_one(&self.pool)
        .await?;

        if exists {
            Err(StorageError::RevisionConflict {
                expected: u64::try_from(expected_revision)
                    .map_err(|_| StorageError::NegativeRevision)?,
            })
        } else {
            Err(StorageError::InstanceNotFound)
        }
    }
}

fn row_to_instance(row: &sqlx::sqlite::SqliteRow) -> Result<InstanceRecord, StorageError> {
    let id = parse_uuid(row.try_get("id")?, "instances.id")?;
    let root_id = parse_uuid(row.try_get("root_id")?, "instances.root_id")?;
    let mode_value: String = row.try_get("mode")?;
    let management_value: String = row.try_get("management_mode")?;
    let name_value: String = row.try_get("name")?;
    let relative_path: String = row.try_get("relative_path")?;
    let root_path: String = row.try_get("root_path")?;
    let loader_value: String = row.try_get("loader_kind")?;
    let setup_state_value: String = row.try_get("setup_state")?;
    let revision: i64 = row.try_get("revision")?;
    let memory_mb: i64 = row.try_get("memory_mb")?;
    let mod_count: i64 = row.try_get("mod_count")?;
    let favorite: i64 = row.try_get("favorite")?;
    let modpack_provider: Option<String> = row.try_get("modpack_provider")?;
    let modpack_source = modpack_provider
        .map(|provider| -> Result<ModpackSourceRecord, StorageError> {
            Ok(ModpackSourceRecord {
                provider: Provider::from_str(&provider)
                    .map_err(|_| invalid_value("instance_modpacks.provider", provider))?,
                project_id: row.try_get("modpack_project_id")?,
                version_id: row.try_get("modpack_version_id")?,
                selected_optional: serde_json::from_str(
                    &row.try_get::<String, _>("selected_optional_json")?,
                )?,
                display_name: row.try_get("modpack_display_name")?,
                icon_url: row.try_get("modpack_icon_url")?,
                banner_url: row.try_get("modpack_banner_url")?,
            })
        })
        .transpose()?;

    let relative_path = ManagedRelativePath::parse(&relative_path)?;
    let storage_path = relative_path.resolve_under(std::path::Path::new(&root_path));
    Ok(InstanceRecord {
        id: InstanceId::from_uuid(id),
        name: InstanceName::parse(name_value)
            .map_err(|_| invalid_value("instances.name", "invalid instance name".to_owned()))?,
        mode: InstanceMode::try_from(mode_value.as_str())
            .map_err(|_| invalid_value("instances.mode", mode_value))?,
        management_mode: ManagementMode::try_from(management_value.as_str())
            .map_err(|_| invalid_value("instances.management_mode", management_value))?,
        root_id: StorageRootId::from_uuid(root_id),
        relative_path,
        storage_path,
        favorite: favorite != 0,
        revision: u64::try_from(revision).map_err(|_| StorageError::NegativeRevision)?,
        minecraft_version: row.try_get("minecraft_version")?,
        loader_kind: LoaderFamily::try_from(loader_value.as_str())
            .map_err(|_| invalid_value("instance_configuration.loader_kind", loader_value))?,
        loader_version: row.try_get("loader_version")?,
        memory_mb: u32::try_from(memory_mb).map_err(|_| {
            invalid_value("instance_configuration.memory_mb", memory_mb.to_string())
        })?,
        mod_count: u32::try_from(mod_count)
            .map_err(|_| invalid_value("instance_mods.mod_count", mod_count.to_string()))?,
        setup_state: InstanceSetupState::try_from(setup_state_value.as_str())
            .map_err(|_| invalid_value("instance_configuration.setup_state", setup_state_value))?,
        settings: settings_from_row(row)?,
        modpack_source,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        last_played: row.try_get("last_played")?,
    })
}

fn trashed_instance_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<TrashedInstanceRecord, StorageError> {
    let id = parse_uuid(row.try_get("id")?, "instances.id")?;
    let name: String = row.try_get("name")?;
    let revision: i64 = row.try_get("revision")?;
    let relative_path: String = row.try_get("relative_path")?;
    let root_path: String = row.try_get("root_path")?;
    let loader_kind: String = row.try_get("loader_kind")?;
    let relative_path = ManagedRelativePath::parse(&relative_path)?;
    Ok(TrashedInstanceRecord {
        id: InstanceId::from_uuid(id),
        name: InstanceName::parse(name)
            .map_err(|_| invalid_value("instances.name", "invalid instance name".to_owned()))?,
        revision: u64::try_from(revision).map_err(|_| StorageError::NegativeRevision)?,
        storage_path: relative_path.resolve_under(std::path::Path::new(&root_path)),
        minecraft_version: row.try_get("minecraft_version")?,
        loader_kind: LoaderFamily::try_from(loader_kind.as_str())
            .map_err(|_| invalid_value("instance_configuration.loader_kind", loader_kind))?,
        loader_version: row.try_get("loader_version")?,
        source_name: row.try_get("source_name")?,
        icon_url: row.try_get("icon_url")?,
        trashed_at: row.try_get("trashed_at")?,
    })
}

fn invalid_value(field: &'static str, value: String) -> StorageError {
    StorageError::InvalidStoredValue { field, value }
}

fn revision_to_i64(revision: u64) -> Result<i64, StorageError> {
    i64::try_from(revision).map_err(|_| StorageError::RevisionConflict { expected: revision })
}

fn parse_uuid(value: String, field: &'static str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(&value).map_err(|source| StorageError::InvalidStoredId { field, source })
}

#[cfg(test)]
mod tests;
