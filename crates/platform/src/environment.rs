use std::collections::BTreeMap;
use std::path::Path;

/// Builds the restricted environment inherited by Java child processes.
///
/// Minecraft mods routinely use platform directories and desktop-session
/// variables. We intentionally copy only runtime-related values and exclude
/// Java injection variables, credentials, and arbitrary application secrets.
#[must_use]
pub fn restricted_child_environment(java_executable: Option<&Path>) -> BTreeMap<String, String> {
    let mut environment = BTreeMap::new();
    for name in allowed_variable_names() {
        if let Some(value) = std::env::var_os(name) {
            environment.insert((*name).to_owned(), value.to_string_lossy().into_owned());
        }
    }

    if let Some(executable) = java_executable
        && let Some(bin) = executable.parent()
    {
        if let Some(home) = bin.parent() {
            environment.insert(
                "JAVA_HOME".to_owned(),
                home.as_os_str().to_string_lossy().into_owned(),
            );
        }

        let mut paths = vec![bin.to_path_buf()];
        if let Some(existing) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        if let Ok(path) = std::env::join_paths(paths) {
            environment.insert("PATH".to_owned(), path.to_string_lossy().into_owned());
        }
    }

    environment
}

#[cfg(target_os = "windows")]
const fn allowed_variable_names() -> &'static [&'static str] {
    &[
        "APPDATA",
        "CommonProgramFiles",
        "CommonProgramFiles(x86)",
        "CommonProgramW6432",
        "COMPUTERNAME",
        "ComSpec",
        "HOMEDRIVE",
        "HOMEPATH",
        "LANG",
        "LANGUAGE",
        "LC_ALL",
        "LOCALAPPDATA",
        "NUMBER_OF_PROCESSORS",
        "OS",
        "Path",
        "PATHEXT",
        "PROCESSOR_ARCHITECTURE",
        "PROCESSOR_IDENTIFIER",
        "PROCESSOR_LEVEL",
        "PROCESSOR_REVISION",
        "ProgramData",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "SystemDrive",
        "SystemRoot",
        "TEMP",
        "TMP",
        "USERDOMAIN",
        "USERNAME",
        "USERPROFILE",
        "WINDIR",
    ]
}

#[cfg(target_os = "macos")]
const fn allowed_variable_names() -> &'static [&'static str] {
    &[
        "HOME",
        "LANG",
        "LANGUAGE",
        "LC_ALL",
        "LOGNAME",
        "PATH",
        "SHELL",
        "TMPDIR",
        "USER",
        "XDG_CACHE_HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
const fn allowed_variable_names() -> &'static [&'static str] {
    &[
        "DBUS_SESSION_BUS_ADDRESS",
        "DESKTOP_SESSION",
        "DISPLAY",
        "HOME",
        "LANG",
        "LANGUAGE",
        "LC_ALL",
        "LOGNAME",
        "PATH",
        "SHELL",
        "TMPDIR",
        "USER",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "XDG_CACHE_HOME",
        "XDG_CONFIG_DIRS",
        "XDG_CONFIG_HOME",
        "XDG_CURRENT_DESKTOP",
        "XDG_DATA_DIRS",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_DESKTOP",
        "XDG_SESSION_TYPE",
    ]
}

#[cfg(not(any(unix, target_os = "windows")))]
const fn allowed_variable_names() -> &'static [&'static str] {
    &["HOME", "LANG", "LANGUAGE", "LC_ALL", "PATH", "TEMP", "TMP"]
}

#[cfg(test)]
mod tests {
    use super::{allowed_variable_names, restricted_child_environment};

    #[test]
    fn java_injection_and_secret_variables_are_not_inherited() {
        let allowed = allowed_variable_names();
        for denied in [
            "CLASSPATH",
            "JAVA_TOOL_OPTIONS",
            "JDK_JAVA_OPTIONS",
            "_JAVA_OPTIONS",
            "AWS_SECRET_ACCESS_KEY",
        ] {
            assert!(!allowed.contains(&denied));
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_mod_runtime_directories_are_inherited() {
        let allowed = allowed_variable_names();
        for required in [
            "APPDATA",
            "LOCALAPPDATA",
            "SystemRoot",
            "TEMP",
            "USERPROFILE",
            "WINDIR",
        ] {
            assert!(allowed.contains(&required), "missing {required}");
        }

        let environment = restricted_child_environment(None);
        assert!(environment.contains_key("LOCALAPPDATA"));
    }
}
