use crate::database::now_rfc3339;
use crate::{Database, StorageError};
use slate_domain::{
    InstanceId, InstanceMode, InstanceName, InstanceSetupState, LoaderFamily, ManagementMode,
    StorageRootId,
};
use slate_modpack_api_contracts::Provider;
use slate_platform::ManagedRelativePath;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewInstance {
    pub name: InstanceName,
    pub mode: InstanceMode,
    pub management_mode: ManagementMode,
    pub root_id: StorageRootId,
    pub minecraft_version: String,
    pub loader_kind: LoaderFamily,
    pub loader_version: Option<String>,
    pub memory_mb: u32,
    pub modpack_source: Option<NewModpackSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewModpackSource {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub selected_optional: Vec<String>,
    pub display_name: String,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModpackSourceRecord {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub selected_optional: Vec<String>,
    pub display_name: String,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceRecord {
    pub id: InstanceId,
    pub name: InstanceName,
    pub mode: InstanceMode,
    pub management_mode: ManagementMode,
    pub root_id: StorageRootId,
    pub relative_path: ManagedRelativePath,
    pub favorite: bool,
    pub revision: u64,
    pub minecraft_version: String,
    pub loader_kind: LoaderFamily,
    pub loader_version: Option<String>,
    pub memory_mb: u32,
    pub setup_state: InstanceSetupState,
    pub modpack_source: Option<ModpackSourceRecord>,
    pub created_at: String,
    pub updated_at: String,
    pub last_played: Option<String>,
}

impl Database {
    pub async fn create_storage_root(
        &self,
        canonical_path: &str,
        device_fingerprint: Option<&str>,
    ) -> Result<StorageRootId, StorageError> {
        let id = StorageRootId::new();
        let now = now_rfc3339()?;
        sqlx::query(
            "INSERT INTO storage_roots \
             (id, canonical_path, device_fingerprint, state, created_at, updated_at) \
             VALUES (?, ?, ?, 'available', ?, ?)",
        )
        .bind(id.to_string())
        .bind(canonical_path)
        .bind(device_fingerprint)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn get_or_create_storage_root(
        &self,
        canonical_path: &str,
    ) -> Result<StorageRootId, StorageError> {
        if let Some(value) =
            sqlx::query_scalar::<_, String>("SELECT id FROM storage_roots WHERE canonical_path = ?")
                .bind(canonical_path)
                .fetch_optional(&self.pool)
                .await?
        {
            return parse_uuid(value, "storage_roots.id").map(StorageRootId::from_uuid);
        }

        match self.create_storage_root(canonical_path, None).await {
            Ok(id) => Ok(id),
            Err(StorageError::Database(error))
                if error
                    .as_database_error()
                    .is_some_and(|database_error| database_error.is_unique_violation()) =>
            {
                let value: String =
                    sqlx::query_scalar("SELECT id FROM storage_roots WHERE canonical_path = ?")
                        .bind(canonical_path)
                        .fetch_one(&self.pool)
                        .await?;
                parse_uuid(value, "storage_roots.id").map(StorageRootId::from_uuid)
            }
            Err(error) => Err(error),
        }
    }

    pub async fn create_instance(
        &self,
        instance: NewInstance,
    ) -> Result<InstanceRecord, StorageError> {
        let id = InstanceId::new();
        let relative_path_value = format!("instances/{id}");
        let relative_path = ManagedRelativePath::parse(&relative_path_value)?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO instances \
             (id, name, mode, management_mode, root_id, relative_path, favorite, revision, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, 0, ?, ?)",
        )
        .bind(id.to_string())
        .bind(instance.name.as_str())
        .bind(instance.mode.as_storage_value())
        .bind(instance.management_mode.as_storage_value())
        .bind(instance.root_id.to_string())
        .bind(relative_path.as_str())
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "INSERT INTO instance_configuration \
             (instance_id, minecraft_version, loader_kind, loader_version, memory_mb, setup_state) \
             VALUES (?, ?, ?, ?, ?, 'configured')",
        )
        .bind(id.to_string())
        .bind(instance.minecraft_version.trim())
        .bind(instance.loader_kind.as_storage_value())
        .bind(instance.loader_version.as_deref().map(str::trim))
        .bind(i64::from(instance.memory_mb))
        .execute(&mut *transaction)
        .await?;

        if let Some(source) = &instance.modpack_source {
            sqlx::query(
                "INSERT INTO instance_modpacks \
                 (instance_id, provider, project_id, version_id, selected_optional_json, \
                  display_name, icon_url, banner_url, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id.to_string())
            .bind(source.provider.as_str())
            .bind(source.project_id.trim())
            .bind(source.version_id.trim())
            .bind(serde_json::to_string(&source.selected_optional)?)
            .bind(source.display_name.trim())
            .bind(source.icon_url.as_deref())
            .bind(source.banner_url.as_deref())
            .bind(&now)
            .bind(&now)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        self.get_instance(id).await
    }

    pub async fn get_instance(&self, id: InstanceId) -> Result<InstanceRecord, StorageError> {
        let row = sqlx::query(
            "SELECT i.id, i.name, i.mode, i.management_mode, i.root_id, i.relative_path, \
             i.favorite, i.revision, i.created_at, i.updated_at, c.minecraft_version, \
             c.loader_kind, c.loader_version, c.memory_mb, c.setup_state, \
             m.provider AS modpack_provider, m.project_id AS modpack_project_id, \
             m.version_id AS modpack_version_id, m.selected_optional_json, \
             m.display_name AS modpack_display_name, m.icon_url AS modpack_icon_url, \
             m.banner_url AS modpack_banner_url, \
             (SELECT MAX(s.started_at) FROM sessions s WHERE s.instance_id = i.id \
              AND s.state IN ('running', 'exited', 'crashed', 'cancelled')) AS last_played \
             FROM instances i INNER JOIN instance_configuration c ON c.instance_id = i.id \
             LEFT JOIN instance_modpacks m ON m.instance_id = i.id \
             WHERE i.id = ? AND i.trashed_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::InstanceNotFound)?;

        row_to_instance(&row)
    }

    pub async fn list_instances(&self, limit: u32) -> Result<Vec<InstanceRecord>, StorageError> {
        if !(1..=1_000).contains(&limit) {
            return Err(StorageError::InvalidPageLimit);
        }

        let rows = sqlx::query(
            "SELECT i.id, i.name, i.mode, i.management_mode, i.root_id, i.relative_path, \
             i.favorite, i.revision, i.created_at, i.updated_at, c.minecraft_version, \
             c.loader_kind, c.loader_version, c.memory_mb, c.setup_state, \
             m.provider AS modpack_provider, m.project_id AS modpack_project_id, \
             m.version_id AS modpack_version_id, m.selected_optional_json, \
             m.display_name AS modpack_display_name, m.icon_url AS modpack_icon_url, \
             m.banner_url AS modpack_banner_url, \
             (SELECT MAX(s.started_at) FROM sessions s WHERE s.instance_id = i.id \
              AND s.state IN ('running', 'exited', 'crashed', 'cancelled')) AS last_played \
             FROM instances i INNER JOIN instance_configuration c ON c.instance_id = i.id \
             LEFT JOIN instance_modpacks m ON m.instance_id = i.id \
             WHERE i.trashed_at IS NULL \
             ORDER BY i.favorite DESC, i.name COLLATE NOCASE, i.id LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_instance).collect()
    }

    pub async fn rename_instance(
        &self,
        id: InstanceId,
        name: &InstanceName,
        expected_revision: u64,
    ) -> Result<InstanceRecord, StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let result = sqlx::query(
            "UPDATE instances SET name = ?, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(name.as_str())
        .bind(now_rfc3339()?)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&self.pool)
        .await?;

        self.ensure_updated(id, expected_revision, result.rows_affected())
            .await?;
        self.get_instance(id).await
    }

    pub async fn update_instance_configuration(
        &self,
        id: InstanceId,
        minecraft_version: &str,
        loader_kind: LoaderFamily,
        loader_version: Option<&str>,
        memory_mb: u32,
        expected_revision: u64,
    ) -> Result<InstanceRecord, StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE instances SET revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(&now)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&mut *transaction)
        .await?;

        if result.rows_affected() == 0 {
            transaction.rollback().await?;
            return self.revision_error(id, expected_revision).await;
        }

        let current = sqlx::query(
            "SELECT minecraft_version, loader_kind, loader_version \
             FROM instance_configuration WHERE instance_id = ?",
        )
        .bind(id.to_string())
        .fetch_one(&mut *transaction)
        .await?;
        let current_minecraft_version: String = current.try_get("minecraft_version")?;
        let current_loader_kind: String = current.try_get("loader_kind")?;
        let current_loader_version: Option<String> = current.try_get("loader_version")?;
        let minecraft_version = minecraft_version.trim();
        let loader_version = loader_version.map(str::trim);
        let runtime_changed = current_minecraft_version != minecraft_version
            || current_loader_kind != loader_kind.as_storage_value()
            || current_loader_version.as_deref() != loader_version;

        if runtime_changed {
            sqlx::query(
                "UPDATE instance_configuration SET minecraft_version = ?, loader_kind = ?, \
                 loader_version = ?, memory_mb = ?, setup_state = 'configured' \
                 WHERE instance_id = ?",
            )
            .bind(minecraft_version)
            .bind(loader_kind.as_storage_value())
            .bind(loader_version)
            .bind(i64::from(memory_mb))
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await?;
            // A manually changed runtime can no longer claim the source pack's exact compatibility.
            sqlx::query("DELETE FROM instance_modpacks WHERE instance_id = ?")
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM instance_mods WHERE instance_id = ?")
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await?;
        } else {
            sqlx::query("UPDATE instance_configuration SET memory_mb = ? WHERE instance_id = ?")
                .bind(i64::from(memory_mb))
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        self.get_instance(id).await
    }

    pub async fn set_instance_favorite(
        &self,
        id: InstanceId,
        favorite: bool,
        expected_revision: u64,
    ) -> Result<InstanceRecord, StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let result = sqlx::query(
            "UPDATE instances SET favorite = ?, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(i64::from(favorite))
        .bind(now_rfc3339()?)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&self.pool)
        .await?;

        self.ensure_updated(id, expected_revision, result.rows_affected())
            .await?;
        self.get_instance(id).await
    }

    pub async fn trash_instance(
        &self,
        id: InstanceId,
        expected_revision: u64,
    ) -> Result<(), StorageError> {
        let expected_revision = revision_to_i64(expected_revision)?;
        let now = now_rfc3339()?;
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE instances SET trashed_at = ?, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND revision = ? AND trashed_at IS NULL",
        )
        .bind(&now)
        .bind(&now)
        .bind(id.to_string())
        .bind(expected_revision)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            transaction.rollback().await?;
            return self.revision_error(id, expected_revision).await;
        }
        sqlx::query(
            "UPDATE jobs SET state = 'cancelled', phase = 'cancelled', \
             progress_json = json_set(progress_json, '$.message', \
                 'Installation stopped because the instance was moved to trash.', \
                 '$.completedItems', NULL, '$.totalItems', NULL), updated_at = ? \
             WHERE kind = 'instance_install' AND entity_id = ? \
                 AND state IN ('queued', 'running')",
        )
        .bind(&now)
        .bind(id.to_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE instance_revisions SET status = 'failed' WHERE instance_id = ? \
             AND status IN ('proposed', 'staged')",
        )
        .bind(id.to_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn ensure_updated(
        &self,
        id: InstanceId,
        expected_revision: i64,
        rows_affected: u64,
    ) -> Result<(), StorageError> {
        if rows_affected == 0 {
            return self.revision_error(id, expected_revision).await;
        }
        Ok(())
    }

    async fn revision_error<T>(
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

fn row_to_instance(row: &sqlx::sqlite::SqliteRow) -> Result<InstanceRecord, StorageError> {
    let id = parse_uuid(row.try_get("id")?, "instances.id")?;
    let root_id = parse_uuid(row.try_get("root_id")?, "instances.root_id")?;
    let mode_value: String = row.try_get("mode")?;
    let management_value: String = row.try_get("management_mode")?;
    let name_value: String = row.try_get("name")?;
    let relative_path: String = row.try_get("relative_path")?;
    let loader_value: String = row.try_get("loader_kind")?;
    let setup_state_value: String = row.try_get("setup_state")?;
    let revision: i64 = row.try_get("revision")?;
    let memory_mb: i64 = row.try_get("memory_mb")?;
    let favorite: i64 = row.try_get("favorite")?;
    let modpack_provider: Option<String> = row.try_get("modpack_provider")?;
    let modpack_source = modpack_provider
        .map(|provider| -> Result<ModpackSourceRecord, StorageError> {
            Ok(ModpackSourceRecord {
                provider: Provider::from_str(&provider)
                    .map_err(|_| invalid_value("instance_modpacks.provider", provider))?,
                project_id: row.try_get("modpack_project_id")?,
                version_id: row.try_get("modpack_version_id")?,
                selected_optional: serde_json::from_str(
                    &row.try_get::<String, _>("selected_optional_json")?,
                )?,
                display_name: row.try_get("modpack_display_name")?,
                icon_url: row.try_get("modpack_icon_url")?,
                banner_url: row.try_get("modpack_banner_url")?,
            })
        })
        .transpose()?;

    Ok(InstanceRecord {
        id: InstanceId::from_uuid(id),
        name: InstanceName::parse(name_value)
            .map_err(|_| invalid_value("instances.name", "invalid instance name".to_owned()))?,
        mode: InstanceMode::try_from(mode_value.as_str())
            .map_err(|_| invalid_value("instances.mode", mode_value))?,
        management_mode: ManagementMode::try_from(management_value.as_str())
            .map_err(|_| invalid_value("instances.management_mode", management_value))?,
        root_id: StorageRootId::from_uuid(root_id),
        relative_path: ManagedRelativePath::parse(&relative_path)?,
        favorite: favorite != 0,
        revision: u64::try_from(revision).map_err(|_| StorageError::NegativeRevision)?,
        minecraft_version: row.try_get("minecraft_version")?,
        loader_kind: LoaderFamily::try_from(loader_value.as_str())
            .map_err(|_| invalid_value("instance_configuration.loader_kind", loader_value))?,
        loader_version: row.try_get("loader_version")?,
        memory_mb: u32::try_from(memory_mb).map_err(|_| {
            invalid_value("instance_configuration.memory_mb", memory_mb.to_string())
        })?,
        setup_state: InstanceSetupState::try_from(setup_state_value.as_str())
            .map_err(|_| invalid_value("instance_configuration.setup_state", setup_state_value))?,
        modpack_source,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        last_played: row.try_get("last_played")?,
    })
}

fn invalid_value(field: &'static str, value: String) -> StorageError {
    StorageError::InvalidStoredValue { field, value }
}

fn revision_to_i64(revision: u64) -> Result<i64, StorageError> {
    i64::try_from(revision).map_err(|_| StorageError::RevisionConflict { expected: revision })
}

fn parse_uuid(value: String, field: &'static str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(&value).map_err(|source| StorageError::InvalidStoredId { field, source })
}

#[cfg(test)]
mod tests {
    use super::{NewInstance, NewModpackSource};
    use crate::{Database, StorageError};
    use slate_domain::{
        InstanceMode, InstanceName, InstanceSetupState, LoaderFamily, ManagementMode,
    };
    use slate_modpack_api_contracts::Provider;

    fn vanilla(
        root_id: slate_domain::StorageRootId,
        name: &str,
    ) -> Result<NewInstance, Box<dyn std::error::Error>> {
        Ok(NewInstance {
            name: InstanceName::parse(name)?,
            mode: InstanceMode::Vanilla,
            management_mode: ManagementMode::Local,
            root_id,
            minecraft_version: "1.21.1".to_owned(),
            loader_kind: LoaderFamily::Vanilla,
            loader_version: None,
            memory_mb: 4096,
            modpack_source: None,
        })
    }

    #[tokio::test]
    async fn configured_instance_roundtrip_and_revision_guard()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let root = database
            .create_storage_root("C:/slate", Some("test-disk"))
            .await?;
        let created = database.create_instance(vanilla(root, "Survival")?).await?;
        let renamed = database
            .rename_instance(created.id, &InstanceName::parse("Survival Two")?, 0)
            .await?;
        let stale = database
            .rename_instance(created.id, &InstanceName::parse("Stale")?, 0)
            .await;

        assert_eq!(created.minecraft_version, "1.21.1");
        assert_eq!(created.loader_kind, LoaderFamily::Vanilla);
        assert_eq!(renamed.name.as_str(), "Survival Two");
        assert_eq!(renamed.revision, 1);
        assert!(matches!(
            stale,
            Err(StorageError::RevisionConflict { expected: 0 })
        ));
        database.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn trashes_without_deleting_the_instance_record() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let root = database.create_storage_root("C:/slate", None).await?;
        let created = database
            .create_instance(vanilla(root, "Temporary")?)
            .await?;

        database.trash_instance(created.id, 0).await?;

        assert!(matches!(
            database.get_instance(created.id).await,
            Err(StorageError::InstanceNotFound)
        ));
        database.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn modpack_source_roundtrips_and_detaches_after_runtime_change()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let root = database.create_storage_root("C:/slate", None).await?;
        let mut instance = vanilla(root, "Pack")?;
        instance.mode = InstanceMode::Modded;
        instance.loader_kind = LoaderFamily::Fabric;
        instance.loader_version = Some("0.18.4".to_owned());
        instance.modpack_source = Some(NewModpackSource {
            provider: Provider::Modrinth,
            project_id: "pack-id".to_owned(),
            version_id: "version-id".to_owned(),
            selected_optional: vec!["optional-shaders".to_owned()],
            display_name: "Example Pack".to_owned(),
            icon_url: Some("https://cdn.modrinth.com/icon.png".to_owned()),
            banner_url: Some("https://cdn.modrinth.com/banner.png".to_owned()),
        });
        let created = database.create_instance(instance).await?;
        let source = created.modpack_source.ok_or("missing source")?;
        assert_eq!(source.provider, Provider::Modrinth);
        assert_eq!(source.selected_optional, ["optional-shaders"]);
        assert_eq!(
            source.banner_url.as_deref(),
            Some("https://cdn.modrinth.com/banner.png")
        );

        let updated = database
            .update_instance_configuration(
                created.id,
                "1.21.1",
                LoaderFamily::Vanilla,
                None,
                4096,
                created.revision,
            )
            .await?;
        assert!(updated.modpack_source.is_none());
        database.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn memory_change_preserves_ready_installation_and_modpack_source()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let root = database.create_storage_root("C:/slate", None).await?;
        let mut instance = vanilla(root, "Pack")?;
        instance.mode = InstanceMode::Modded;
        instance.loader_kind = LoaderFamily::Fabric;
        instance.loader_version = Some("0.18.4".to_owned());
        instance.modpack_source = Some(NewModpackSource {
            provider: Provider::Modrinth,
            project_id: "pack-id".to_owned(),
            version_id: "version-id".to_owned(),
            selected_optional: Vec::new(),
            display_name: "Example Pack".to_owned(),
            icon_url: None,
            banner_url: None,
        });
        let created = database.create_instance(instance).await?;
        sqlx::query(
            "UPDATE instance_configuration SET setup_state = 'ready' WHERE instance_id = ?",
        )
        .bind(created.id.to_string())
        .execute(&database.pool)
        .await?;

        let updated = database
            .update_instance_configuration(
                created.id,
                "1.21.1",
                LoaderFamily::Fabric,
                Some("0.18.4"),
                8192,
                created.revision,
            )
            .await?;

        assert_eq!(updated.memory_mb, 8192);
        assert_eq!(updated.setup_state, InstanceSetupState::Ready);
        assert!(updated.modpack_source.is_some());
        database.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn storage_root_foreign_key_is_enforced() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = Database::connect(&directory.path().join("state.sqlite")).await?;
        let result = database
            .create_instance(vanilla(slate_domain::StorageRootId::new(), "No root")?)
            .await;

        assert!(matches!(result, Err(StorageError::Database(_))));
        database.close().await;
        Ok(())
    }
}
