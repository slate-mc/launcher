//! Versioned handshake contract between the Slate launcher and the in-game Kotlin client.

use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

pub const CLIENT_HANDSHAKE_SCHEMA: u32 = 1;
pub const CLIENT_PROTOCOL_VERSION: u32 = 1;
const MAX_HANDSHAKE_BYTES: u64 = 64 * 1024;
const MAX_MODULES: usize = 128;
const MAX_IDENTIFIER_LENGTH: usize = 80;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientHandshake {
    pub schema: u32,
    pub protocol_version: u32,
    pub process_id: u64,
    pub started_at_epoch_millis: u64,
    pub adapter_status: AdapterStatus,
    pub target: ClientTarget,
    pub modules: Vec<HandshakeModule>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterStatus {
    ContractOnly,
    Active,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientTarget {
    pub minecraft_version: String,
    pub loader: ClientLoader,
    pub loader_version: String,
    pub java_major: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientLoader {
    Fabric,
    NeoForge,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HandshakeModule {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedClient {
    pub process_id: u64,
    pub started_at_epoch_millis_minimum: u64,
    pub minecraft_version: String,
    pub loader: ClientLoader,
    pub loader_version: String,
    pub java_major: u32,
    pub require_active_adapter: bool,
}

impl ClientHandshake {
    pub fn read(path: &Path) -> Result<Self, ClientHandshakeError> {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ClientHandshakeError::InvalidFileType);
        }
        if metadata.len() > MAX_HANDSHAKE_BYTES {
            return Err(ClientHandshakeError::TooLarge);
        }
        let bytes = std::fs::read(path)?;
        let handshake: Self = serde_json::from_slice(&bytes)?;
        handshake.validate_shape()?;
        Ok(handshake)
    }

    pub fn validate(&self, expected: &ExpectedClient) -> Result<(), ClientHandshakeError> {
        self.validate_shape()?;
        if self.process_id != expected.process_id {
            return Err(ClientHandshakeError::ProcessMismatch);
        }
        if self.started_at_epoch_millis < expected.started_at_epoch_millis_minimum {
            return Err(ClientHandshakeError::Stale);
        }
        if self.target.minecraft_version != expected.minecraft_version
            || self.target.loader != expected.loader
            || self.target.loader_version != expected.loader_version
            || self.target.java_major != expected.java_major
        {
            return Err(ClientHandshakeError::TargetMismatch);
        }
        if expected.require_active_adapter && self.adapter_status != AdapterStatus::Active {
            return Err(ClientHandshakeError::AdapterInactive);
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), ClientHandshakeError> {
        if self.schema != CLIENT_HANDSHAKE_SCHEMA {
            return Err(ClientHandshakeError::UnsupportedSchema);
        }
        if self.protocol_version != CLIENT_PROTOCOL_VERSION {
            return Err(ClientHandshakeError::UnsupportedProtocol);
        }
        if self.process_id == 0 || self.started_at_epoch_millis == 0 {
            return Err(ClientHandshakeError::InvalidProcessIdentity);
        }
        if !valid_identifier(&self.target.minecraft_version)
            || !valid_identifier(&self.target.loader_version)
            || self.target.java_major == 0
        {
            return Err(ClientHandshakeError::InvalidTarget);
        }
        if self.modules.len() > MAX_MODULES {
            return Err(ClientHandshakeError::TooManyModules);
        }
        let mut module_ids = BTreeSet::new();
        for module in &self.modules {
            if !valid_module_id(&module.id) || Version::parse(&module.version).is_err() {
                return Err(ClientHandshakeError::InvalidModule);
            }
            if !module_ids.insert(module.id.as_str()) {
                return Err(ClientHandshakeError::DuplicateModule);
            }
        }
        Ok(())
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-_+".contains(&byte))
}

fn valid_module_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_LENGTH
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

#[derive(Debug, thiserror::Error)]
pub enum ClientHandshakeError {
    #[error("Slate Client handshake file is unavailable")]
    Io(#[from] std::io::Error),
    #[error("Slate Client handshake is not valid JSON")]
    Json(#[from] serde_json::Error),
    #[error("Slate Client handshake must be a regular file")]
    InvalidFileType,
    #[error("Slate Client handshake is too large")]
    TooLarge,
    #[error("Slate Client handshake schema is unsupported")]
    UnsupportedSchema,
    #[error("Slate Client protocol version is unsupported")]
    UnsupportedProtocol,
    #[error("Slate Client process identity is invalid")]
    InvalidProcessIdentity,
    #[error("Slate Client handshake belongs to another process")]
    ProcessMismatch,
    #[error("Slate Client handshake predates this game session")]
    Stale,
    #[error("Slate Client target is invalid")]
    InvalidTarget,
    #[error("Slate Client target does not match this instance")]
    TargetMismatch,
    #[error("Slate Client adapter has not passed compatibility acceptance")]
    AdapterInactive,
    #[error("Slate Client reported too many modules")]
    TooManyModules,
    #[error("Slate Client reported an invalid module")]
    InvalidModule,
    #[error("Slate Client reported the same module more than once")]
    DuplicateModule,
}

#[cfg(test)]
mod tests {
    use super::{
        AdapterStatus, CLIENT_HANDSHAKE_SCHEMA, CLIENT_PROTOCOL_VERSION, ClientHandshake,
        ClientHandshakeError, ClientLoader, ClientTarget, ExpectedClient, HandshakeModule,
        MAX_HANDSHAKE_BYTES,
    };

    fn handshake() -> ClientHandshake {
        ClientHandshake {
            schema: CLIENT_HANDSHAKE_SCHEMA,
            protocol_version: CLIENT_PROTOCOL_VERSION,
            process_id: 42,
            started_at_epoch_millis: 1_000,
            adapter_status: AdapterStatus::ContractOnly,
            target: ClientTarget {
                minecraft_version: "1.21.1".to_owned(),
                loader: ClientLoader::Fabric,
                loader_version: "0.19.5".to_owned(),
                java_major: 21,
            },
            modules: vec![HandshakeModule {
                id: "slate.qol".to_owned(),
                version: "0.1.0".to_owned(),
            }],
        }
    }

    fn expected(require_active_adapter: bool) -> ExpectedClient {
        ExpectedClient {
            process_id: 42,
            started_at_epoch_millis_minimum: 999,
            minecraft_version: "1.21.1".to_owned(),
            loader: ClientLoader::Fabric,
            loader_version: "0.19.5".to_owned(),
            java_major: 21,
            require_active_adapter,
        }
    }

    #[test]
    fn reads_and_validates_the_kotlin_wire_shape() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("client-handshake.json");
        std::fs::write(
            &path,
            r#"{
                "schema": 1,
                "protocolVersion": 1,
                "processId": 42,
                "startedAtEpochMillis": 1000,
                "adapterStatus": "contract_only",
                "target": {
                    "minecraftVersion": "1.21.1",
                    "loader": "fabric",
                    "loaderVersion": "0.19.5",
                    "javaMajor": 21
                },
                "modules": [{"id": "slate.qol", "version": "0.1.0"}]
            }"#,
        )?;

        let parsed = ClientHandshake::read(&path)?;

        parsed.validate(&expected(false))?;
        assert!(matches!(
            parsed.validate(&expected(true)),
            Err(ClientHandshakeError::AdapterInactive)
        ));
        Ok(())
    }

    #[test]
    fn rejects_wrong_process_stale_target_and_duplicate_modules() {
        let mut value = handshake();
        value.process_id = 7;
        assert!(matches!(
            value.validate(&expected(false)),
            Err(ClientHandshakeError::ProcessMismatch)
        ));

        let mut value = handshake();
        value.started_at_epoch_millis = 1;
        assert!(matches!(
            value.validate(&expected(false)),
            Err(ClientHandshakeError::Stale)
        ));

        let mut value = handshake();
        value.target.loader_version = "0.18.0".to_owned();
        assert!(matches!(
            value.validate(&expected(false)),
            Err(ClientHandshakeError::TargetMismatch)
        ));

        let mut value = handshake();
        value.modules.push(value.modules[0].clone());
        assert!(matches!(
            value.validate(&expected(false)),
            Err(ClientHandshakeError::DuplicateModule)
        ));
    }

    #[test]
    fn rejects_oversized_documents() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("client-handshake.json");
        std::fs::write(&path, vec![b' '; usize::try_from(MAX_HANDSHAKE_BYTES + 1)?])?;

        assert!(matches!(
            ClientHandshake::read(&path),
            Err(ClientHandshakeError::TooLarge)
        ));
        Ok(())
    }
}
