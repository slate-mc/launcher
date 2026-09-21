use super::{
    CompletedInstall, CompletedModpackUpdate, InstalledRuntime, JobState, valid_install_operation,
};
use crate::{AuthenticatedAccount, Database, NewInstance, NewModpackSource, SessionState};
use slate_domain::{
    InstanceMode, InstanceName, LoaderFamily, ManagementMode, RequestId, SessionId,
};
use slate_modpack_api_contracts::Provider;
use uuid::Uuid;

#[test]
fn job_states_match_database_values() {
    assert_eq!(JobState::Running.as_storage_value(), "running");
    assert_eq!(JobState::Paused.as_storage_value(), "paused");
    assert_eq!(
        JobState::try_from("succeeded").ok(),
        Some(JobState::Succeeded)
    );
}

#[test]
fn install_operations_cover_every_retryable_job() {
    for operation in [
        "instance_install",
        "external_instance_import",
        "imported_pack_install",
        "mod_install",
        "mod_update",
        "modpack_update",
    ] {
        assert!(valid_install_operation(operation), "rejected {operation}");
    }
    assert!(!valid_install_operation("unknown"));
}

#[tokio::test]
async fn active_install_can_pause_resume_and_cancel() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = Database::connect(&directory.path().join("state.sqlite")).await?;
    let root = database.create_storage_root("C:/slate", None).await?;
    let instance = database
        .create_instance(NewInstance {
            name: InstanceName::parse("Paused")?,
            mode: InstanceMode::Vanilla,
            management_mode: ManagementMode::Local,
            root_id: root,
            minecraft_version: "1.21.1".to_owned(),
            loader_kind: LoaderFamily::Vanilla,
            loader_version: None,
            memory_mb: 4096,
            modpack_source: None,
        })
        .await?;
    let pending = database
        .begin_instance_install(instance.id, instance.revision, RequestId::new())
        .await?;

    assert!(database.set_install_paused(pending.job.id, true).await?);
    assert_eq!(
        database.get_install_job(pending.job.id).await?.state,
        JobState::Paused
    );
    assert!(database.has_active_install_jobs().await?);
    assert!(database.set_install_paused(pending.job.id, false).await?);
    assert_eq!(
        database.get_install_job(pending.job.id).await?.state,
        JobState::Running
    );
    database
        .cancel_instance_install(
            pending.job.id,
            pending.revision_id,
            "Installation cancelled",
        )
        .await?;
    assert_eq!(
        database.get_install_job(pending.job.id).await?.state,
        JobState::Cancelled
    );
    database.close().await;
    Ok(())
}

#[tokio::test]
async fn completed_modpack_update_advances_source_and_loader_together()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = Database::connect(&directory.path().join("state.sqlite")).await?;
    let root = database.create_storage_root("C:/slate", None).await?;
    let instance = database
        .create_instance(NewInstance {
            name: InstanceName::parse("Updated Pack")?,
            mode: InstanceMode::Modded,
            management_mode: ManagementMode::Local,
            root_id: root,
            minecraft_version: "1.21.1".to_owned(),
            loader_kind: LoaderFamily::NeoForge,
            loader_version: Some("21.1.100".to_owned()),
            memory_mb: 8192,
            modpack_source: Some(NewModpackSource {
                provider: Provider::CurseForge,
                project_id: "123".to_owned(),
                version_id: "old".to_owned(),
                selected_optional: Vec::new(),
                display_name: "Updated Pack".to_owned(),
                icon_url: None,
                banner_url: None,
            }),
        })
        .await?;
    let pending = database
        .begin_instance_install(instance.id, instance.revision, RequestId::new())
        .await?;

    database
        .complete_instance_modpack_update(
            CompletedInstall {
                job_id: pending.job.id,
                revision_id: pending.revision_id,
                manifest_digest: "manifest-digest".to_owned(),
                client_version: "1.21.1-neoforge-21.1.200".to_owned(),
                runtime: InstalledRuntime {
                    vendor: "test".to_owned(),
                    release_name: "java-21".to_owned(),
                    java_version: "21.0.1".to_owned(),
                    major: 21,
                    os: "windows".to_owned(),
                    arch: "x86_64".to_owned(),
                    executable_ref: "C:/slate/runtimes/java.exe".to_owned(),
                    source_digest: "runtime-digest".to_owned(),
                },
                message: "Updated".to_owned(),
            },
            CompletedModpackUpdate {
                version_id: "new".to_owned(),
                loader_version: Some("21.1.200".to_owned()),
            },
        )
        .await?;

    let updated = database.get_instance(instance.id).await?;
    assert_eq!(updated.loader_version.as_deref(), Some("21.1.200"));
    assert_eq!(
        updated
            .modpack_source
            .as_ref()
            .map(|source| source.version_id.as_str()),
        Some("new")
    );
    assert_eq!(updated.setup_state, slate_domain::InstanceSetupState::Ready);
    database.close().await;
    Ok(())
}

#[tokio::test]
async fn interrupted_install_progress_is_recovered_on_startup()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = Database::connect(&directory.path().join("state.sqlite")).await?;
    let root = database.create_storage_root("C:/slate", None).await?;
    let instance = database
        .create_instance(NewInstance {
            name: InstanceName::parse("Interrupted")?,
            mode: InstanceMode::Vanilla,
            management_mode: ManagementMode::Local,
            root_id: root,
            minecraft_version: "1.21.1".to_owned(),
            loader_kind: LoaderFamily::Vanilla,
            loader_version: None,
            memory_mb: 4096,
            modpack_source: None,
        })
        .await?;
    let pending = database
        .begin_instance_install(instance.id, instance.revision, RequestId::new())
        .await?;
    database
        .update_install_progress(
            pending.job.id,
            "assets",
            "Checking game assets",
            Some(42),
            Some(100),
        )
        .await?;

    assert_eq!(database.recover_interrupted_installs().await?, 1);
    let job = database
        .list_install_jobs(10)
        .await?
        .into_iter()
        .next()
        .ok_or("missing install job")?;
    let recovered = database.get_instance(instance.id).await?;
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.phase, "interrupted");
    assert_eq!(job.completed_items, None);
    assert_eq!(
        recovered.setup_state,
        slate_domain::InstanceSetupState::Blocked
    );
    database.close().await;
    Ok(())
}

#[tokio::test]
async fn trashing_an_instance_cancels_its_active_install() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let database = Database::connect(&directory.path().join("state.sqlite")).await?;
    let root = database.create_storage_root("C:/slate", None).await?;
    let instance = database
        .create_instance(NewInstance {
            name: InstanceName::parse("Temporary")?,
            mode: InstanceMode::Vanilla,
            management_mode: ManagementMode::Local,
            root_id: root,
            minecraft_version: "1.21.1".to_owned(),
            loader_kind: LoaderFamily::Vanilla,
            loader_version: None,
            memory_mb: 4096,
            modpack_source: None,
        })
        .await?;
    database
        .begin_instance_install(instance.id, instance.revision, RequestId::new())
        .await?;

    database
        .trash_instance(instance.id, instance.revision + 1)
        .await?;

    let job = database
        .list_install_jobs(10)
        .await?
        .into_iter()
        .next()
        .ok_or("missing install job")?;
    assert_eq!(job.state, JobState::Cancelled);
    assert_eq!(job.phase, "cancelled");
    database.close().await;
    Ok(())
}

#[tokio::test]
async fn cancelling_an_install_restores_a_retryable_instance_state()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = Database::connect(&directory.path().join("state.sqlite")).await?;
    let root = database.create_storage_root("C:/slate", None).await?;
    let instance = database
        .create_instance(NewInstance {
            name: InstanceName::parse("Cancelled")?,
            mode: InstanceMode::Vanilla,
            management_mode: ManagementMode::Local,
            root_id: root,
            minecraft_version: "1.21.1".to_owned(),
            loader_kind: LoaderFamily::Vanilla,
            loader_version: None,
            memory_mb: 4096,
            modpack_source: None,
        })
        .await?;
    let pending = database
        .begin_instance_install_with_context(
            instance.id,
            instance.revision,
            RequestId::new(),
            "mod_install",
            Some(serde_json::json!({"mods": [{"projectId": "example"}]})),
        )
        .await?;

    database
        .cancel_instance_install(
            pending.job.id,
            pending.revision_id,
            "Installation cancelled",
        )
        .await?;

    let job = database.get_install_job(pending.job.id).await?;
    let updated = database.get_instance(instance.id).await?;
    assert_eq!(job.state, JobState::Cancelled);
    assert_eq!(job.phase, "cancelled");
    assert_eq!(job.operation.as_deref(), Some("mod_install"));
    assert_eq!(
        job.retry_payload
            .as_ref()
            .and_then(|value| value.pointer("/mods/0/projectId"))
            .and_then(serde_json::Value::as_str),
        Some("example")
    );
    assert_eq!(
        updated.setup_state,
        slate_domain::InstanceSetupState::Configured
    );
    database.close().await;
    Ok(())
}

#[tokio::test]
async fn instance_last_played_tracks_sessions_that_reached_running()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = Database::connect(&directory.path().join("state.sqlite")).await?;
    let root = database.create_storage_root("C:/slate", None).await?;
    let instance = database
        .create_instance(NewInstance {
            name: InstanceName::parse("Played")?,
            mode: InstanceMode::Vanilla,
            management_mode: ManagementMode::Local,
            root_id: root,
            minecraft_version: "1.21.1".to_owned(),
            loader_kind: LoaderFamily::Vanilla,
            loader_version: None,
            memory_mb: 4096,
            modpack_source: None,
        })
        .await?;
    let pending = database
        .begin_instance_install(instance.id, instance.revision, RequestId::new())
        .await?;
    database
        .complete_instance_install(
            pending.job.id,
            pending.revision_id,
            "manifest-digest",
            "1.21.1",
            InstalledRuntime {
                vendor: "test".to_owned(),
                release_name: "java-21".to_owned(),
                java_version: "21.0.1".to_owned(),
                major: 21,
                os: "windows".to_owned(),
                arch: "x86_64".to_owned(),
                executable_ref: "C:/slate/runtimes/java.exe".to_owned(),
                source_digest: "runtime-digest".to_owned(),
            },
            "Installed",
        )
        .await?;
    let account = database
        .upsert_authenticated_account(AuthenticatedAccount {
            profile_id: Uuid::new_v4(),
            display_name: "Player".to_owned(),
            credential_ref: "credential-ref".to_owned(),
            skin_url: None,
        })
        .await?;
    let session_id = SessionId::new();
    database
        .create_session_starting(instance.id, pending.revision_id, session_id, account.id)
        .await?;

    assert!(
        database
            .get_instance(instance.id)
            .await?
            .last_played
            .is_none()
    );

    database.mark_session_running(session_id, 42).await?;

    let played = database
        .get_instance(instance.id)
        .await?
        .last_played
        .ok_or("missing last played timestamp")?;
    assert!(!played.is_empty());
    assert_eq!(
        database.list_instances(10).await?[0].last_played.as_deref(),
        Some(played.as_str())
    );
    database.finish_session(session_id, Some(1), false).await?;
    let recent = database.list_recent_sessions(instance.id, 10).await?;
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].id, session_id);
    assert_eq!(recent[0].state, SessionState::Crashed);
    assert_eq!(database.get_session(session_id).await?, recent[0]);
    database.close().await;
    Ok(())
}
