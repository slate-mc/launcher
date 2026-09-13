use std::fmt;
use std::path::{Path, PathBuf};

const MAX_PATH_BYTES: usize = 1_024;
const MAX_COMPONENT_BYTES: usize = 255;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ManagedRelativePath(String);

impl ManagedRelativePath {
    pub fn parse(candidate: &str) -> Result<Self, PathPolicyError> {
        if candidate.is_empty() {
            return Err(PathPolicyError::Empty);
        }
        if candidate.len() > MAX_PATH_BYTES {
            return Err(PathPolicyError::PathTooLong);
        }
        if candidate.starts_with(['/', '\\']) {
            return Err(PathPolicyError::Absolute);
        }

        let mut normalized = Vec::new();
        for component in candidate.split(['/', '\\']) {
            validate_component(component)?;
            normalized.push(component);
        }

        Ok(Self(normalized.join("/")))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn resolve_under(&self, root: &Path) -> PathBuf {
        self.0
            .split('/')
            .fold(root.to_path_buf(), |path, part| path.join(part))
    }
}

impl fmt::Display for ManagedRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

fn validate_component(component: &str) -> Result<(), PathPolicyError> {
    if component.is_empty() {
        return Err(PathPolicyError::EmptyComponent);
    }
    if matches!(component, "." | "..") {
        return Err(PathPolicyError::Traversal);
    }
    if component.len() > MAX_COMPONENT_BYTES {
        return Err(PathPolicyError::ComponentTooLong);
    }
    if component.ends_with(['.', ' ']) {
        return Err(PathPolicyError::AmbiguousWindowsComponent);
    }
    if component.contains(':') {
        return Err(PathPolicyError::DriveOrStreamSyntax);
    }
    if component
        .chars()
        .any(|character| character == '\0' || character.is_control())
    {
        return Err(PathPolicyError::ControlCharacter);
    }

    let stem = component.split('.').next().unwrap_or_default();
    let upper = stem.to_ascii_uppercase();
    let is_reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || upper.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        });

    if is_reserved {
        return Err(PathPolicyError::ReservedWindowsName);
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PathPolicyError {
    #[error("managed path is empty")]
    Empty,
    #[error("absolute paths are not allowed")]
    Absolute,
    #[error("path traversal components are not allowed")]
    Traversal,
    #[error("empty path components are not allowed")]
    EmptyComponent,
    #[error("managed path exceeds the length limit")]
    PathTooLong,
    #[error("a path component exceeds the length limit")]
    ComponentTooLong,
    #[error("Windows drive and alternate-stream syntax is not allowed")]
    DriveOrStreamSyntax,
    #[error("path contains a control character")]
    ControlCharacter,
    #[error("path ends in a dot or space and is ambiguous on Windows")]
    AmbiguousWindowsComponent,
    #[error("path uses a reserved Windows device name")]
    ReservedWindowsName,
}

#[cfg(test)]
mod tests {
    use super::{ManagedRelativePath, PathPolicyError};
    use std::path::Path;

    #[test]
    fn normalizes_separators_and_resolves_below_root() -> Result<(), PathPolicyError> {
        let path = ManagedRelativePath::parse("config\\slate/options.json")?;

        assert_eq!(path.as_str(), "config/slate/options.json");
        assert_eq!(
            path.resolve_under(Path::new("root")),
            Path::new("root")
                .join("config")
                .join("slate")
                .join("options.json")
        );
        Ok(())
    }

    #[test]
    fn rejects_escape_and_windows_ambiguities() {
        let invalid = [
            "../outside",
            "/absolute",
            "C:\\absolute",
            "mods//duplicate.jar",
            "config/CON.txt",
            "config/trailing. ",
            "config/file:stream",
        ];

        for candidate in invalid {
            assert!(
                ManagedRelativePath::parse(candidate).is_err(),
                "{candidate} must be rejected"
            );
        }
    }
}
