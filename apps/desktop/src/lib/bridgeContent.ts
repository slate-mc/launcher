import { invoke } from "@tauri-apps/api/core";
import {
  installJobSchema,
  instanceContentFileListSchema,
  instanceContentHistorySchema,
  instanceModHistorySchema,
  instanceModListSchema,
  instanceModReferenceListSchema,
  instanceModResolutionListSchema,
  instanceSummarySchema,
  instanceWorldListSchema,
  modpackInstallStartedSchema,
  modpackProjectSchema,
  modpackProvidersSchema,
  modpackSearchResultSchema,
  modpackUpdateSummarySchema,
  modpackVersionPageSchema,
  modpackVersionSchema,
  modVersionListSchema,
  type ContentLoader,
  type InstallJob,
  type InstanceContentFile,
  type InstanceContentKind,
  type InstanceMod,
  type InstanceModResolution,
  type LauncherInstance,
  type ModpackInstallStarted,
  type ModpackProject,
  type ModpackProviders,
  type ModpackSearchResult,
  type ModpackUpdateSummary,
  type ModpackVersion,
  type ModVersionOption,
  type Provider,
} from "../types/launcher";
import { bridgeMode, requireNativeContent } from "./bridgeRuntime";

export async function getModpackProviders(): Promise<ModpackProviders> {
  if (bridgeMode === "native") {
    return modpackProvidersSchema.parse(await invoke("modpack_providers"));
  }
  return {
    providers: [
      { id: "curseforge", name: "CurseForge", available: false },
      { id: "modrinth", name: "Modrinth", available: false },
      { id: "ftb", name: "FTB", available: false },
    ],
  };
}

export type ModpackSearchInput = {
  query?: string;
  provider?: Provider;
  minecraftVersion?: string;
  loader?: ContentLoader;
  category?: string;
  sort: "relevance" | "downloads" | "updated" | "newest";
  page?: number;
  limit?: number;
};

export async function searchModpacks(
  input: ModpackSearchInput,
): Promise<ModpackSearchResult> {
  if (bridgeMode !== "native") {
    return { items: [], has_more: false, provider_status: {} };
  }
  return modpackSearchResultSchema.parse(
    await invoke("modpacks_search", { request: input }),
  );
}

export async function getModpack(
  provider: Provider,
  projectId: string,
): Promise<ModpackProject> {
  requireNativeContent();
  return modpackProjectSchema.parse(
    await invoke("modpack_get", { request: { provider, projectId } }),
  );
}

export async function listModpackVersions(input: {
  provider: Provider;
  projectId: string;
  minecraftVersion?: string;
  loader?: ContentLoader;
  releaseType?: "release" | "beta" | "alpha" | "unknown";
  page?: number;
  limit?: number;
}) {
  requireNativeContent();
  return modpackVersionPageSchema.parse(
    await invoke("modpack_versions_list", { request: input }),
  );
}

export async function getModpackVersion(input: {
  provider: Provider;
  projectId: string;
  versionId: string;
}): Promise<ModpackVersion> {
  requireNativeContent();
  return modpackVersionSchema.parse(
    await invoke("modpack_version_get", { request: input }),
  );
}

export async function installModpack(input: {
  provider: Provider;
  projectId: string;
  versionId: string;
  instanceName: string;
  includeOptional: string[];
}): Promise<ModpackInstallStarted> {
  requireNativeContent();
  return modpackInstallStartedSchema.parse(
    await invoke("modpack_install", { request: input }),
  );
}

export async function checkModpackUpdate(
  instanceId: string,
): Promise<ModpackUpdateSummary> {
  requireNativeContent();
  return modpackUpdateSummarySchema.parse(
    await invoke("modpack_update_check", { request: { instanceId } }),
  );
}

export async function applyModpackUpdate(input: {
  instanceId: string;
  expectedRevision: number;
  targetVersionId: string;
}): Promise<InstallJob> {
  requireNativeContent();
  return installJobSchema.parse(
    await invoke("modpack_update_apply", { request: input }),
  );
}

export async function searchMods(input: {
  instanceId: string;
  query?: string;
  provider?: Exclude<Provider, "ftb">;
  sort: "relevance" | "downloads" | "updated" | "newest";
  page?: number;
  limit?: number;
}): Promise<ModpackSearchResult> {
  if (bridgeMode !== "native") {
    return { items: [], has_more: false, provider_status: {} };
  }
  return modpackSearchResultSchema.parse(
    await invoke("mods_search", { request: input }),
  );
}

export async function searchContent(input: {
  instanceId: string;
  kind: InstanceContentKind;
  query?: string;
  sort: "relevance" | "downloads" | "updated" | "newest";
  page?: number;
  limit?: number;
}): Promise<ModpackSearchResult> {
  if (bridgeMode !== "native") {
    return { items: [], has_more: false, provider_status: {} };
  }
  return modpackSearchResultSchema.parse(
    await invoke("content_search", { request: input }),
  );
}

export async function installContent(input: {
  instanceId: string;
  expectedRevision: number;
  kind: InstanceContentKind;
  worldName?: string;
  content: Array<{
    provider: Exclude<Provider, "ftb">;
    projectId: string;
    displayName: string;
    iconUrl?: string;
  }>;
}): Promise<InstallJob> {
  requireNativeContent();
  return installJobSchema.parse(
    await invoke("instance_content_install", { request: input }),
  );
}

export async function listInstanceMods(
  instanceId: string,
): Promise<InstanceMod[]> {
  if (bridgeMode !== "native") return [];
  return instanceModListSchema.parse(
    await invoke("instance_mods_list", { request: { instanceId } }),
  );
}

export async function listInstanceModVersions(input: {
  instanceId: string;
  provider: Exclude<Provider, "ftb">;
  projectId: string;
}): Promise<ModVersionOption[]> {
  requireNativeContent();
  return modVersionListSchema.parse(
    await invoke("instance_mod_versions", { request: input }),
  ).items;
}

export async function listInstanceModHistory(input: {
  instanceId: string;
  provider: Exclude<Provider, "ftb">;
  projectId: string;
}) {
  requireNativeContent();
  return instanceModHistorySchema.parse(
    await invoke("instance_mod_history", { request: input }),
  );
}

export async function resolveInstanceModRelationships(input: {
  instanceId: string;
  provider: Exclude<Provider, "ftb">;
  projectId: string;
  versionId: string;
  filePath: string;
}) {
  requireNativeContent();
  return instanceModReferenceListSchema.parse(
    await invoke("instance_mod_relationships_resolve", { request: input }),
  );
}

export async function updateInstanceMod(input: {
  instanceId: string;
  expectedRevision: number;
  provider: Exclude<Provider, "ftb">;
  projectId: string;
  filePath: string;
  displayName: string;
  targetVersionId: string;
}): Promise<InstallJob> {
  requireNativeContent();
  return installJobSchema.parse(
    await invoke("instance_mod_update", { request: input }),
  );
}

export async function importLocalMod(input: {
  instanceId: string;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_mod_import", { request: input }),
  );
}

export async function listInstanceContentFiles(
  instanceId: string,
  kind: InstanceContentKind,
): Promise<InstanceContentFile[]> {
  if (bridgeMode !== "native") return [];
  return instanceContentFileListSchema.parse(
    await invoke("instance_content_files_list", {
      request: { instanceId, kind },
    }),
  );
}

export async function setInstanceContentPinned(input: {
  instanceId: string;
  kind: InstanceContentKind;
  provider: Exclude<Provider, "ftb">;
  projectId: string;
  pinned: boolean;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_content_set_pinned", { request: input }),
  );
}

export async function listInstanceContentVersions(input: {
  instanceId: string;
  kind: InstanceContentKind;
  provider: Exclude<Provider, "ftb">;
  projectId: string;
}): Promise<ModVersionOption[]> {
  requireNativeContent();
  return modVersionListSchema.parse(
    await invoke("instance_content_versions", { request: input }),
  ).items;
}

export async function listInstanceContentHistory(input: {
  instanceId: string;
  kind: InstanceContentKind;
  provider: Exclude<Provider, "ftb">;
  projectId: string;
}) {
  requireNativeContent();
  return instanceContentHistorySchema.parse(
    await invoke("instance_content_history", { request: input }),
  );
}

export async function updateInstanceContent(input: {
  instanceId: string;
  expectedRevision: number;
  kind: InstanceContentKind;
  provider: Exclude<Provider, "ftb">;
  projectId: string;
  filePath: string;
  displayName: string;
  targetVersionId: string;
}): Promise<InstallJob> {
  requireNativeContent();
  return installJobSchema.parse(
    await invoke("instance_content_update", { request: input }),
  );
}

export async function listInstanceWorlds(
  instanceId: string,
): Promise<string[]> {
  if (bridgeMode !== "native") return [];
  return instanceWorldListSchema.parse(
    await invoke("instance_worlds_list", { request: { instanceId } }),
  );
}

export async function importLocalContentFile(input: {
  instanceId: string;
  kind: InstanceContentKind;
  worldName?: string;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_content_file_import", { request: input }),
  );
}

export async function setInstanceContentFileEnabled(input: {
  instanceId: string;
  kind: InstanceContentKind;
  filePath: string;
  enabled: boolean;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_content_file_set_enabled", { request: input }),
  );
}

export async function setInstanceResourcePackActive(input: {
  instanceId: string;
  filePath: string;
  active: boolean;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_resource_pack_set_active", { request: input }),
  );
}

export async function moveInstanceResourcePack(input: {
  instanceId: string;
  filePath: string;
  direction: "higher" | "lower";
  expectedRevision: number;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_resource_pack_move", { request: input }),
  );
}

export async function removeInstanceContentFile(input: {
  instanceId: string;
  kind: InstanceContentKind;
  filePath: string;
  expectedRevision: number;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_content_file_remove", { request: input }),
  );
}

export async function resolveInstanceMods(
  instanceId: string,
): Promise<InstanceModResolution[]> {
  if (bridgeMode !== "native") return [];
  return instanceModResolutionListSchema.parse(
    await invoke("instance_mods_resolve", { request: { instanceId } }),
  );
}

export async function setInstanceModEnabled(input: {
  instanceId: string;
  expectedRevision: number;
  filePath: string;
  provider?: Exclude<Provider, "ftb">;
  projectId?: string;
  enabled: boolean;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_mod_set_enabled", { request: input }),
  );
}

export async function setInstanceModPinned(input: {
  instanceId: string;
  expectedRevision: number;
  filePath: string;
  provider?: Exclude<Provider, "ftb">;
  projectId?: string;
  pinned: boolean;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_mod_set_pinned", { request: input }),
  );
}

export async function removeInstanceMod(input: {
  instanceId: string;
  expectedRevision: number;
  filePath: string;
  provider?: Exclude<Provider, "ftb">;
  projectId?: string;
}): Promise<LauncherInstance> {
  requireNativeContent();
  return instanceSummarySchema.parse(
    await invoke("instance_mod_remove", { request: input }),
  );
}

export async function installMods(input: {
  instanceId: string;
  expectedRevision: number;
  mods: Array<{
    provider: Exclude<Provider, "ftb">;
    projectId: string;
    displayName: string;
  }>;
}): Promise<InstallJob> {
  requireNativeContent();
  return installJobSchema.parse(
    await invoke("instance_mod_install", { request: input }),
  );
}
