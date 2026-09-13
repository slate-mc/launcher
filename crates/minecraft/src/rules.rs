use crate::metadata::{Rule, RuleAction};
use regex::Regex;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatingSystem {
    Windows,
    Linux,
    MacOs,
}

impl OperatingSystem {
    #[must_use]
    pub const fn metadata_name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::MacOs => "osx",
        }
    }

    #[must_use]
    pub const fn classpath_separator(self) -> &'static str {
        match self {
            Self::Windows => ";",
            Self::Linux | Self::MacOs => ":",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Architecture {
    X86,
    X86_64,
    Arm64,
}

impl Architecture {
    #[must_use]
    pub const fn metadata_name(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X86_64 => "x86_64",
            Self::Arm64 => "arm64",
        }
    }

    #[must_use]
    pub const fn bit_width(self) -> &'static str {
        match self {
            Self::X86 => "32",
            Self::X86_64 | Self::Arm64 => "64",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuleContext {
    pub operating_system: OperatingSystem,
    pub architecture: Architecture,
    pub os_version: String,
    pub features: BTreeMap<String, bool>,
}

impl RuleContext {
    #[must_use]
    pub fn new(
        operating_system: OperatingSystem,
        architecture: Architecture,
        os_version: impl Into<String>,
    ) -> Self {
        Self {
            operating_system,
            architecture,
            os_version: os_version.into(),
            features: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_feature(mut self, key: impl Into<String>, enabled: bool) -> Self {
        self.features.insert(key.into(), enabled);
        self
    }

    pub fn rules_allow(&self, rules: &[Rule]) -> Result<bool, RuleError> {
        if rules.is_empty() {
            return Ok(true);
        }

        let mut allowed = false;
        for rule in rules {
            if self.matches(rule)? {
                allowed = rule.action == RuleAction::Allow;
            }
        }
        Ok(allowed)
    }

    fn matches(&self, rule: &Rule) -> Result<bool, RuleError> {
        if let Some(os) = &rule.os {
            if os
                .name
                .as_deref()
                .is_some_and(|name| name != self.operating_system.metadata_name())
            {
                return Ok(false);
            }
            if os
                .arch
                .as_deref()
                .is_some_and(|arch| arch != self.architecture.metadata_name())
            {
                return Ok(false);
            }
            if let Some(pattern) = &os.version {
                let expression =
                    Regex::new(pattern).map_err(|source| RuleError::InvalidOsVersion {
                        pattern: pattern.clone(),
                        source,
                    })?;
                if !expression.is_match(&self.os_version) {
                    return Ok(false);
                }
            }
        }

        Ok(rule
            .features
            .iter()
            .all(|(key, expected)| self.features.get(key).copied().unwrap_or(false) == *expected))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    #[error("metadata contains an invalid OS version rule: {pattern}")]
    InvalidOsVersion {
        pattern: String,
        #[source]
        source: regex::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::{Architecture, OperatingSystem, RuleContext};
    use crate::metadata::VersionMetadata;

    #[test]
    fn applies_ordered_allow_and_disallow_rules() -> Result<(), Box<dyn std::error::Error>> {
        let metadata = VersionMetadata::from_json_slice(
            br#"{
                "id":"rules",
                "libraries":[{
                    "name":"example:library:1.0",
                    "rules":[
                        {"action":"allow"},
                        {"action":"disallow","os":{"name":"windows"}}
                    ]
                }]
            }"#,
        )?;
        let windows = RuleContext::new(OperatingSystem::Windows, Architecture::X86_64, "10.0");
        let linux = RuleContext::new(OperatingSystem::Linux, Architecture::X86_64, "6.8");

        assert!(!windows.rules_allow(&metadata.libraries[0].rules)?);
        assert!(linux.rules_allow(&metadata.libraries[0].rules)?);
        Ok(())
    }

    #[test]
    fn absent_features_are_false() -> Result<(), Box<dyn std::error::Error>> {
        let metadata = VersionMetadata::from_json_slice(
            br#"{
                "id":"features",
                "arguments":{"game":[{
                    "rules":[{"action":"allow","features":{"is_demo_user":true}}],
                    "value":"--demo"
                }]}
            }"#,
        )?;
        let context = RuleContext::new(OperatingSystem::Windows, Architecture::X86_64, "10.0");
        let rules = match &metadata.arguments.as_ref().ok_or("arguments missing")?.game[0] {
            crate::metadata::ArgumentEntry::Conditional { rules, .. } => rules,
            crate::metadata::ArgumentEntry::Literal(_) => return Err("expected conditional".into()),
        };

        assert!(!context.rules_allow(rules)?);
        Ok(())
    }
}
