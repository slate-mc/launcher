import { invoke } from "@tauri-apps/api/core";
import {
  instanceSummarySchema,
  storageCleanupResultSchema,
  storageOverviewSchema,
  type LauncherInstance,
  type StorageCategory,
  type StorageCleanupResult,
  type StorageOverview,
} from "../types/launcher";
import { bridgeMode, requirePreview } from "./bridgeRuntime";

export async function getStorageOverview(): Promise<StorageOverview> {
  if (bridgeMode === "native") {
    return storageOverviewSchema.parse(await invoke("storage_overview"));
  }
  return storageOverviewSchema.parse({
    categories: [
      { category: "instances", sizeBytes: 8_430_000_000, fileCount: 14_280 },
      { category: "sharedGameFiles", sizeBytes: 1_840_000_000, fileCount: 8_420 },
      { category: "managedJava", sizeBytes: 612_000_000, fileCount: 1_850 },
      { category: "logs", sizeBytes: 48_000_000, fileCount: 84 },
      { category: "temporaryFiles", sizeBytes: 132_000_000, fileCount: 19 },
      { category: "removedContent", sizeBytes: 286_000_000, fileCount: 42 },
    ],
    totalSizeBytes: 11_348_000_000,
    reclaimableSizeBytes: 466_000_000,
    trashedInstances: [],
  });
}

export async function clearStorageCategory(input: {
  category: StorageCategory;
  confirmManagedData?: boolean;
}): Promise<StorageCleanupResult> {
  if (bridgeMode === "native") {
    return storageCleanupResultSchema.parse(
      await invoke("storage_clear_category", { request: input }),
    );
  }
  requirePreview();
  return { reclaimedBytes: 0, removedFiles: 0 };
}

export async function restoreTrashedInstance(input: {
  id: string;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  if (bridgeMode === "native") {
    return instanceSummarySchema.parse(
      await invoke("trashed_instance_restore", { request: input }),
    );
  }
  throw new Error("Instance recovery is available only in the slate desktop app.");
}

export async function deleteTrashedInstance(input: {
  id: string;
  expectedRevision: number;
  confirmationName: string;
}): Promise<StorageCleanupResult> {
  if (bridgeMode === "native") {
    return storageCleanupResultSchema.parse(
      await invoke("trashed_instance_delete", { request: input }),
    );
  }
  throw new Error("Permanent deletion is available only in the slate desktop app.");
}

export async function emptyInstanceTrash(expectedCount: number): Promise<StorageCleanupResult> {
  if (bridgeMode === "native") {
    return storageCleanupResultSchema.parse(
      await invoke("trashed_instances_empty", {
        request: { expectedCount },
      }),
    );
  }
  throw new Error("Permanent deletion is available only in the slate desktop app.");
}

