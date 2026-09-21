use crate::database::now_rfc3339;
use crate::instance_content::insert_provider_content;
use crate::{
    Database, NewInstanceMod, NewInstanceModDependencySet, NewInstanceProviderContent, StorageError,
};
use slate_domain::{AccountId, InstanceId, JobId, LoaderFamily, RequestId, RevisionId, SessionId};
use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;

mod completion;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobState {
    Queued,
    Running,
    Paused,
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
            Self::Paused => "paused",
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
            "paused" => Ok(Self::Paused),
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

fn valid_install_operation(operation: &str) -> bool {
    matches!(
        operation,
        "instance_install"
            | "content_install"
            | "external_instance_import"
            | "imported_pack_install"
            | "mod_install"
            | "mod_update"
            | "modpack_update"
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallJobRecord {
    pub id: JobId,
    pub instance_id: InstanceId,
    pub revision_id: RevisionId,
    pub state: JobState,
    pub operation: Option<String>,
    pub retry_payload: Option<serde_json::Value>,
    pub phase: String,
    pub progress_json: String,
    pub completed_items: Option<u64>,
    pub total_items: Option<u64>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedInstall {
    pub job_id: JobId,
    pub revision_id: RevisionId,
    pub manifest_digest: String,
    pub client_version: String,
    pub runtime: InstalledRuntime,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedModpackUpdate {
    pub version_id: String,
    pub loader_version: Option<String>,
}

impl Database {
    pub async fn has_active_install_jobs(&self) -> Result<bool, StorageError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM jobs WHERE kind = 'instance_install' \
             AND state IN ('queued', 'running', 'paused'))",
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn begin_instance_install(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        request_id: RequestId,
    ) -> Result<PendingInstall, StorageError> {
        self.begin_instance_install_with_context(
            instance_id,
            expected_revision,
            request_id,
            "instance_install",
            None,
        )
        .await
    }

    pub async fn begin_instance_install_with_context(
        &self,
        instance_id: InstanceId,
        expected_revision: u64,
        request_id: RequestId,
        operation: &str,
        retry_payload: Option<serde_json::Value>,
    ) -> Result<PendingInstall, StorageError> {
        if !valid_install_operation(operation) {
            return Err(StorageError::InvalidStoredValue {
                field: "jobs.operation",
                value: operation.to_owned(),
            });
        }
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
            return self
                .revision_error_for_install(instance_id, expected_revision)
                .await;
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

        let mut progress = serde_json::json!({
            "revisionId": revision_id.to_string(),
            "message": "Queued",
            "operation": operation,
        });
        if let Some(payload) = retry_payload.clone() {
            progress["retry"] = payload;
        }
        let progress = serde_json::to_string(&progress)?;
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

        sqlx::query("UPDATE instances SET revision = revision + 1, updated_at = ? WHERE id = ?")
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
                operation: Some(operation.to_owned()),
                retry_payload,
                phase: "metadata".to_owned(),
                progress_json: progress,
                completed_items: None,
                total_items: None,
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

    pub async fn set_install_paused(
        &self,
        job_id: JobId,
        paused: bool,
    ) -> Result<bool, StorageError> {
        let now = now_rfc3339()?;
        let updated = if paused {
            sqlx::query(
                "UPDATE jobs SET state = 'paused', \
                 progress_json = json_set(progress_json, '$.message', 'Installation paused'), \
                 updated_at = ? WHERE id = ? AND kind = 'instance_install' \
                 AND state IN ('queued', 'running')",
            )
            .bind(&now)
            .bind(job_id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                "UPDATE jobs SET state = 'running', \
                 progress_json = json_set(progress_json, '$.message', 'Resuming installation'), \
                 updated_at = ? WHERE id = ? AND kind = 'instance_install' AND state = 'paused'",
            )
            .bind(&now)
            .bind(job_id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected()
        };
        Ok(updated == 1)
    }

    pub async fn update_install_progress(
        &self,
        job_id: JobId,
        phase: &str,
        message: &str,
        completed_items: Option<u64>,
        total_items: Option<u64>,
    ) -> Result<(), StorageError> {
        let completed_items = completed_items
            .map(i64::try_from)
            .transpose()
            .map_err(|_| StorageError::ProgressOutOfRange)?;
        let total_items = total_items
            .map(i64::try_from)
            .transpose()
            .map_err(|_| StorageError::ProgressOutOfRange)?;
        let now = now_rfc3339()?;
        sqlx::query(
            "UPDATE jobs SET state = 'running', phase = ?, progress_json = json_set(\
             progress_json, '$.message', ?, '$.completedItems', ?, '$.totalItems', ?), \
             updated_at = ? WHERE id = ? AND state IN ('queued', 'running')",
        )
        .bind(phase)
        .bind(message)
        .bind(completed_items)
        .bind(total_items)
        .bind(now)
        .bind(job_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Resolves jobs whose worker belonged to a previous launcher process.
    pub async fn recover_interrupted_installs(&self) -> Result<u64, StorageError> {
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE instance_revisions SET status = 'failed' WHERE status IN ('proposed', 'staged') \
             AND id IN (SELECT json_extract(progress_json, '$.revisionId') FROM jobs \
             WHERE kind = 'instance_install' AND state IN ('queued', 'running', 'paused'))",
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE instance_configuration SET setup_state = 'blocked' WHERE setup_state = 'preparing' \
             AND instance_id IN (SELECT id FROM instances WHERE trashed_at IS NULL)",
        )
        .execute(&mut *transaction)
        .await?;
        let affected = sqlx::query(
            "UPDATE jobs SET \
             state = CASE WHEN EXISTS (SELECT 1 FROM instances i WHERE i.id = jobs.entity_id \
                 AND i.trashed_at IS NULL) THEN 'failed' ELSE 'cancelled' END, \
             phase = CASE WHEN EXISTS (SELECT 1 FROM instances i WHERE i.id = jobs.entity_id \
                 AND i.trashed_at IS NULL) THEN 'interrupted' ELSE 'cancelled' END, \
             progress_json = json_set(progress_json, '$.message', \
                 CASE WHEN EXISTS (SELECT 1 FROM instances i WHERE i.id = jobs.entity_id \
                     AND i.trashed_at IS NULL) \
                 THEN 'Installation was interrupted when slate closed. Retry the installation.' \
                 ELSE 'Installation stopped because the instance was moved to trash.' END, \
                 '$.completedItems', NULL, '$.totalItems', NULL), \
             updated_at = ? WHERE kind = 'instance_install' AND state IN ('queued', 'running', 'paused')",
        )
        .bind(now)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        transaction.commit().await?;
        Ok(affected)
    }

    pub async fn fail_instance_install(
        &self,
        job_id: JobId,
        revision_id: RevisionId,
        message: &str,
    ) -> Result<(), StorageError> {
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM jobs WHERE id = ? AND state IN ('queued', 'running', 'paused'))",
        )
        .bind(job_id.to_string())
        .fetch_one(&mut *transaction)
        .await?;
        if !active {
            transaction.rollback().await?;
            return Ok(());
        }
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
                "UPDATE instance_configuration SET setup_state = \
                 CASE WHEN EXISTS(SELECT 1 FROM instances WHERE id = ? \
                 AND active_revision_id IS NOT NULL) THEN 'ready' ELSE 'blocked' END \
                 WHERE instance_id = ?",
            )
            .bind(&instance_id)
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

    pub async fn cancel_instance_install(
        &self,
        job_id: JobId,
        revision_id: RevisionId,
        message: &str,
    ) -> Result<(), StorageError> {
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let instance_id: Option<String> = sqlx::query_scalar(
            "SELECT entity_id FROM jobs WHERE id = ? AND state IN ('queued', 'running', 'paused') \
             AND json_extract(progress_json, '$.revisionId') = ?",
        )
        .bind(job_id.to_string())
        .bind(revision_id.to_string())
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(instance_id) = instance_id else {
            transaction.rollback().await?;
            return Err(StorageError::InstallNoLongerActive);
        };
        sqlx::query("UPDATE instance_revisions SET status = 'failed' WHERE id = ?")
            .bind(revision_id.to_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "UPDATE instance_configuration SET setup_state = \
             CASE WHEN EXISTS(SELECT 1 FROM instances WHERE id = ? \
             AND active_revision_id IS NOT NULL) THEN 'ready' ELSE 'configured' END \
             WHERE instance_id = ?",
        )
        .bind(&instance_id)
        .bind(&instance_id)
        .execute(&mut *transaction)
        .await?;
        update_job_transaction(
            &mut transaction,
            job_id,
            JobState::Cancelled,
            "cancelled",
            message,
            &now,
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn get_install_job(&self, job_id: JobId) -> Result<InstallJobRecord, StorageError> {
        let row = sqlx::query(
            "SELECT j.id, j.entity_id, j.state, j.phase, j.progress_json, j.created_at, \
             j.updated_at, json_extract(j.progress_json, '$.revisionId') AS revision_id, \
             json_extract(j.progress_json, '$.completedItems') AS completed_items, \
             json_extract(j.progress_json, '$.totalItems') AS total_items \
             FROM jobs j WHERE j.kind = 'instance_install' AND j.id = ?",
        )
        .bind(job_id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::InstallNoLongerActive)?;
        row_to_install_job(&row)
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
             j.updated_at, json_extract(j.progress_json, '$.revisionId') AS revision_id, \
             json_extract(j.progress_json, '$.completedItems') AS completed_items, \
             json_extract(j.progress_json, '$.totalItems') AS total_items \
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
        account_id: AccountId,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO sessions \
             (id, instance_id, revision_id, account_id, started_at, state, readiness) \
             VALUES (?, ?, ?, ?, ?, 'starting', 'unknown')",
        )
        .bind(session_id.to_string())
        .bind(instance_id.to_string())
        .bind(revision_id.to_string())
        .bind(account_id.to_string())
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
        force_stopped: bool,
    ) -> Result<(), StorageError> {
        let state = if force_stopped {
            "cancelled"
        } else if exit_code == Some(0) {
            "exited"
        } else {
            "crashed"
        };
        sqlx::query("UPDATE sessions SET ended_at = ?, state = ?, exit_code = ? WHERE id = ?")
            .bind(now_rfc3339()?)
            .bind(state)
            .bind(exit_code)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn fail_session_start(&self, session_id: SessionId) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE sessions SET ended_at = ?, state = 'failed' WHERE id = ? AND state = 'starting'",
        )
        .bind(now_rfc3339()?)
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
    let progress_json: String = row.try_get("progress_json")?;
    let progress: serde_json::Value = serde_json::from_str(&progress_json)?;
    Ok(InstallJobRecord {
        id: JobId::from_uuid(parse_uuid(row.try_get("id")?, "jobs.id")?),
        instance_id: InstanceId::from_uuid(parse_uuid(entity_id, "jobs.entity_id")?),
        revision_id: RevisionId::from_uuid(parse_uuid(
            revision_id,
            "jobs.progress_json.revisionId",
        )?),
        state: JobState::try_from(state.as_str())?,
        operation: progress
            .get("operation")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        retry_payload: progress.get("retry").cloned(),
        phase: row.try_get("phase")?,
        progress_json,
        completed_items: row
            .try_get::<Option<i64>, _>("completed_items")?
            .map(|value| u64::try_from(value).map_err(|_| StorageError::ProgressOutOfRange))
            .transpose()?,
        total_items: row
            .try_get::<Option<i64>, _>("total_items")?
            .map(|value| u64::try_from(value).map_err(|_| StorageError::ProgressOutOfRange))
            .transpose()?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn parse_uuid(value: String, field: &'static str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(&value).map_err(|source| StorageError::InvalidStoredId { field, source })
}

#[cfg(test)]
mod tests;
