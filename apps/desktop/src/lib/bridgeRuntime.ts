declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const runningInTauri = window.__TAURI_INTERNALS__ !== undefined;
const developmentPreview =
  import.meta.env.DEV &&
  !runningInTauri &&
  import.meta.env.VITE_DATA_ADAPTER !== "native";

export const bridgeMode = runningInTauri
  ? "native"
  : developmentPreview
    ? "preview"
    : "unavailable";

export function requirePreview() {
  if (bridgeMode !== "preview") {
    throw new Error("The local bridge is unavailable.");
  }
}

export function requireNativeContent() {
  if (bridgeMode !== "native") {
    throw new Error(
      "Modpack content is available only in the slate desktop app.",
    );
  }
}
