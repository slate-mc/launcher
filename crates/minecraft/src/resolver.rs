use crate::maven::{MavenCoordinate, MavenError};
use crate::metadata::{
    ArgumentEntry, AssetIndex, ClientLogging, DownloadInfo, JavaVersion, Library, VersionArguments,
    VersionMetadata,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct ResolvedVersion {
    id: String,
    source_layers: Vec<String>,
    client_jar_version: String,
    client_download: DownloadInfo,
    main_class: String,
    version_type: String,
    assets: String,
    asset_index: AssetIndex,
    java_version: JavaVersion,
    libraries: Vec<Library>,
    arguments: VersionArguments,
    legacy_game_arguments: Vec<String>,
    logging: Option<ClientLogging>,
}

impl ResolvedVersion {
    pub fn resolve(layers: Vec<VersionMetadata>) -> Result<Self, ResolveError> {
        let first = layers.first().ok_or(ResolveError::EmptyChain)?;
        if let Some(parent) = &first.inherits_from {
            return Err(ResolveError::MissingParent {
                child: first.id.clone(),
                parent: parent.clone(),
            });
        }

        for pair in layers.windows(2) {
            let parent = &pair[0];
            let child = &pair[1];
            if child.inherits_from.as_deref() != Some(parent.id.as_str()) {
                return Err(ResolveError::InheritanceMismatch {
                    child: child.id.clone(),
                    expected_parent: parent.id.clone(),
                    declared_parent: child.inherits_from.clone(),
                });
            }
        }

        let mut source_layers = Vec::with_capacity(layers.len());
        let mut final_id = None;
        let mut client = None;
        let mut main_class = None;
        let mut version_type = None;
        let mut assets = None;
        let mut asset_index = None;
        let mut java_version = None;
        let mut libraries = Vec::new();
        let mut library_positions = BTreeMap::<String, usize>::new();
        let mut arguments = VersionArguments::default();
        let mut legacy_game_arguments = Vec::new();
        let mut logging = None;

        for layer in layers {
            source_layers.push(layer.id.clone());
            final_id = Some(layer.id.clone());
            if let Some(download) = layer.downloads.client {
                client = Some((layer.id.clone(), download));
            }
            if let Some(value) = layer.main_class {
                main_class = Some(value);
            }
            if let Some(value) = layer.version_type {
                version_type = Some(value);
            }
            if let Some(value) = layer.assets {
                assets = Some(value);
            }
            if let Some(value) = layer.asset_index {
                asset_index = Some(value);
            }
            if let Some(value) = layer.java_version {
                java_version = Some(value);
            }
            if let Some(value) = layer.logging.and_then(|value| value.client) {
                logging = Some(value);
            }
            if let Some(value) = layer.minecraft_arguments {
                legacy_game_arguments.push(value);
            }
            if let Some(layer_arguments) = layer.arguments {
                arguments.jvm.extend(layer_arguments.jvm);
                arguments.game.extend(layer_arguments.game);
            }

            for library in layer.libraries {
                let key = MavenCoordinate::parse(&library.name)?.dependency_key();
                if let Some(position) = library_positions.get(&key).copied() {
                    libraries[position] = library;
                } else {
                    library_positions.insert(key, libraries.len());
                    libraries.push(library);
                }
            }
        }

        let (client_jar_version, client_download) =
            client.ok_or(ResolveError::MissingField("downloads.client"))?;
        Ok(Self {
            id: final_id.ok_or(ResolveError::EmptyChain)?,
            source_layers,
            client_jar_version,
            client_download,
            main_class: required(main_class, "mainClass")?,
            version_type: required(version_type, "type")?,
            assets: required(assets, "assets")?,
            asset_index: asset_index.ok_or(ResolveError::MissingField("assetIndex"))?,
            java_version: java_version.ok_or(ResolveError::MissingField("javaVersion"))?,
            libraries,
            arguments,
            legacy_game_arguments,
            logging,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn source_layers(&self) -> &[String] {
        &self.source_layers
    }

    #[must_use]
    pub fn client_jar_version(&self) -> &str {
        &self.client_jar_version
    }

    #[must_use]
    pub fn client_download(&self) -> &DownloadInfo {
        &self.client_download
    }

    #[must_use]
    pub fn main_class(&self) -> &str {
        &self.main_class
    }

    #[must_use]
    pub fn version_type(&self) -> &str {
        &self.version_type
    }

    #[must_use]
    pub fn assets(&self) -> &str {
        &self.assets
    }

    #[must_use]
    pub fn asset_index(&self) -> &AssetIndex {
        &self.asset_index
    }

    #[must_use]
    pub fn java_version(&self) -> &JavaVersion {
        &self.java_version
    }

    #[must_use]
    pub fn libraries(&self) -> &[Library] {
        &self.libraries
    }

    #[must_use]
    pub fn jvm_arguments(&self) -> &[ArgumentEntry] {
        &self.arguments.jvm
    }

    #[must_use]
    pub fn game_arguments(&self) -> &[ArgumentEntry] {
        &self.arguments.game
    }

    #[must_use]
    pub fn legacy_game_arguments(&self) -> &[String] {
        &self.legacy_game_arguments
    }

    #[must_use]
    pub fn logging(&self) -> Option<&ClientLogging> {
        self.logging.as_ref()
    }
}

fn required(value: Option<String>, field: &'static str) -> Result<String, ResolveError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or(ResolveError::MissingField(field))
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("version metadata chain is empty")]
    EmptyChain,
    #[error("metadata for {child} requires missing parent {parent}")]
    MissingParent { child: String, parent: String },
    #[error(
        "metadata inheritance mismatch for {child}: expected {expected_parent}, declared {declared_parent:?}"
    )]
    InheritanceMismatch {
        child: String,
        expected_parent: String,
        declared_parent: Option<String>,
    },
    #[error("resolved version is missing required field {0}")]
    MissingField(&'static str),
    #[error(transparent)]
    Maven(#[from] MavenError),
}

#[cfg(test)]
mod tests {
    use super::ResolvedVersion;
    use crate::metadata::VersionMetadata;

    fn base() -> Result<VersionMetadata, crate::metadata::MetadataError> {
        VersionMetadata::from_json_slice(
            br#"{
                "id":"1.21.1",
                "type":"release",
                "mainClass":"net.minecraft.client.main.Main",
                "assets":"17",
                "assetIndex":{"id":"17","sha1":"aa","size":1,"url":"https://example.com/index"},
                "downloads":{"client":{"sha1":"bb","size":1,"url":"https://example.com/client"}},
                "javaVersion":{"component":"java-runtime-delta","majorVersion":21},
                "libraries":[{"name":"org.example:shared:1.0","url":"https://example.com/"}],
                "arguments":{"game":["--base"],"jvm":["-Dbase=true"]}
            }"#,
        )
    }

    #[test]
    fn child_overrides_library_and_main_class() -> Result<(), Box<dyn std::error::Error>> {
        let child = VersionMetadata::from_json_slice(
            br#"{
                "id":"fabric-loader-test-1.21.1",
                "inheritsFrom":"1.21.1",
                "mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
                "libraries":[
                    {"name":"org.example:shared:2.0","url":"https://example.com/"},
                    {"name":"net.fabricmc:fabric-loader:0.1","url":"https://example.com/"}
                ],
                "arguments":{"game":["--child"],"jvm":[]}
            }"#,
        )?;

        let resolved = ResolvedVersion::resolve(vec![base()?, child])?;

        assert_eq!(
            resolved.main_class(),
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert_eq!(resolved.libraries().len(), 2);
        assert_eq!(resolved.libraries()[0].name, "org.example:shared:2.0");
        assert_eq!(resolved.game_arguments().len(), 2);
        Ok(())
    }

    #[test]
    fn rejects_a_loader_profile_for_another_parent() -> Result<(), Box<dyn std::error::Error>> {
        let child =
            VersionMetadata::from_json_slice(br#"{"id":"loader","inheritsFrom":"1.20.1"}"#)?;
        let result = ResolvedVersion::resolve(vec![base()?, child]);

        assert!(result.is_err());
        Ok(())
    }
}
