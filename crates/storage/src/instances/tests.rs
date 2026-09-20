use super::{NewInstance, NewModpackSource};
use crate::{Database, StorageError};
use slate_domain::{InstanceMode, InstanceName, InstanceSetupState, LoaderFamily, ManagementMode};
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
async fn configured_instance_roundtrip_and_revision_guard() -> Result<(), Box<dyn std::error::Error>>
{
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
async fn trashes_without_deleting_the_instance_record() -> Result<(), Box<dyn std::error::Error>> {
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
async fn trashed_instances_can_be_restored_or_permanently_deleted()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = Database::connect(&directory.path().join("state.sqlite")).await?;
    let root_path = directory.path().join("storage");
    std::fs::create_dir_all(&root_path)?;
    let root = database
        .create_storage_root(root_path.to_string_lossy().as_ref(), None)
        .await?;
    let created = database
        .create_instance(vanilla(root, "Recoverable")?)
        .await?;

    database
        .trash_instance(created.id, created.revision)
        .await?;
    let trashed = database.list_trashed_instances(10).await?;
    assert_eq!(trashed.len(), 1);
    assert_eq!(trashed[0].name.as_str(), "Recoverable");
    let restored = database
        .restore_trashed_instance(created.id, trashed[0].revision)
        .await?;
    assert_eq!(restored.revision, 2);

    database
        .trash_instance(restored.id, restored.revision)
        .await?;
    let trashed = database.get_trashed_instance(restored.id).await?;
    database
        .permanently_delete_trashed_instance_record(restored.id, trashed.revision)
        .await?;
    assert!(matches!(
        database.get_trashed_instance(restored.id).await,
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
    sqlx::query("UPDATE instance_configuration SET setup_state = 'ready' WHERE instance_id = ?")
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
