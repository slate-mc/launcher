use std::fmt;
use url::Url;

const DEFAULT_LIBRARY_REPOSITORY: &str = "https://libraries.minecraft.net/";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MavenCoordinate {
    group: String,
    artifact: String,
    version: String,
    classifier: Option<String>,
    extension: String,
}

impl MavenCoordinate {
    pub fn parse(value: &str) -> Result<Self, MavenError> {
        let (coordinate, extension) = value
            .rsplit_once('@')
            .map_or((value, "jar"), |(coordinate, extension)| {
                (coordinate, extension)
            });
        let parts: Vec<&str> = coordinate.split(':').collect();
        if !(3..=4).contains(&parts.len()) {
            return Err(MavenError::InvalidCoordinate(value.to_owned()));
        }

        validate_group(parts[0], value)?;
        for component in &parts[1..] {
            validate_component(component, value)?;
        }
        validate_component(extension, value)?;

        Ok(Self {
            group: parts[0].to_owned(),
            artifact: parts[1].to_owned(),
            version: parts[2].to_owned(),
            classifier: parts.get(3).map(|value| (*value).to_owned()),
            extension: extension.to_owned(),
        })
    }

    #[must_use]
    pub fn repository_path(&self) -> String {
        let group = self.group.replace('.', "/");
        let classifier = self
            .classifier
            .as_ref()
            .map_or_else(String::new, |value| format!("-{value}"));
        format!(
            "{group}/{artifact}/{version}/{artifact}-{version}{classifier}.{extension}",
            artifact = self.artifact,
            version = self.version,
            extension = self.extension,
        )
    }

    #[must_use]
    pub fn dependency_key(&self) -> String {
        format!(
            "{}:{}:{}@{}",
            self.group,
            self.artifact,
            self.classifier.as_deref().unwrap_or(""),
            self.extension
        )
    }

    pub fn artifact_url(&self, repository: Option<&str>) -> Result<Url, MavenError> {
        let repository = repository.unwrap_or(DEFAULT_LIBRARY_REPOSITORY);
        let base = Url::parse(repository).map_err(MavenError::InvalidRepositoryUrl)?;
        if base.scheme() != "https" || base.host_str().is_none() {
            return Err(MavenError::InsecureRepositoryUrl(repository.to_owned()));
        }
        base.join(&self.repository_path())
            .map_err(MavenError::InvalidRepositoryUrl)
    }
}

impl fmt::Display for MavenCoordinate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.group, self.artifact, self.version
        )?;
        if let Some(classifier) = &self.classifier {
            write!(formatter, ":{classifier}")?;
        }
        if self.extension != "jar" {
            write!(formatter, "@{}", self.extension)?;
        }
        Ok(())
    }
}

fn validate_group(group: &str, original: &str) -> Result<(), MavenError> {
    if group.split('.').any(|part| part.is_empty()) {
        return Err(MavenError::InvalidCoordinate(original.to_owned()));
    }
    for part in group.split('.') {
        validate_component(part, original)?;
    }
    Ok(())
}

fn validate_component(component: &str, original: &str) -> Result<(), MavenError> {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.len() > 255
        || !component
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-+".contains(character))
    {
        return Err(MavenError::InvalidCoordinate(original.to_owned()));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MavenError {
    #[error("invalid Maven coordinate: {0}")]
    InvalidCoordinate(String),
    #[error("invalid Maven repository URL")]
    InvalidRepositoryUrl(#[source] url::ParseError),
    #[error("Maven repository URL must use HTTPS: {0}")]
    InsecureRepositoryUrl(String),
}

#[cfg(test)]
mod tests {
    use super::MavenCoordinate;

    #[test]
    fn resolves_classifier_and_extension() -> Result<(), Box<dyn std::error::Error>> {
        let coordinate =
            MavenCoordinate::parse("net.neoforged:neoform:1.21.1-20240808.144430:mappings@txt")?;

        assert_eq!(
            coordinate.repository_path(),
            "net/neoforged/neoform/1.21.1-20240808.144430/neoform-1.21.1-20240808.144430-mappings.txt"
        );
        assert_eq!(
            coordinate
                .artifact_url(Some("https://maven.neoforged.net/releases/"))?
                .as_str(),
            "https://maven.neoforged.net/releases/net/neoforged/neoform/1.21.1-20240808.144430/neoform-1.21.1-20240808.144430-mappings.txt"
        );
        Ok(())
    }

    #[test]
    fn rejects_path_and_origin_injection() {
        assert!(MavenCoordinate::parse("g:a:../../escape").is_err());
        let coordinate = MavenCoordinate::parse("g:a:1");
        assert!(coordinate.is_ok());
        if let Ok(coordinate) = coordinate {
            assert!(
                coordinate
                    .artifact_url(Some("http://example.com/"))
                    .is_err()
            );
        }
    }
}
