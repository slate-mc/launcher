import type { InstallJob } from "../types/launcher";

export function installPhaseLabel(phase: string) {
  const labels: Record<string, string> = {
    metadata: "Version details",
    "base-game": "Base game",
    assets: "Game assets",
    runtime: "Java runtime",
    loader: "Mod loader",
    "launch-files": "Launch files",
    natives: "Platform files",
    content: "Instance content",
    verification: "Verification",
    commit: "Finalizing",
  };
  return labels[phase] ?? "Installation";
}

export function installJobMessage(job: InstallJob) {
  if (job.state === "failed") {
    return "Installation did not complete. Retry or repair the instance.";
  }
  if (job.state === "cancelled") {
    return "Installation was cancelled.";
  }
  return job.message;
}
