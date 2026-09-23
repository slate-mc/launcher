use sha2::{Digest, Sha256};
use slate_contracts::AppError;
use slate_domain::{InstanceMode, LoaderFamily};
use slate_installer::ManagedContentArtifact;
use slate_storage::InstanceRecord;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tauri::Manager;

const MANIFEST_SCHEMA: u32 = 1;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Default)]
pub(super) struct ClientArtifactCatalog {
    artifacts: Arc<Vec<VerifiedClientArtifact>>,
}

#[derive(Clone, Debug)]
struct VerifiedClientArtifact {
    minecraft_version: String,
    loader_kind: LoaderFamily,
    loader_version: String,
    source_path: PathBuf,
    destination: String,
    sha256: String,
    display_name: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientArtifactManifest {
    schema: u32,
    artifacts: Vec<ClientArtifactDeclaration>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientArtifactDeclaration {
    minecraft_version: String,
    loader: String,
    loader_version: String,
    file: String,
    destination: String,
    sha256: String,
    display_name: String,
}

impl ClientArtifactCatalog {
    pub(super) fn discover(app: &tauri::AppHandle) -> Self {
        let mut candidates = Vec::new();
        if let Ok(resource_dir) = app.path().resource_dir() {
            candidates.push(resource_dir.join("client"));
        }
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../desktop-resources/client"),
        );
        candidates
            .into_iter()
            .find_map(|directory| Self::load(&directory).ok())
            .unwrap_or_default()
    }

    fn load(directory: &Path) -> Result<Self, ClientArtifactError> {
        let manifest_path = directory.join("manifest.json");
        let metadata = std::fs::symlink_metadata(&manifest_path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAX_MANIFEST_BYTES
        {
            return Err(ClientArtifactError::InvalidManifest);
        }
        let manifest_bytes = std::fs::read(manifest_path)?;
        let manifest: ClientArtifactManifest = serde_json::from_slice(&manifest_bytes)?;
        if manifest.schema != MANIFEST_SCHEMA || manifest.artifacts.is_empty() {
            return Err(ClientArtifactError::InvalidManifest);
        }
        let mut artifacts = Vec::with_capacity(manifest.artifacts.len());
        let mut targets = BTreeSet::new();
        for declaration in manifest.artifacts {
            let loader_kind = match declaration.loader.as_str() {
                "fabric" => LoaderFamily::Fabric,
                "neoforge" => LoaderFamily::NeoForge,
                _ => return Err(ClientArtifactError::InvalidManifest),
            };
            if declaration.minecraft_version.trim().is_empty()
                || declaration.loader_version.trim().is_empty()
                || declaration.display_name.trim().is_empty()
                || !valid_sha256(&declaration.sha256)
            {
                return Err(ClientArtifactError::InvalidManifest);
            }
            let destination = managed_mod_destination(&declaration.destination)?;
            let target = (
                declaration.minecraft_version.clone(),
                declaration.loader.clone(),
                declaration.loader_version.clone(),
                destination.clone(),
            );
            if !targets.insert(target) {
                return Err(ClientArtifactError::InvalidManifest);
            }
            let file = single_file_name(&declaration.file)?;
            let source_path = directory.join(file);
            let artifact_metadata = std::fs::symlink_metadata(&source_path)?;
            if !artifact_metadata.is_file()
                || artifact_metadata.file_type().is_symlink()
                || artifact_metadata.len() > MAX_ARTIFACT_BYTES
            {
                return Err(ClientArtifactError::InvalidArtifact);
            }
            let actual_sha256 = sha256_file(&source_path)?;
            if actual_sha256 != declaration.sha256.to_ascii_lowercase() {
                return Err(ClientArtifactError::InvalidArtifact);
            }
            artifacts.push(VerifiedClientArtifact {
                minecraft_version: declaration.minecraft_version,
                loader_kind,
                loader_version: declaration.loader_version,
                source_path,
                destination,
                sha256: actual_sha256,
                display_name: declaration.display_name,
            });
        }
        Ok(Self {
            artifacts: Arc::new(artifacts),
        })
    }

    pub(super) fn is_available(&self) -> bool {
        self.artifacts
            .iter()
            .any(|artifact| artifact.destination == "mods/slate-client.jar")
    }

    pub(super) fn for_instance(
        &self,
        instance: &InstanceRecord,
    ) -> Result<Vec<ManagedContentArtifact>, AppError> {
        if instance.mode != InstanceMode::SlateClient {
            return Ok(Vec::new());
        }
        let artifacts = self
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact.minecraft_version == instance.minecraft_version
                    && artifact.loader_kind == instance.loader_kind
                    && Some(artifact.loader_version.as_str()) == instance.loader_version.as_deref()
            })
            .collect::<Vec<_>>();
        if !artifacts
            .iter()
            .any(|artifact| artifact.destination == "mods/slate-client.jar")
        {
            return Err(AppError::new(
                "local.slate_client_unavailable",
                "Slate Client is not available for this Minecraft and loader version yet.",
            ));
        }
        Ok(artifacts
            .into_iter()
            .map(|artifact| ManagedContentArtifact {
                source_path: artifact.source_path.clone(),
                destination: artifact.destination.clone(),
                sha256: artifact.sha256.clone(),
                display_name: artifact.display_name.clone(),
            })
            .collect())
    }
}

fn managed_mod_destination(value: &str) -> Result<String, ClientArtifactError> {
    let mut components = Path::new(value).components();
    let Some(Component::Normal(directory)) = components.next() else {
        return Err(ClientArtifactError::InvalidManifest);
    };
    let Some(Component::Normal(file)) = components.next() else {
        return Err(ClientArtifactError::InvalidManifest);
    };
    if directory != "mods"
        || components.next().is_some()
        || !file
            .to_str()
            .is_some_and(|file| file.ends_with(".jar") && file.len() > 4)
    {
        return Err(ClientArtifactError::InvalidManifest);
    }
    Ok(format!("mods/{}", file.to_string_lossy()))
}

fn single_file_name(value: &str) -> Result<&str, ClientArtifactError> {
    let mut components = Path::new(value).components();
    let Some(Component::Normal(file)) = components.next() else {
        return Err(ClientArtifactError::InvalidManifest);
    };
    if components.next().is_some() {
        return Err(ClientArtifactError::InvalidManifest);
    }
    file.to_str().ok_or(ClientArtifactError::InvalidManifest)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
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
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[derive(Debug, thiserror::Error)]
enum ClientArtifactError {
    #[error("Slate Client artifact manifest is invalid")]
    InvalidManifest,
    #[error("Slate Client artifact failed verification")]
    InvalidArtifact,
    #[error("Slate Client artifact could not be read")]
    Io(#[from] std::io::Error),
    #[error("Slate Client artifact manifest could not be parsed")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::{ClientArtifactCatalog, sha256_file};

    #[test]
    fn loads_only_hash_verified_artifacts() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let artifact = directory.path().join("slate-client.jar");
        std::fs::write(&artifact, b"verified client")?;
        let sha256 = sha256_file(&artifact)?;
        std::fs::write(
            directory.path().join("manifest.json"),
            format!(
                r#"{{"schema":1,"artifacts":[{{"minecraftVersion":"1.21.1","loader":"fabric","loaderVersion":"0.19.5","file":"slate-client.jar","destination":"mods/slate-client.jar","sha256":"{sha256}","displayName":"Slate Client"}}]}}"#
            ),
        )?;

        let catalog = ClientArtifactCatalog::load(directory.path())?;
        assert!(catalog.is_available());
        std::fs::write(&artifact, b"changed")?;
        assert!(ClientArtifactCatalog::load(directory.path()).is_err());
        Ok(())
    }
}
