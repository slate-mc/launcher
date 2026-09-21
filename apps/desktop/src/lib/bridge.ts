import { invoke } from "@tauri-apps/api/core";
import {
  bootstrapSchema,
  createInstanceSchema,
  installJobListSchema,
  installJobSchema,
  instanceArtworkAssetSchema,
  instanceGameOptionsSchema,
  instanceSnapshotListSchema,
  instanceSnapshotSchema,
  instanceSummaryListSchema,
  instanceSummarySchema,
  onboardingStateSchema,
  preferencesSchema,
  preflightSchema,
  supportReportExportSchema,
  supportReportPreviewSchema,
  defaultInstanceSettings,
  type AppPreferences,
  type Bootstrap,
  type CreateInstanceInput,
  type LauncherInstance,
  type LauncherUpdate,
  type LoaderKind,
  type OnboardingState,
  type InstallJob,
  type InstanceSettings,
  type InstanceArtworkKind,
  type InstanceGameOptions,
  type InstanceSnapshot,
  type Preflight,
  type SupportReportExport,
  type SupportReportPreview,
} from "../types/launcher";

import {
  bridgeMode,
  requireNativeContent,
  requirePreview,
} from "./bridgeRuntime";

export { bridgeMode } from "./bridgeRuntime";
export {
  cancelMinecraftAuth,
  getMinecraftAuthStatus,
  listAccounts,
  refreshMinecraftAccount,
  removeMinecraftAccount,
  setDefaultMinecraftAccount,
  startMinecraftAuth,
} from "./bridgeAccounts";
export {
  getLoaderVersionCatalog,
  getMinecraftVersionCatalog,
} from "./bridgeCatalog";
export {
  clearStorageCategory,
  deleteTrashedInstance,
  emptyInstanceTrash,
  getStorageOverview,
  restoreTrashedInstance,
} from "./bridgeStorage";
export {
  forceStopGameSession,
  launchInstance,
  listGameSessions,
  listInstanceSessions,
  readSessionLog,
  subscribeSessionLog,
} from "./bridgeSessions";
export {
  createSavedServer,
  listSavedServers,
  pingServer,
  removeSavedServer,
  updateSavedServer,
} from "./bridgeServers";
export {
  checkForLauncherUpdate,
  installLauncherUpdate,
  type LauncherUpdateCheck,
  type LauncherUpdateProgress,
} from "./bridgeUpdates";
export * from "./bridgeContent";

const previewStorageKey = "slate.preview.instances.v2";
const previewPreferencesKey = "slate.preview.preferences.v1";
const previewNow = "2026-09-13T12:00:00Z";

const initialPreviewInstances: LauncherInstance[] = [
  {
    id: "98d7fe64-acb5-454c-a7e5-7127a3af0c12",
    name: "Survival",
    storagePath: "C:\\slate-preview\\instances\\survival",
    mode: "modded",
    managementMode: "local",
    favorite: true,
    revision: 3,
    minecraftVersion: "1.21.1",
    loaderKind: "fabric",
    loaderVersion: "0.16.10",
    memoryMb: 6144,
    settings: {
      ...defaultInstanceSettings,
      description: "My main survival world",
      effectiveMemoryMb: 6144,
    },
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
    storagePath: "C:\\slate-preview\\instances\\create-workshop",
    mode: "modded",
    managementMode: "local",
    favorite: true,
    revision: 8,
    minecraftVersion: "1.20.1",
    loaderKind: "neoForge",
    loaderVersion: "47.1.106",
    memoryMb: 8192,
    settings: {
      ...defaultInstanceSettings,
      description: "Automation and building",
      effectiveMemoryMb: 8192,
    },
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
    storagePath: "C:\\slate-preview\\instances\\vanilla",
    mode: "vanilla",
    managementMode: "local",
    favorite: false,
    revision: 1,
    minecraftVersion: "1.21.1",
    loaderKind: "vanilla",
    memoryMb: 4096,
    settings: {
      ...defaultInstanceSettings,
      description: "A clean vanilla experience",
    },
    setupState: "configured",
    createdAt: previewNow,
    updatedAt: previewNow,
    modCount: 0,
    lastPlayed: "5 days ago",
    description: "A clean vanilla experience",
    artworkTone: "vanilla",
  },
  {
    id: "13fcbabf-cc0b-448c-bc47-a44da89c6a61",
    name: "PvP",
    storagePath: "C:\\slate-preview\\instances\\pvp",
    mode: "pvp",
    managementMode: "local",
    favorite: false,
    revision: 2,
    minecraftVersion: "1.20.4",
    loaderKind: "fabric",
    loaderVersion: "0.15.11",
    memoryMb: 4096,
    settings: {
      ...defaultInstanceSettings,
      description: "Practice and minigames",
    },
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
const previewInstallJobs: InstallJob[] = [];

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
    databaseSchemaVersion: 17,
    capabilities: [
      { id: "instance.library", available: true },
      { id: "instance.create", available: true },
      { id: "instance.configure", available: true },
      { id: "content.modpacks", available: false },
      { id: "settings.local", available: true },
      {
        id: "minecraft.install",
        available: false,
        unavailableReason:
          "Download and installation jobs are not implemented yet.",
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

export async function getOnboardingState(): Promise<OnboardingState> {
  if (bridgeMode === "native") {
    return onboardingStateSchema.parse(await invoke("onboarding_get"));
  }
  return { completed: true, customStorageSelected: false };
}

export async function selectOnboardingStorage(): Promise<OnboardingState> {
  if (bridgeMode === "native") {
    return onboardingStateSchema.parse(
      await invoke("onboarding_select_storage"),
    );
  }
  requirePreview();
  return { completed: true, customStorageSelected: false };
}

export async function completeOnboarding(): Promise<OnboardingState> {
  if (bridgeMode === "native") {
    return onboardingStateSchema.parse(await invoke("onboarding_complete"));
  }
  requirePreview();
  return { completed: true, customStorageSelected: false };
}

export async function getSupportReportPreview(): Promise<SupportReportPreview> {
  if (bridgeMode === "native") {
    return supportReportPreviewSchema.parse(
      await invoke("support_report_preview_get"),
    );
  }
  return { diagnosticFileCount: 0, diagnosticBytes: 0 };
}

export async function exportSupportReport(input: {
  includeLauncherLogs: boolean;
  includeInstallActivity: boolean;
  includeInstanceSummary: boolean;
}): Promise<SupportReportExport | undefined> {
  requireNativeContent();
  const result = await invoke("support_report_export", { request: input });
  if (result === null) return undefined;
  return supportReportExportSchema.parse(result);
}

export async function listInstances(): Promise<LauncherInstance[]> {
  if (bridgeMode === "native") {
    return instanceSummaryListSchema.parse(await invoke("instances_list"));
  }
  return bridgeMode === "preview" ? structuredClone(previewInstances) : [];
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
  const id = crypto.randomUUID();
  const instance = instanceSummarySchema.parse({
    ...request,
    loaderVersion: cleanLoaderVersion(
      request.loaderKind,
      request.loaderVersion,
    ),
    id,
    storagePath: `C:\\slate-preview\\instances\\${id}`,
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
  return updatePreviewInstance(
    input.id,
    input.expectedRevision,
    (instance) => ({
      ...instance,
      name: input.name.trim(),
    }),
  );
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
  return updatePreviewInstance(input.id, input.expectedRevision, (instance) => {
    const minecraftVersion = input.minecraftVersion.trim();
    const loaderVersion = cleanLoaderVersion(
      input.loaderKind,
      input.loaderVersion,
    );
    const runtimeChanged =
      instance.minecraftVersion !== minecraftVersion ||
      instance.loaderKind !== input.loaderKind ||
      instance.loaderVersion !== loaderVersion;
    return {
      ...instance,
      minecraftVersion,
      loaderKind: input.loaderKind,
      loaderVersion,
      memoryMb: input.memoryMb,
      settings: {
        ...instance.settings,
        effectiveMemoryMb: input.memoryMb,
      },
      setupState: runtimeChanged ? "configured" : instance.setupState,
      artworkTone: toneForLoader(input.loaderKind),
    };
  });
}

export type EditableInstanceSettings = Omit<
  InstanceSettings,
  "hasCustomIcon" | "hasCustomBanner" | "effectiveMemoryMb" | "customJavaLabel"
>;

export async function updateInstanceSettings(
  input: EditableInstanceSettings & {
    id: string;
    maximumMemoryMb: number;
    expectedRevision: number;
  },
): Promise<LauncherInstance> {
  if (bridgeMode === "native") {
    return instanceSummarySchema.parse(
      await invoke("instance_update_settings", { request: input }),
    );
  }
  const { id, maximumMemoryMb, expectedRevision, ...settings } = input;
  return updatePreviewInstance(id, expectedRevision, (instance) => ({
    ...instance,
    memoryMb: maximumMemoryMb,
    settings: {
      ...instance.settings,
      ...settings,
      effectiveMemoryMb: maximumMemoryMb,
    },
  }));
}

export async function selectInstanceJava(input: {
  id: string;
  mode: "managed" | "detected" | "custom";
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Java selection is available only in the slate desktop app.",
    );
  }
  return instanceSummarySchema.parse(
    await invoke("instance_select_java", { request: input }),
  );
}

export async function selectInstanceArtwork(input: {
  id: string;
  kind: InstanceArtworkKind;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Artwork selection is available only in the slate desktop app.",
    );
  }
  return instanceSummarySchema.parse(
    await invoke("instance_select_artwork", { request: input }),
  );
}

export async function resetInstanceArtwork(input: {
  id: string;
  kind: InstanceArtworkKind;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Artwork selection is available only in the slate desktop app.",
    );
  }
  return instanceSummarySchema.parse(
    await invoke("instance_reset_artwork", { request: input }),
  );
}

export async function getInstanceArtwork(
  id: string,
  kind: InstanceArtworkKind,
): Promise<string | undefined> {
  if (bridgeMode !== "native") return undefined;
  const value = await invoke("instance_get_artwork", {
    request: { id, kind },
  });
  if (value === null || value === undefined) return undefined;
  const asset = instanceArtworkAssetSchema.parse(value);
  return `data:${asset.mimeType};base64,${asset.dataBase64}`;
}

export async function getInstanceGameOptions(
  id: string,
): Promise<InstanceGameOptions> {
  if (bridgeMode !== "native") return { fileExists: false, values: {} };
  return instanceGameOptionsSchema.parse(
    await invoke("instance_game_options_get", { request: { id } }),
  );
}

export async function updateInstanceGameOptions(input: {
  id: string;
  values: Record<string, string>;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Game options are available only in the slate desktop app.",
    );
  }
  return instanceSummarySchema.parse(
    await invoke("instance_game_options_update", { request: input }),
  );
}

export async function openInstanceDirectory(
  id: string,
  kind: "game" | "mods" | "logs" | "screenshots" | "saves" | "crashReports",
): Promise<void> {
  if (bridgeMode !== "native") return;
  await invoke("instance_open_directory", { request: { id, kind } });
}

export async function duplicateInstance(input: {
  id: string;
  name: string;
  includeWorlds: boolean;
  includeScreenshots: boolean;
  includeSettings: boolean;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode !== "native") {
    throw new Error("Duplication is available only in the slate desktop app.");
  }
  return instanceSummarySchema.parse(
    await invoke("instance_duplicate", { request: input }),
  );
}

export async function moveInstanceStorage(input: {
  id: string;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Storage moves are available only in the slate desktop app.",
    );
  }
  return instanceSummarySchema.parse(
    await invoke("instance_move_storage", { request: input }),
  );
}

export async function exportInstance(input: {
  id: string;
  expectedRevision: number;
}): Promise<void> {
  if (bridgeMode !== "native") {
    throw new Error("Export is available only in the slate desktop app.");
  }
  await invoke("instance_export", { request: input });
}

export async function importInstance(): Promise<LauncherInstance | undefined> {
  if (bridgeMode !== "native") {
    throw new Error("Import is available only in the slate desktop app.");
  }
  const response = await invoke<unknown>("instance_import", { request: {} });
  return response === null ? undefined : instanceSummarySchema.parse(response);
}

export async function importLauncherInstance(): Promise<
  LauncherInstance | undefined
> {
  if (bridgeMode !== "native") {
    throw new Error(
      "Launcher imports are available only in the slate desktop app.",
    );
  }
  const response = await invoke<unknown>("instance_import_from_launcher", {
    request: {},
  });
  return response === null ? undefined : instanceSummarySchema.parse(response);
}

export async function listInstanceSnapshots(
  id: string,
): Promise<InstanceSnapshot[]> {
  if (bridgeMode !== "native") return [];
  return instanceSnapshotListSchema.parse(
    await invoke("instance_snapshots_list", { request: { id } }),
  );
}

export async function createInstanceSnapshot(input: {
  id: string;
  expectedRevision: number;
}): Promise<InstanceSnapshot> {
  if (bridgeMode !== "native") {
    throw new Error("Snapshots are available only in the slate desktop app.");
  }
  return instanceSnapshotSchema.parse(
    await invoke("instance_snapshot_create", { request: input }),
  );
}

export async function restoreInstanceSnapshot(input: {
  id: string;
  snapshotId: string;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode !== "native") {
    throw new Error("Snapshots are available only in the slate desktop app.");
  }
  return instanceSummarySchema.parse(
    await invoke("instance_snapshot_restore", { request: input }),
  );
}

export async function deleteInstanceSnapshot(input: {
  id: string;
  snapshotId: string;
}): Promise<void> {
  if (bridgeMode !== "native") return;
  await invoke("instance_snapshot_delete", { request: input });
}

export async function setInstanceSnapshotPinned(input: {
  id: string;
  snapshotId: string;
  pinned: boolean;
}): Promise<InstanceSnapshot> {
  if (bridgeMode !== "native") {
    throw new Error("Snapshots are available only in the slate desktop app.");
  }
  return instanceSnapshotSchema.parse(
    await invoke("instance_snapshot_set_pinned", { request: input }),
  );
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
  return updatePreviewInstance(
    input.id,
    input.expectedRevision,
    (instance) => ({
      ...instance,
      favorite: input.favorite,
    }),
  );
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
  const instance = previewInstances.find(
    (candidate) => candidate.id === input.id,
  );
  if (!instance) throw new Error("That instance no longer exists.");
  if (instance.revision !== input.expectedRevision) {
    throw new Error("That instance changed. Reload and try again.");
  }
  previewInstances = previewInstances.filter(
    (candidate) => candidate.id !== input.id,
  );
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
  throw new Error("Installation is available only in the slate desktop app.");
}

export async function listInstallJobs(): Promise<InstallJob[]> {
  if (bridgeMode === "native") {
    return installJobListSchema.parse(await invoke("install_jobs_list"));
  }
  return structuredClone(previewInstallJobs);
}

export async function cancelInstallJob(input: {
  jobId: string;
  revisionId: string;
}): Promise<InstallJob> {
  requireNativeContent();
  return installJobSchema.parse(
    await invoke("install_job_cancel", { request: input }),
  );
}

export async function setInstallJobPaused(input: {
  jobId: string;
  revisionId: string;
  paused: boolean;
}): Promise<InstallJob> {
  requireNativeContent();
  return installJobSchema.parse(
    await invoke("install_job_set_paused", { request: input }),
  );
}

export async function moveInstallJob(input: {
  jobId: string;
  direction: "up" | "down";
}): Promise<InstallJob> {
  requireNativeContent();
  return installJobSchema.parse(
    await invoke("install_job_move", { request: input }),
  );
}

export async function retryInstallJob(jobId: string): Promise<InstallJob> {
  requireNativeContent();
  return installJobSchema.parse(
    await invoke("install_job_retry", { request: { jobId } }),
  );
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
    downloadBandwidthLimitMib: 0,
    telemetryEnabled: false,
    reduceMotion: "system",
    trashRetentionDays: 30,
  };
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
