import { invoke } from "@tauri-apps/api/core";
import {
  savedServerListSchema,
  savedServerSchema,
  serverStatusSchema,
  type SavedServer,
  type ServerStatus,
} from "../types/launcher";
import { bridgeMode, requirePreview } from "./bridgeRuntime";

const previewStorageKey = "slate.preview.servers.v1";

const initialPreviewServers: SavedServer[] = [
  {
    id: "e4ea515a-32b5-4be9-9947-4c5db4a0c0fe",
    name: "Blockhaven SMP",
    address: "play.blockhaven.gg",
    createdAt: "2026-09-13T12:00:00Z",
    updatedAt: "2026-09-13T12:00:00Z",
  },
  {
    id: "70481ddc-6107-4b19-8d6d-68768b99264f",
    name: "PixelRealms",
    address: "mc.pixelrealms.net",
    createdAt: "2026-09-13T12:00:00Z",
    updatedAt: "2026-09-13T12:00:00Z",
  },
];

let previewServers = loadPreviewServers();

export async function listSavedServers(): Promise<SavedServer[]> {
  if (bridgeMode === "native") {
    return savedServerListSchema.parse(await invoke("servers_list"));
  }
  requirePreview();
  return structuredClone(previewServers);
}

export async function createSavedServer(input: {
  name: string;
  address: string;
  preferredInstanceId?: string;
}): Promise<SavedServer> {
  if (bridgeMode === "native") {
    return savedServerSchema.parse(
      await invoke("server_create", { request: input }),
    );
  }
  requirePreview();
  const now = new Date().toISOString();
  const server = savedServerSchema.parse({
    ...input,
    id: crypto.randomUUID(),
    createdAt: now,
    updatedAt: now,
  });
  previewServers = [...previewServers, server];
  savePreviewServers();
  return structuredClone(server);
}

export async function updateSavedServer(input: {
  id: string;
  name: string;
  address: string;
  preferredInstanceId?: string;
}): Promise<SavedServer> {
  if (bridgeMode === "native") {
    return savedServerSchema.parse(
      await invoke("server_update", { request: input }),
    );
  }
  requirePreview();
  const current = previewServers.find((server) => server.id === input.id);
  if (!current) throw new Error("That server no longer exists.");
  const updated = savedServerSchema.parse({
    ...current,
    ...input,
    updatedAt: new Date().toISOString(),
  });
  previewServers = previewServers.map((server) =>
    server.id === updated.id ? updated : server,
  );
  savePreviewServers();
  return structuredClone(updated);
}

export async function removeSavedServer(id: string): Promise<void> {
  if (bridgeMode === "native") {
    await invoke("server_remove", { request: { id } });
    return;
  }
  requirePreview();
  previewServers = previewServers.filter((server) => server.id !== id);
  savePreviewServers();
}

export async function pingServer(address: string): Promise<ServerStatus> {
  if (bridgeMode === "native") {
    return serverStatusSchema.parse(
      await invoke("server_ping", { request: { address } }),
    );
  }
  requirePreview();
  return serverStatusSchema.parse({
    address,
    online: true,
    latencyMs: 42,
    versionName: "1.21.1",
    protocol: 767,
    playersOnline: 42,
    playersMax: 100,
    description: "A saved Minecraft server",
    descriptionSegments: [
      {
        text: "A saved ",
        color: "#55FF55",
        bold: false,
        italic: false,
        underlined: false,
        strikethrough: false,
        obfuscated: false,
      },
      {
        text: "Minecraft server",
        color: "#FFAA00",
        bold: true,
        italic: false,
        underlined: false,
        strikethrough: false,
        obfuscated: false,
      },
    ],
  });
}

function loadPreviewServers(): SavedServer[] {
  try {
    const stored = window.localStorage.getItem(previewStorageKey);
    return stored
      ? savedServerListSchema.parse(JSON.parse(stored))
      : structuredClone(initialPreviewServers);
  } catch {
    return structuredClone(initialPreviewServers);
  }
}

function savePreviewServers() {
  window.localStorage.setItem(
    previewStorageKey,
    JSON.stringify(previewServers),
  );
}
