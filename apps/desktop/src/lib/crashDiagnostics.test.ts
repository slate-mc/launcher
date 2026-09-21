import { describe, expect, it } from "vitest";
import { analyzeMinecraftLog } from "./crashDiagnostics";

describe("analyzeMinecraftLog", () => {
  it("recognizes duplicate Java packages from NeoForge output", () => {
    const findings = analyzeMinecraftLog(
      "Modules minecraft and example_mod export package com.example.shared to module integration",
    );

    expect(findings[0]?.id).toBe("duplicate-java-package");
    expect(findings[0]?.confidence).toBe("high");
    expect(findings[0]?.destinations[0]?.route).toBe("content");
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

  it("recognizes loader dependency failures", () => {
    const findings = analyzeMinecraftLog(
      "Incompatible mods found! Mod 'Mouse Tweaks' requires fabric-api version 1.2.0, which is missing!",
    );

    expect(findings.some((finding) => finding.id === "dependency-mismatch")).toBe(true);
  });

  it("recognizes damaged jars and graphics startup failures", () => {
    const findings = analyzeMinecraftLog(`java.util.zip.ZipException: zip END header not found
GLFW error 65542: WGL: The driver does not appear to support OpenGL`);

    expect(findings.map((finding) => finding.id)).toEqual([
      "damaged-archive",
      "graphics-driver",
    ]);
  });
});
