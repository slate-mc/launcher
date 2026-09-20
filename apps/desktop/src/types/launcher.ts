import { z } from "zod";

export const capabilitySchema = z.object({
  id: z.string(),
  available: z.boolean(),
  unavailableReason: z.string().optional(),
});

export const bootstrapSchema = z.object({
  productName: z.literal("slate"),
  ipcSchemaVersion: z.number().int().nonnegative(),
  databaseSchemaVersion: z.number().int().nonnegative(),
  capabilities: z.array(capabilitySchema),
});

export const loaderKindSchema = z.enum(["vanilla", "fabric", "neoForge"]);
export const instanceModeSchema = z.enum(["vanilla", "modded", "pvp"]);
export const setupStateSchema = z.enum([
  "configured",
  "preparing",
  "ready",
  "blocked",
]);

export const instanceWindowModeSchema = z.enum([
  "windowed",
  "maximized",
  "fullscreen",
]);
export const launcherBehaviorSchema = z.enum([
  "keepOpen",
  "minimize",
  "hide",
]);
export const processPrioritySchema = z.enum([
  "low",
  "belowNormal",
  "normal",
  "aboveNormal",
  "high",
]);
export const memoryModeSchema = z.enum(["auto", "custom"]);
export const javaSelectionModeSchema = z.enum([
  "managed",
  "detected",
  "custom",
]);
export const performancePresetSchema = z.enum([
  "balanced",
  "throughput",
  "lowLatency",
  "custom",
]);

export const defaultInstanceSettings = {
  description: "",
  notes: "",
  tags: [] as string[],
  hasCustomIcon: false,
  hasCustomBanner: false,
  bannerPositionX: 50,
  bannerPositionY: 50,
  windowMode: "windowed" as const,
  launcherBehavior: "keepOpen" as const,
  gameLanguage: "en_us",
  processPriority: "normal" as const,
  memoryMode: "auto" as const,
  initialMemoryMb: 512,
  effectiveMemoryMb: 4096,
  javaMode: "managed" as const,
  performancePreset: "balanced" as const,
  jvmArguments: [] as string[],
  environment: {} as Record<string, string>,
  backupBeforeChanges: true,
  backupRetention: 5,
  logRetentionDays: 30,
};

export const instanceSettingsSchema = z.object({
  description: z.string(),
  notes: z.string(),
  groupName: z.string().optional(),
  tags: z.array(z.string()),
  hasCustomIcon: z.boolean(),
  hasCustomBanner: z.boolean(),
  bannerPositionX: z.number().int().min(0).max(100),
  bannerPositionY: z.number().int().min(0).max(100),
  preferredAccountId: z.string().uuid().optional(),
  windowMode: instanceWindowModeSchema,
  resolutionWidth: z.number().int().min(320).max(16384).optional(),
  resolutionHeight: z.number().int().min(240).max(16384).optional(),
  launcherBehavior: launcherBehaviorSchema,
  gameLanguage: z.string().min(2).max(32),
  quickPlayServer: z.string().optional(),
  processPriority: processPrioritySchema,
  memoryMode: memoryModeSchema,
  initialMemoryMb: z.number().int().min(256).max(32768),
  effectiveMemoryMb: z.number().int().min(1024).max(32768),
  javaMode: javaSelectionModeSchema,
  customJavaLabel: z.string().optional(),
  performancePreset: performancePresetSchema,
  jvmArguments: z.array(z.string()),
  environment: z.record(z.string(), z.string()),
  backupBeforeChanges: z.boolean(),
  backupRetention: z.number().int().min(1).max(50),
  logRetentionDays: z.number().int().min(1).max(365),
});

export const instanceArtworkAssetSchema = z.object({
  mimeType: z.enum(["image/png", "image/jpeg", "image/webp"]),
  dataBase64: z.string(),
});

export const instanceGameOptionsSchema = z.object({
  fileExists: z.boolean(),
  values: z.record(z.string(), z.string()),
});

export const instanceSnapshotSchema = z.object({
  id: z.string().uuid(),
  sizeBytes: z.number().int().nonnegative(),
  pinned: z.boolean(),
  createdAt: z.string(),
});

export const instanceSnapshotListSchema = z.array(instanceSnapshotSchema);

export const providerSchema = z.enum(["curseforge", "modrinth", "ftb"]);
export const contentLoaderSchema = z.enum([
  "vanilla",
  "forge",
  "neoforge",
  "fabric",
  "quilt",
]);
export const releaseTypeSchema = z.enum([
  "release",
  "beta",
  "alpha",
  "unknown",
]);

export const modpackSourceSchema = z.object({
  provider: providerSchema,
  projectId: z.string().min(1),
  versionId: z.string().min(1),
  displayName: z.string().min(1),
  iconUrl: z.string().url().optional(),
  bannerUrl: z.string().url().optional(),
});

export const instanceSummarySchema = z.object({
  id: z.string().uuid(),
  name: z.string().min(1),
  storagePath: z.string().min(1),
  mode: instanceModeSchema,
  managementMode: z.enum(["local", "community"]),
  favorite: z.boolean(),
  revision: z.number().int().nonnegative(),
  minecraftVersion: z.string().min(1),
  loaderKind: loaderKindSchema,
  loaderVersion: z.string().min(1).optional(),
  memoryMb: z.number().int().min(1024).max(32768),
  modCount: z.number().int().nonnegative().default(0),
  setupState: setupStateSchema,
  settings: instanceSettingsSchema.default(defaultInstanceSettings),
  modpackSource: modpackSourceSchema.optional(),
  createdAt: z.string(),
  updatedAt: z.string(),
  description: z.string().optional(),
  lastPlayed: z.string().optional(),
  artworkTone: z.enum(["meadow", "workshop", "vanilla", "nether"]).optional(),
});

export const instanceSummaryListSchema = z.array(instanceSummarySchema);

export const createInstanceSchema = z
  .object({
    name: z.string().trim().min(1, "Enter a name.").max(80),
    mode: instanceModeSchema,
    minecraftVersion: z
      .string()
      .trim()
      .min(1, "Enter a Minecraft version.")
      .max(64)
      .regex(
        /^[A-Za-z0-9._+-]+$/,
        "Use letters, numbers, dots, dashes, underscores, or plus signs.",
      ),
    loaderKind: loaderKindSchema,
    loaderVersion: z.string().trim().max(64).optional(),
    memoryMb: z.number().int().min(1024).max(32768),
  })
  .superRefine((value, context) => {
    if (value.mode === "vanilla" && value.loaderKind !== "vanilla") {
      context.addIssue({
        code: "custom",
        path: ["loaderKind"],
        message: "Vanilla instances cannot use a mod loader.",
      });
    }
    if (value.mode === "modded" && value.loaderKind === "vanilla") {
      context.addIssue({
        code: "custom",
        path: ["loaderKind"],
        message: "Modded instances require Fabric or NeoForge.",
      });
    }
    if (value.loaderKind !== "vanilla" && !value.loaderVersion?.trim().length) {
      context.addIssue({
        code: "custom",
        path: ["loaderVersion"],
        message: "Enter the exact loader version.",
      });
    }
    if (value.loaderKind === "vanilla" && value.loaderVersion?.trim()) {
      context.addIssue({
        code: "custom",
        path: ["loaderVersion"],
        message: "Vanilla does not use a loader version.",
      });
    }
  });

export const preferencesSchema = z.object({
  theme: z.enum(["dark", "light", "system"]),
  downloadConcurrency: z.number().int().min(1).max(8),
  telemetryEnabled: z.boolean(),
  reduceMotion: z.enum(["system", "on", "off"]),
});

export const preflightSchema = z.object({
  databaseReady: z.boolean(),
  storageReady: z.boolean(),
  accountConfigured: z.boolean(),
  java: z.object({
    available: z.boolean(),
    version: z.string().optional(),
    unavailableReason: z.string().optional(),
  }),
  launchImplemented: z.boolean(),
});

export const minecraftVersionCatalogSchema = z.object({
  latestRelease: z.string().min(1),
  versions: z.array(
    z.object({
      id: z.string().min(1),
      kind: z.enum(["release", "snapshot"]),
    }),
  ),
});

export const loaderVersionCatalogSchema = z.object({
  loaderKind: loaderKindSchema,
  minecraftVersion: z.string().min(1),
  recommendedVersion: z.string().min(1).optional(),
  versions: z.array(z.string().min(1)),
  unavailableReason: z.string().optional(),
});

export const installJobSchema = z.object({
  id: z.string().uuid(),
  instanceId: z.string().uuid(),
  revisionId: z.string().uuid(),
  state: z.enum(["queued", "running", "succeeded", "failed", "cancelled"]),
  phase: z.string(),
  message: z.string(),
  completedItems: z.number().int().nonnegative().optional(),
  totalItems: z.number().int().nonnegative().optional(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const installJobListSchema = z.array(installJobSchema);

export const gameSessionSchema = z.object({
  id: z.string().uuid(),
  instanceId: z.string().uuid(),
  state: z.enum(["running", "stopping"]),
  mode: z.literal("authenticated"),
  pid: z.number().int().nonnegative(),
  logName: z.string(),
});

export const gameSessionListSchema = z.array(gameSessionSchema);

export const sessionLogSubscriptionSchema = z.object({
  id: z.string().uuid(),
  sessionId: z.string().uuid(),
});

export const sessionLogEventSchema = z.object({
  subscriptionId: z.string().uuid(),
  sessionId: z.string().uuid(),
  kind: z.enum(["snapshot", "append", "reset", "closed", "error"]),
  offset: z.string().regex(/^\d+$/),
  truncated: z.boolean(),
  text: z.string(),
});

export const minecraftAccountSchema = z.object({
  id: z.string().uuid(),
  profileId: z.string().uuid(),
  displayName: z.string().min(1).max(16),
  skinUrl: z.string().url().optional(),
  status: z.enum(["ready", "reauthenticationRequired"]),
  isDefault: z.boolean(),
  lastValidatedAt: z.string().optional(),
});

export const minecraftAccountListSchema = z.array(minecraftAccountSchema);

export const authFlowStateSchema = z.enum([
  "waitingForBrowser",
  "verifying",
  "succeeded",
  "failed",
  "cancelled",
]);

export const authStartSchema = z.object({
  flowId: z.string().uuid(),
  expiresAt: z.string(),
});

export const authFlowStatusSchema = z.object({
  flowId: z.string().uuid(),
  state: authFlowStateSchema,
  account: minecraftAccountSchema.optional(),
  userMessage: z.string().optional(),
});

const authorSchema = z.object({
  name: z.string().min(1),
  url: z.string().url().nullable().optional(),
});

export const modpackSummarySchema = z.object({
  provider: providerSchema,
  id: z.string().min(1),
  slug: z.string(),
  name: z.string().min(1),
  summary: z.string(),
  authors: z.array(authorSchema),
  icon_url: z.string().url().nullable().optional(),
  downloads: z.number().int().nonnegative(),
  updated_at: z.string(),
  minecraft_versions: z.array(z.string()),
  loaders: z.array(contentLoaderSchema),
  categories: z.array(z.string()),
});

export const modpackSearchResultSchema = z.object({
  items: z.array(modpackSummarySchema),
  next_cursor: z.string().nullable().optional(),
  has_more: z.boolean(),
  provider_status: z.record(z.string(), z.enum(["ok", "unavailable"])),
});

export const modpackProjectSchema = modpackSummarySchema
  .omit({ minecraft_versions: true, loaders: true, authors: true })
  .extend({
    description: z.string(),
    authors: z.array(authorSchema),
    banner_url: z.string().url().nullable().optional(),
    minecraft_versions: z.array(z.string()),
    loaders: z.array(contentLoaderSchema),
    links: z.object({
      website: z.string().url().nullable().optional(),
      source: z.string().url().nullable().optional(),
      issues: z.string().url().nullable().optional(),
    }),
    latest_version: z
      .object({ id: z.string().min(1), name: z.string().min(1) })
      .nullable()
      .optional(),
  });

export const modpackLoaderSchema = z.object({
  type: contentLoaderSchema,
  version: z.string().nullable().optional(),
});

export const modpackVersionSummarySchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  release_type: releaseTypeSchema,
  minecraft_version: z.string().min(1),
  loader: modpackLoaderSchema,
  published_at: z.string(),
  changelog: z.string().nullable().optional(),
});

export const modpackVersionPageSchema = z.object({
  items: z.array(modpackVersionSummarySchema),
  next_cursor: z.string().nullable().optional(),
  has_more: z.boolean(),
});

const hashesSchema = z.object({
  sha512: z.string().optional(),
  sha256: z.string().optional(),
  sha1: z.string().optional(),
});

export const modpackVersionSchema = z.object({
  provider: providerSchema,
  project_id: z.string().min(1),
  id: z.string().min(1),
  name: z.string().min(1),
  release_type: releaseTypeSchema,
  minecraft: z.object({ version: z.string().min(1) }),
  loader: modpackLoaderSchema,
  memory: z.object({
    minimum_mb: z.number().int().nonnegative(),
    recommended_mb: z.number().int().nonnegative(),
  }),
  files: z.array(
    z.object({
      id: z.string().min(1),
      type: z.enum([
        "mod",
        "config",
        "resource_pack",
        "shader_pack",
        "data_pack",
        "library",
        "override",
        "archive",
        "other",
      ]),
      path: z.string().min(1),
      size: z.number().int().nonnegative(),
      hashes: hashesSchema,
      side: z.enum(["both", "client", "server"]),
      optional: z.boolean(),
      option: z
        .object({
          id: z.string().min(1),
          name: z.string().min(1),
          default: z.boolean(),
        })
        .optional(),
    }),
  ),
  total_download_size: z.number().int().nonnegative(),
  changelog: z.string().nullable().optional(),
  published_at: z.string(),
});

export const modpackProvidersSchema = z.object({
  providers: z.array(
    z.object({
      id: providerSchema,
      name: z.string().min(1),
      available: z.boolean(),
    }),
  ),
});

export const modpackInstallStartedSchema = z.object({
  instance: instanceSummarySchema,
  job: installJobSchema,
});

export const instanceModSchema = z.object({
  provider: z.enum(["curseforge", "modrinth"]).nullable(),
  projectId: z.string().min(1).nullable(),
  versionId: z.string().min(1).nullable(),
  displayName: z.string().min(1),
  filePath: z.string().min(1),
  enabled: z.boolean(),
  pinned: z.boolean(),
  installedAt: z.string().nullable(),
  origin: z.enum(["added", "modpack", "local"]),
  fileSize: z.number().int().nonnegative(),
  iconUrl: z.string().url().nullable(),
});

export const instanceModListSchema = z.array(instanceModSchema);

export const instanceModResolutionSchema = z.object({
  filePath: z.string().min(1),
  provider: z.enum(["curseforge", "modrinth"]),
  projectId: z.string().min(1),
  versionId: z.string().min(1).nullable(),
  displayName: z.string().min(1).nullable(),
  iconUrl: z.string().url().nullable(),
});

export const instanceModResolutionListSchema = z.array(
  instanceModResolutionSchema,
);

export type Bootstrap = z.infer<typeof bootstrapSchema>;
export type LauncherInstance = z.infer<typeof instanceSummarySchema>;
export type InstanceSettings = z.infer<typeof instanceSettingsSchema>;
export type InstanceArtworkKind = "icon" | "banner";
export type InstanceGameOptions = z.infer<typeof instanceGameOptionsSchema>;
export type InstanceSnapshot = z.infer<typeof instanceSnapshotSchema>;
export type CreateInstanceInput = z.infer<typeof createInstanceSchema>;
export type LoaderKind = z.infer<typeof loaderKindSchema>;
export type AppPreferences = z.infer<typeof preferencesSchema>;
export type Preflight = z.infer<typeof preflightSchema>;
export type MinecraftVersionCatalog = z.infer<
  typeof minecraftVersionCatalogSchema
>;
export type LoaderVersionCatalog = z.infer<typeof loaderVersionCatalogSchema>;
export type InstallJob = z.infer<typeof installJobSchema>;
export type GameSession = z.infer<typeof gameSessionSchema>;
export type SessionLogSubscription = z.infer<
  typeof sessionLogSubscriptionSchema
>;
export type SessionLogEvent = z.infer<typeof sessionLogEventSchema>;
export type MinecraftAccount = z.infer<typeof minecraftAccountSchema>;
export type AuthStart = z.infer<typeof authStartSchema>;
export type AuthFlowStatus = z.infer<typeof authFlowStatusSchema>;
export type Provider = z.infer<typeof providerSchema>;
export type ContentLoader = z.infer<typeof contentLoaderSchema>;
export type ModpackSummary = z.infer<typeof modpackSummarySchema>;
export type ModpackSearchResult = z.infer<typeof modpackSearchResultSchema>;
export type ModpackProject = z.infer<typeof modpackProjectSchema>;
export type ModpackVersionSummary = z.infer<typeof modpackVersionSummarySchema>;
export type ModpackVersion = z.infer<typeof modpackVersionSchema>;
export type ModpackProviders = z.infer<typeof modpackProvidersSchema>;
export type ModpackInstallStarted = z.infer<typeof modpackInstallStartedSchema>;
export type InstanceMod = z.infer<typeof instanceModSchema>;
export type InstanceModResolution = z.infer<typeof instanceModResolutionSchema>;

export type ServerPreview = {
  id: string;
  name: string;
  address: string;
  players: string;
  latencyBars: number;
};

export type LauncherUpdate = {
  id: string;
  title: string;
  description: string;
  date: string;
  kind: "game" | "loader";
};
