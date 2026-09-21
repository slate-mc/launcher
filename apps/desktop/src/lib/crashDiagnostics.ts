export type CrashDiagnostic = {
  id: string;
  title: string;
  summary: string;
  actions: string[];
  confidence: "high" | "medium";
  destinations: Array<{
    label: string;
    route: "content" | "settings" | "accounts";
  }>;
};

const CLASS_VERSION_TO_JAVA = new Map([
  [52, 8],
  [60, 16],
  [61, 17],
  [65, 21],
]);

export function analyzeMinecraftLog(log: string): CrashDiagnostic[] {
  if (!log.trim()) return [];

  const findings: CrashDiagnostic[] = [];
  const normalized = log.replaceAll("\r\n", "\n");

  if (
    /Modules?\s+\S+\s+and\s+\S+\s+export package\s+\S+/i.test(normalized) ||
    /package\s+\S+\s+is declared in module\s+\S+.*module\s+\S+/is.test(
      normalized,
    )
  ) {
    findings.push({
      id: "duplicate-java-package",
      title: "Duplicate mod code detected",
      summary:
        "Two installed files contain the same Java package, so the mod loader cannot safely choose between them.",
      actions: [
        "Open Mods and review recently added or duplicate entries.",
        "If this is an untouched modpack, repair the instance and report the pack release if it still fails.",
      ],
      confidence: "high",
      destinations: [{ label: "Review installed mods", route: "content" }],
    });
  }

  if (
    /java\.lang\.OutOfMemoryError|GC overhead limit exceeded/i.test(normalized)
  ) {
    findings.push({
      id: "out-of-memory",
      title: "Minecraft ran out of memory",
      summary:
        "The game exhausted the memory available to Java before it could continue.",
      actions: [
        "Increase this instance’s maximum memory, then try again.",
        "Close memory-heavy apps and avoid assigning nearly all system memory to Minecraft.",
      ],
      confidence: "high",
      destinations: [{ label: "Adjust memory", route: "settings" }],
    });
  }

  const classVersion = normalized.match(
    /class file version\s+(\d+)(?:\.\d+)?.*recognizes class file versions? up to\s+(\d+)/is,
  );
  if (/UnsupportedClassVersionError/i.test(normalized) || classVersion) {
    const required = classVersion
      ? CLASS_VERSION_TO_JAVA.get(Number(classVersion[1]))
      : undefined;
    findings.push({
      id: "wrong-java-version",
      title: "This content needs a newer Java version",
      summary: required
        ? `At least Java ${required} is required, but the selected runtime is older.`
        : "A mod or loader was compiled for a newer Java runtime than this instance is using.",
      actions: [
        "Set Java to Managed in instance settings and repair the instance.",
        "If you selected a custom Java executable, replace it with a compatible version.",
      ],
      confidence: "high",
      destinations: [{ label: "Check Java settings", route: "settings" }],
    });
  }

  if (
    /Mixin(?:TransformerError|ApplyError)|Mixin apply failed|Critical injection failure/i.test(
      normalized,
    )
  ) {
    findings.push({
      id: "mixin-failure",
      title: "An installed mod is incompatible",
      summary:
        "A mod could not apply one of its game changes. This usually follows a mod, loader, or dependency version mismatch.",
      actions: [
        "Review recently added or updated mods and disable the likely conflict.",
        "For a managed modpack, repair the instance before changing pack-owned files.",
      ],
      confidence: "medium",
      destinations: [{ label: "Review installed mods", route: "content" }],
    });
  }

  if (
    /Caused by:\s+java\.lang\.(?:NoClassDefFoundError|ClassNotFoundException):/i.test(
      normalized,
    ) &&
    !findings.some((finding) => finding.id === "mixin-failure")
  ) {
    findings.push({
      id: "missing-mod-code",
      title: "A required mod component is missing",
      summary:
        "Minecraft tried to load code that is not present, usually because a dependency is missing or the installed versions do not match.",
      actions: [
        "Repair the instance to restore managed dependencies.",
        "If you added mods manually, verify that every mod supports this Minecraft version and loader.",
      ],
      confidence: "medium",
      destinations: [{ label: "Review installed mods", route: "content" }],
    });
  }

  if (
    /DuplicateModsFoundException|Duplicate mods found|ModResolutionException:.*duplicate/is.test(
      normalized,
    )
  ) {
    findings.push({
      id: "duplicate-mod",
      title: "The same mod is installed more than once",
      summary:
        "The loader found duplicate mod identities. This can happen when two versions of one mod are present under different file names.",
      actions: [
        "Open Mods and remove or disable the older duplicate.",
        "If this is an untouched modpack, repair it to restore the published file set.",
      ],
      confidence: "high",
      destinations: [{ label: "Review installed mods", route: "content" }],
    });
  }

  if (
    /(?:requires|depends on)\s+(?:mod\s+)?["']?[a-z0-9_.-]+["']?.*(?:missing|not installed|version)/is.test(
      normalized,
    ) || /Incompatible mods found!/i.test(normalized)
  ) {
    findings.push({
      id: "dependency-mismatch",
      title: "A mod dependency does not match",
      summary:
        "At least one installed mod is missing a required dependency or needs a different dependency version.",
      actions: [
        "Review recently added mods and their required dependencies.",
        "Repair a managed modpack to restore its published dependency versions.",
      ],
      confidence: "high",
      destinations: [{ label: "Review installed mods", route: "content" }],
    });
  }

  if (
    /ZipException|zip END header not found|invalid LOC header|error in opening zip file/i.test(
      normalized,
    )
  ) {
    findings.push({
      id: "damaged-archive",
      title: "An installed file is damaged",
      summary:
        "A mod or library archive could not be read completely. The download may have been interrupted or changed on disk.",
      actions: [
        "Repair the instance to download and verify managed files again.",
        "For a manually added file, remove it and install a fresh compatible copy.",
      ],
      confidence: "high",
      destinations: [{ label: "Open repair settings", route: "settings" }],
    });
  }

  if (
    /GLFW error 65542|OpenGL.*(?:not supported|unavailable)|Failed to create (?:the )?window/i.test(
      normalized,
    )
  ) {
    findings.push({
      id: "graphics-driver",
      title: "Minecraft could not start graphics",
      summary:
        "The game could not create an OpenGL window, usually because the graphics driver is missing, outdated, or unavailable to this session.",
      actions: [
        "Install the current graphics driver from the GPU or device manufacturer.",
        "Restart the device before launching Minecraft again.",
      ],
      confidence: "high",
      destinations: [],
    });
  }

  if (/No space left on device|There is not enough space on the disk/i.test(normalized)) {
    findings.push({
      id: "disk-full",
      title: "The game drive is out of space",
      summary:
        "Minecraft could not finish writing a required file because the selected storage drive has no usable free space.",
      actions: [
        "Free space on the instance drive, then repair the instance.",
        "Open Storage settings to clear safe caches or move the instance.",
      ],
      confidence: "high",
      destinations: [{ label: "Open instance settings", route: "settings" }],
    });
  }

  if (/Invalid session|Failed to log in:.*authentication/i.test(normalized)) {
    findings.push({
      id: "session-authentication",
      title: "The Minecraft sign-in expired",
      summary:
        "The selected Minecraft account no longer has a valid game session.",
      actions: ["Refresh the account, then launch the instance again."],
      confidence: "high",
      destinations: [{ label: "Open accounts", route: "accounts" }],
    });
  }

  return uniqueFindings(findings).slice(0, 3);
}

function uniqueFindings(findings: CrashDiagnostic[]) {
  const seen = new Set<string>();
  return findings.filter((finding) => {
    if (seen.has(finding.id)) return false;
    seen.add(finding.id);
    return true;
  });
}
