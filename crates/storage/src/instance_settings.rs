use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_domain::{AccountId, InstanceId};
use sqlx::Row;
use std::collections::BTreeMap;
use uuid::Uuid;

macro_rules! storage_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            #[must_use]
            pub const fn as_storage_value(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }

        impl TryFrom<&str> for $name {
            type Error = ();

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(()),
                }
            }
        }
    };
}

storage_enum!(InstanceWindowMode {
    Windowed => "windowed",
    Maximized => "maximized",
    Fullscreen => "fullscreen",
});
storage_enum!(LauncherBehavior {
    KeepOpen => "keep_open",
    Minimize => "minimize",
    Hide => "hide",
});
storage_enum!(ProcessPriority {
    Low => "low",
    BelowNormal => "below_normal",
    Normal => "normal",
    AboveNormal => "above_normal",
    High => "high",
});
storage_enum!(MemoryMode {
    Auto => "auto",
    Custom => "custom",
});
storage_enum!(JavaSelectionMode {
    Managed => "managed",
    Detected => "detected",
    Custom => "custom",
});
storage_enum!(PerformancePreset {
    Balanced => "balanced",
    Throughput => "throughput",
    LowLatency => "low_latency",
    Custom => "custom",
});

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceSettingsRecord {
    pub description: String,
    pub notes: String,
    pub group_name: Option<String>,
    pub tags: Vec<String>,
    pub icon_mime: Option<String>,
    pub banner_mime: Option<String>,
    pub banner_position_x: u8,
    pub banner_position_y: u8,
    pub preferred_account_id: Option<AccountId>,
    pub window_mode: InstanceWindowMode,
    pub resolution_width: Option<u32>,
    pub resolution_height: Option<u32>,
    pub launcher_behavior: LauncherBehavior,
    pub game_language: String,
    pub quick_play_server: Option<String>,
    pub process_priority: ProcessPriority,
    pub memory_mode: MemoryMode,
    pub initial_memory_mb: u32,
    pub java_mode: JavaSelectionMode,
    pub custom_java_path: Option<String>,
    pub custom_java_label: Option<String>,
    pub performance_preset: PerformancePreset,
    pub jvm_arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub backup_before_changes: bool,
    pub backup_retention: u8,
    pub log_retention_days: u16,
}

#[derive(Clone, Debug)]
pub struct UpdateInstanceSettings {
    pub description: String,
    pub notes: String,
    pub group_name: Option<String>,
    pub tags: Vec<String>,
    pub preferred_account_id: Option<AccountId>,
    pub banner_position_x: u8,
    pub banner_position_y: u8,
    pub window_mode: InstanceWindowMode,
    pub resolution_width: Option<u32>,
    pub resolution_height: Option<u32>,
    pub launcher_behavior: LauncherBehavior,
    pub game_language: String,
    pub quick_play_server: Option<String>,
    pub process_priority: ProcessPriority,
    pub memory_mode: MemoryMode,
    pub initial_memory_mb: u32,
    pub maximum_memory_mb: u32,
    pub java_mode: JavaSelectionMode,
    pub performance_preset: PerformancePreset,
    pub jvm_arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub backup_before_changes: bool,
    pub backup_retention: u8,
    pub log_retention_days: u16,
}

impl Database {
    pub async fn copy_instance_profile(
        &self,
        source_id: InstanceId,
        destination_id: InstanceId,
    ) -> Result<(), StorageError> {
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE instances SET group_id = (SELECT group_id FROM instances WHERE id = ?), \
             preferred_account_id = (SELECT preferred_account_id FROM instances WHERE id = ?), \
             updated_at = ? WHERE id = ? AND trashed_at IS NULL",
        )
        .bind(source_id.to_string())
        .bind(source_id.to_string())
        .bind(&now)
        .bind(destination_id.to_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM instance_settings WHERE instance_id = ?")
            .bind(destination_id.to_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO instance_settings \
             (instance_id, description, notes, tags_json, icon_mime, banner_mime, \
              banner_position_x, banner_position_y, window_mode, resolution_width, \
              resolution_height, launcher_behavior, game_language, quick_play_server, \
              process_priority, memory_mode, initial_memory_mb, java_mode, custom_java_path, \
              custom_java_label, performance_preset, jvm_arguments_json, environment_json, \
              backup_before_changes, backup_retention, log_retention_days, created_at, updated_at) \
             SELECT ?, description, notes, tags_json, icon_mime, banner_mime, banner_position_x, \
              banner_position_y, window_mode, resolution_width, resolution_height, \
              launcher_behavior, game_language, quick_play_server, process_priority, memory_mode, \
              initial_memory_mb, java_mode, custom_java_path, custom_java_label, \
              performance_preset, jvm_arguments_json, environment_json, backup_before_changes, \
              backup_retention, log_retention_days, ?, ? FROM instance_settings WHERE instance_id = ?",
        )
        .bind(destination_id.to_string())
        .bind(&now)
        .bind(&now)
        .bind(source_id.to_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn advance_instance_revision(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
    ) -> Result<(), StorageError> {
        let stored_revision =
            i64::try_from(expected_revision).map_err(|_| StorageError::RevisionConflict {
                expected: expected_revision,
            })?;
        let updated = sqlx::query(
            "UPDATE instances SET revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(now_rfc3339()?)
        .bind(instance_id.to_string())
        .bind(stored_revision)
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() == 0 {
            return self.revision_error(instance_id, stored_revision).await;
        }
        Ok(())
    }

    pub async fn update_instance_settings(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        settings: UpdateInstanceSettings,
    ) -> Result<(), StorageError> {
        let expected_revision =
            i64::try_from(expected_revision).map_err(|_| StorageError::RevisionConflict {
                expected: expected_revision,
            })?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let group_id = if let Some(group_name) = settings.group_name.as_deref() {
            let existing = sqlx::query_scalar::<_, String>(
                "SELECT id FROM instance_groups WHERE name = ? COLLATE NOCASE AND archived_at IS NULL",
            )
            .bind(group_name)
            .fetch_optional(&mut *transaction)
            .await?;
            if let Some(id) = existing {
                Some(id)
            } else {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO instance_groups (id, name, sort_order, revision, created_at, updated_at) \
                     VALUES (?, ?, 0, 0, ?, ?)",
                )
                .bind(&id)
                .bind(group_name)
                .bind(&now)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
                Some(id)
            }
        } else {
            None
        };
        let updated = sqlx::query(
            "UPDATE instances SET group_id = ?, preferred_account_id = ?, revision = revision + 1, \
             updated_at = ? WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(group_id)
        .bind(settings.preferred_account_id.map(|id| id.to_string()))
        .bind(&now)
        .bind(instance_id.to_string())
        .bind(expected_revision)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            transaction.rollback().await?;
            return self.revision_error(instance_id, expected_revision).await;
        }
        sqlx::query(
            "UPDATE instance_settings SET description = ?, notes = ?, tags_json = ?, \
             banner_position_x = ?, banner_position_y = ?, window_mode = ?, \
             resolution_width = ?, resolution_height = ?, launcher_behavior = ?, \
             game_language = ?, quick_play_server = ?, process_priority = ?, memory_mode = ?, \
             initial_memory_mb = ?, java_mode = ?, performance_preset = ?, \
             jvm_arguments_json = ?, environment_json = ?, backup_before_changes = ?, \
             backup_retention = ?, log_retention_days = ?, updated_at = ? WHERE instance_id = ?",
        )
        .bind(settings.description)
        .bind(settings.notes)
        .bind(serde_json::to_string(&settings.tags)?)
        .bind(i64::from(settings.banner_position_x))
        .bind(i64::from(settings.banner_position_y))
        .bind(settings.window_mode.as_storage_value())
        .bind(settings.resolution_width.map(i64::from))
        .bind(settings.resolution_height.map(i64::from))
        .bind(settings.launcher_behavior.as_storage_value())
        .bind(settings.game_language)
        .bind(settings.quick_play_server)
        .bind(settings.process_priority.as_storage_value())
        .bind(settings.memory_mode.as_storage_value())
        .bind(i64::from(settings.initial_memory_mb))
        .bind(settings.java_mode.as_storage_value())
        .bind(settings.performance_preset.as_storage_value())
        .bind(serde_json::to_string(&settings.jvm_arguments)?)
        .bind(serde_json::to_string(&settings.environment)?)
        .bind(i64::from(settings.backup_before_changes))
        .bind(i64::from(settings.backup_retention))
        .bind(i64::from(settings.log_retention_days))
        .bind(&now)
        .bind(instance_id.to_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query("UPDATE instance_configuration SET memory_mb = ? WHERE instance_id = ?")
            .bind(i64::from(settings.maximum_memory_mb))
            .bind(instance_id.to_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn set_instance_artwork_mime(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        column: &'static str,
        mime: Option<&str>,
    ) -> Result<(), StorageError> {
        let expected_revision =
            i64::try_from(expected_revision).map_err(|_| StorageError::RevisionConflict {
                expected: expected_revision,
            })?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE instances SET revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(&now)
        .bind(instance_id.to_string())
        .bind(expected_revision)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            transaction.rollback().await?;
            return self.revision_error(instance_id, expected_revision).await;
        }
        let statement = match column {
            "icon_mime" => {
                "UPDATE instance_settings SET icon_mime = ?, updated_at = ? WHERE instance_id = ?"
            }
            "banner_mime" => {
                "UPDATE instance_settings SET banner_mime = ?, updated_at = ? WHERE instance_id = ?"
            }
            _ => {
                return Err(StorageError::InvalidStoredValue {
                    field: "instance_settings.artwork_column",
                    value: column.to_owned(),
                });
            }
        };
        sqlx::query(statement)
            .bind(mime)
            .bind(&now)
            .bind(instance_id.to_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn set_instance_java_selection(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        mode: JavaSelectionMode,
        path: Option<&str>,
        label: Option<&str>,
    ) -> Result<(), StorageError> {
        let expected_revision =
            i64::try_from(expected_revision).map_err(|_| StorageError::RevisionConflict {
                expected: expected_revision,
            })?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE instances SET revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(&now)
        .bind(instance_id.to_string())
        .bind(expected_revision)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            transaction.rollback().await?;
            return self.revision_error(instance_id, expected_revision).await;
        }
        sqlx::query(
            "UPDATE instance_settings SET java_mode = ?, custom_java_path = ?, \
             custom_java_label = ?, updated_at = ? WHERE instance_id = ?",
        )
        .bind(mode.as_storage_value())
        .bind(path)
        .bind(label)
        .bind(&now)
        .bind(instance_id.to_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }
}

pub(crate) fn settings_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<InstanceSettingsRecord, StorageError> {
    let preferred_account_id = row
        .try_get::<Option<String>, _>("preferred_account_id")?
        .map(|value| {
            Uuid::parse_str(&value)
                .map(AccountId::from_uuid)
                .map_err(|source| StorageError::InvalidStoredId {
                    field: "instances.preferred_account_id",
                    source,
                })
        })
        .transpose()?;
    Ok(InstanceSettingsRecord {
        description: row.try_get("settings_description")?,
        notes: row.try_get("settings_notes")?,
        group_name: row.try_get("group_name")?,
        tags: serde_json::from_str(&row.try_get::<String, _>("settings_tags_json")?)?,
        icon_mime: row.try_get("settings_icon_mime")?,
        banner_mime: row.try_get("settings_banner_mime")?,
        banner_position_x: bounded_integer(row, "settings_banner_position_x")?,
        banner_position_y: bounded_integer(row, "settings_banner_position_y")?,
        preferred_account_id,
        window_mode: stored_enum(row, "settings_window_mode")?,
        resolution_width: optional_integer(row, "settings_resolution_width")?,
        resolution_height: optional_integer(row, "settings_resolution_height")?,
        launcher_behavior: stored_enum(row, "settings_launcher_behavior")?,
        game_language: row.try_get("settings_game_language")?,
        quick_play_server: row.try_get("settings_quick_play_server")?,
        process_priority: stored_enum(row, "settings_process_priority")?,
        memory_mode: stored_enum(row, "settings_memory_mode")?,
        initial_memory_mb: bounded_integer(row, "settings_initial_memory_mb")?,
        java_mode: stored_enum(row, "settings_java_mode")?,
        custom_java_path: row.try_get("settings_custom_java_path")?,
        custom_java_label: row.try_get("settings_custom_java_label")?,
        performance_preset: stored_enum(row, "settings_performance_preset")?,
        jvm_arguments: serde_json::from_str(
            &row.try_get::<String, _>("settings_jvm_arguments_json")?,
        )?,
        environment: serde_json::from_str(&row.try_get::<String, _>("settings_environment_json")?)?,
        backup_before_changes: row.try_get::<i64, _>("settings_backup_before_changes")? != 0,
        backup_retention: bounded_integer(row, "settings_backup_retention")?,
        log_retention_days: bounded_integer(row, "settings_log_retention_days")?,
    })
}

fn stored_enum<T>(row: &sqlx::sqlite::SqliteRow, field: &'static str) -> Result<T, StorageError>
where
    T: for<'a> TryFrom<&'a str, Error = ()>,
{
    let value: String = row.try_get(field)?;
    T::try_from(value.as_str()).map_err(|()| StorageError::InvalidStoredValue { field, value })
}

fn bounded_integer<T>(row: &sqlx::sqlite::SqliteRow, field: &'static str) -> Result<T, StorageError>
where
    T: TryFrom<i64>,
{
    let value: i64 = row.try_get(field)?;
    T::try_from(value).map_err(|_| StorageError::InvalidStoredValue {
        field,
        value: value.to_string(),
    })
}

fn optional_integer<T>(
    row: &sqlx::sqlite::SqliteRow,
    field: &'static str,
) -> Result<Option<T>, StorageError>
where
    T: TryFrom<i64>,
{
    row.try_get::<Option<i64>, _>(field)?
        .map(|value| {
            T::try_from(value).map_err(|_| StorageError::InvalidStoredValue {
                field,
                value: value.to_string(),
            })
        })
        .transpose()
}
