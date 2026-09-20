use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_domain::StorageRootId;
use sqlx::Row;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnboardingStateRecord {
    pub completed: bool,
    pub default_storage_root_id: Option<StorageRootId>,
}

impl Database {
    pub async fn get_onboarding_state(&self) -> Result<OnboardingStateRecord, StorageError> {
        let row = sqlx::query(
            "SELECT completed, default_storage_root_id FROM onboarding_state WHERE singleton_id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        let completed: i64 = row.try_get("completed")?;
        let default_storage_root_id = row
            .try_get::<Option<String>, _>("default_storage_root_id")?
            .map(|value| {
                uuid::Uuid::parse_str(&value)
                    .map(StorageRootId::from_uuid)
                    .map_err(|source| StorageError::InvalidStoredId {
                        field: "onboarding_state.default_storage_root_id",
                        source,
                    })
            })
            .transpose()?;
        Ok(OnboardingStateRecord {
            completed: completed != 0,
            default_storage_root_id,
        })
    }

    pub async fn set_onboarding_storage_root(
        &self,
        root_id: StorageRootId,
    ) -> Result<OnboardingStateRecord, StorageError> {
        sqlx::query(
            "UPDATE onboarding_state SET default_storage_root_id = ?, updated_at = ? WHERE singleton_id = 1",
        )
        .bind(root_id.to_string())
        .bind(now_rfc3339()?)
        .execute(&self.pool)
        .await?;
        self.get_onboarding_state().await
    }

    pub async fn complete_onboarding(&self) -> Result<OnboardingStateRecord, StorageError> {
        sqlx::query(
            "UPDATE onboarding_state SET completed = 1, updated_at = ? WHERE singleton_id = 1",
        )
        .bind(now_rfc3339()?)
        .execute(&self.pool)
        .await?;
        self.get_onboarding_state().await
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    #[tokio::test]
    async fn fresh_database_tracks_onboarding_and_storage_choice()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let initial = database.get_onboarding_state().await?;
        assert!(!initial.completed);
        assert!(initial.default_storage_root_id.is_none());

        let root = database
            .create_storage_root("C:/slate-custom", None)
            .await?;
        let selected = database.set_onboarding_storage_root(root).await?;
        assert_eq!(selected.default_storage_root_id, Some(root));
        assert!(database.complete_onboarding().await?.completed);
        database.close().await;
        Ok(())
    }
}
