export type CrashDiagnostic = {
  id: string;
  title: string;
  summary: string;
  actions: string[];
  confidence: "high" | "medium";
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
    });
  }

  return findings.slice(0, 3);
}
