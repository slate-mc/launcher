use crate::http::{BoundedHttpClient, HttpError};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use slate_minecraft::{Library, MetadataError, VersionMetadata};
use std::{collections::BTreeMap, io::Read};
use url::Url;
use zip::ZipArchive;

pub const NEOFORGE_MAVEN_ORIGIN: &str = "https://maven.neoforged.net/releases/";
const NEOFORGE_MAVEN_HOST: &str = "maven.neoforged.net";
pub const MAX_NEOFORGE_INSTALLER_BYTES: usize = 64 * 1024 * 1024;
const MAX_INSTALL_PROFILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_SHA1_SIDECAR_BYTES: usize = 256;
const MAX_MAVEN_METADATA_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct NeoForgeAdapter {
    http: BoundedHttpClient,
}

impl NeoForgeAdapter {
    pub fn new() -> Result<Self, NeoForgeError> {
        Ok(Self {
            http: BoundedHttpClient::new("slate/0.1 (launcher metadata)")?,
        })
    }

    pub async fn fetch_installer(
        &self,
        minecraft_version: &str,
        loader_version: &str,
    ) -> Result<NeoForgeInstallerBundle, NeoForgeError> {
        validate_version_segment(minecraft_version)?;
        validate_version_segment(loader_version)?;
        let installer_url = installer_url(loader_version)?;
        let sha1_url = Url::parse(&format!("{installer_url}.sha1"))?;
        let sha1_bytes = self
            .http
            .get(sha1_url, NEOFORGE_MAVEN_HOST, MAX_SHA1_SIDECAR_BYTES)
            .await?;
        let expected_sha1 = parse_sha1_sidecar(&sha1_bytes)?;
        let installer_bytes = self
            .http
            .get(
                installer_url.clone(),
                NEOFORGE_MAVEN_HOST,
                MAX_NEOFORGE_INSTALLER_BYTES,
            )
            .await?;
        verify_sha1(&installer_bytes, &expected_sha1)?;
        let profile = Self::parse_installer(&installer_bytes, minecraft_version, loader_version)?;
        Ok(NeoForgeInstallerBundle {
            profile,
            installer_bytes,
            installer_url,
            sha1: expected_sha1,
        })
    }

    pub async fn fetch_loader_versions(
        &self,
        minecraft_version: &str,
    ) -> Result<Vec<String>, NeoForgeError> {
        let prefix = neoforge_version_prefix(minecraft_version)?;
        let url = Url::parse(&format!(
            "{NEOFORGE_MAVEN_ORIGIN}net/neoforged/neoforge/maven-metadata.xml"
        ))?;
        let bytes = self
            .http
            .get(url, NEOFORGE_MAVEN_HOST, MAX_MAVEN_METADATA_BYTES)
            .await?;
        Self::parse_loader_versions(&bytes, &prefix)
    }

    pub fn parse_loader_versions(
        bytes: &[u8],
        version_prefix: &str,
    ) -> Result<Vec<String>, NeoForgeError> {
        let document =
            std::str::from_utf8(bytes).map_err(|_| NeoForgeError::InvalidMavenMetadata)?;
        let versions = maven_versions(document)?;
        let compatible: Vec<&str> = versions
            .into_iter()
            .filter(|version| version.starts_with(version_prefix))
            .collect();
        let recommended = compatible
            .iter()
            .rev()
            .find(|version| !version.contains('-'))
            .or_else(|| compatible.last())
            .copied();
        let mut ordered = Vec::with_capacity(compatible.len());
        if let Some(version) = recommended {
            ordered.push(version.to_owned());
        }
        ordered.extend(
            compatible
                .into_iter()
                .rev()
                .filter(|version| Some(*version) != recommended)
                .map(ToOwned::to_owned),
        );
        Ok(ordered)
    }

    pub fn parse_installer(
        installer_bytes: &[u8],
        minecraft_version: &str,
        loader_version: &str,
    ) -> Result<NeoForgeInstallProfile, NeoForgeError> {
        if installer_bytes.len() > MAX_NEOFORGE_INSTALLER_BYTES {
            return Err(NeoForgeError::InstallerTooLarge(installer_bytes.len()));
        }
        validate_version_segment(minecraft_version)?;
        validate_version_segment(loader_version)?;
        let cursor = std::io::Cursor::new(installer_bytes);
        let mut archive = ZipArchive::new(cursor)?;
        let version_bytes = read_unique_entry(
            &mut archive,
            "version.json",
            slate_minecraft::MAX_VERSION_METADATA_BYTES,
        )?;
        let install_bytes = read_unique_entry(
            &mut archive,
            "install_profile.json",
            MAX_INSTALL_PROFILE_BYTES,
        )?;
        let version = VersionMetadata::from_json_slice(&version_bytes)?;
        let install: NeoForgeInstallProfileDocument = serde_json::from_slice(&install_bytes)?;
        install.validate()?;

        let expected_id = format!("neoforge-{loader_version}");
        if version.id != expected_id {
            return Err(NeoForgeError::UnexpectedVersionId {
                expected: expected_id,
                actual: version.id,
            });
        }
        if version.inherits_from.as_deref() != Some(minecraft_version) {
            return Err(NeoForgeError::UnexpectedParent {
                expected: minecraft_version.to_owned(),
                actual: version.inherits_from,
            });
        }
        if install.minecraft != minecraft_version {
            return Err(NeoForgeError::UnexpectedMinecraftVersion {
                expected: minecraft_version.to_owned(),
                actual: install.minecraft,
            });
        }
        if install
            .path
            .as_deref()
            .is_some_and(|path| path != version.id)
        {
            return Err(NeoForgeError::UnexpectedInstallPath {
                expected: version.id.clone(),
                actual: install.path,
            });
        }

        Ok(NeoForgeInstallProfile {
            version,
            data: install.data,
            processors: install.processors,
            installer_libraries: install.libraries,
        })
    }
}

fn neoforge_version_prefix(minecraft_version: &str) -> Result<String, NeoForgeError> {
    validate_version_segment(minecraft_version)?;
    let mut parts: Vec<&str> = minecraft_version.split('.').collect();
    if parts.iter().any(|part| part.parse::<u16>().is_err()) {
        return Err(NeoForgeError::UnsupportedMinecraftVersion(
            minecraft_version.to_owned(),
        ));
    }
    if parts.first() == Some(&"1") {
        parts.remove(0);
    }
    if parts.len() == 1 || (parts.len() == 2 && !minecraft_version.starts_with("1.")) {
        parts.push("0");
    }
    if !(2..=3).contains(&parts.len()) {
        return Err(NeoForgeError::UnsupportedMinecraftVersion(
            minecraft_version.to_owned(),
        ));
    }
    Ok(format!("{}.", parts.join(".")))
}

fn maven_versions(document: &str) -> Result<Vec<&str>, NeoForgeError> {
    let mut remaining = document;
    let mut versions = Vec::new();
    while let Some(start) = remaining.find("<version>") {
        let value_start = start + "<version>".len();
        let after_start = &remaining[value_start..];
        let end = after_start
            .find("</version>")
            .ok_or(NeoForgeError::InvalidMavenMetadata)?;
        let version = after_start[..end].trim();
        validate_version_segment(version)?;
        versions.push(version);
        if versions.len() > 10_000 {
            return Err(NeoForgeError::TooManyLoaderVersions(versions.len()));
        }
        remaining = &after_start[end + "</version>".len()..];
    }
    if versions.is_empty() {
        return Err(NeoForgeError::InvalidMavenMetadata);
    }
    Ok(versions)
}

#[derive(Debug)]
pub struct NeoForgeInstallerBundle {
    pub profile: NeoForgeInstallProfile,
    pub installer_bytes: Vec<u8>,
    pub installer_url: Url,
    pub sha1: String,
}

#[derive(Debug)]
pub struct NeoForgeInstallProfile {
    pub version: VersionMetadata,
    pub data: BTreeMap<String, SidedDataValue>,
    pub processors: Vec<NeoForgeProcessor>,
    pub installer_libraries: Vec<Library>,
}

impl NeoForgeInstallProfile {
    pub fn client_processors(&self) -> impl Iterator<Item = &NeoForgeProcessor> {
        self.processors.iter().filter(|processor| {
            processor.sides.is_empty() || processor.sides.iter().any(|side| side == "client")
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SidedDataValue {
    pub client: String,
    pub server: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NeoForgeProcessor {
    #[serde(default)]
    pub sides: Vec<String>,
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct NeoForgeInstallProfileDocument {
    minecraft: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    data: BTreeMap<String, SidedDataValue>,
    #[serde(default)]
    processors: Vec<NeoForgeProcessor>,
    #[serde(default)]
    libraries: Vec<Library>,
}

impl NeoForgeInstallProfileDocument {
    fn validate(&self) -> Result<(), NeoForgeError> {
        if self.processors.len() > 256 {
            return Err(NeoForgeError::TooManyProcessors(self.processors.len()));
        }
        if self.libraries.len() > 4_096 {
            return Err(NeoForgeError::TooManyLibraries(self.libraries.len()));
        }
        for processor in &self.processors {
            validate_coordinate(&processor.jar)?;
            if processor.classpath.len() > 1_024 || processor.args.len() > 4_096 {
                return Err(NeoForgeError::ProcessorTooLarge(processor.jar.clone()));
            }
            for coordinate in &processor.classpath {
                validate_coordinate(coordinate)?;
            }
            if processor
                .sides
                .iter()
                .any(|side| side != "client" && side != "server")
            {
                return Err(NeoForgeError::InvalidProcessorSide(processor.jar.clone()));
            }
        }
        Ok(())
    }
}

fn installer_url(loader_version: &str) -> Result<Url, NeoForgeError> {
    validate_version_segment(loader_version)?;
    Url::parse(&format!(
        "{NEOFORGE_MAVEN_ORIGIN}net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar"
    ))
    .map_err(NeoForgeError::Url)
}

fn validate_version_segment(value: &str) -> Result<(), NeoForgeError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
    {
        return Err(NeoForgeError::InvalidVersion(value.to_owned()));
    }
    Ok(())
}

fn validate_coordinate(value: &str) -> Result<(), NeoForgeError> {
    slate_minecraft::MavenCoordinate::parse(value)
        .map(|_| ())
        .map_err(|_| NeoForgeError::InvalidProcessorCoordinate(value.to_owned()))
}

fn parse_sha1_sidecar(bytes: &[u8]) -> Result<String, NeoForgeError> {
    let text = std::str::from_utf8(bytes).map_err(|_| NeoForgeError::InvalidSha1Sidecar)?;
    let sha1 = text
        .split_ascii_whitespace()
        .next()
        .ok_or(NeoForgeError::InvalidSha1Sidecar)?;
    if sha1.len() != 40 || !sha1.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(NeoForgeError::InvalidSha1Sidecar);
    }
    Ok(sha1.to_ascii_lowercase())
}

fn verify_sha1(bytes: &[u8], expected: &str) -> Result<(), NeoForgeError> {
    let actual = digest_hex(&Sha1::digest(bytes));
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(NeoForgeError::InstallerHashMismatch {
            expected: expected.to_owned(),
            actual,
        })
    }
}

fn digest_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn read_unique_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &'static str,
    maximum: usize,
) -> Result<Vec<u8>, NeoForgeError> {
    let matches = archive.file_names().filter(|entry| *entry == name).count();
    if matches != 1 {
        return Err(NeoForgeError::ArchiveEntryCount { name, matches });
    }
    let mut entry = archive.by_name(name)?;
    if entry.size() > maximum as u64 {
        return Err(NeoForgeError::ArchiveEntryTooLarge {
            name,
            actual: entry.size(),
            maximum,
        });
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(entry.size())
            .unwrap_or(maximum)
            .min(maximum),
    );
    entry
        .by_ref()
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(NeoForgeError::ArchiveEntryTooLarge {
            name,
            actual: bytes.len() as u64,
            maximum,
        });
    }
    Ok(bytes)
}

#[derive(Debug, thiserror::Error)]
pub enum NeoForgeError {
    #[error("NeoForge version is not a safe metadata path segment: {0}")]
    InvalidVersion(String),
    #[error("NeoForge installer is too large: {0} bytes")]
    InstallerTooLarge(usize),
    #[error("NeoForge SHA-1 sidecar is invalid")]
    InvalidSha1Sidecar,
    #[error("NeoForge installer SHA-1 mismatch: expected {expected}, got {actual}")]
    InstallerHashMismatch { expected: String, actual: String },
    #[error("NeoForge installer contains {matches} copies of required entry {name}")]
    ArchiveEntryCount { name: &'static str, matches: usize },
    #[error("NeoForge installer entry {name} is too large: {actual} bytes exceeds {maximum}")]
    ArchiveEntryTooLarge {
        name: &'static str,
        actual: u64,
        maximum: usize,
    },
    #[error("NeoForge returned version id {actual}; expected {expected}")]
    UnexpectedVersionId { expected: String, actual: String },
    #[error("NeoForge profile inherits from {actual:?}; expected {expected}")]
    UnexpectedParent {
        expected: String,
        actual: Option<String>,
    },
    #[error("NeoForge install profile targets Minecraft {actual}; expected {expected}")]
    UnexpectedMinecraftVersion { expected: String, actual: String },
    #[error("NeoForge install profile path is {actual:?}; expected {expected}")]
    UnexpectedInstallPath {
        expected: String,
        actual: Option<String>,
    },
    #[error("NeoForge install profile contains too many processors: {0}")]
    TooManyProcessors(usize),
    #[error("NeoForge install profile contains too many libraries: {0}")]
    TooManyLibraries(usize),
    #[error("NeoForge processor declaration is too large: {0}")]
    ProcessorTooLarge(String),
    #[error("NeoForge processor uses an invalid side: {0}")]
    InvalidProcessorSide(String),
    #[error("NeoForge processor contains an invalid Maven coordinate: {0}")]
    InvalidProcessorCoordinate(String),
    #[error("NeoForge installer URL is invalid")]
    Url(#[from] url::ParseError),
    #[error("NeoForge metadata request failed")]
    Http(#[from] HttpError),
    #[error("NeoForge installer archive is invalid")]
    Zip(#[from] zip::result::ZipError),
    #[error("NeoForge installer entry could not be read")]
    Io(#[from] std::io::Error),
    #[error("NeoForge version metadata is invalid")]
    Metadata(#[from] MetadataError),
    #[error("NeoForge Maven metadata is invalid")]
    InvalidMavenMetadata,
    #[error("Minecraft version is not supported by NeoForge version mapping: {0}")]
    UnsupportedMinecraftVersion(String),
    #[error("NeoForge returned too many loader versions: {0}")]
    TooManyLoaderVersions(usize),
    #[error("NeoForge install profile JSON is invalid")]
    InstallProfile(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::{NeoForgeAdapter, NeoForgeError, neoforge_version_prefix, parse_sha1_sidecar};
    use std::io::{Cursor, Write};
    use zip::{ZipWriter, write::SimpleFileOptions};

    const VERSION: &[u8] = include_bytes!("../tests/fixtures/neoforge-1.21.1-version.json");
    const INSTALL: &[u8] = include_bytes!("../tests/fixtures/neoforge-1.21.1-install-profile.json");

    #[test]
    fn parses_current_installer_shape_and_filters_client_processors() {
        let bytes = installer_fixture();
        let profile = NeoForgeAdapter::parse_installer(&bytes, "1.21.1", "21.1.250");

        assert!(profile.is_ok());
        let profile = match profile {
            Ok(profile) => profile,
            Err(error) => panic!("installer should parse: {error}"),
        };
        assert_eq!(profile.client_processors().count(), 2);
        assert_eq!(
            profile.version.main_class.as_deref(),
            Some("cpw.mods.bootstraplauncher.BootstrapLauncher")
        );
    }

    #[test]
    fn rejects_an_invalid_sha1_sidecar() {
        assert!(matches!(
            parse_sha1_sidecar(b"not-a-hash"),
            Err(NeoForgeError::InvalidSha1Sidecar)
        ));
    }

    #[test]
    fn maps_minecraft_versions_to_neoforge_versions() {
        assert_eq!(
            neoforge_version_prefix("1.21.1").ok().as_deref(),
            Some("21.1.")
        );
        assert_eq!(
            neoforge_version_prefix("1.20.4").ok().as_deref(),
            Some("20.4.")
        );
        assert_eq!(
            neoforge_version_prefix("26.1").ok().as_deref(),
            Some("26.1.0.")
        );
    }

    #[test]
    fn lists_the_latest_stable_compatible_neoforge_version_first() {
        let document = br#"<metadata><versioning><versions>
            <version>21.1.250</version>
            <version>21.1.251-beta</version>
            <version>21.2.1</version>
        </versions></versioning></metadata>"#;

        let versions = NeoForgeAdapter::parse_loader_versions(document, "21.1.");
        assert!(versions.is_ok());
        assert_eq!(
            versions.ok(),
            Some(vec!["21.1.250".to_owned(), "21.1.251-beta".to_owned()])
        );
    }

    fn installer_fixture() -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        if let Err(error) = writer.start_file("version.json", options) {
            panic!("fixture entry should start: {error}");
        }
        if let Err(error) = writer.write_all(VERSION) {
            panic!("fixture version should write: {error}");
        }
        if let Err(error) = writer.start_file("install_profile.json", options) {
            panic!("fixture entry should start: {error}");
        }
        if let Err(error) = writer.write_all(INSTALL) {
            panic!("fixture profile should write: {error}");
        }
        match writer.finish() {
            Ok(cursor) => cursor.into_inner(),
            Err(error) => panic!("fixture zip should finish: {error}"),
        }
    }
}
