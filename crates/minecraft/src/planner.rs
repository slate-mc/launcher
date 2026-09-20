use crate::artifact::{ArtifactError, ArtifactKind, ArtifactRequirement, NativeExtraction};
use crate::launch_plan::{EnvironmentValue, LaunchArgument, LaunchPlan, LaunchPlanError};
use crate::maven::{MavenCoordinate, MavenError};
use crate::metadata::{ArgumentEntry, DownloadInfo, Library, MetadataError, VersionMetadata};
use crate::resolver::{ResolveError, ResolvedVersion};
use crate::rules::{Architecture, RuleContext, RuleError};
use secrecy::{ExposeSecret, SecretString};
use slate_domain::PRODUCT_NAME;
use slate_platform::{ManagedRelativePath, PathPolicyError};
use std::collections::{BTreeMap, btree_map::Entry};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaRuntime {
    executable: PathBuf,
    major_version: u32,
    architecture: Architecture,
}

impl JavaRuntime {
    pub fn new(
        executable: PathBuf,
        major_version: u32,
        architecture: Architecture,
    ) -> Result<Self, LaunchBuildError> {
        if !executable.is_absolute() {
            return Err(LaunchBuildError::LayoutPathMustBeAbsolute(
                "java executable",
            ));
        }
        Ok(Self {
            executable,
            major_version,
            architecture,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchLayout {
    pub game_directory: PathBuf,
    pub libraries_directory: PathBuf,
    pub versions_directory: PathBuf,
    pub assets_directory: PathBuf,
    pub natives_directory: PathBuf,
}

impl LaunchLayout {
    pub fn validate(&self) -> Result<(), LaunchBuildError> {
        for (name, path) in [
            ("game directory", &self.game_directory),
            ("libraries directory", &self.libraries_directory),
            ("versions directory", &self.versions_directory),
            ("assets directory", &self.assets_directory),
            ("natives directory", &self.natives_directory),
        ] {
            if !path.is_absolute() {
                return Err(LaunchBuildError::LayoutPathMustBeAbsolute(name));
            }
            path_to_string(path)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct LaunchIdentity {
    player_name: String,
    profile_id: Uuid,
    access_token: SecretString,
    client_id: SecretString,
    xuid: SecretString,
}

impl LaunchIdentity {
    pub fn new(
        player_name: impl Into<String>,
        profile_id: Uuid,
        access_token: impl Into<String>,
        client_id: impl Into<String>,
        xuid: impl Into<String>,
    ) -> Result<Self, LaunchBuildError> {
        let player_name = player_name.into();
        validate_identity_value("player name", &player_name)?;
        let access_token = access_token.into();
        validate_identity_value("access token", &access_token)?;
        let client_id = client_id.into();
        validate_identity_value("client id", &client_id)?;
        let xuid = xuid.into();
        validate_identity_value("xuid", &xuid)?;
        Ok(Self {
            player_name,
            profile_id,
            access_token: SecretString::from(access_token),
            client_id: SecretString::from(client_id),
            xuid: SecretString::from(xuid),
        })
    }
}

impl std::fmt::Debug for LaunchIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LaunchIdentity")
            .field("player_name", &"<redacted>")
            .field("profile_id", &"<redacted>")
            .field("access_token", &"<redacted>")
            .field("client_id", &"<redacted>")
            .field("xuid", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuickPlay {
    Singleplayer(String),
    Multiplayer(String),
    Realms(String),
}

#[derive(Clone, Debug)]
pub struct LaunchOptions {
    pub initial_memory_mib: u32,
    pub maximum_memory_mib: u32,
    pub additional_jvm_arguments: Vec<String>,
    pub custom_resolution: Option<(u32, u32)>,
    pub demo_user: bool,
    pub quick_play: Option<QuickPlay>,
    pub additional_features: BTreeMap<String, bool>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            initial_memory_mib: 512,
            maximum_memory_mib: 2_048,
            additional_jvm_arguments: Vec::new(),
            custom_resolution: None,
            demo_user: false,
            quick_play: None,
            additional_features: BTreeMap::new(),
        }
    }
}

pub struct LaunchRequest {
    pub runtime: JavaRuntime,
    pub layout: LaunchLayout,
    pub identity: LaunchIdentity,
    pub rules: RuleContext,
    pub options: LaunchOptions,
    pub environment: BTreeMap<String, EnvironmentValue>,
}

pub struct LaunchPreparation {
    pub plan: LaunchPlan,
    pub required_artifacts: Vec<ArtifactRequirement>,
    pub native_extractions: Vec<NativeExtraction>,
    pub version_id: String,
    pub source_layers: Vec<String>,
    pub required_java_major: u32,
}

pub struct InstallPreparation {
    pub required_artifacts: Vec<ArtifactRequirement>,
    pub native_extractions: Vec<NativeExtraction>,
    pub version_id: String,
    pub source_layers: Vec<String>,
    pub required_java_major: u32,
}

pub struct LaunchPlanner;

impl LaunchPlanner {
    pub fn prepare_install(
        version: &ResolvedVersion,
        layout: &LaunchLayout,
        rules: &RuleContext,
    ) -> Result<InstallPreparation, LaunchBuildError> {
        layout.validate()?;
        let files = resolve_launch_files(version, layout, rules)?;
        Ok(InstallPreparation {
            required_artifacts: files.artifacts.into_values().collect(),
            native_extractions: files.native_extractions,
            version_id: version.id().to_owned(),
            source_layers: version.source_layers().to_vec(),
            required_java_major: version.java_version().major_version,
        })
    }

    pub fn prepare(
        version: &ResolvedVersion,
        request: LaunchRequest,
    ) -> Result<LaunchPreparation, LaunchBuildError> {
        request.layout.validate()?;
        validate_options(&request.options)?;
        if request.runtime.major_version != version.java_version().major_version {
            return Err(LaunchBuildError::JavaMajorMismatch {
                required: version.java_version().major_version,
                actual: request.runtime.major_version,
            });
        }
        if request.runtime.architecture != request.rules.architecture {
            return Err(LaunchBuildError::RuntimeArchitectureMismatch);
        }

        let mut rules = request.rules.clone();
        apply_option_features(&mut rules, &request.options);
        let files = resolve_launch_files(version, &request.layout, &rules)?;
        let mut artifacts = files.artifacts;
        let native_extractions = files.native_extractions;
        let classpath = files.classpath;

        let classpath = classpath
            .iter()
            .map(|path| path_to_string(path))
            .collect::<Result<Vec<_>, _>>()?
            .join(rules.operating_system.classpath_separator());
        let mut templates = template_values(version, &request, &classpath)?;

        let mut plan = LaunchPlan::new(
            request.runtime.executable,
            request.layout.game_directory.clone(),
        )?;
        plan.push_argument(LaunchArgument::public(format!(
            "-Xms{}M",
            request.options.initial_memory_mib
        )));
        plan.push_argument(LaunchArgument::public(format!(
            "-Xmx{}M",
            request.options.maximum_memory_mib
        )));
        for argument in &request.options.additional_jvm_arguments {
            plan.push_argument(LaunchArgument::public(argument.clone()));
        }

        let mut has_classpath = false;
        let mut has_native_path = false;
        for argument in version.jvm_arguments() {
            for raw in enabled_argument_values(argument, &rules)? {
                has_classpath |= raw.contains("${classpath}");
                has_native_path |= raw.contains("${natives_directory}");
                plan.push_argument(expand_template(raw, &templates)?);
            }
        }
        if !has_native_path {
            plan.push_argument(expand_template(
                "-Djava.library.path=${natives_directory}",
                &templates,
            )?);
        }
        if !has_classpath {
            plan.push_argument(LaunchArgument::public("-cp"));
            plan.push_argument(expand_template("${classpath}", &templates)?);
        }

        if let Some(logging) = version.logging() {
            let file_id = logging
                .file
                .id
                .clone()
                .or_else(|| UrlFileName::from_url(&logging.file.url).map(|value| value.0))
                .ok_or(LaunchBuildError::MissingLoggingFileId)?;
            let logging_relative = ManagedRelativePath::parse(&format!("log_configs/{file_id}"))?;
            let logging_path = logging_relative.resolve_under(&request.layout.assets_directory);
            add_requirement(
                &mut artifacts,
                ArtifactRequirement::from_download(
                    ArtifactKind::LoggingConfiguration,
                    logging_path.clone(),
                    &logging.file,
                )?,
            )?;
            templates.insert(
                "path".to_owned(),
                TemplateValue::sensitive("managed-path", path_to_string(&logging_path)?),
            );
            plan.push_argument(expand_template(&logging.argument, &templates)?);
        }

        plan.push_argument(LaunchArgument::public(version.main_class()));
        for legacy in version.legacy_game_arguments() {
            let values = shlex::split(legacy).ok_or(LaunchBuildError::InvalidLegacyArguments)?;
            for value in values {
                plan.push_argument(expand_template(&value, &templates)?);
            }
        }
        for argument in version.game_arguments() {
            for raw in enabled_argument_values(argument, &rules)? {
                plan.push_argument(expand_template(raw, &templates)?);
            }
        }
        for (key, value) in request.environment {
            plan.set_environment(key, value)?;
        }

        Ok(LaunchPreparation {
            plan,
            required_artifacts: artifacts.into_values().collect(),
            native_extractions,
            version_id: version.id().to_owned(),
            source_layers: version.source_layers().to_vec(),
            required_java_major: version.java_version().major_version,
        })
    }

    pub fn prepare_from_metadata(
        layers: Vec<VersionMetadata>,
        request: LaunchRequest,
    ) -> Result<LaunchPreparation, LaunchBuildError> {
        let resolved = ResolvedVersion::resolve(layers)?;
        Self::prepare(&resolved, request)
    }
}

struct LaunchFiles {
    artifacts: BTreeMap<PathBuf, ArtifactRequirement>,
    classpath: Vec<PathBuf>,
    native_extractions: Vec<NativeExtraction>,
}

fn resolve_launch_files(
    version: &ResolvedVersion,
    layout: &LaunchLayout,
    rules: &RuleContext,
) -> Result<LaunchFiles, LaunchBuildError> {
    let mut artifacts = BTreeMap::<PathBuf, ArtifactRequirement>::new();
    let mut classpath = Vec::new();
    let mut native_extractions = Vec::new();
    for library in version.libraries() {
        if !rules.rules_allow(&library.rules)? {
            continue;
        }
        add_library(
            library,
            rules,
            layout,
            &mut artifacts,
            &mut classpath,
            &mut native_extractions,
        )?;
    }

    let client_relative =
        ManagedRelativePath::parse(&format!("{0}/{0}.jar", version.client_jar_version()))?;
    let client_path = client_relative.resolve_under(&layout.versions_directory);
    add_requirement(
        &mut artifacts,
        ArtifactRequirement::from_download(
            ArtifactKind::Client,
            client_path.clone(),
            version.client_download(),
        )?,
    )?;
    classpath.push(client_path);

    let asset_index_relative =
        ManagedRelativePath::parse(&format!("indexes/{}.json", version.asset_index().id))?;
    let asset_index_download = DownloadInfo {
        id: Some(version.asset_index().id.clone()),
        path: None,
        sha1: Some(version.asset_index().sha1.clone()),
        sha256: None,
        size: Some(version.asset_index().size),
        url: version.asset_index().url.clone(),
    };
    add_requirement(
        &mut artifacts,
        ArtifactRequirement::from_download(
            ArtifactKind::AssetIndex,
            asset_index_relative.resolve_under(&layout.assets_directory),
            &asset_index_download,
        )?,
    )?;

    if let Some(logging) = version.logging() {
        let file_id = logging
            .file
            .id
            .clone()
            .or_else(|| UrlFileName::from_url(&logging.file.url).map(|value| value.0))
            .ok_or(LaunchBuildError::MissingLoggingFileId)?;
        let logging_relative = ManagedRelativePath::parse(&format!("log_configs/{file_id}"))?;
        let logging_path = logging_relative.resolve_under(&layout.assets_directory);
        add_requirement(
            &mut artifacts,
            ArtifactRequirement::from_download(
                ArtifactKind::LoggingConfiguration,
                logging_path,
                &logging.file,
            )?,
        )?;
    }

    Ok(LaunchFiles {
        artifacts,
        classpath,
        native_extractions,
    })
}

struct UrlFileName(String);

impl UrlFileName {
    fn from_url(value: &str) -> Option<Self> {
        url::Url::parse(value)
            .ok()?
            .path_segments()?
            .next_back()
            .filter(|name| !name.is_empty())
            .map(|name| Self(name.to_owned()))
    }
}

fn add_library(
    library: &Library,
    rules: &RuleContext,
    layout: &LaunchLayout,
    artifacts: &mut BTreeMap<PathBuf, ArtifactRequirement>,
    classpath: &mut Vec<PathBuf>,
    native_extractions: &mut Vec<NativeExtraction>,
) -> Result<(), LaunchBuildError> {
    let coordinate = MavenCoordinate::parse(&library.name)?;
    let repository_path = coordinate.repository_path();
    let (target_path, requirement) = if let Some(download) = library
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.artifact.as_ref())
    {
        let relative = ManagedRelativePath::parse(
            download.path.as_deref().unwrap_or(repository_path.as_str()),
        )?;
        let target = relative.resolve_under(&layout.libraries_directory);
        let requirement =
            ArtifactRequirement::from_download(ArtifactKind::Library, target.clone(), download)?;
        (target, requirement)
    } else {
        let relative = ManagedRelativePath::parse(&repository_path)?;
        let target = relative.resolve_under(&layout.libraries_directory);
        let source = coordinate.artifact_url(library.url.as_deref())?;
        let requirement = ArtifactRequirement::from_maven(
            ArtifactKind::Library,
            source,
            target.clone(),
            library.size,
            library.sha1.as_deref(),
            library.sha256.as_deref(),
        )?;
        (target, requirement)
    };
    add_requirement(artifacts, requirement)?;
    classpath.push(target_path);

    if let Some(classifier_template) = library.natives.get(rules.operating_system.metadata_name()) {
        let classifier = classifier_template.replace("${arch}", rules.architecture.bit_width());
        let download = library
            .downloads
            .as_ref()
            .and_then(|downloads| downloads.classifiers.get(&classifier))
            .ok_or_else(|| LaunchBuildError::MissingNativeClassifier {
                library: library.name.clone(),
                classifier: classifier.clone(),
            })?;
        let native_coordinate = append_classifier(&coordinate, &classifier)?;
        let native_repository_path = native_coordinate.repository_path();
        let relative = ManagedRelativePath::parse(
            download
                .path
                .as_deref()
                .unwrap_or(native_repository_path.as_str()),
        )?;
        let archive_path = relative.resolve_under(&layout.libraries_directory);
        add_requirement(
            artifacts,
            ArtifactRequirement::from_download(
                ArtifactKind::NativeArchive,
                archive_path.clone(),
                download,
            )?,
        )?;
        native_extractions.push(NativeExtraction {
            archive_path,
            destination: layout.natives_directory.clone(),
            excludes: library
                .extract
                .as_ref()
                .map_or_else(Vec::new, |extract| extract.exclude.clone()),
        });
    }
    Ok(())
}

fn append_classifier(
    coordinate: &MavenCoordinate,
    classifier: &str,
) -> Result<MavenCoordinate, MavenError> {
    let base = coordinate.to_string();
    let (coordinate, extension) = base
        .rsplit_once('@')
        .map_or((base.as_str(), None), |parts| (parts.0, Some(parts.1)));
    let value = extension.map_or_else(
        || format!("{coordinate}:{classifier}"),
        |extension| format!("{coordinate}:{classifier}@{extension}"),
    );
    MavenCoordinate::parse(&value)
}

fn add_requirement(
    artifacts: &mut BTreeMap<PathBuf, ArtifactRequirement>,
    requirement: ArtifactRequirement,
) -> Result<(), LaunchBuildError> {
    match artifacts.entry(requirement.target_path().to_path_buf()) {
        Entry::Vacant(entry) => {
            entry.insert(requirement);
            Ok(())
        }
        Entry::Occupied(entry) if entry.get() == &requirement => Ok(()),
        Entry::Occupied(entry) => Err(LaunchBuildError::ArtifactTargetConflict(
            entry.key().to_path_buf(),
        )),
    }
}

fn enabled_argument_values<'a>(
    argument: &'a ArgumentEntry,
    rules: &RuleContext,
) -> Result<Vec<&'a str>, RuleError> {
    match argument {
        ArgumentEntry::Literal(value) => Ok(vec![value]),
        ArgumentEntry::Conditional {
            rules: conditions,
            value,
        } if rules.rules_allow(conditions)? => Ok(value.values().collect()),
        ArgumentEntry::Conditional { .. } => Ok(Vec::new()),
    }
}

fn apply_option_features(rules: &mut RuleContext, options: &LaunchOptions) {
    rules
        .features
        .insert("is_demo_user".to_owned(), options.demo_user);
    rules.features.insert(
        "has_custom_resolution".to_owned(),
        options.custom_resolution.is_some(),
    );
    rules.features.insert(
        "has_quick_plays_support".to_owned(),
        options.quick_play.is_some(),
    );
    rules.features.insert(
        "is_quick_play_singleplayer".to_owned(),
        matches!(
            options.quick_play.as_ref(),
            Some(QuickPlay::Singleplayer(_))
        ),
    );
    rules.features.insert(
        "is_quick_play_multiplayer".to_owned(),
        matches!(options.quick_play.as_ref(), Some(QuickPlay::Multiplayer(_))),
    );
    rules.features.insert(
        "is_quick_play_realms".to_owned(),
        matches!(options.quick_play.as_ref(), Some(QuickPlay::Realms(_))),
    );
    rules.features.extend(options.additional_features.clone());
}

fn validate_options(options: &LaunchOptions) -> Result<(), LaunchBuildError> {
    if options.initial_memory_mib == 0
        || options.maximum_memory_mib == 0
        || options.initial_memory_mib > options.maximum_memory_mib
    {
        return Err(LaunchBuildError::InvalidMemoryRange);
    }
    if options.custom_resolution.is_some_and(|(width, height)| {
        width < 320 || height < 240 || width > 16_384 || height > 16_384
    }) {
        return Err(LaunchBuildError::InvalidResolution);
    }
    if options.additional_jvm_arguments.len() > 64
        || options
            .additional_jvm_arguments
            .iter()
            .any(|argument| !is_safe_additional_jvm_argument(argument))
    {
        return Err(LaunchBuildError::InvalidAdditionalJvmArgument);
    }
    Ok(())
}

fn is_safe_additional_jvm_argument(argument: &str) -> bool {
    const BLOCKED_PREFIXES: &[&str] = &[
        "-xms",
        "-xmx",
        "-cp",
        "-classpath",
        "--class-path",
        "--module-path",
        "-jar",
        "-javaagent",
        "-agentlib",
        "-agentpath",
        "-djava.library.path",
    ];
    if argument.is_empty()
        || argument.len() > 512
        || argument.starts_with('@')
        || argument.chars().any(char::is_control)
    {
        return false;
    }
    let normalized = argument.to_ascii_lowercase();
    !BLOCKED_PREFIXES
        .iter()
        .any(|prefix| normalized == *prefix || normalized.starts_with(&format!("{prefix}=")))
}

fn validate_identity_value(field: &'static str, value: &str) -> Result<(), LaunchBuildError> {
    if value.is_empty() || value.len() > 4_096 || value.chars().any(char::is_control) {
        return Err(LaunchBuildError::InvalidIdentityField(field));
    }
    Ok(())
}

#[derive(Clone)]
enum TemplateValue {
    Public(String),
    Sensitive { label: String, value: SecretString },
}

impl TemplateValue {
    fn sensitive(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Sensitive {
            label: label.into(),
            value: SecretString::from(value.into()),
        }
    }

    fn exposed(&self) -> &str {
        match self {
            Self::Public(value) => value,
            Self::Sensitive { value, .. } => value.expose_secret(),
        }
    }
}

fn template_values(
    version: &ResolvedVersion,
    request: &LaunchRequest,
    classpath: &str,
) -> Result<BTreeMap<String, TemplateValue>, LaunchBuildError> {
    let mut values = BTreeMap::from([
        (
            "auth_player_name".to_owned(),
            TemplateValue::sensitive("player-name", request.identity.player_name.clone()),
        ),
        (
            "version_name".to_owned(),
            TemplateValue::Public(version.client_jar_version().to_owned()),
        ),
        (
            "game_directory".to_owned(),
            TemplateValue::sensitive(
                "managed-path",
                path_to_string(&request.layout.game_directory)?,
            ),
        ),
        (
            "assets_root".to_owned(),
            TemplateValue::sensitive(
                "managed-path",
                path_to_string(&request.layout.assets_directory)?,
            ),
        ),
        (
            "game_assets".to_owned(),
            TemplateValue::sensitive(
                "managed-path",
                path_to_string(&request.layout.assets_directory)?,
            ),
        ),
        (
            "assets_index_name".to_owned(),
            TemplateValue::Public(version.asset_index().id.clone()),
        ),
        (
            "auth_uuid".to_owned(),
            TemplateValue::sensitive(
                "player-uuid",
                request.identity.profile_id.simple().to_string(),
            ),
        ),
        (
            "auth_access_token".to_owned(),
            TemplateValue::Sensitive {
                label: "access-token".to_owned(),
                value: request.identity.access_token.clone(),
            },
        ),
        (
            "auth_session".to_owned(),
            TemplateValue::sensitive(
                "auth-session",
                format!(
                    "token:{}:{}",
                    request.identity.access_token.expose_secret(),
                    request.identity.profile_id.simple()
                ),
            ),
        ),
        (
            "clientid".to_owned(),
            TemplateValue::Sensitive {
                label: "client-id".to_owned(),
                value: request.identity.client_id.clone(),
            },
        ),
        (
            "auth_xuid".to_owned(),
            TemplateValue::Sensitive {
                label: "account-xuid".to_owned(),
                value: request.identity.xuid.clone(),
            },
        ),
        (
            "user_type".to_owned(),
            TemplateValue::Public("msa".to_owned()),
        ),
        (
            "user_properties".to_owned(),
            TemplateValue::Public("{}".to_owned()),
        ),
        (
            "version_type".to_owned(),
            TemplateValue::Public(version.version_type().to_owned()),
        ),
        (
            "natives_directory".to_owned(),
            TemplateValue::sensitive(
                "managed-path",
                path_to_string(&request.layout.natives_directory)?,
            ),
        ),
        (
            "library_directory".to_owned(),
            TemplateValue::sensitive(
                "managed-path",
                path_to_string(&request.layout.libraries_directory)?,
            ),
        ),
        (
            "launcher_name".to_owned(),
            TemplateValue::Public(PRODUCT_NAME.to_owned()),
        ),
        (
            "launcher_version".to_owned(),
            TemplateValue::Public(env!("CARGO_PKG_VERSION").to_owned()),
        ),
        (
            "classpath".to_owned(),
            TemplateValue::sensitive("managed-classpath", classpath.to_owned()),
        ),
        (
            "classpath_separator".to_owned(),
            TemplateValue::Public(
                request
                    .rules
                    .operating_system
                    .classpath_separator()
                    .to_owned(),
            ),
        ),
    ]);

    if let Some((width, height)) = request.options.custom_resolution {
        values.insert(
            "resolution_width".to_owned(),
            TemplateValue::Public(width.to_string()),
        );
        values.insert(
            "resolution_height".to_owned(),
            TemplateValue::Public(height.to_string()),
        );
    }
    if let Some(quick_play) = &request.options.quick_play {
        let (key, label, value) = match quick_play {
            QuickPlay::Singleplayer(value) => ("quickPlaySingleplayer", "world-destination", value),
            QuickPlay::Multiplayer(value) => ("quickPlayMultiplayer", "server-destination", value),
            QuickPlay::Realms(value) => ("quickPlayRealms", "realm-destination", value),
        };
        values.insert(
            key.to_owned(),
            TemplateValue::sensitive(label, value.clone()),
        );
        values.insert(
            "quickPlayPath".to_owned(),
            TemplateValue::sensitive("quick-play-path", value.clone()),
        );
    }
    Ok(values)
}

fn expand_template(
    template: &str,
    values: &BTreeMap<String, TemplateValue>,
) -> Result<LaunchArgument, LaunchBuildError> {
    let mut rendered = String::with_capacity(template.len());
    let mut remaining = template;
    let mut sensitive_labels = Vec::new();

    while let Some(start) = remaining.find("${") {
        rendered.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let end = after_start
            .find('}')
            .ok_or_else(|| LaunchBuildError::MalformedPlaceholder(template.to_owned()))?;
        let key = &after_start[..end];
        let value = values
            .get(key)
            .ok_or_else(|| LaunchBuildError::UnknownPlaceholder(key.to_owned()))?;
        rendered.push_str(value.exposed());
        if let TemplateValue::Sensitive { label, .. } = value
            && !sensitive_labels.contains(label)
        {
            sensitive_labels.push(label.clone());
        }
        remaining = &after_start[end + 1..];
    }
    rendered.push_str(remaining);

    if sensitive_labels.is_empty() {
        Ok(LaunchArgument::public(rendered))
    } else {
        Ok(LaunchArgument::sensitive(
            sensitive_labels.join("+"),
            rendered,
        )?)
    }
}

fn path_to_string(path: &Path) -> Result<String, LaunchBuildError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or(LaunchBuildError::NonUnicodePath)
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchBuildError {
    #[error("{0} must be an absolute path")]
    LayoutPathMustBeAbsolute(&'static str),
    #[error("launch paths must be valid Unicode")]
    NonUnicodePath,
    #[error("selected Java major does not match metadata: required {required}, got {actual}")]
    JavaMajorMismatch { required: u32, actual: u32 },
    #[error("selected Java architecture does not match the launch platform")]
    RuntimeArchitectureMismatch,
    #[error("initial and maximum memory values form an invalid range")]
    InvalidMemoryRange,
    #[error("custom resolution is outside supported safety bounds")]
    InvalidResolution,
    #[error("an additional JVM argument overrides a slate-managed launch setting")]
    InvalidAdditionalJvmArgument,
    #[error("identity field is empty, oversized, or contains control characters: {0}")]
    InvalidIdentityField(&'static str),
    #[error("library {library} is missing native classifier {classifier}")]
    MissingNativeClassifier { library: String, classifier: String },
    #[error("two artifacts resolve to conflicting target {0}")]
    ArtifactTargetConflict(PathBuf),
    #[error("logging configuration has no safe file identity")]
    MissingLoggingFileId,
    #[error("legacy Minecraft arguments contain invalid quoting")]
    InvalidLegacyArguments,
    #[error("launch argument contains an unknown placeholder: {0}")]
    UnknownPlaceholder(String),
    #[error("launch argument contains a malformed placeholder: {0}")]
    MalformedPlaceholder(String),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Metadata(#[from] MetadataError),
    #[error(transparent)]
    Rule(#[from] RuleError),
    #[error(transparent)]
    Maven(#[from] MavenError),
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
    #[error(transparent)]
    PathPolicy(#[from] PathPolicyError),
    #[error(transparent)]
    Plan(#[from] LaunchPlanError),
}

#[cfg(test)]
mod tests {
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
            "../tests/fixtures/vanilla-1.21.1.json"
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
        let result =
            LaunchPlanner::prepare_from_metadata(vec![metadata], request(directory.path())?);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn inherited_profiles_use_the_client_jar_version_for_version_name()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let minecraft = VersionMetadata::from_json_slice(include_bytes!(
            "../tests/fixtures/vanilla-1.21.1.json"
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
}
