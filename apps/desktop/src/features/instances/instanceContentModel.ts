import type { InstanceContentKind, InstanceMod, Provider } from "../../types/launcher";

export type InstalledModSortKey =
  | "name"
  | "source"
  | "version"
  | "status"
  | "size"
  | "installed";

export type InstalledModSort = {
  key: InstalledModSortKey;
  direction: "ascending" | "descending";
};

export type ContentSectionKind = "mods" | InstanceContentKind;

export function compareInstalledMods(
  left: InstanceMod,
  right: InstanceMod,
  sort: InstalledModSort,
) {
  const sourceValue = (item: InstanceMod) =>
    `${item.provider ?? "local"}:${item.origin}`;
  const installedValue = (item: InstanceMod) =>
    item.installedAt ? Date.parse(item.installedAt) || 0 : 0;
  let result =
    sort.key === "name"
      ? left.displayName.localeCompare(right.displayName)
      : sort.key === "source"
        ? sourceValue(left).localeCompare(sourceValue(right))
        : sort.key === "version"
          ? (left.versionId ?? "").localeCompare(right.versionId ?? "")
          : sort.key === "status"
            ? Number(right.enabled) - Number(left.enabled)
            : sort.key === "size"
              ? left.fileSize - right.fileSize
              : installedValue(left) - installedValue(right);
  if (result === 0) result = left.filePath.localeCompare(right.filePath);
  return sort.direction === "ascending" ? result : -result;
}

export function normalizedModPath(value: string) {
  return value
    .replaceAll("\\", "/")
    .toLocaleLowerCase()
    .replace(/\.disabled$/, "");
}

export function modProviderName(provider: Provider) {
  if (provider === "curseforge") return "CurseForge";
  if (provider === "modrinth") return "Modrinth";
  return "FTB";
}
