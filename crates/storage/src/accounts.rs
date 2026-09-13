use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_domain::{AccountId, InstanceId};
use sqlx::Row;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountStatus {
    Ready,
    ReauthenticationRequired,
}

impl AccountStatus {
    #[must_use]
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::ReauthenticationRequired => "reauthentication_required",
        }
    }
}

impl TryFrom<&str> for AccountStatus {
    type Error = StorageError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "ready" => Ok(Self::Ready),
            "reauthentication_required" => Ok(Self::ReauthenticationRequired),
            other => Err(StorageError::InvalidStoredValue {
                field: "accounts.status",
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedAccount {
    pub profile_id: Uuid,
    pub display_name: String,
    pub credential_ref: String,
    pub skin_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountRecord {
    pub id: AccountId,
    pub profile_id: Uuid,
    pub display_name: String,
    pub credential_ref: String,
    pub skin_url: Option<String>,
    pub status: AccountStatus,
    pub is_default: bool,
    pub last_validated_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchAccount {
    pub id: AccountId,
    pub profile_id: Uuid,
    pub display_name: String,
    pub credential_ref: String,
    pub status: AccountStatus,
}

impl Database {
    pub async fn upsert_authenticated_account(
        &self,
        account: AuthenticatedAccount,
    ) -> Result<AccountRecord, StorageError> {
        validate_display_name(&account.display_name)?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let existing_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM accounts WHERE provider = 'microsoft' AND external_profile_id = ?",
        )
        .bind(account.profile_id.to_string())
        .fetch_optional(&mut *transaction)
        .await?;
        let id = existing_id.unwrap_or_else(|| AccountId::new().to_string());
        sqlx::query(
            "INSERT INTO accounts \
             (id, provider, external_profile_id, display_name, credential_ref, status, \
              last_validated_at, created_at, updated_at) \
             VALUES (?, 'microsoft', ?, ?, ?, 'ready', ?, ?, ?) \
             ON CONFLICT(provider, external_profile_id) DO UPDATE SET \
              display_name = excluded.display_name, credential_ref = excluded.credential_ref, \
              status = 'ready', last_validated_at = excluded.last_validated_at, \
              updated_at = excluded.updated_at",
        )
        .bind(&id)
        .bind(account.profile_id.to_string())
        .bind(&account.display_name)
        .bind(&account.credential_ref)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO account_preferences (account_id, default_flag, skin_ref) \
             VALUES (?, 0, ?) ON CONFLICT(account_id) DO UPDATE SET skin_ref = excluded.skin_ref",
        )
        .bind(&id)
        .bind(&account.skin_url)
        .execute(&mut *transaction)
        .await?;
        let has_default: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM account_preferences WHERE default_flag = 1)",
        )
        .fetch_one(&mut *transaction)
        .await?;
        if !has_default {
            sqlx::query("UPDATE account_preferences SET default_flag = 1 WHERE account_id = ?")
                .bind(&id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        self.get_account(AccountId::from_uuid(parse_uuid(id, "accounts.id")?))
            .await
    }

    pub async fn list_accounts(&self) -> Result<Vec<AccountRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT a.id, a.external_profile_id, a.display_name, a.credential_ref, a.status, \
             a.last_validated_at, p.skin_ref, p.default_flag \
             FROM accounts a INNER JOIN account_preferences p ON p.account_id = a.id \
             WHERE a.provider = 'microsoft' ORDER BY p.default_flag DESC, a.display_name COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_account).collect()
    }

    pub async fn get_account(&self, id: AccountId) -> Result<AccountRecord, StorageError> {
        let row = sqlx::query(
            "SELECT a.id, a.external_profile_id, a.display_name, a.credential_ref, a.status, \
             a.last_validated_at, p.skin_ref, p.default_flag \
             FROM accounts a INNER JOIN account_preferences p ON p.account_id = a.id \
             WHERE a.id = ? AND a.provider = 'microsoft'",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::AccountNotFound)?;
        row_to_account(&row)
    }

    pub async fn account_for_launch(
        &self,
        instance_id: InstanceId,
        requested: Option<AccountId>,
    ) -> Result<LaunchAccount, StorageError> {
        let account_id = if let Some(requested) = requested {
            requested.to_string()
        } else {
            sqlx::query_scalar::<_, String>(
                "SELECT COALESCE(\
                    (SELECT preferred_account_id FROM instances WHERE id = ?),\
                    (SELECT account_id FROM account_preferences WHERE default_flag = 1),\
                    (SELECT id FROM accounts ORDER BY created_at LIMIT 1)\
                 )",
            )
            .bind(instance_id.to_string())
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StorageError::AccountNotFound)?
        };
        let row = sqlx::query(
            "SELECT id, external_profile_id, display_name, credential_ref, status \
             FROM accounts WHERE id = ? AND provider = 'microsoft'",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::AccountNotFound)?;
        let status: String = row.try_get("status")?;
        Ok(LaunchAccount {
            id: AccountId::from_uuid(parse_uuid(row.try_get("id")?, "accounts.id")?),
            profile_id: parse_uuid(
                row.try_get("external_profile_id")?,
                "accounts.external_profile_id",
            )?,
            display_name: row.try_get("display_name")?,
            credential_ref: row.try_get("credential_ref")?,
            status: AccountStatus::try_from(status.as_str())?,
        })
    }

    pub async fn update_account_validation(
        &self,
        id: AccountId,
        display_name: &str,
        skin_url: Option<&str>,
    ) -> Result<AccountRecord, StorageError> {
        validate_display_name(display_name)?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let affected = sqlx::query(
            "UPDATE accounts SET display_name = ?, status = 'ready', last_validated_at = ?, \
             updated_at = ? WHERE id = ?",
        )
        .bind(display_name)
        .bind(&now)
        .bind(&now)
        .bind(id.to_string())
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if affected == 0 {
            transaction.rollback().await?;
            return Err(StorageError::AccountNotFound);
        }
        sqlx::query("UPDATE account_preferences SET skin_ref = ? WHERE account_id = ?")
            .bind(skin_url)
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.get_account(id).await
    }

    pub async fn mark_account_reauthentication_required(
        &self,
        id: AccountId,
    ) -> Result<(), StorageError> {
        let affected = sqlx::query(
            "UPDATE accounts SET status = 'reauthentication_required', updated_at = ? WHERE id = ?",
        )
        .bind(now_rfc3339()?)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(StorageError::AccountNotFound);
        }
        Ok(())
    }

    pub async fn set_default_account(&self, id: AccountId) -> Result<AccountRecord, StorageError> {
        let mut transaction = self.pool.begin().await?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?)")
            .bind(id.to_string())
            .fetch_one(&mut *transaction)
            .await?;
        if !exists {
            transaction.rollback().await?;
            return Err(StorageError::AccountNotFound);
        }
        sqlx::query("UPDATE account_preferences SET default_flag = 0 WHERE default_flag = 1")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE account_preferences SET default_flag = 1 WHERE account_id = ?")
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.get_account(id).await
    }

    pub async fn remove_account(&self, id: AccountId) -> Result<String, StorageError> {
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT a.credential_ref, p.default_flag FROM accounts a \
             INNER JOIN account_preferences p ON p.account_id = a.id WHERE a.id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(StorageError::AccountNotFound)?;
        let credential_ref: String = row.try_get("credential_ref")?;
        let was_default: i64 = row.try_get("default_flag")?;
        sqlx::query("DELETE FROM accounts WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await?;
        if was_default != 0 {
            sqlx::query(
                "UPDATE account_preferences SET default_flag = 1 WHERE account_id = \
                 (SELECT account_id FROM account_preferences ORDER BY account_id LIMIT 1)",
            )
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(credential_ref)
    }
}

fn row_to_account(row: &sqlx::sqlite::SqliteRow) -> Result<AccountRecord, StorageError> {
    let status: String = row.try_get("status")?;
    Ok(AccountRecord {
        id: AccountId::from_uuid(parse_uuid(row.try_get("id")?, "accounts.id")?),
        profile_id: parse_uuid(
            row.try_get("external_profile_id")?,
            "accounts.external_profile_id",
        )?,
        display_name: row.try_get("display_name")?,
        credential_ref: row.try_get("credential_ref")?,
        skin_url: row.try_get("skin_ref")?,
        status: AccountStatus::try_from(status.as_str())?,
        is_default: row.try_get::<i64, _>("default_flag")? != 0,
        last_validated_at: row.try_get("last_validated_at")?,
    })
}

fn parse_uuid(value: String, field: &'static str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(&value).map_err(|source| StorageError::InvalidStoredId { field, source })
}

fn validate_display_name(value: &str) -> Result<(), StorageError> {
    if value.is_empty()
        || value.len() > 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(StorageError::InvalidStoredValue {
            field: "accounts.display_name",
            value: "<invalid>".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AccountStatus, AuthenticatedAccount};
    use crate::{Database, StorageError};
    use uuid::Uuid;

    #[tokio::test]
    async fn authenticated_accounts_roundtrip_and_default_safely()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let first = database
            .upsert_authenticated_account(AuthenticatedAccount {
                profile_id: Uuid::new_v4(),
                display_name: "FirstPlayer".to_owned(),
                credential_ref: format!("microsoft-refresh:{}", Uuid::new_v4()),
                skin_url: None,
            })
            .await?;
        let second = database
            .upsert_authenticated_account(AuthenticatedAccount {
                profile_id: Uuid::new_v4(),
                display_name: "SecondPlayer".to_owned(),
                credential_ref: format!("microsoft-refresh:{}", Uuid::new_v4()),
                skin_url: Some("https://example.com/skin.png".to_owned()),
            })
            .await?;
        assert!(first.is_default);
        assert!(!second.is_default);

        let second = database.set_default_account(second.id).await?;
        assert!(second.is_default);
        database
            .mark_account_reauthentication_required(second.id)
            .await?;
        assert_eq!(
            database.get_account(second.id).await?.status,
            AccountStatus::ReauthenticationRequired
        );
        let credential_ref = database.remove_account(second.id).await?;
        assert!(credential_ref.starts_with("microsoft-refresh:"));
        assert!(
            database
                .get_account(second.id)
                .await
                .is_err_and(|error| { matches!(error, StorageError::AccountNotFound) })
        );
        assert!(database.list_accounts().await?[0].is_default);
        database.close().await;
        Ok(())
    }
}
