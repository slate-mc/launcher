use sha2::{Digest, Sha384};
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Connection, SqliteConnection, SqlitePool};
use std::collections::BTreeMap;
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

        compatible_migrator(&pool).await?.run(&pool).await?;
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

async fn compatible_migrator(pool: &SqlitePool) -> Result<Migrator, StorageError> {
    let has_history: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    let applied: BTreeMap<i64, Vec<u8>> = if has_history {
        sqlx::query_as::<_, (i64, Vec<u8>)>("SELECT version, checksum FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect()
    } else {
        BTreeMap::new()
    };
    let migrations = MIGRATOR
        .iter()
        .cloned()
        .map(|mut migration| {
            let canonical_sql = migration.sql.as_str().replace("\r\n", "\n");
            let canonical_checksum = Sha384::digest(canonical_sql.as_bytes()).to_vec();
            let windows_checksum =
                Sha384::digest(canonical_sql.replace('\n', "\r\n").as_bytes()).to_vec();
            // Keep existing history untouched. Only accept the exact LF/CRLF
            // variants of this migration; SQLx still rejects all other changes.
            migration.checksum = if applied.get(&migration.version) == Some(&windows_checksum) {
                windows_checksum.into()
            } else {
                canonical_checksum.into()
            };
            migration
        })
        .collect();
    Ok(Migrator::with_migrations(migrations))
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
    #[error("database migration failed: {0}")]
    Migration(#[from] MigrateError),
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("failed to format a UTC timestamp")]
    Timestamp(#[from] time::error::Format),
    #[error("stored JSON is invalid")]
    Json(#[from] serde_json::Error),
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
    #[error("instance content cannot change while an installation is active")]
    InstanceBusy,
    #[error("page limit must be between 1 and 1000")]
    InvalidPageLimit,
    #[error("managed relative path is invalid")]
    ManagedPath(#[from] slate_platform::PathPolicyError),
}

#[cfg(test)]
mod tests {
    use super::{Database, MIGRATOR, StorageError};
    use sha2::{Digest, Sha384};
    use sqlx::migrate::{MigrateError, Migration, Migrator};
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::{AssertSqlSafe, Row, SqlSafeStr, SqlitePool};

    #[tokio::test]
    async fn upgrades_lf_and_crlf_databases_without_rewriting_history_or_losing_data()
    -> Result<(), Box<dyn std::error::Error>> {
        for line_ending in ["\n", "\r\n"] {
            let directory = tempfile::tempdir()?;
            let path = directory.path().join("state.sqlite");
            let pool = SqlitePool::connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await?;
            let migrations = MIGRATOR
                .iter()
                .take(2)
                .map(|migration| {
                    let sql = migration
                        .sql
                        .as_str()
                        .replace("\r\n", "\n")
                        .replace('\n', line_ending);
                    Migration::new(
                        migration.version,
                        migration.description.clone(),
                        migration.migration_type,
                        AssertSqlSafe(sql).into_sql_str(),
                        migration.no_tx,
                    )
                })
                .collect();
            Migrator::with_migrations(migrations).run(&pool).await?;
            sqlx::query("INSERT INTO accounts (id, provider, external_profile_id, display_name, credential_ref, status, created_at, updated_at) VALUES ('saved-account', 'microsoft', 'profile', 'Saved player', 'vault:fixture', 'ready', '2026-11-21', '2026-11-21')")
                .execute(&pool).await?;
            sqlx::query("INSERT INTO storage_roots (id, canonical_path, state, created_at, updated_at) VALUES ('saved-root', 'fixture:/storage', 'available', '2026-11-21', '2026-11-21')")
                .execute(&pool).await?;
            sqlx::query("INSERT INTO instances (id, name, mode, management_mode, root_id, relative_path, created_at, updated_at) VALUES ('saved-instance', 'Saved pack', 'modded', 'local', 'saved-root', 'instances/saved-pack', '2026-11-21', '2026-11-21')")
                .execute(&pool).await?;
            let history: Vec<(i64, Vec<u8>)> =
                sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
                    .fetch_all(&pool)
                    .await?;
            pool.close().await;

            let database = Database::connect(&path).await?;
            let name: String =
                sqlx::query_scalar("SELECT display_name FROM accounts WHERE id = 'saved-account'")
                    .fetch_one(&database.pool)
                    .await?;
            assert_eq!(name, "Saved player");
            let instance_name: String =
                sqlx::query_scalar("SELECT name FROM instances WHERE id = 'saved-instance'")
                    .fetch_one(&database.pool)
                    .await?;
            assert_eq!(instance_name, "Saved pack");
            let upgraded_history: Vec<(i64, Vec<u8>)> =
                sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
                    .fetch_all(&database.pool)
                    .await?;
            assert_eq!(&upgraded_history[..history.len()], &history);
            assert_eq!(upgraded_history.len(), MIGRATOR.iter().count());
            for ((_, checksum), migration) in upgraded_history
                .iter()
                .zip(MIGRATOR.iter())
                .skip(history.len())
            {
                assert_eq!(
                    *checksum,
                    Sha384::digest(migration.sql.as_str().replace("\r\n", "\n").as_bytes())
                        .to_vec()
                );
            }
            database.close().await;
            // The same database must also survive the next startup.
            Database::connect(&path).await?.close().await;
        }
        Ok(())
    }

    #[tokio::test]
    async fn rejects_unknown_migration_checksums() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state.sqlite");
        let database = Database::connect(&path).await?;
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = 1")
            .bind(vec![0_u8; 48])
            .execute(&database.pool)
            .await?;
        database.close().await;
        assert!(matches!(
            Database::connect(&path).await,
            Err(StorageError::Migration(MigrateError::VersionMismatch(1)))
        ));
        Ok(())
    }

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
