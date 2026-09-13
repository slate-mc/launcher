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
pub struct AppPreferences {
    pub theme: ThemePreference,
    pub download_concurrency: u8,
    pub telemetry_enabled: bool,
    pub reduce_motion: ReduceMotionPreference,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::Dark,
            download_concurrency: 4,
            telemetry_enabled: false,
            reduce_motion: ReduceMotionPreference::System,
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
            "SELECT theme, download_concurrency, telemetry_enabled, reduce_motion \
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
        let telemetry_enabled: i64 = row.try_get("telemetry_enabled")?;

        Ok(AppPreferences {
            theme: ThemePreference::try_from(theme.as_str())?,
            download_concurrency: u8::try_from(download_concurrency).map_err(|_| {
                StorageError::InvalidStoredValue {
                    field: "app_preferences.download_concurrency",
                    value: download_concurrency.to_string(),
                }
            })?,
            telemetry_enabled: telemetry_enabled != 0,
            reduce_motion: ReduceMotionPreference::try_from(reduce_motion.as_str())?,
        })
    }

    pub async fn update_app_preferences(
        &self,
        preferences: AppPreferences,
    ) -> Result<AppPreferences, StorageError> {
        sqlx::query(
            "INSERT INTO app_preferences \
             (singleton_id, theme, download_concurrency, telemetry_enabled, reduce_motion, updated_at) \
             VALUES (1, ?, ?, ?, ?, ?) \
             ON CONFLICT(singleton_id) DO UPDATE SET theme = excluded.theme, \
             download_concurrency = excluded.download_concurrency, \
             telemetry_enabled = excluded.telemetry_enabled, \
             reduce_motion = excluded.reduce_motion, updated_at = excluded.updated_at",
        )
        .bind(preferences.theme.as_storage_value())
        .bind(i64::from(preferences.download_concurrency))
        .bind(if preferences.telemetry_enabled { 1_i64 } else { 0 })
        .bind(preferences.reduce_motion.as_storage_value())
        .bind(now_rfc3339()?)
        .execute(&self.pool)
        .await?;

        self.get_app_preferences().await
    }
}

#[cfg(test)]
mod tests {
    use super::{AppPreferences, ReduceMotionPreference, ThemePreference};
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
            telemetry_enabled: true,
            reduce_motion: ReduceMotionPreference::On,
        };
        assert_eq!(database.update_app_preferences(expected).await?, expected);
        database.close().await;
        Ok(())
    }
}
