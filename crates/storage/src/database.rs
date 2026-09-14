use sqlx::migrate::{MigrateError, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Connection, SqliteConnection, SqlitePool};
use std::path::{Path, PathBuf};
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone, Debug)]
pub struct Database {
    pub(crate) pool: SqlitePool,
}

impl Database {
    pub async fn connect(path: &Path) -> Result<Self, StorageError> {
        ensure_parent_exists(path)?;
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .min_connections(1)
            .max_connections(5)
            .connect_with(options)
            .await?;

        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn close(self) {
        self.pool.close().await;
    }

    pub async fn health_check(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Creates a transactionally consistent copy before a migration that has
    /// been classified as destructive.
    pub async fn backup_existing(path: &Path) -> Result<Option<PathBuf>, StorageError> {
        if !path.is_file() {
            return Ok(None);
        }

        let stamp = OffsetDateTime::now_utc().unix_timestamp_nanos();
        let backup_path = path.with_extension(format!("pre-migration-{stamp}.sqlite"));
        let options = SqliteConnectOptions::new().filename(path);
        let mut source = SqliteConnection::connect_with(&options).await?;
        sqlx::query("VACUUM INTO ?")
            .bind(backup_path.to_string_lossy().as_ref())
            .execute(&mut source)
            .await?;
        source.close().await?;
        Ok(Some(backup_path))
    }
}

fn ensure_parent_exists(path: &Path) -> Result<(), StorageError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| StorageError::DatabaseParentMissing(path.to_path_buf()))?;
    if !parent.is_dir() {
        return Err(StorageError::DatabaseParentMissing(parent.to_path_buf()));
    }
    Ok(())
}

pub(crate) fn now_rfc3339() -> Result<String, StorageError> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database parent directory does not exist: {0}")]
    DatabaseParentMissing(PathBuf),
    #[error("database migration failed")]
    Migration(#[from] MigrateError),
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("failed to format a UTC timestamp")]
    Timestamp(#[from] time::error::Format),
    #[error("stored UUID in {field} is invalid")]
    InvalidStoredId {
        field: &'static str,
        #[source]
        source: uuid::Error,
    },
    #[error("stored value for {field} is invalid: {value}")]
    InvalidStoredValue { field: &'static str, value: String },
    #[error("stored revision is negative")]
    NegativeRevision,
    #[error("stored job progress is outside the supported range")]
    ProgressOutOfRange,
    #[error("instance was not found")]
    InstanceNotFound,
    #[error("saved server was not found")]
    ServerNotFound,
    #[error("Minecraft account was not found")]
    AccountNotFound,
    #[error("instance revision was not found")]
    RevisionNotFound,
    #[error("installation job is no longer active")]
    InstallNoLongerActive,
    #[error("instance does not have an installed revision")]
    InstalledRevisionNotFound,
    #[error("instance revision changed; expected {expected}")]
    RevisionConflict { expected: u64 },
    #[error("page limit must be between 1 and 1000")]
    InvalidPageLimit,
    #[error("managed relative path is invalid")]
    ManagedPath(#[from] slate_platform::PathPolicyError),
}

#[cfg(test)]
mod tests {
    use super::Database;
    use sqlx::Row;

    #[tokio::test]
    async fn enables_required_sqlite_durability_settings() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let database_path = directory.path().join("state.sqlite");
        let database = Database::connect(&database_path).await?;

        let foreign_keys: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(&database.pool)
            .await?
            .try_get(0)?;
        let journal_mode: String = sqlx::query("PRAGMA journal_mode")
            .fetch_one(&database.pool)
            .await?
            .try_get(0)?;

        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode, "wal");
        database.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn creates_a_consistent_backup_of_an_existing_database()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database_path = directory.path().join("state.sqlite");
        let database = Database::connect(&database_path).await?;
        database.close().await;

        let backup = Database::backup_existing(&database_path)
            .await?
            .ok_or("expected a backup path")?;

        assert!(backup.is_file());
        Ok(())
    }
}
