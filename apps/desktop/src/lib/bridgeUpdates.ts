import {
  check,
  type DownloadEvent,
  type Update,
} from "@tauri-apps/plugin-updater";
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

let pendingUpdate: Update | undefined;

export async function checkForLauncherUpdate(): Promise<
  LauncherUpdateCheck | undefined
> {
  if (bridgeMode !== "native") {
    throw new Error("Update checks are available only in the desktop app.");
  }
  if (pendingUpdate) {
    await pendingUpdate.close();
    pendingUpdate = undefined;
  }
  const update = await check({ timeout: 30_000 });
  if (!update) return undefined;
  pendingUpdate = update;
  return {
    currentVersion: update.currentVersion,
    version: update.version,
    date: update.date,
    notes: update.body,
  };
}

export async function installLauncherUpdate(
  onProgress: (progress: LauncherUpdateProgress) => void,
): Promise<void> {
  const update = pendingUpdate;
  if (!update) {
    throw new Error("Check for an update again before installing it.");
  }
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
  await update.downloadAndInstall(receive, {
    timeout: 10 * 60_000,
    restartAfterInstall: true,
  });
  pendingUpdate = undefined;
}
