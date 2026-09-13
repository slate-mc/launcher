use secrecy::{ExposeSecret, SecretString};
use slate_contracts::RedactedLaunchPlan;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const REDACTED_VALUE: &str = "<redacted>";

#[derive(Clone)]
pub enum LaunchArgument {
    Public(String),
    Sensitive { label: String, value: SecretString },
}

impl LaunchArgument {
    #[must_use]
    pub fn public(value: impl Into<String>) -> Self {
        Self::Public(value.into())
    }

    pub fn sensitive(
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, LaunchPlanError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(LaunchPlanError::EmptySensitiveLabel);
        }
        Ok(Self::Sensitive {
            label,
            value: SecretString::from(value.into()),
        })
    }

    fn exposed(&self) -> &str {
        match self {
            Self::Public(value) => value,
            Self::Sensitive { value, .. } => value.expose_secret(),
        }
    }

    fn redacted(&self) -> String {
        match self {
            Self::Public(value) => value.clone(),
            Self::Sensitive { label, .. } => format!("<redacted:{label}>"),
        }
    }
}

impl fmt::Debug for LaunchArgument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("LaunchArgument")
            .field(&self.redacted())
            .finish()
    }
}

#[derive(Clone)]
pub enum EnvironmentValue {
    Public(String),
    Sensitive(SecretString),
}

impl EnvironmentValue {
    #[must_use]
    pub fn public(value: impl Into<String>) -> Self {
        Self::Public(value.into())
    }

    #[must_use]
    pub fn sensitive(value: impl Into<String>) -> Self {
        Self::Sensitive(SecretString::from(value.into()))
    }

    fn exposed(&self) -> &str {
        match self {
            Self::Public(value) => value,
            Self::Sensitive(value) => value.expose_secret(),
        }
    }

    fn redacted(&self) -> String {
        match self {
            Self::Public(value) => value.clone(),
            Self::Sensitive(_) => REDACTED_VALUE.to_owned(),
        }
    }
}

impl fmt::Debug for EnvironmentValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("EnvironmentValue")
            .field(&self.redacted())
            .finish()
    }
}

pub struct LaunchPlan {
    executable: PathBuf,
    arguments: Vec<LaunchArgument>,
    working_directory: PathBuf,
    environment: BTreeMap<String, EnvironmentValue>,
}

impl LaunchPlan {
    pub fn new(
        executable: impl Into<PathBuf>,
        working_directory: impl Into<PathBuf>,
    ) -> Result<Self, LaunchPlanError> {
        let executable = executable.into();
        if !executable.is_absolute() {
            return Err(LaunchPlanError::ExecutableMustBeAbsolute);
        }

        let working_directory = working_directory.into();
        if !working_directory.is_absolute() {
            return Err(LaunchPlanError::WorkingDirectoryMustBeAbsolute);
        }

        Ok(Self {
            executable,
            arguments: Vec::new(),
            working_directory,
            environment: BTreeMap::new(),
        })
    }

    pub fn push_argument(&mut self, argument: LaunchArgument) {
        self.arguments.push(argument);
    }

    pub fn set_environment(
        &mut self,
        key: impl Into<String>,
        value: EnvironmentValue,
    ) -> Result<(), LaunchPlanError> {
        let key = key.into();
        if key.is_empty() || key.contains(['=', '\0']) {
            return Err(LaunchPlanError::InvalidEnvironmentKey);
        }
        self.environment.insert(key, value);
        Ok(())
    }

    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    #[must_use]
    pub fn arguments(&self) -> &[LaunchArgument] {
        &self.arguments
    }

    #[must_use]
    pub fn redacted(&self) -> RedactedLaunchPlan {
        RedactedLaunchPlan {
            executable: self.executable.file_name().map_or_else(
                || "<managed-runtime>".to_owned(),
                |name| name.to_string_lossy().into(),
            ),
            arguments: self
                .arguments
                .iter()
                .map(LaunchArgument::redacted)
                .collect(),
            working_directory: "<managed-instance>".to_owned(),
            environment: self
                .environment
                .iter()
                .map(|(key, value)| (key.clone(), value.redacted()))
                .collect(),
        }
    }

    /// Materializes a process command without invoking a shell.
    ///
    /// The caller remains responsible for validating artifacts, locks, auth,
    /// and runtime compatibility before spawning the returned command.
    #[must_use]
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command.current_dir(&self.working_directory);
        command.env_clear();
        command.args(self.arguments.iter().map(LaunchArgument::exposed));
        command.envs(
            self.environment
                .iter()
                .map(|(key, value)| (key, value.exposed())),
        );
        command
    }
}

impl fmt::Debug for LaunchPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.redacted().fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LaunchPlanError {
    #[error("Java executable path must be absolute")]
    ExecutableMustBeAbsolute,
    #[error("instance working directory must be absolute")]
    WorkingDirectoryMustBeAbsolute,
    #[error("environment variable key is invalid")]
    InvalidEnvironmentKey,
    #[error("sensitive argument label must not be empty")]
    EmptySensitiveLabel,
}

#[cfg(test)]
mod tests {
    use super::{EnvironmentValue, LaunchArgument, LaunchPlan, REDACTED_VALUE};

    #[test]
    fn redacted_views_and_debug_never_contain_marked_secrets()
    -> Result<(), Box<dyn std::error::Error>> {
        let executable = std::env::current_exe()?;
        let working_directory = std::env::current_dir()?;
        let mut plan = LaunchPlan::new(executable, working_directory)?;
        plan.push_argument(LaunchArgument::public("--accessToken"));
        plan.push_argument(LaunchArgument::sensitive(
            "access-token",
            "do-not-leak-token",
        )?);
        plan.set_environment(
            "SLATE_SESSION_SECRET",
            EnvironmentValue::sensitive("do-not-leak-env"),
        )?;

        let redacted_json = serde_json::to_string(&plan.redacted())?;
        let debug = format!("{plan:?}");

        assert!(!redacted_json.contains("do-not-leak-token"));
        assert!(!redacted_json.contains("do-not-leak-env"));
        assert!(!debug.contains("do-not-leak-token"));
        assert!(!debug.contains("do-not-leak-env"));
        assert!(redacted_json.contains("<redacted:access-token>"));
        assert!(redacted_json.contains(REDACTED_VALUE));
        Ok(())
    }

    #[test]
    fn relative_process_paths_are_rejected() {
        let result = LaunchPlan::new("java", "instance");
        assert!(result.is_err());
    }
}
