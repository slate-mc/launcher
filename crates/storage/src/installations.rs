use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_domain::{InstanceId, JobId, LoaderFamily, RequestId, RevisionId, SessionId};
use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobState {
    #[must_use]
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for JobState {
    type Error = StorageError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(StorageError::InvalidStoredValue {
                field: "jobs.state",
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallJobRecord {
    pub id: JobId,
    pub instance_id: InstanceId,
    pub revision_id: RevisionId,
    pub state: JobState,
    pub phase: String,
    pub progress_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingInstall {
    pub job: InstallJobRecord,
    pub revision_id: RevisionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledRevisionRecord {
    pub id: RevisionId,
    pub instance_id: InstanceId,
    pub manifest_digest: String,
    pub game_version: String,
    pub loader_kind: LoaderFamily,
    pub loader_version: Option<String>,
    pub runtime_executable: PathBuf,
    pub runtime_major: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledRuntime {
    pub vendor: String,
    pub release_name: String,
    pub java_version: String,
    pub major: u32,
    pub os: String,
    pub arch: String,
    pub executable_ref: String,
    pub source_digest: String,
}

impl Database {
    pub async fn begin_instance_install(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        request_id: RequestId,
    ) -> Result<PendingInstall, StorageError> {
        let expected_revision =
            i64::try_from(expected_revision).map_err(|_| StorageError::NegativeRevision)?;
        let job_id = JobId::new();
        let revision_id = RevisionId::new();
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;

        let current = sqlx::query(
            "SELECT i.active_revision_id, c.minecraft_version, c.loader_kind, c.loader_version \
             FROM instances i INNER JOIN instance_configuration c ON c.instance_id = i.id \
             WHERE i.id = ? AND i.revision = ? AND i.trashed_at IS NULL",
        )
        .bind(instance_id.to_string())
        .bind(expected_revision)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(current) = current else {
            transaction.rollback().await?;
            return self.revision_error_for_install(instance_id, expected_revision).await;
        };
        let game_version: String = current.try_get("minecraft_version")?;
        let loader_kind: String = current.try_get("loader_kind")?;
        let loader_version: Option<String> = current.try_get("loader_version")?;
        let parent_id: Option<String> = current.try_get("active_revision_id")?;

        sqlx::query(
            "INSERT INTO instance_revisions \
             (id, instance_id, parent_id, manifest_digest, game_version, loader_kind, \
              loader_version, status, created_at) VALUES (?, ?, ?, 'pending', ?, ?, ?, 'proposed', ?)",
        )
        .bind(revision_id.to_string())
        .bind(instance_id.to_string())
        .bind(parent_id)
        .bind(game_version)
        .bind(loader_kind)
        .bind(loader_version)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;

        let progress = format!(r#"{{"revisionId":"{revision_id}","message":"Queued"}}"#);
        sqlx::query(
            "INSERT INTO jobs \
             (id, kind, entity_id, request_id, state, phase, progress_json, created_at, updated_at) \
             VALUES (?, 'instance_install', ?, ?, 'queued', 'metadata', ?, ?, ?)",
        )
        .bind(job_id.to_string())
        .bind(instance_id.to_string())
        .bind(request_id.to_string())
        .bind(&progress)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "UPDATE instances SET revision = revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(instance_id.to_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE instance_configuration SET setup_state = 'preparing' WHERE instance_id = ?",
        )
        .bind(instance_id.to_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        Ok(PendingInstall {
            revision_id,
            job: InstallJobRecord {
                id: job_id,
                instance_id,
                revision_id,
                state: JobState::Queued,
                phase: "metadata".to_owned(),
                progress_json: progress,
                created_at: now.clone(),
                updated_at: now,
            },
        })
    }

    pub async fn mark_install_running(
        &self,
        job_id: JobId,
        phase: &str,
        message: &str,
    ) -> Result<(), StorageError> {
        update_job(&self.pool, job_id, JobState::Running, phase, message).await
    }

    pub async fn complete_instance_install(
        &self,
        job_id: JobId,
        revision_id: RevisionId,
        manifest_digest: &str,
        client_version: &str,
        runtime: InstalledRuntime,
        message: &str,
    ) -> Result<(), StorageError> {
        let now = now_rfc3339()?;
        let runtime_id = Uuid::new_v4().to_string();
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO runtime_installations \
             (id, vendor, release_name, java_version, major, os, arch, executable_ref, \
              source_digest, managed, verified_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?) \
             ON CONFLICT(vendor, release_name, os, arch) DO UPDATE SET \
              java_version = excluded.java_version, executable_ref = excluded.executable_ref, \
              source_digest = excluded.source_digest, verified_at = excluded.verified_at",
        )
        .bind(&runtime_id)
        .bind(&runtime.vendor)
        .bind(&runtime.release_name)
        .bind(&runtime.java_version)
        .bind(i64::from(runtime.major))
        .bind(&runtime.os)
        .bind(&runtime.arch)
        .bind(&runtime.executable_ref)
        .bind(&runtime.source_digest)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        let stored_runtime_id: String = sqlx::query_scalar(
            "SELECT id FROM runtime_installations \
             WHERE vendor = ? AND release_name = ? AND os = ? AND arch = ?",
        )
        .bind(&runtime.vendor)
        .bind(&runtime.release_name)
        .bind(&runtime.os)
        .bind(&runtime.arch)
        .fetch_one(&mut *transaction)
        .await?;

        let instance_id: String =
            sqlx::query_scalar("SELECT instance_id FROM instance_revisions WHERE id = ?")
                .bind(revision_id.to_string())
                .fetch_optional(&mut *transaction)
                .await?
                .ok_or(StorageError::RevisionNotFound)?;
        sqlx::query(
            "UPDATE instance_revisions SET manifest_digest = ?, client_version = ?, \
             runtime_id = ?, installed_at = ?, status = 'installed' WHERE id = ?",
        )
        .bind(manifest_digest)
        .bind(client_version)
        .bind(stored_runtime_id)
        .bind(&now)
        .bind(revision_id.to_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE instances SET active_revision_id = ?, updated_at = ? WHERE id = ?",
        )
        .bind(revision_id.to_string())
        .bind(&now)
        .bind(&instance_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE instance_configuration SET setup_state = 'ready' WHERE instance_id = ?",
        )
        .bind(&instance_id)
        .execute(&mut *transaction)
        .await?;
        update_job_transaction(
            &mut transaction,
            job_id,
            JobState::Succeeded,
            "complete",
            message,
            &now,
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn fail_instance_install(
        &self,
        job_id: JobId,
        revision_id: RevisionId,
        message: &str,
    ) -> Result<(), StorageError> {
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let instance_id: Option<String> =
            sqlx::query_scalar("SELECT instance_id FROM instance_revisions WHERE id = ?")
                .bind(revision_id.to_string())
                .fetch_optional(&mut *transaction)
                .await?;
        sqlx::query("UPDATE instance_revisions SET status = 'failed' WHERE id = ?")
            .bind(revision_id.to_string())
            .execute(&mut *transaction)
            .await?;
        if let Some(instance_id) = instance_id {
            sqlx::query(
                "UPDATE instance_configuration SET setup_state = 'blocked' WHERE instance_id = ?",
            )
            .bind(instance_id)
            .execute(&mut *transaction)
            .await?;
        }
        update_job_transaction(
            &mut transaction,
            job_id,
            JobState::Failed,
            "failed",
            message,
            &now,
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn list_install_jobs(
        &self,
        limit: u32,
    ) -> Result<Vec<InstallJobRecord>, StorageError> {
        if !(1..=1_000).contains(&limit) {
            return Err(StorageError::InvalidPageLimit);
        }
        let rows = sqlx::query(
            "SELECT j.id, j.entity_id, j.state, j.phase, j.progress_json, j.created_at, \
             j.updated_at, json_extract(j.progress_json, '$.revisionId') AS revision_id \
             FROM jobs j WHERE j.kind = 'instance_install' \
             ORDER BY j.updated_at DESC LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_install_job).collect()
    }

    pub async fn get_installed_revision(
        &self,
        instance_id: InstanceId,
    ) -> Result<InstalledRevisionRecord, StorageError> {
        let row = sqlx::query(
            "SELECT r.id, r.instance_id, r.manifest_digest, r.game_version, r.loader_kind, \
             r.loader_version, rt.executable_ref, rt.major \
             FROM instances i \
             INNER JOIN instance_revisions r ON r.id = i.active_revision_id \
             INNER JOIN runtime_installations rt ON rt.id = r.runtime_id \
             WHERE i.id = ? AND i.trashed_at IS NULL AND r.status = 'installed'",
        )
        .bind(instance_id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::InstalledRevisionNotFound)?;
        let loader_kind: String = row.try_get("loader_kind")?;
        let major: i64 = row.try_get("major")?;
        Ok(InstalledRevisionRecord {
            id: RevisionId::from_uuid(parse_uuid(row.try_get("id")?, "instance_revisions.id")?),
            instance_id: InstanceId::from_uuid(parse_uuid(
                row.try_get("instance_id")?,
                "instance_revisions.instance_id",
            )?),
            manifest_digest: row.try_get("manifest_digest")?,
            game_version: row.try_get("game_version")?,
            loader_kind: LoaderFamily::try_from(loader_kind.as_str()).map_err(|_| {
                StorageError::InvalidStoredValue {
                    field: "instance_revisions.loader_kind",
                    value: loader_kind,
                }
            })?,
            loader_version: row.try_get("loader_version")?,
            runtime_executable: PathBuf::from(row.try_get::<String, _>("executable_ref")?),
            runtime_major: u32::try_from(major).map_err(|_| StorageError::InvalidStoredValue {
                field: "runtime_installations.major",
                value: major.to_string(),
            })?,
        })
    }

    pub async fn create_session_starting(
        &self,
        instance_id: InstanceId,
        revision_id: RevisionId,
        session_id: SessionId,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO sessions \
             (id, instance_id, revision_id, started_at, state, readiness) \
             VALUES (?, ?, ?, ?, 'starting', 'unknown')",
        )
        .bind(session_id.to_string())
        .bind(instance_id.to_string())
        .bind(revision_id.to_string())
        .bind(now_rfc3339()?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_session_running(
        &self,
        session_id: SessionId,
        pid: u32,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE sessions SET pid = ?, state = 'running', readiness = 'process_started' \
             WHERE id = ? AND state = 'starting'",
        )
        .bind(i64::from(pid))
        .bind(session_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn finish_session(
        &self,
        session_id: SessionId,
        exit_code: Option<i32>,
    ) -> Result<(), StorageError> {
        let state = if exit_code == Some(0) { "exited" } else { "crashed" };
        sqlx::query(
            "UPDATE sessions SET ended_at = ?, state = ?, exit_code = ? WHERE id = ?",
        )
        .bind(now_rfc3339()?)
        .bind(state)
        .bind(exit_code)
        .bind(session_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn revision_error_for_install<T>(
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

async fn update_job(
    pool: &sqlx::SqlitePool,
    job_id: JobId,
    state: JobState,
    phase: &str,
    message: &str,
) -> Result<(), StorageError> {
    let now = now_rfc3339()?;
    sqlx::query(
        "UPDATE jobs SET state = ?, phase = ?, \
         progress_json = json_set(progress_json, '$.message', ?), updated_at = ? WHERE id = ?",
    )
    .bind(state.as_storage_value())
    .bind(phase)
    .bind(message)
    .bind(now)
    .bind(job_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

async fn update_job_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    job_id: JobId,
    state: JobState,
    phase: &str,
    message: &str,
    now: &str,
) -> Result<(), StorageError> {
    sqlx::query(
        "UPDATE jobs SET state = ?, phase = ?, \
         progress_json = json_set(progress_json, '$.message', ?), updated_at = ? WHERE id = ?",
    )
    .bind(state.as_storage_value())
    .bind(phase)
    .bind(message)
    .bind(now)
    .bind(job_id.to_string())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn row_to_install_job(row: &sqlx::sqlite::SqliteRow) -> Result<InstallJobRecord, StorageError> {
    let entity_id: String = row.try_get("entity_id")?;
    let revision_id: String = row.try_get("revision_id")?;
    let state: String = row.try_get("state")?;
    Ok(InstallJobRecord {
        id: JobId::from_uuid(parse_uuid(row.try_get("id")?, "jobs.id")?),
        instance_id: InstanceId::from_uuid(parse_uuid(entity_id, "jobs.entity_id")?),
        revision_id: RevisionId::from_uuid(parse_uuid(
            revision_id,
            "jobs.progress_json.revisionId",
        )?),
        state: JobState::try_from(state.as_str())?,
        phase: row.try_get("phase")?,
        progress_json: row.try_get("progress_json")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn parse_uuid(value: String, field: &'static str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(&value).map_err(|source| StorageError::InvalidStoredId { field, source })
}

#[cfg(test)]
mod tests {
    use super::JobState;

    #[test]
    fn job_states_match_database_values() {
        assert_eq!(JobState::Running.as_storage_value(), "running");
        assert_eq!(JobState::try_from("succeeded").ok(), Some(JobState::Succeeded));
    }
}
