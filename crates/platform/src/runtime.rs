use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaArchitecture {
    X86,
    X86_64,
    Arm64,
}

impl JavaArchitecture {
    #[must_use]
    pub const fn platform_id(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X86_64 => "x64",
            Self::Arm64 => "aarch64",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaRuntimeProbe {
    pub available: bool,
    pub version: Option<String>,
    pub executable: Option<PathBuf>,
    pub major_version: Option<u32>,
    pub architecture: Option<JavaArchitecture>,
    pub os_version: Option<String>,
    pub unavailable_reason: Option<String>,
}

#[must_use]
pub fn detect_java_runtime() -> JavaRuntimeProbe {
    let executable = find_on_path().unwrap_or_else(|| PathBuf::from("java"));
    probe_java_executable(&executable)
}

#[must_use]
pub fn probe_java_executable(executable: &Path) -> JavaRuntimeProbe {
    match Command::new(executable)
        .args(["-XshowSettings:properties", "-version"])
        .output()
    {
        Ok(output) if output.status.success() => parse_probe_output(executable, &output),
        Ok(_) => unavailable("The selected Java runtime returned an error."),
        Err(_) => unavailable("No usable Java runtime was found."),
    }
}

fn parse_probe_output(executable: &Path, output: &std::process::Output) -> JavaRuntimeProbe {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}\n{stdout}");
    let version =
        first_nonempty_line(&output.stderr).or_else(|| first_nonempty_line(&output.stdout));
    let specification = property(&combined, "java.specification.version");
    let architecture = property(&combined, "os.arch").and_then(parse_architecture);
    let major_version = specification.as_deref().and_then(parse_major);
    let os_version = property(&combined, "os.version");
    let canonical = std::fs::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
    if major_version.is_none() || architecture.is_none() || !canonical.is_absolute() {
        return unavailable(
            "The selected Java runtime did not report a valid version and architecture.",
        );
    }
    JavaRuntimeProbe {
        available: true,
        version,
        executable: Some(canonical),
        major_version,
        architecture,
        os_version,
        unavailable_reason: None,
    }
}

fn unavailable(reason: &str) -> JavaRuntimeProbe {
    JavaRuntimeProbe {
        available: false,
        version: None,
        executable: None,
        major_version: None,
        architecture: None,
        os_version: None,
        unavailable_reason: Some(reason.to_owned()),
    }
}

fn property(output: &str, name: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.trim().split_once('=')?;
        (key.trim() == name).then(|| value.trim().to_owned())
    })
}

fn parse_major(value: &str) -> Option<u32> {
    let first = value.split('.').next()?;
    let parsed = first.parse::<u32>().ok()?;
    if parsed == 1 {
        value.split('.').nth(1)?.parse().ok()
    } else {
        Some(parsed)
    }
}

fn parse_architecture(value: String) -> Option<JavaArchitecture> {
    match value.to_ascii_lowercase().as_str() {
        "amd64" | "x86_64" => Some(JavaArchitecture::X86_64),
        "x86" | "i386" | "i686" => Some(JavaArchitecture::X86),
        "aarch64" | "arm64" => Some(JavaArchitecture::Arm64),
        _ => None,
    }
}

fn find_on_path() -> Option<PathBuf> {
    let executable = if cfg!(windows) { "java.exe" } else { "java" };
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .map(|directory| directory.join(executable))
        .find(|candidate| candidate.is_file())
}

fn first_nonempty_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{JavaArchitecture, first_nonempty_line, parse_architecture, parse_major, property};

    #[test]
    fn extracts_a_single_safe_version_line() {
        assert_eq!(
            first_nonempty_line(b"\nopenjdk version \"21.0.3\"\nmore details"),
            Some("openjdk version \"21.0.3\"".to_owned())
        );
    }

    #[test]
    fn parses_java_properties_without_localized_version_text() {
        let output = "  java.specification.version = 21\n  os.arch = amd64\n";
        assert_eq!(
            property(output, "java.specification.version").as_deref(),
            Some("21")
        );
        assert_eq!(parse_major("1.8"), Some(8));
        assert_eq!(parse_major("21"), Some(21));
        assert_eq!(
            parse_architecture("amd64".to_owned()),
            Some(JavaArchitecture::X86_64)
        );
    }
}
