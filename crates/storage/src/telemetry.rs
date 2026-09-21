use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_modpack_api_contracts::CaptureProductEventRequest;
use sqlx::Row;
use uuid::Uuid;

const MAX_QUEUED_EVENTS: i64 = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedProductEvent {
    pub id: Uuid,
    pub request: CaptureProductEventRequest,
}

impl Database {
    pub async fn get_or_create_telemetry_installation_id(&self) -> Result<Uuid, StorageError> {
        let candidate = Uuid::new_v4();
        sqlx::query(
            "INSERT OR IGNORE INTO telemetry_identity (singleton_id, installation_id, created_at) \
             VALUES (1, ?, ?)",
        )
        .bind(candidate.to_string())
        .bind(now_rfc3339()?)
        .execute(&self.pool)
        .await?;
        let value: String = sqlx::query_scalar(
            "SELECT installation_id FROM telemetry_identity WHERE singleton_id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Uuid::parse_str(&value).map_err(|source| StorageError::InvalidStoredId {
            field: "telemetry_identity.installation_id",
            source,
        })
    }

    pub async fn enqueue_product_event(
        &self,
        request: &CaptureProductEventRequest,
    ) -> Result<(), StorageError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO telemetry_queue (id, payload_json, attempts, created_at) VALUES (?, ?, 0, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(serde_json::to_string(request)?)
        .bind(now_rfc3339()?)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "DELETE FROM telemetry_queue WHERE id IN (\
             SELECT id FROM telemetry_queue ORDER BY created_at DESC, id DESC LIMIT -1 OFFSET ?\
             )",
        )
        .bind(MAX_QUEUED_EVENTS)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn queued_product_events(
        &self,
        limit: u32,
    ) -> Result<Vec<QueuedProductEvent>, StorageError> {
        if !(1..=100).contains(&limit) {
            return Err(StorageError::InvalidPageLimit);
        }
        let rows = sqlx::query(
            "SELECT id, payload_json FROM telemetry_queue ORDER BY created_at, id LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                let payload: String = row.try_get("payload_json")?;
                Ok(QueuedProductEvent {
                    id: Uuid::parse_str(&id).map_err(|source| StorageError::InvalidStoredId {
                        field: "telemetry_queue.id",
                        source,
                    })?,
                    request: serde_json::from_str(&payload)?,
                })
            })
            .collect()
    }

    pub async fn remove_product_event(&self, id: Uuid) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM telemetry_queue WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_product_event_attempt(&self, id: Uuid) -> Result<(), StorageError> {
        sqlx::query("UPDATE telemetry_queue SET attempts = attempts + 1 WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn clear_product_events(&self) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM telemetry_queue")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MAX_QUEUED_EVENTS;
    use crate::Database;
    use slate_modpack_api_contracts::{CaptureProductEventRequest, ProductEvent, ProductPlatform};

    #[tokio::test]
    async fn identity_is_stable_and_queue_is_bounded() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let installation_id = database.get_or_create_telemetry_installation_id().await?;
        assert_eq!(
            installation_id,
            database.get_or_create_telemetry_installation_id().await?
        );
        let request = CaptureProductEventRequest {
            installation_id,
            event: ProductEvent::LauncherStarted,
            app_version: "0.1.0".to_owned(),
            platform: ProductPlatform::Windows,
            architecture: "x86_64".to_owned(),
            loader: None,
            provider: None,
        };
        for _ in 0..=MAX_QUEUED_EVENTS {
            database.enqueue_product_event(&request).await?;
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM telemetry_queue")
            .fetch_one(&database.pool)
            .await?;
        assert_eq!(count, MAX_QUEUED_EVENTS);
        let first = database.queued_product_events(1).await?;
        assert_eq!(first.first().map(|entry| &entry.request), Some(&request));
        database.clear_product_events().await?;
        assert!(database.queued_product_events(10).await?.is_empty());
        database.close().await;
        Ok(())
    }
}
