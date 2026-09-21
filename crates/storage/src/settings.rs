use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use sqlx::Row;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemePreference {
    Dark,
    Light,
    System,
}

impl ThemePreference {
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }
}

impl TryFrom<&str> for ThemePreference {
    type Error = StorageError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "dark" => Ok(Self::Dark),
            "light" => Ok(Self::Light),
            "system" => Ok(Self::System),
            other => Err(StorageError::InvalidStoredValue {
                field: "app_preferences.theme",
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReduceMotionPreference {
    System,
    On,
    Off,
}

impl ReduceMotionPreference {
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

impl TryFrom<&str> for ReduceMotionPreference {
    type Error = StorageError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "system" => Ok(Self::System),
            "on" => Ok(Self::On),
            "off" => Ok(Self::Off),
            other => Err(StorageError::InvalidStoredValue {
                field: "app_preferences.reduce_motion",
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl UpdateChannel {
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

impl TryFrom<&str> for UpdateChannel {
    type Error = StorageError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "stable" => Ok(Self::Stable),
            "beta" => Ok(Self::Beta),
            other => Err(StorageError::InvalidStoredValue {
                field: "app_preferences.update_channel",
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppPreferences {
    pub theme: ThemePreference,
    pub download_concurrency: u8,
    pub download_bandwidth_limit_mib: u32,
    pub telemetry_enabled: bool,
    pub crash_reporting_enabled: bool,
    pub update_channel: UpdateChannel,
    pub reduce_motion: ReduceMotionPreference,
    pub trash_retention_days: u16,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::Dark,
            download_concurrency: 4,
            download_bandwidth_limit_mib: 0,
            telemetry_enabled: false,
            crash_reporting_enabled: false,
            update_channel: UpdateChannel::Stable,
            reduce_motion: ReduceMotionPreference::System,
            trash_retention_days: 30,
        }
    }
}

impl Database {
    pub async fn has_configured_account(&self) -> Result<bool, StorageError> {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts)")
            .fetch_one(&self.pool)
            .await?;
        Ok(exists)
    }

    pub async fn get_app_preferences(&self) -> Result<AppPreferences, StorageError> {
        let row = sqlx::query(
            "SELECT theme, download_concurrency, download_bandwidth_limit_mib, telemetry_enabled, crash_reporting_enabled, update_channel, reduce_motion, trash_retention_days \
             FROM app_preferences WHERE singleton_id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(AppPreferences::default());
        };
        let theme: String = row.try_get("theme")?;
        let reduce_motion: String = row.try_get("reduce_motion")?;
        let download_concurrency: i64 = row.try_get("download_concurrency")?;
        let download_bandwidth_limit_mib: i64 = row.try_get("download_bandwidth_limit_mib")?;
        let telemetry_enabled: i64 = row.try_get("telemetry_enabled")?;
        let crash_reporting_enabled: i64 = row.try_get("crash_reporting_enabled")?;
        let update_channel: String = row.try_get("update_channel")?;
        let trash_retention_days: i64 = row.try_get("trash_retention_days")?;

        Ok(AppPreferences {
            theme: ThemePreference::try_from(theme.as_str())?,
            download_concurrency: u8::try_from(download_concurrency).map_err(|_| {
                StorageError::InvalidStoredValue {
                    field: "app_preferences.download_concurrency",
                    value: download_concurrency.to_string(),
                }
            })?,
            download_bandwidth_limit_mib: u32::try_from(download_bandwidth_limit_mib).map_err(
                |_| StorageError::InvalidStoredValue {
                    field: "app_preferences.download_bandwidth_limit_mib",
                    value: download_bandwidth_limit_mib.to_string(),
                },
            )?,
            telemetry_enabled: telemetry_enabled != 0,
            crash_reporting_enabled: crash_reporting_enabled != 0,
            update_channel: UpdateChannel::try_from(update_channel.as_str())?,
            reduce_motion: ReduceMotionPreference::try_from(reduce_motion.as_str())?,
            trash_retention_days: u16::try_from(trash_retention_days).map_err(|_| {
                StorageError::InvalidStoredValue {
                    field: "app_preferences.trash_retention_days",
                    value: trash_retention_days.to_string(),
                }
            })?,
        })
    }

    pub async fn update_app_preferences(
        &self,
        preferences: AppPreferences,
    ) -> Result<AppPreferences, StorageError> {
        sqlx::query(
            "INSERT INTO app_preferences \
             (singleton_id, theme, download_concurrency, download_bandwidth_limit_mib, telemetry_enabled, crash_reporting_enabled, update_channel, reduce_motion, trash_retention_days, updated_at) \
             VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(singleton_id) DO UPDATE SET theme = excluded.theme, \
             download_concurrency = excluded.download_concurrency, \
             download_bandwidth_limit_mib = excluded.download_bandwidth_limit_mib, \
             telemetry_enabled = excluded.telemetry_enabled, \
             crash_reporting_enabled = excluded.crash_reporting_enabled, \
             update_channel = excluded.update_channel, \
             reduce_motion = excluded.reduce_motion, \
             trash_retention_days = excluded.trash_retention_days, updated_at = excluded.updated_at",
        )
        .bind(preferences.theme.as_storage_value())
        .bind(i64::from(preferences.download_concurrency))
        .bind(i64::from(preferences.download_bandwidth_limit_mib))
        .bind(if preferences.telemetry_enabled { 1_i64 } else { 0 })
        .bind(if preferences.crash_reporting_enabled { 1_i64 } else { 0 })
        .bind(preferences.update_channel.as_storage_value())
        .bind(preferences.reduce_motion.as_storage_value())
        .bind(i64::from(preferences.trash_retention_days))
        .bind(now_rfc3339()?)
        .execute(&self.pool)
        .await?;

        self.get_app_preferences().await
    }
}

#[cfg(test)]
mod tests {
    use super::{AppPreferences, ReduceMotionPreference, ThemePreference, UpdateChannel};
    use crate::Database;

    #[tokio::test]
    async fn preferences_default_and_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;

        assert_eq!(
            database.get_app_preferences().await?,
            AppPreferences::default()
        );
        let expected = AppPreferences {
            theme: ThemePreference::Light,
            download_concurrency: 2,
            download_bandwidth_limit_mib: 25,
            telemetry_enabled: true,
            crash_reporting_enabled: true,
            update_channel: UpdateChannel::Beta,
            reduce_motion: ReduceMotionPreference::On,
            trash_retention_days: 60,
        };
        assert_eq!(database.update_app_preferences(expected).await?, expected);
        database.close().await;
        Ok(())
    }
}
