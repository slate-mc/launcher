use super::*;

pub(super) async fn hash_launch_artifacts(
    paths: &AppPaths,
    requirements: Vec<ArtifactRequirement>,
) -> Result<Vec<InstalledArtifactDigest>, InstallError> {
    let storage_root = paths.storage_root().to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut artifacts = Vec::with_capacity(requirements.len());
        for requirement in requirements {
            artifacts.push(InstalledArtifactDigest {
                relative_path: relative_artifact_path(&storage_root, requirement.target_path())?,
                sha256: sha256_file(requirement.target_path())?,
            });
        }
        artifacts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        validate_installed_artifact_declarations(&artifacts)?;
        Ok(artifacts)
    })
    .await?
}

pub async fn verify_installed_launch_artifacts(
    storage_root: &Path,
    requirements: &[ArtifactRequirement],
    installed: &[InstalledArtifactDigest],
) -> Result<(), InstallError> {
    let storage_root = storage_root.to_path_buf();
    let requirements = requirements.to_vec();
    let installed = installed.to_vec();
    tokio::task::spawn_blocking(move || {
        validate_installed_artifact_declarations(&installed)?;
        let declared = installed
            .into_iter()
            .map(|artifact| (artifact.relative_path, artifact.sha256))
            .collect::<BTreeMap<_, _>>();
        for requirement in requirements {
            let relative = relative_artifact_path(&storage_root, requirement.target_path())?;
            let expected = declared
                .get(&relative)
                .ok_or_else(|| InstallError::LaunchArtifactNotDeclared(relative.clone()))?;
            let actual = sha256_file(requirement.target_path())?;
            if actual != *expected {
                return Err(InstallError::InstalledArtifactHashMismatch(relative));
            }
        }
        Ok(())
    })
    .await?
}

pub async fn verify_installed_content_artifact(
    storage_root: &Path,
    target: &Path,
    expected_sha256: &str,
    installed: &[InstalledArtifactDigest],
) -> Result<(), InstallError> {
    if !valid_sha256(expected_sha256) {
        return Err(InstallError::InvalidInstalledManifest);
    }
    let relative = relative_artifact_path(storage_root, target)?;
    let declared = installed
        .iter()
        .find(|artifact| artifact.relative_path == relative)
        .ok_or_else(|| InstallError::LaunchArtifactNotDeclared(relative.clone()))?;
    if declared.sha256 != expected_sha256.to_ascii_lowercase() {
        return Err(InstallError::InstalledArtifactHashMismatch(relative));
    }
    let target = target.to_path_buf();
    let actual = tokio::task::spawn_blocking(move || sha256_file(&target)).await??;
    if actual != declared.sha256 {
        return Err(InstallError::InstalledArtifactHashMismatch(relative));
    }
    Ok(())
}

pub(super) fn validate_installed_artifact_declarations(
    artifacts: &[InstalledArtifactDigest],
) -> Result<(), InstallError> {
    if artifacts.is_empty() || artifacts.len() > MAX_LAUNCH_ARTIFACTS {
        return Err(InstallError::InvalidInstalledManifest);
    }
    let mut unique = BTreeSet::new();
    for artifact in artifacts {
        let path = ManagedRelativePath::parse(&artifact.relative_path)
            .map_err(|_| InstallError::InvalidInstalledManifest)?;
        if path.as_str() != artifact.relative_path
            || !valid_sha256(&artifact.sha256)
            || !unique.insert(artifact.relative_path.as_str())
        {
            return Err(InstallError::InvalidInstalledManifest);
        }
    }
    Ok(())
}

pub(super) fn relative_artifact_path(root: &Path, target: &Path) -> Result<String, InstallError> {
    let relative = match (std::fs::canonicalize(root), std::fs::canonicalize(target)) {
        (Ok(canonical_root), Ok(canonical_target)) => canonical_target
            .strip_prefix(canonical_root)
            .map(Path::to_path_buf)
            .map_err(|_| InstallError::ArtifactOutsideStorageRoot)?,
        _ => target
            .strip_prefix(root)
            .map(Path::to_path_buf)
            .map_err(|_| InstallError::ArtifactOutsideStorageRoot)?,
    };
    let mut segments = Vec::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(InstallError::ArtifactOutsideStorageRoot);
        };
        segments.push(
            segment
                .to_str()
                .ok_or(InstallError::ArtifactOutsideStorageRoot)?,
        );
    }
    let relative = segments.join("/");
    let managed = ManagedRelativePath::parse(&relative)
        .map_err(|_| InstallError::ArtifactOutsideStorageRoot)?;
    Ok(managed.as_str().to_owned())
}

pub(super) async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), InstallError> {
    let parent = path
        .parent()
        .ok_or_else(|| InstallError::ParentMissing(path.to_path_buf()))?;
    tokio::fs::create_dir_all(parent).await?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| InstallError::ParentMissing(path.to_path_buf()))?;
    let partial = path.with_file_name(format!(".{name}.partial-{}", Uuid::new_v4()));
    tokio::fs::write(&partial, bytes).await?;
    if path.exists() {
        tokio::fs::remove_file(path).await?;
    }
    tokio::fs::rename(partial, path).await?;
    Ok(())
}

pub(super) fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest_hex(&digest.finalize()))
}

pub(super) fn digest_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

pub(super) fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
