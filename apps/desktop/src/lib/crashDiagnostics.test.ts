import { describe, expect, it } from "vitest";
import { analyzeMinecraftLog } from "./crashDiagnostics";

describe("analyzeMinecraftLog", () => {
  it("recognizes duplicate Java packages from NeoForge output", () => {
    const findings = analyzeMinecraftLog(
      "Modules minecraft and example_mod export package com.example.shared to module integration",
    );

    expect(findings[0]?.id).toBe("duplicate-java-package");
    expect(findings[0]?.confidence).toBe("high");
  });

  it("turns class file versions into an actionable Java requirement", () => {
    const findings = analyzeMinecraftLog(
      "java.lang.UnsupportedClassVersionError: Example has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version only recognizes class file versions up to 61.0",
    );

    expect(findings[0]?.id).toBe("wrong-java-version");
    expect(findings[0]?.summary).toContain("Java 21");
  });

  it("prefers a mixin explanation over a secondary missing class", () => {
    const findings = analyzeMinecraftLog(`Mixin apply failed example.mixins.json
Caused by: java.lang.NoClassDefFoundError: example/Dependency`);

    expect(findings.map((finding) => finding.id)).toEqual(["mixin-failure"]);
  });

  it("does not diagnose ordinary output", () => {
    expect(analyzeMinecraftLog("[main/INFO]: Game started")).toEqual([]);
  });
});
