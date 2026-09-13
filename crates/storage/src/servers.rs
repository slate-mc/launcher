use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_domain::{InstanceId, ServerId};
use sqlx::Row;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSavedServer {
    pub name: String,
    pub address: String,
    pub preferred_instance_id: Option<InstanceId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedServerRecord {
    pub id: ServerId,
    pub name: String,
    pub address: String,
    pub preferred_instance_id: Option<InstanceId>,
    pub created_at: String,
    pub updated_at: String,
}

impl Database {
    pub async fn create_saved_server(
        &self,
        server: NewSavedServer,
    ) -> Result<SavedServerRecord, StorageError> {
        let id = ServerId::new();
        let now = now_rfc3339()?;
        sqlx::query(
            "INSERT INTO saved_servers \
             (id, name, address, preferred_instance_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(server.name.trim())
        .bind(server.address.trim())
        .bind(server.preferred_instance_id.map(|value| value.to_string()))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_saved_server(id).await
    }

    pub async fn list_saved_servers(
        &self,
        limit: u32,
    ) -> Result<Vec<SavedServerRecord>, StorageError> {
        if !(1..=1_000).contains(&limit) {
            return Err(StorageError::InvalidPageLimit);
        }
        let rows = sqlx::query(
            "SELECT id, name, address, preferred_instance_id, created_at, updated_at \
             FROM saved_servers ORDER BY name COLLATE NOCASE, id LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_server).collect()
    }

    pub async fn remove_saved_server(&self, id: ServerId) -> Result<(), StorageError> {
        let result = sqlx::query("DELETE FROM saved_servers WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::ServerNotFound);
        }
        Ok(())
    }

    async fn get_saved_server(&self, id: ServerId) -> Result<SavedServerRecord, StorageError> {
        let row = sqlx::query(
            "SELECT id, name, address, preferred_instance_id, created_at, updated_at \
             FROM saved_servers WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::ServerNotFound)?;
        row_to_server(&row)
    }
}

fn row_to_server(row: &sqlx::sqlite::SqliteRow) -> Result<SavedServerRecord, StorageError> {
    let id: String = row.try_get("id")?;
    let preferred_instance_id: Option<String> = row.try_get("preferred_instance_id")?;
    Ok(SavedServerRecord {
        id: ServerId::from_uuid(parse_uuid(id, "saved_servers.id")?),
        name: row.try_get("name")?,
        address: row.try_get("address")?,
        preferred_instance_id: preferred_instance_id
            .map(|value| parse_uuid(value, "saved_servers.preferred_instance_id"))
            .transpose()?
            .map(InstanceId::from_uuid),
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn parse_uuid(value: String, field: &'static str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(&value).map_err(|source| StorageError::InvalidStoredId { field, source })
}

#[cfg(test)]
mod tests {
    use super::NewSavedServer;
    use crate::{Database, StorageError};

    #[tokio::test]
    async fn saved_server_roundtrip_and_remove() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let created = database
            .create_saved_server(NewSavedServer {
                name: "Home server".to_owned(),
                address: "play.example.test:25565".to_owned(),
                preferred_instance_id: None,
            })
            .await?;

        assert_eq!(
            database.list_saved_servers(20).await?,
            vec![created.clone()]
        );
        database.remove_saved_server(created.id).await?;
        assert!(database.list_saved_servers(20).await?.is_empty());
        assert!(matches!(
            database.remove_saved_server(created.id).await,
            Err(StorageError::ServerNotFound)
        ));
        database.close().await;
        Ok(())
    }
}
