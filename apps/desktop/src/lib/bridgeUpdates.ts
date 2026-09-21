import {
  check,
  type DownloadEvent,
  type Update,
} from "@tauri-apps/plugin-updater";
import { getVersion } from "@tauri-apps/api/app";
import { bridgeMode } from "./bridgeRuntime";

export type LauncherUpdateCheck = {
  currentVersion: string;
  version: string;
  date?: string;
  notes?: string;
};

export type LauncherUpdateProgress = {
  downloadedBytes: number;
  totalBytes?: number;
  finished: boolean;
};

export type LauncherUpdateRecord = {
  fromVersion: string;
  toVersion: string;
  startedAt: string;
  state: "installing" | "installed" | "interrupted";
};

let pendingUpdate: Update | undefined;
const updateCohortKey = "slate.update.cohort.v1";
const updateHistoryKey = "slate.update.history.v1";

export async function checkForLauncherUpdate(
  channel: "stable" | "beta" = "stable",
): Promise<LauncherUpdateCheck | undefined> {
  if (bridgeMode !== "native") {
    throw new Error("Update checks are available only in the desktop app.");
  }
  if (pendingUpdate) {
    await pendingUpdate.close();
    pendingUpdate = undefined;
  }
  const update = await check({
    timeout: 30_000,
    allowDowngrades: false,
    headers: {
      "x-slate-update-channel": channel,
      "x-slate-update-cohort": updateCohortId(),
    },
  });
  if (!update) return undefined;
  pendingUpdate = update;
  return {
    currentVersion: update.currentVersion,
    version: update.version,
    date: update.date,
    notes: update.body,
  };
}

function updateCohortId() {
  const stored = localStorage.getItem(updateCohortKey);
  if (
    stored &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
      stored,
    )
  ) {
    return stored;
  }
  const cohort = crypto.randomUUID();
  localStorage.setItem(updateCohortKey, cohort);
  return cohort;
}

export async function installLauncherUpdate(
  onProgress: (progress: LauncherUpdateProgress) => void,
): Promise<void> {
  const update = pendingUpdate;
  if (!update) {
    throw new Error("Check for an update again before installing it.");
  }
  const attempt: LauncherUpdateRecord = {
    fromVersion: update.currentVersion,
    toVersion: update.version,
    startedAt: new Date().toISOString(),
    state: "installing",
  };
  saveUpdateRecord(attempt);
  let downloadedBytes = 0;
  let totalBytes: number | undefined;
  const receive = (event: DownloadEvent) => {
    if (event.event === "Started") {
      totalBytes = event.data.contentLength;
    } else if (event.event === "Progress") {
      downloadedBytes += event.data.chunkLength;
    }
    onProgress({
      downloadedBytes,
      totalBytes,
      finished: event.event === "Finished",
    });
  };
  try {
    await update.downloadAndInstall(receive, {
      timeout: 10 * 60_000,
      restartAfterInstall: true,
    });
    saveUpdateRecord({ ...attempt, state: "installed" });
    pendingUpdate = undefined;
  } catch (error) {
    saveUpdateRecord({ ...attempt, state: "interrupted" });
    throw error;
  }
}

export async function getLauncherUpdateHistory(): Promise<LauncherUpdateRecord[]> {
  const currentVersion = await getVersion();
  const history = readUpdateHistory().map((record): LauncherUpdateRecord =>
    record.state === "installing"
      ? {
          ...record,
          state: currentVersion === record.toVersion ? "installed" : "interrupted",
        }
      : record,
  );
  localStorage.setItem(updateHistoryKey, JSON.stringify(history));
  return history;
}

function saveUpdateRecord(record: LauncherUpdateRecord) {
  const history = readUpdateHistory().filter(
    (entry) => entry.startedAt !== record.startedAt,
  );
  localStorage.setItem(
    updateHistoryKey,
    JSON.stringify([record, ...history].slice(0, 5)),
  );
}

function readUpdateHistory(): LauncherUpdateRecord[] {
  try {
    const value = JSON.parse(localStorage.getItem(updateHistoryKey) ?? "[]");
    if (!Array.isArray(value)) return [];
    return value.filter(isUpdateRecord).slice(0, 5);
  } catch {
    return [];
  }
}

function isUpdateRecord(value: unknown): value is LauncherUpdateRecord {
  if (!value || typeof value !== "object") return false;
  const record = value as Partial<LauncherUpdateRecord>;
  return (
    typeof record.fromVersion === "string" &&
    typeof record.toVersion === "string" &&
    typeof record.startedAt === "string" &&
    ["installing", "installed", "interrupted"].includes(record.state ?? "")
  );
}
