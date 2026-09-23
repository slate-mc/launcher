use super::{
    InstallError, InstallRequest, InstalledArtifactDigest, clone_directory_tree,
    load_installed_revision, relative_artifact_path, remove_deleted_content_artifacts,
    validate_installed_artifact_declarations, validate_request,
};
use slate_domain::{InstanceId, LoaderFamily, RevisionId};
use slate_platform::AppPaths;
use std::path::PathBuf;

#[test]
fn validates_loader_version_shape() {
    let request = InstallRequest {
        instance_id: InstanceId::new(),
        revision_id: RevisionId::new(),
        minecraft_version: "1.21.1".to_owned(),
        loader_kind: LoaderFamily::Fabric,
        loader_version: None,
        modpack_plan: None,
        managed_content: Vec::new(),
        download_concurrency: 4,
        download_bandwidth_limit_mib: 0,
        paths: AppPaths::from_roots(PathBuf::from("C:/slate"), PathBuf::from("C:/slate/storage")),
    };
    assert!(matches!(
        validate_request(&request),
        Err(InstallError::LoaderVersionRequired)
    ));
}

#[test]
fn installed_artifact_declarations_are_unique_and_managed() {
    let valid = InstalledArtifactDigest {
        relative_path: "artifacts/minecraft/libraries/example.jar".to_owned(),
        sha256: "a".repeat(64),
    };
    assert!(validate_installed_artifact_declarations(std::slice::from_ref(&valid)).is_ok());
    assert!(matches!(
        validate_installed_artifact_declarations(&[valid.clone(), valid]),
        Err(InstallError::InvalidInstalledManifest)
    ));
    assert!(
        relative_artifact_path(
            std::path::Path::new("C:/slate/storage"),
            std::path::Path::new("C:/slate/outside.jar")
        )
        .is_err()
    );
}

#[test]
fn artifact_paths_accept_equivalent_canonical_windows_roots()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let artifact = directory.path().join("artifacts/example.jar");
    std::fs::create_dir_all(artifact.parent().ok_or("artifact parent is missing")?)?;
    std::fs::write(&artifact, b"artifact")?;

    let canonical_artifact = std::fs::canonicalize(&artifact)?;
    assert_eq!(
        relative_artifact_path(directory.path(), &canonical_artifact)?,
        "artifacts/example.jar"
    );
    Ok(())
}

#[tokio::test]
async fn installed_manifest_is_bound_to_its_database_digest()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("installed-revision.json");
    tokio::fs::write(&path, b"{}").await?;
    let result =
        load_installed_revision(&path, InstanceId::new(), RevisionId::new(), &"0".repeat(64)).await;
    assert!(matches!(
        result,
        Err(InstallError::InstalledManifestDigestMismatch)
    ));
    Ok(())
}

#[test]
fn content_updates_clone_revision_native_files() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("parent-natives");
    let nested = source.join("nested");
    let destination = directory.path().join("next-natives");
    std::fs::create_dir_all(&nested)?;
    std::fs::write(source.join("lwjgl.dll"), b"native-one")?;
    std::fs::write(nested.join("helper.dll"), b"native-two")?;

    clone_directory_tree(&source, &destination)?;

    assert_eq!(std::fs::read(destination.join("lwjgl.dll"))?, b"native-one");
    assert_eq!(
        std::fs::read(destination.join("nested/helper.dll"))?,
        b"native-two"
    );
    Ok(())
}

#[test]
fn content_updates_remove_deleted_files_from_the_verified_index()
-> Result<(), Box<dyn std::error::Error>> {
    let storage_root = PathBuf::from("C:/slate/storage");
    let game_directory = storage_root.join("instances/example/game");
    let removed_path = "instances/example/game/mods/old.jar".to_owned();
    let retained_path = "instances/example/game/mods/kept.jar".to_owned();
    let mut artifacts = std::collections::BTreeMap::from([
        (
            removed_path.clone(),
            InstalledArtifactDigest {
                relative_path: removed_path,
                sha256: "a".repeat(64),
            },
        ),
        (
            retained_path.clone(),
            InstalledArtifactDigest {
                relative_path: retained_path.clone(),
                sha256: "b".repeat(64),
            },
        ),
    ]);

    remove_deleted_content_artifacts(
        &mut artifacts,
        &["mods/old.jar".to_owned()],
        &game_directory,
        &storage_root,
    )?;

    assert_eq!(artifacts.len(), 1);
    assert!(artifacts.contains_key(&retained_path));
    Ok(())
}
