use super::{
    JavaRuntime, LaunchIdentity, LaunchLayout, LaunchOptions, LaunchPlanner, LaunchRequest,
};
use crate::metadata::VersionMetadata;
use crate::rules::{Architecture, OperatingSystem, RuleContext};
use std::collections::BTreeMap;
use std::path::Path;
use uuid::Uuid;

fn request(root: &Path) -> Result<LaunchRequest, Box<dyn std::error::Error>> {
    Ok(LaunchRequest {
        runtime: JavaRuntime::new(std::env::current_exe()?, 21, Architecture::X86_64)?,
        layout: LaunchLayout {
            game_directory: root.join("game"),
            libraries_directory: root.join("libraries"),
            versions_directory: root.join("versions"),
            assets_directory: root.join("assets"),
            natives_directory: root.join("natives"),
        },
        identity: LaunchIdentity::new(
            "Player",
            Uuid::nil(),
            "secret-token",
            "secret-client-id",
            "secret-xuid",
        )?,
        rules: RuleContext::new(OperatingSystem::Windows, Architecture::X86_64, "10.0"),
        options: LaunchOptions::default(),
        environment: BTreeMap::new(),
    })
}

#[test]
fn prepares_vanilla_and_redacts_identity_and_paths() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let metadata = VersionMetadata::from_json_slice(include_bytes!(
        "../../tests/fixtures/vanilla-1.21.1.json"
    ))?;
    let preparation =
        LaunchPlanner::prepare_from_metadata(vec![metadata], request(directory.path())?)?;
    let redacted = serde_json::to_string(&preparation.plan.redacted())?;

    assert_eq!(preparation.version_id, "1.21.1");
    assert_eq!(preparation.required_java_major, 21);
    assert_eq!(preparation.required_artifacts.len(), 4);
    assert!(redacted.contains("net.minecraft.client.main.Main"));
    assert!(!redacted.contains("secret-token"));
    assert!(!redacted.contains("Player"));
    assert!(!redacted.contains(&directory.path().to_string_lossy().to_string()));
    Ok(())
}

#[test]
fn rejects_an_unresolved_placeholder() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let metadata = VersionMetadata::from_json_slice(
        br#"{
          "id":"bad","type":"release","mainClass":"example.Main","assets":"1",
          "assetIndex":{"id":"1","sha1":"0000000000000000000000000000000000000000","size":1,"url":"https://example.com/index"},
          "downloads":{"client":{"sha1":"0000000000000000000000000000000000000000","size":1,"url":"https://example.com/client"}},
          "javaVersion":{"component":"java","majorVersion":21},
          "arguments":{"game":["${not_known}"],"jvm":[]}
        }"#,
    )?;
    let result = LaunchPlanner::prepare_from_metadata(vec![metadata], request(directory.path())?);

    assert!(result.is_err());
    Ok(())
}

#[test]
fn inherited_profiles_use_the_client_jar_version_for_version_name()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let minecraft = VersionMetadata::from_json_slice(include_bytes!(
        "../../tests/fixtures/vanilla-1.21.1.json"
    ))?;
    let neoforge = VersionMetadata::from_json_slice(
        br#"{
          "id":"neoforge-21.1.249",
          "inheritsFrom":"1.21.1",
          "mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher",
          "arguments":{
            "jvm":["-DignoreList=client-extra,${version_name}.jar"],
            "game":[]
          }
        }"#,
    )?;

    let preparation = LaunchPlanner::prepare_from_metadata(
        vec![minecraft, neoforge],
        request(directory.path())?,
    )?;
    let arguments = preparation.plan.redacted().arguments;

    assert_eq!(preparation.version_id, "neoforge-21.1.249");
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "-DignoreList=client-extra,1.21.1.jar")
    );
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["--version", "1.21.1"])
    );
    Ok(())
}
