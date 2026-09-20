use super::{
    InstallError, InstallRequest, InstalledArtifactDigest, clone_directory_tree,
    load_installed_revision, relative_artifact_path, validate_installed_artifact_declarations,
    validate_request,
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
        download_concurrency: 4,
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
