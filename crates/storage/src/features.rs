use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_modpack_api_contracts::LauncherFeatureConfig;
use sqlx::Row;

impl Database {
    pub async fn cached_launcher_feature_config(
        &self,
        now_unix: i64,
    ) -> Result<Option<(LauncherFeatureConfig, i64)>, StorageError> {
        let row = sqlx::query(
            "SELECT config_json, expires_at_unix FROM launcher_feature_config WHERE singleton_id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let expires_at: i64 = row.try_get("expires_at_unix")?;
        if expires_at <= now_unix {
            return Ok(None);
        }
        let config: String = row.try_get("config_json")?;
        Ok(Some((serde_json::from_str(&config)?, expires_at)))
    }

    pub async fn save_launcher_feature_config(
        &self,
        config: &LauncherFeatureConfig,
        now_unix: i64,
    ) -> Result<i64, StorageError> {
        let ttl = i64::from(config.cache_seconds.clamp(60, 3_600));
        let expires_at = now_unix.saturating_add(ttl);
        sqlx::query(
            "INSERT INTO launcher_feature_config \
             (singleton_id, config_json, expires_at_unix, updated_at) VALUES (1, ?, ?, ?) \
             ON CONFLICT(singleton_id) DO UPDATE SET config_json = excluded.config_json, \
             expires_at_unix = excluded.expires_at_unix, updated_at = excluded.updated_at",
        )
        .bind(serde_json::to_string(config)?)
        .bind(expires_at)
        .bind(now_rfc3339()?)
        .execute(&self.pool)
        .await?;
        Ok(expires_at)
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;
    use slate_modpack_api_contracts::LauncherFeatureConfig;

    #[tokio::test]
    async fn feature_config_expires_to_safe_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let config = LauncherFeatureConfig {
            installs_enabled: false,
            cache_seconds: 60,
            ..LauncherFeatureConfig::default()
        };
        assert_eq!(
            database
                .save_launcher_feature_config(&config, 1_000)
                .await?,
            1_060
        );
        assert_eq!(
            database.cached_launcher_feature_config(1_030).await?,
            Some((config, 1_060))
        );
        assert!(
            database
                .cached_launcher_feature_config(1_060)
                .await?
                .is_none()
        );
        database.close().await;
        Ok(())
    }
}
