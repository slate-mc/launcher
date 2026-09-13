import { invoke } from "@tauri-apps/api/core";
import {
  bootstrapSchema,
  createInstanceSchema,
  gameSessionSchema,
  installJobListSchema,
  installJobSchema,
  instanceSummaryListSchema,
  instanceSummarySchema,
  loaderVersionCatalogSchema,
  minecraftVersionCatalogSchema,
  preferencesSchema,
  preflightSchema,
  type AppPreferences,
  type Bootstrap,
  type CreateInstanceInput,
  type LauncherInstance,
  type LauncherUpdate,
  type LoaderKind,
  type LoaderVersionCatalog,
  type MinecraftVersionCatalog,
  type GameSession,
  type InstallJob,
  type Preflight,
  type ServerPreview,
} from "../types/launcher";

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

const previewStorageKey = "slate.preview.instances.v2";
const previewPreferencesKey = "slate.preview.preferences.v1";
const previewNow = "2026-09-13T12:00:00Z";

const initialPreviewInstances: LauncherInstance[] = [
  {
    id: "98d7fe64-acb5-454c-a7e5-7127a3af0c12",
    name: "Survival",
    mode: "modded",
    managementMode: "local",
    favorite: true,
    revision: 3,
    minecraftVersion: "1.21.1",
    loaderKind: "fabric",
    loaderVersion: "0.16.10",
    memoryMb: 6144,
    setupState: "configured",
    createdAt: previewNow,
    updatedAt: previewNow,
    modCount: 18,
    lastPlayed: "3 hours ago",
    description: "My main survival world",
    artworkTone: "meadow",
  },
  {
    id: "6f720f47-422f-4ebc-8983-13c8ab0bc488",
    name: "Create workshop",
    mode: "modded",
    managementMode: "local",
    favorite: true,
    revision: 8,
    minecraftVersion: "1.20.1",
    loaderKind: "neoForge",
    loaderVersion: "47.1.106",
    memoryMb: 8192,
    setupState: "configured",
    createdAt: previewNow,
    updatedAt: previewNow,
    modCount: 42,
    lastPlayed: "2 days ago",
    description: "Automation and building",
    artworkTone: "workshop",
  },
  {
    id: "00b6b709-aa78-43af-9e4a-4898be859ffc",
    name: "Vanilla",
    mode: "vanilla",
    managementMode: "local",
    favorite: false,
    revision: 1,
    minecraftVersion: "1.21.1",
    loaderKind: "vanilla",
    memoryMb: 4096,
    setupState: "configured",
    createdAt: previewNow,
    updatedAt: previewNow,
    lastPlayed: "5 days ago",
    description: "A clean vanilla experience",
    artworkTone: "vanilla",
  },
  {
    id: "13fcbabf-cc0b-448c-bc47-a44da89c6a61",
    name: "PvP",
    mode: "pvp",
    managementMode: "local",
    favorite: false,
    revision: 2,
    minecraftVersion: "1.20.4",
    loaderKind: "fabric",
    loaderVersion: "0.15.11",
    memoryMb: 4096,
    setupState: "blocked",
    createdAt: previewNow,
    updatedAt: previewNow,
    modCount: 7,
    lastPlayed: "1 week ago",
    description: "Practice and minigames",
    artworkTone: "nether",
  },
];

let previewInstances = loadPreviewInstances();
let previewPreferences = loadPreviewPreferences();
let previewInstallJobs: InstallJob[] = [];

export const previewServers: ServerPreview[] = [
  {
    id: "blockhaven",
    name: "Blockhaven SMP",
    address: "play.blockhaven.gg",
    players: "42 / 100",
    latencyBars: 4,
  },
  {
    id: "pixelrealms",
    name: "PixelRealms",
    address: "mc.pixelrealms.net",
    players: "128 / 500",
    latencyBars: 4,
  },
];

export const previewUpdates: LauncherUpdate[] = [
  {
    id: "minecraft-release",
    title: "Provider connection pending",
    description: "Release metadata will appear after providers are connected.",
    date: "Local",
    kind: "game",
  },
];

export async function getBootstrap(): Promise<Bootstrap> {
  if (bridgeMode === "native") {
    return bootstrapSchema.parse(await invoke("app_bootstrap"));
  }
  return bootstrapSchema.parse({
    productName: "slate",
    ipcSchemaVersion: 1,
    databaseSchemaVersion: 2,
    capabilities: [
      { id: "instance.library", available: true },
      { id: "instance.create", available: true },
      { id: "instance.configure", available: true },
      { id: "settings.local", available: true },
      {
        id: "minecraft.install",
        available: false,
        unavailableReason: "Download and installation jobs are not implemented yet.",
      },
      {
        id: "minecraft.launch",
        available: false,
        unavailableReason:
          "Launching Minecraft is intentionally not implemented in this build.",
      },
      {
        id: "minecraft.account",
        available: false,
        unavailableReason:
          "Microsoft account sign-in requires the product app registration.",
      },
    ],
  });
}

export async function listInstances(): Promise<LauncherInstance[]> {
  if (bridgeMode === "native") {
    return instanceSummaryListSchema.parse(await invoke("instances_list"));
  }
  return bridgeMode === "preview" ? structuredClone(previewInstances) : [];
}

export async function getMinecraftVersionCatalog(): Promise<MinecraftVersionCatalog> {
  if (bridgeMode === "native") {
    return minecraftVersionCatalogSchema.parse(
      await invoke("minecraft_versions_list"),
    );
  }
  requirePreview();
  return minecraftVersionCatalogSchema.parse({
    latestRelease: "1.21.1",
    versions: [
      { id: "1.21.1", kind: "release" },
      { id: "1.20.4", kind: "release" },
      { id: "1.20.1", kind: "release" },
    ],
  });
}

export async function getLoaderVersionCatalog(input: {
  minecraftVersion: string;
  loaderKind: LoaderKind;
}): Promise<LoaderVersionCatalog> {
  if (bridgeMode === "native") {
    return loaderVersionCatalogSchema.parse(
      await invoke("loader_versions_list", { request: input }),
    );
  }
  requirePreview();
  if (input.loaderKind === "vanilla") {
    return {
      loaderKind: "vanilla",
      minecraftVersion: input.minecraftVersion,
      versions: [],
    };
  }
  if (input.loaderKind === "fabric") {
    return {
      loaderKind: "fabric",
      minecraftVersion: input.minecraftVersion,
      recommendedVersion: "0.19.5",
      versions: ["0.19.5", "0.18.4", "0.16.10"],
    };
  }
  if (input.minecraftVersion === "1.21.1") {
    return {
      loaderKind: "neoForge",
      minecraftVersion: input.minecraftVersion,
      recommendedVersion: "21.1.250",
      versions: ["21.1.250", "21.1.249", "21.1.248-beta"],
    };
  }
  return {
    loaderKind: "neoForge",
    minecraftVersion: input.minecraftVersion,
    versions: [],
    unavailableReason:
      "The preview catalog has no NeoForge fixture for this release.",
  };
}

export async function getInstance(id: string): Promise<LauncherInstance> {
  if (bridgeMode === "native") {
    return instanceSummarySchema.parse(await invoke("instance_get", { id }));
  }
  const instance = previewInstances.find((candidate) => candidate.id === id);
  if (!instance) throw new Error("That instance no longer exists.");
  return structuredClone(instance);
}

export async function createInstance(
  input: CreateInstanceInput,
): Promise<LauncherInstance> {
  const request = createInstanceSchema.parse(input);
  if (bridgeMode === "native") {
    return instanceSummarySchema.parse(
      await invoke("instance_create", {
        request: {
          name: request.name,
          mode: request.mode,
          minecraftVersion: request.minecraftVersion,
          loaderKind: request.loaderKind,
          loaderVersion: request.loaderVersion,
          memoryMb: request.memoryMb,
        },
      }),
    );
  }
  requirePreview();
  const timestamp = new Date().toISOString();
  const instance = instanceSummarySchema.parse({
    ...request,
    loaderVersion: cleanLoaderVersion(request.loaderKind, request.loaderVersion),
    id: crypto.randomUUID(),
    managementMode: "local",
    favorite: false,
    revision: 0,
    setupState: "configured",
    createdAt: timestamp,
    updatedAt: timestamp,
    description: "Local instance",
    artworkTone: toneForLoader(request.loaderKind),
  });
  previewInstances = [instance, ...previewInstances];
  savePreviewInstances();
  return structuredClone(instance);
}

export async function renameInstance(input: {
  id: string;
  name: string;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode === "native") {
    return instanceSummarySchema.parse(
      await invoke("instance_rename", { request: input }),
    );
  }
  return updatePreviewInstance(input.id, input.expectedRevision, (instance) => ({
    ...instance,
    name: input.name.trim(),
  }));
}

export async function updateInstanceConfiguration(input: {
  id: string;
  minecraftVersion: string;
  loaderKind: LoaderKind;
  loaderVersion?: string;
  memoryMb: number;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode === "native") {
    return instanceSummarySchema.parse(
      await invoke("instance_update_configuration", {
        request: {
          id: input.id,
          minecraftVersion: input.minecraftVersion,
          loaderKind: input.loaderKind,
          loaderVersion: input.loaderVersion,
          memoryMb: input.memoryMb,
          expectedRevision: input.expectedRevision,
        },
      }),
    );
  }
  return updatePreviewInstance(input.id, input.expectedRevision, (instance) => ({
    ...instance,
    minecraftVersion: input.minecraftVersion.trim(),
    loaderKind: input.loaderKind,
    loaderVersion: cleanLoaderVersion(input.loaderKind, input.loaderVersion),
    memoryMb: input.memoryMb,
    setupState: "configured",
    artworkTone: toneForLoader(input.loaderKind),
  }));
}

export async function setInstanceFavorite(input: {
  id: string;
  favorite: boolean;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode === "native") {
    return instanceSummarySchema.parse(
      await invoke("instance_set_favorite", { request: input }),
    );
  }
  return updatePreviewInstance(input.id, input.expectedRevision, (instance) => ({
    ...instance,
    favorite: input.favorite,
  }));
}

export async function trashInstance(input: {
  id: string;
  expectedRevision: number;
}): Promise<void> {
  if (bridgeMode === "native") {
    await invoke("instance_trash", { request: input });
    return;
  }
  requirePreview();
  const instance = previewInstances.find((candidate) => candidate.id === input.id);
  if (!instance) throw new Error("That instance no longer exists.");
  if (instance.revision !== input.expectedRevision) {
    throw new Error("That instance changed. Reload and try again.");
  }
  previewInstances = previewInstances.filter((candidate) => candidate.id !== input.id);
  savePreviewInstances();
}

export async function installInstance(input: {
  id: string;
  expectedRevision: number;
}): Promise<InstallJob> {
  if (bridgeMode === "native") {
    return installJobSchema.parse(
      await invoke("instance_install", { request: input }),
    );
  }
  requirePreview();
  const updated = updatePreviewInstance(input.id, input.expectedRevision, (instance) => ({
    ...instance,
    setupState: "ready",
  }));
  const timestamp = new Date().toISOString();
  const job = installJobSchema.parse({
    id: crypto.randomUUID(),
    instanceId: input.id,
    revisionId: crypto.randomUUID(),
    state: "succeeded",
    phase: "complete",
    message: "Preview installation completed",
    createdAt: timestamp,
    updatedAt: timestamp,
  });
  previewInstallJobs = [job, ...previewInstallJobs];
  await updated;
  return structuredClone(job);
}

export async function listInstallJobs(): Promise<InstallJob[]> {
  if (bridgeMode === "native") {
    return installJobListSchema.parse(await invoke("install_jobs_list"));
  }
  return structuredClone(previewInstallJobs);
}

export async function launchDemo(id: string): Promise<GameSession> {
  if (bridgeMode === "native") {
    return gameSessionSchema.parse(
      await invoke("instance_launch_demo", { request: { id } }),
    );
  }
  requirePreview();
  return gameSessionSchema.parse({
    id: crypto.randomUUID(),
    instanceId: id,
    state: "running",
    mode: "demo",
    pid: 1,
    logName: "preview-session.log",
  });
}

export async function getPreferences(): Promise<AppPreferences> {
  if (bridgeMode === "native") {
    return preferencesSchema.parse(await invoke("preferences_get"));
  }
  return structuredClone(previewPreferences);
}

export async function updatePreferences(
  request: AppPreferences,
): Promise<AppPreferences> {
  const parsed = preferencesSchema.parse(request);
  if (bridgeMode === "native") {
    return preferencesSchema.parse(
      await invoke("preferences_update", { request: parsed }),
    );
  }
  requirePreview();
  previewPreferences = parsed;
  localStorage.setItem(previewPreferencesKey, JSON.stringify(parsed));
  return structuredClone(parsed);
}

export async function getPreflight(): Promise<Preflight> {
  if (bridgeMode === "native") {
    return preflightSchema.parse(await invoke("preflight_get"));
  }
  return preflightSchema.parse({
    databaseReady: true,
    storageReady: true,
    accountConfigured: false,
    java: {
      available: true,
      version: 'openjdk version "21.0.3"',
    },
    launchImplemented: true,
  });
}

function updatePreviewInstance(
  id: string,
  expectedRevision: number,
  update: (instance: LauncherInstance) => LauncherInstance,
) {
  requirePreview();
  const index = previewInstances.findIndex((candidate) => candidate.id === id);
  const current = previewInstances[index];
  if (!current) throw new Error("That instance no longer exists.");
  if (current.revision !== expectedRevision) {
    throw new Error("That instance changed. Reload and try again.");
  }
  const next = instanceSummarySchema.parse({
    ...update(current),
    revision: current.revision + 1,
    updatedAt: new Date().toISOString(),
  });
  const nextInstances = [...previewInstances];
  nextInstances[index] = next;
  previewInstances = nextInstances;
  savePreviewInstances();
  return structuredClone(next);
}

function loadPreviewInstances() {
  try {
    const stored = localStorage.getItem(previewStorageKey);
    return stored
      ? instanceSummaryListSchema.parse(JSON.parse(stored))
      : structuredClone(initialPreviewInstances);
  } catch {
    return structuredClone(initialPreviewInstances);
  }
}

function savePreviewInstances() {
  localStorage.setItem(previewStorageKey, JSON.stringify(previewInstances));
}

function loadPreviewPreferences(): AppPreferences {
  try {
    const stored = localStorage.getItem(previewPreferencesKey);
    return stored
      ? preferencesSchema.parse(JSON.parse(stored))
      : defaultPreferences();
  } catch {
    return defaultPreferences();
  }
}

function defaultPreferences(): AppPreferences {
  return {
    theme: "dark",
    downloadConcurrency: 4,
    telemetryEnabled: false,
    reduceMotion: "system",
  };
}

function requirePreview() {
  if (bridgeMode !== "preview") {
    throw new Error("The local bridge is unavailable.");
  }
}

function cleanLoaderVersion(kind: LoaderKind, version?: string) {
  if (kind === "vanilla") return undefined;
  const value = version?.trim();
  return value ? value : undefined;
}

function toneForLoader(kind: LoaderKind): LauncherInstance["artworkTone"] {
  if (kind === "fabric") return "meadow";
  if (kind === "neoForge") return "workshop";
  return "vanilla";
}
