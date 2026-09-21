use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_domain::{AccountId, InstanceId, RevisionId, SessionId};
use sqlx::Row;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Starting,
    Running,
    Exited,
    Failed,
    Crashed,
    Cancelled,
}

impl TryFrom<&str> for SessionState {
    type Error = StorageError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "exited" => Ok(Self::Exited),
            "failed" => Ok(Self::Failed),
            "crashed" => Ok(Self::Crashed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(StorageError::InvalidStoredValue {
                field: "sessions.state",
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRecord {
    pub id: SessionId,
    pub instance_id: InstanceId,
    pub state: SessionState,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
}

impl Database {
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

    pub async fn list_recent_sessions(
        &self,
        instance_id: InstanceId,
        limit: u8,
    ) -> Result<Vec<SessionRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, instance_id, state, started_at, ended_at, exit_code FROM sessions \
             WHERE instance_id = ? AND state IN ('exited', 'failed', 'crashed', 'cancelled') \
             ORDER BY started_at DESC LIMIT ?",
        )
        .bind(instance_id.to_string())
        .bind(i64::from(limit.clamp(1, 20)))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_session).collect()
    }

    pub async fn get_session(&self, session_id: SessionId) -> Result<SessionRecord, StorageError> {
        let row = sqlx::query(
            "SELECT id, instance_id, state, started_at, ended_at, exit_code FROM sessions \
             WHERE id = ?",
        )
        .bind(session_id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::InvalidStoredValue {
            field: "sessions.id",
            value: session_id.to_string(),
        })?;
        row_to_session(&row)
    }
}

fn row_to_session(row: &sqlx::sqlite::SqliteRow) -> Result<SessionRecord, StorageError> {
    let state: String = row.try_get("state")?;
    Ok(SessionRecord {
        id: SessionId::from_uuid(parse_uuid(row.try_get("id")?, "sessions.id")?),
        instance_id: InstanceId::from_uuid(parse_uuid(
            row.try_get("instance_id")?,
            "sessions.instance_id",
        )?),
        state: SessionState::try_from(state.as_str())?,
        started_at: row.try_get("started_at")?,
        ended_at: row.try_get("ended_at")?,
        exit_code: row.try_get("exit_code")?,
    })
}

fn parse_uuid(value: String, field: &'static str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(&value).map_err(|source| StorageError::InvalidStoredId { field, source })
}
