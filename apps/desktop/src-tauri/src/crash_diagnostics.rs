use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CrashDiagnosticSummary {
    pub code: &'static str,
    pub title: &'static str,
    pub confidence: &'static str,
}

pub(super) fn analyze_minecraft_log(log: &str) -> Vec<CrashDiagnosticSummary> {
    let normalized = log.to_ascii_lowercase();
    let mut findings = Vec::new();
    push_if(
        &mut findings,
        normalized.contains("export package")
            && (normalized.contains("modules ") || normalized.contains("module ")),
        diagnostic(
            "duplicate-java-package",
            "Duplicate mod code detected",
            "high",
        ),
    );
    push_if(
        &mut findings,
        normalized.contains("outofmemoryerror")
            || normalized.contains("gc overhead limit exceeded"),
        diagnostic("out-of-memory", "Minecraft ran out of memory", "high"),
    );
    push_if(
        &mut findings,
        normalized.contains("unsupportedclassversionerror")
            || normalized.contains("class file version"),
        diagnostic(
            "wrong-java-version",
            "This content needs a different Java version",
            "high",
        ),
    );
    push_if(
        &mut findings,
        normalized.contains("duplicate mods found")
            || normalized.contains("duplicatemodsfoundexception"),
        diagnostic(
            "duplicate-mod",
            "The same mod is installed more than once",
            "high",
        ),
    );
    push_if(
        &mut findings,
        normalized.contains("incompatible mods found")
            || (normalized.contains("requires") && normalized.contains("missing")),
        diagnostic(
            "dependency-mismatch",
            "A mod dependency does not match",
            "high",
        ),
    );
    push_if(
        &mut findings,
        normalized.contains("mixin apply failed")
            || normalized.contains("mixintransformererror")
            || normalized.contains("critical injection failure"),
        diagnostic(
            "mixin-failure",
            "An installed mod is incompatible",
            "medium",
        ),
    );
    push_if(
        &mut findings,
        normalized.contains("noclassdeffounderror")
            || normalized.contains("classnotfoundexception"),
        diagnostic(
            "missing-mod-code",
            "A required mod component is missing",
            "medium",
        ),
    );
    push_if(
        &mut findings,
        normalized.contains("zipexception")
            || normalized.contains("zip end header not found")
            || normalized.contains("invalid loc header"),
        diagnostic("damaged-archive", "An installed file is damaged", "high"),
    );
    push_if(
        &mut findings,
        normalized.contains("glfw error 65542")
            || (normalized.contains("opengl") && normalized.contains("not supported"))
            || normalized.contains("failed to create window"),
        diagnostic(
            "graphics-driver",
            "Minecraft could not start graphics",
            "high",
        ),
    );
    push_if(
        &mut findings,
        normalized.contains("no space left on device")
            || normalized.contains("not enough space on the disk"),
        diagnostic("disk-full", "The game drive is out of space", "high"),
    );
    push_if(
        &mut findings,
        normalized.contains("invalid session")
            || (normalized.contains("failed to log in") && normalized.contains("authentication")),
        diagnostic(
            "session-authentication",
            "The Minecraft sign-in expired",
            "high",
        ),
    );
    findings.truncate(3);
    findings
}

const fn diagnostic(
    code: &'static str,
    title: &'static str,
    confidence: &'static str,
) -> CrashDiagnosticSummary {
    CrashDiagnosticSummary {
        code,
        title,
        confidence,
    }
}

fn push_if(
    findings: &mut Vec<CrashDiagnosticSummary>,
    matched: bool,
    diagnostic: CrashDiagnosticSummary,
) {
    if matched
        && !findings
            .iter()
            .any(|finding| finding.code == diagnostic.code)
    {
        findings.push(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use super::analyze_minecraft_log;

    #[test]
    fn recognizes_common_modpack_and_runtime_failures_without_preserving_log_text() {
        let findings = analyze_minecraft_log(
            "Incompatible mods found! mouse_tweaks requires fabric-api, which is missing\n\
             java.util.zip.ZipException: zip END header not found",
        );
        assert_eq!(findings[0].code, "dependency-mismatch");
        assert_eq!(findings[1].code, "damaged-archive");
        assert!(
            findings
                .iter()
                .all(|finding| !finding.title.contains("mouse_tweaks"))
        );
    }
}
