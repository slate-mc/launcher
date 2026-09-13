use crate::metadata::DownloadInfo;
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use url::Url;

const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactKind {
    Client,
    Library,
    NativeArchive,
    AssetIndex,
    LoggingConfiguration,
    AssetObject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HashAlgorithm {
    Sha1,
    Sha256,
}

impl HashAlgorithm {
    const fn digest_length(self) -> usize {
        match self {
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedHash {
    algorithm: HashAlgorithm,
    digest: String,
}

impl ExpectedHash {
    pub fn new(algorithm: HashAlgorithm, digest: &str) -> Result<Self, ArtifactError> {
        if digest.len() != algorithm.digest_length()
            || !digest.bytes().all(|value| value.is_ascii_hexdigit())
        {
            return Err(ArtifactError::InvalidHash {
                algorithm,
                digest: digest.to_owned(),
            });
        }
        Ok(Self {
            algorithm,
            digest: digest.to_ascii_lowercase(),
        })
    }

    #[must_use]
    pub const fn algorithm(&self) -> HashAlgorithm {
        self.algorithm
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRequirement {
    kind: ArtifactKind,
    source_url: Url,
    target_path: PathBuf,
    size: Option<u64>,
    expected_hashes: Vec<ExpectedHash>,
    hash_sidecar: Option<Url>,
}

impl ArtifactRequirement {
    pub fn official_asset_object(
        assets_directory: &Path,
        digest: &str,
        size: u64,
    ) -> Result<Self, ArtifactError> {
        if digest.len() != 40 || !digest.bytes().all(|value| value.is_ascii_hexdigit()) {
            return Err(ArtifactError::InvalidHash {
                algorithm: HashAlgorithm::Sha1,
                digest: digest.to_owned(),
            });
        }
        let prefix = &digest[..2];
        let target_path = assets_directory.join("objects").join(prefix).join(digest);
        let source_url = parse_https_url(&format!(
            "https://resources.download.minecraft.net/{prefix}/{digest}"
        ))?;
        Self::new(
            ArtifactKind::AssetObject,
            source_url,
            target_path,
            Some(size),
            vec![ExpectedHash::new(HashAlgorithm::Sha1, digest)?],
            None,
        )
    }

    #[must_use]
    pub fn with_expected_hash(mut self, hash: ExpectedHash) -> Self {
        self.expected_hashes.push(hash);
        self.hash_sidecar = None;
        self
    }

    #[must_use]
    pub fn with_target_path(mut self, target_path: PathBuf) -> Self {
        self.target_path = target_path;
        self
    }

    pub(crate) fn from_download(
        kind: ArtifactKind,
        target_path: PathBuf,
        download: &DownloadInfo,
    ) -> Result<Self, ArtifactError> {
        let source_url = parse_https_url(&download.url)?;
        let expected_hashes = collect_hashes(download.sha1.as_deref(), download.sha256.as_deref())?;
        let hash_sidecar = if expected_hashes.is_empty() {
            Some(parse_https_url(&format!("{}.sha1", source_url.as_str()))?)
        } else {
            None
        };
        Self::new(
            kind,
            source_url,
            target_path,
            download.size,
            expected_hashes,
            hash_sidecar,
        )
    }

    pub(crate) fn from_maven(
        kind: ArtifactKind,
        source_url: Url,
        target_path: PathBuf,
        size: Option<u64>,
        sha1: Option<&str>,
        sha256: Option<&str>,
    ) -> Result<Self, ArtifactError> {
        let expected_hashes = collect_hashes(sha1, sha256)?;
        let hash_sidecar = if expected_hashes.is_empty() {
            Some(parse_https_url(&format!("{}.sha1", source_url.as_str()))?)
        } else {
            None
        };
        Self::new(
            kind,
            source_url,
            target_path,
            size,
            expected_hashes,
            hash_sidecar,
        )
    }

    fn new(
        kind: ArtifactKind,
        source_url: Url,
        target_path: PathBuf,
        size: Option<u64>,
        expected_hashes: Vec<ExpectedHash>,
        hash_sidecar: Option<Url>,
    ) -> Result<Self, ArtifactError> {
        if !target_path.is_absolute() {
            return Err(ArtifactError::TargetMustBeAbsolute);
        }
        Ok(Self {
            kind,
            source_url,
            target_path,
            size,
            expected_hashes,
            hash_sidecar,
        })
    }

    #[must_use]
    pub const fn kind(&self) -> ArtifactKind {
        self.kind
    }

    #[must_use]
    pub const fn source_url(&self) -> &Url {
        &self.source_url
    }

    #[must_use]
    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    #[must_use]
    pub const fn size(&self) -> Option<u64> {
        self.size
    }

    #[must_use]
    pub fn expected_hashes(&self) -> &[ExpectedHash] {
        &self.expected_hashes
    }

    #[must_use]
    pub const fn hash_sidecar(&self) -> Option<&Url> {
        self.hash_sidecar.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExtraction {
    pub archive_path: PathBuf,
    pub destination: PathBuf,
    pub excludes: Vec<String>,
}

/// Verifies one artifact synchronously.
///
/// Callers must move this work to a bounded blocking worker when invoked from
/// an async service. A sidecar-only requirement must first be enriched with the
/// fetched hash; this function never trusts an unverified file.
pub fn verify_artifact(requirement: &ArtifactRequirement) -> Result<(), VerificationError> {
    if requirement.expected_hashes.is_empty() {
        return Err(VerificationError::IntegrityMetadataRequired);
    }

    let mut file = File::open(&requirement.target_path)?;
    let metadata = file.metadata()?;
    if let Some(expected_size) = requirement.size
        && metadata.len() != expected_size
    {
        return Err(VerificationError::SizeMismatch {
            expected: expected_size,
            actual: metadata.len(),
        });
    }

    let needs_sha1 = requirement
        .expected_hashes
        .iter()
        .any(|hash| hash.algorithm == HashAlgorithm::Sha1);
    let needs_sha256 = requirement
        .expected_hashes
        .iter()
        .any(|hash| hash.algorithm == HashAlgorithm::Sha256);
    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if needs_sha1 {
            sha1.update(&buffer[..read]);
        }
        if needs_sha256 {
            sha256.update(&buffer[..read]);
        }
    }

    let actual_sha1 = needs_sha1.then(|| digest_hex(&sha1.finalize()));
    let actual_sha256 = needs_sha256.then(|| digest_hex(&sha256.finalize()));
    for expected in &requirement.expected_hashes {
        let actual = match expected.algorithm {
            HashAlgorithm::Sha1 => actual_sha1.as_deref(),
            HashAlgorithm::Sha256 => actual_sha256.as_deref(),
        }
        .ok_or(VerificationError::HashComputationMissing)?;
        if actual != expected.digest {
            return Err(VerificationError::HashMismatch {
                algorithm: expected.algorithm,
                expected: expected.digest.clone(),
                actual: actual.to_owned(),
            });
        }
    }
    Ok(())
}

pub(crate) fn digest_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn collect_hashes(
    sha1: Option<&str>,
    sha256: Option<&str>,
) -> Result<Vec<ExpectedHash>, ArtifactError> {
    let mut hashes = Vec::new();
    if let Some(value) = sha1 {
        hashes.push(ExpectedHash::new(HashAlgorithm::Sha1, value)?);
    }
    if let Some(value) = sha256 {
        hashes.push(ExpectedHash::new(HashAlgorithm::Sha256, value)?);
    }
    Ok(hashes)
}

fn parse_https_url(value: &str) -> Result<Url, ArtifactError> {
    let url = Url::parse(value).map_err(ArtifactError::InvalidUrl)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ArtifactError::InsecureUrl(value.to_owned()));
    }
    Ok(url)
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("artifact URL is invalid")]
    InvalidUrl(#[source] url::ParseError),
    #[error("artifact URL must be credential-free HTTPS: {0}")]
    InsecureUrl(String),
    #[error("artifact target path must be absolute")]
    TargetMustBeAbsolute,
    #[error("invalid {algorithm:?} digest: {digest}")]
    InvalidHash {
        algorithm: HashAlgorithm,
        digest: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("artifact I/O failed")]
    Io(#[from] std::io::Error),
    #[error("artifact size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("artifact hash mismatch for {algorithm:?}: expected {expected}, got {actual}")]
    HashMismatch {
        algorithm: HashAlgorithm,
        expected: String,
        actual: String,
    },
    #[error("artifact needs a trusted hash sidecar before verification")]
    IntegrityMetadataRequired,
    #[error("requested artifact hash was not computed")]
    HashComputationMissing,
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactKind, ArtifactRequirement, HashAlgorithm, VerificationError, verify_artifact,
    };
    use crate::metadata::DownloadInfo;

    #[test]
    fn verifies_size_and_sha256() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("artifact.jar");
        std::fs::write(&path, b"slate")?;
        let download = DownloadInfo {
            id: None,
            path: None,
            sha1: None,
            sha256: Some(
                "b62f3b3b1b40cb86438bf4d2affd121f9518814e713d3df7d4c193b2168f28fc".to_owned(),
            ),
            size: Some(5),
            url: "https://example.com/artifact.jar".to_owned(),
        };
        let requirement =
            ArtifactRequirement::from_download(ArtifactKind::Library, path, &download)?;

        verify_artifact(&requirement)?;
        assert_eq!(
            requirement.expected_hashes()[0].algorithm(),
            HashAlgorithm::Sha256
        );
        Ok(())
    }

    #[test]
    fn refuses_unhashed_artifacts_until_sidecar_is_resolved()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("artifact.jar");
        std::fs::write(&path, b"slate")?;
        let download = DownloadInfo {
            id: None,
            path: None,
            sha1: None,
            sha256: None,
            size: None,
            url: "https://example.com/artifact.jar".to_owned(),
        };
        let requirement =
            ArtifactRequirement::from_download(ArtifactKind::Library, path, &download)?;

        assert!(requirement.hash_sidecar().is_some());
        assert!(matches!(
            verify_artifact(&requirement),
            Err(VerificationError::IntegrityMetadataRequired)
        ));
        Ok(())
    }
}
