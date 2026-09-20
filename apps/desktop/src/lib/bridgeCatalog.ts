import { invoke } from "@tauri-apps/api/core";
import {
  loaderVersionCatalogSchema,
  minecraftVersionCatalogSchema,
  type LoaderKind,
  type LoaderVersionCatalog,
  type MinecraftVersionCatalog,
} from "../types/launcher";
import { bridgeMode, requirePreview } from "./bridgeRuntime";

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

