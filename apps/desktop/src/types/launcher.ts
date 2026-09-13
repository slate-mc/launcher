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

export const instanceSummarySchema = z.object({
  id: z.string().uuid(),
  name: z.string().min(1),
  mode: instanceModeSchema,
  managementMode: z.enum(["local", "community"]),
  favorite: z.boolean(),
  revision: z.number().int().nonnegative(),
  minecraftVersion: z.string().min(1),
  loaderKind: loaderKindSchema,
  loaderVersion: z.string().min(1).optional(),
  memoryMb: z.number().int().min(1024).max(32768),
  setupState: setupStateSchema,
  createdAt: z.string(),
  updatedAt: z.string(),
  description: z.string().optional(),
  lastPlayed: z.string().optional(),
  modCount: z.number().int().nonnegative().optional(),
  artworkTone: z
    .enum(["meadow", "workshop", "vanilla", "nether"])
    .optional(),
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
    if (
      value.loaderKind !== "vanilla" &&
      !(value.loaderVersion?.trim().length)
    ) {
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
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const installJobListSchema = z.array(installJobSchema);

export const gameSessionSchema = z.object({
  id: z.string().uuid(),
  instanceId: z.string().uuid(),
  state: z.string(),
  mode: z.literal("demo"),
  pid: z.number().int().nonnegative(),
  logName: z.string(),
});

export type Bootstrap = z.infer<typeof bootstrapSchema>;
export type LauncherInstance = z.infer<typeof instanceSummarySchema>;
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
