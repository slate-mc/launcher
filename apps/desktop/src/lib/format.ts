import type { LauncherInstance, LoaderKind } from "../types/launcher";

export function loaderLabel(loader: LoaderKind) {
  if (loader === "neoForge") return "NeoForge";
  return loader[0].toUpperCase() + loader.slice(1);
}

export function instanceVersionLine(instance: LauncherInstance) {
  const loader = loaderLabel(instance.loaderKind);
  const loaderVersion = instance.loaderVersion
    ? " " + instance.loaderVersion
    : "";
  return `Minecraft ${instance.minecraftVersion} · ${loader}${loaderVersion}`;
}

export function setupStateLabel(state: LauncherInstance["setupState"]) {
  if (state === "configured") return "Configured";
  if (state === "preparing") return "Preparing";
  if (state === "ready") return "Ready";
  return "Needs attention";
}

export function setupStateTone(
  state: LauncherInstance["setupState"],
): "positive" | "neutral" | "warning" | "danger" {
  if (state === "ready") return "positive";
  if (state === "blocked") return "danger";
  if (state === "preparing") return "warning";
  return "neutral";
}

export function formatDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "Unknown";
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  }).format(date);
}
