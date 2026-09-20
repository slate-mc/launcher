import { useQuery } from "@tanstack/react-query";
import {
  ArrowUpDown,
  ChevronLeft,
  LoaderCircle,
  Network,
  Pin,
  PinOff,
  Power,
  RefreshCw,
  RotateCcw,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import { ComboBox } from "../../components/ComboBox";
import { ContentArtwork } from "../../components/ContentArtwork";
import { StatusPill } from "../../components/PageScaffold";
import {
  listInstanceModHistory,
  listInstanceModVersions,
} from "../../lib/bridge";
import { formatDate } from "../../lib/format";
import type { InstanceMod } from "../../types/launcher";
import {
  contentErrorMessage,
  formatContentFileSize,
} from "./instanceContentFormat";
import {
  modProviderName,
  type InstalledModSort,
  type InstalledModSortKey,
} from "./instanceContentModel";

export function InstalledModRow({
  item,
  instanceId,
  disabled,
  confirmingRemove,
  pendingAction,
  onToggle,
  onPin,
  onUpdate,
  onResolveRelationships,
  onRequestRemove,
  onCancelRemove,
  onConfirmRemove,
}: {
  item: InstanceMod;
  disabled: boolean;
  confirmingRemove: boolean;
  pendingAction?: "toggle" | "pin" | "update" | "remove" | "relationships";
  instanceId: string;
  onToggle: () => void;
  onPin: () => void;
  onUpdate: (versionId: string) => void;
  onResolveRelationships: () => Promise<void>;
  onRequestRemove: () => void;
  onCancelRemove: () => void;
  onConfirmRemove: () => void;
}) {
  const [versionOpen, setVersionOpen] = useState(false);
  const [relationshipsOpen, setRelationshipsOpen] = useState(false);
  const [relationshipsRequested, setRelationshipsRequested] = useState(
    item.dependencies.length > 0 || item.requiredBy.length > 0,
  );
  const [selectedVersionId, setSelectedVersionId] = useState(
    item.versionId ?? "",
  );
  const versionQuery = useQuery({
    queryKey: [
      "instance-mod-versions",
      instanceId,
      item.provider,
      item.projectId,
    ],
    queryFn: () => {
      if (!item.provider || !item.projectId) {
        throw new Error("This mod is not linked to a supported provider.");
      }
      return listInstanceModVersions({
        instanceId,
        provider: item.provider,
        projectId: item.projectId,
      });
    },
    enabled: versionOpen && Boolean(item.provider && item.projectId),
    staleTime: 5 * 60_000,
  });
  const historyQuery = useQuery({
    queryKey: [
      "instance-mod-history",
      instanceId,
      item.provider,
      item.projectId,
    ],
    queryFn: () => {
      if (!item.provider || !item.projectId) {
        throw new Error("This mod is not linked to a supported provider.");
      }
      return listInstanceModHistory({
        instanceId,
        provider: item.provider,
        projectId: item.projectId,
      });
    },
    enabled: versionOpen && Boolean(item.provider && item.projectId),
    staleTime: 30_000,
  });
  const versionOptions = (versionQuery.data ?? []).map((version, index) => ({
    value: version.id,
    label: version.name,
    description: version.id,
    recommended: index === 0,
  }));
  if (
    item.versionId &&
    !versionOptions.some((option) => option.value === item.versionId)
  ) {
    versionOptions.push({
      value: item.versionId,
      label: item.versionId,
      description: "Installed version",
      recommended: false,
    });
  }
  const activeVersionId = selectedVersionId || versionQuery.data?.[0]?.id || "";
  const recentVersions = Array.from(
    new Map(
      (historyQuery.data ?? [])
        .filter((history) => history.versionId !== item.versionId)
        .map((history) => [history.versionId, history]),
    ).values(),
  ).slice(0, 5);

  return (
    <>
      <tr className="border-b border-app-separator/40 transition-colors duration-150 hover:bg-app-raised/35">
        <td className="px-4 py-2.5">
          <div className="flex min-w-0 items-center gap-3">
            <ContentArtwork
              src={item.iconUrl}
              name={item.displayName}
              stableKey={
                item.provider && item.projectId
                  ? `${item.provider}:${item.projectId}`
                  : item.filePath
              }
              className="size-9 shrink-0 rounded-control border border-app-separator"
            />
            <div className="min-w-0">
              <p
                className="m-0 truncate text-[12px] font-bold text-app-text"
                title={item.displayName}
              >
                {item.displayName}
              </p>
              <p
                className="mt-1 mb-0 truncate font-mono text-[9px] text-app-muted"
                title={item.filePath}
              >
                {item.filePath}
              </p>
            </div>
          </div>
        </td>
        <td className="px-4 py-2.5">
          <p className="m-0 truncate text-[11px] font-semibold text-app-secondary">
            {item.provider ? modProviderName(item.provider) : "Local"}
          </p>
          <p className="mt-1 mb-0 text-[9px] text-app-muted">
            {modOriginLabel(item.origin)}
            {item.dependencies.length > 0
              ? ` · ${item.dependencies.length} ${item.dependencies.length === 1 ? "dependency" : "dependencies"}`
              : ""}
            {item.requiredBy.length > 0
              ? ` · required by ${item.requiredBy.length}`
              : ""}
          </p>
        </td>
        <td className="px-4 py-2.5">
          <p
            className="m-0 truncate font-mono text-[9px] text-app-secondary"
            title={item.versionId ?? "Version unavailable"}
          >
            {item.versionId ?? "Unknown"}
          </p>
        </td>
        <td className="px-4 py-2.5">
          <StatusPill tone={item.enabled ? "positive" : "neutral"}>
            {item.enabled ? "Enabled" : "Disabled"}
          </StatusPill>
          {item.pinned ? (
            <span className="mt-1 block text-[9px] font-bold text-app-accent">
              Version pinned
            </span>
          ) : null}
        </td>
        <td className="px-4 py-2.5 text-right font-mono text-[9px] text-app-secondary">
          {formatContentFileSize(item.fileSize)}
        </td>
        <td className="px-4 py-2.5 text-[10px] text-app-secondary">
          {item.installedAt ? formatDate(item.installedAt) : "Unknown"}
        </td>
        <td className="px-4 py-2.5">
          <div className="flex items-center justify-end gap-1.5">
            <button
              type="button"
              className={`inline-flex size-8 items-center justify-center rounded-control border bg-app-bg hover:border-app-accent/45 hover:text-app-accent disabled:opacity-45 ${relationshipsOpen ? "border-app-accent/40 text-app-accent" : "border-app-separator text-app-secondary"}`}
              disabled={
                disabled || !item.provider || !item.projectId || !item.versionId
              }
              onClick={() => {
                setVersionOpen(false);
                const opening = !relationshipsOpen;
                setRelationshipsOpen(opening);
                if (
                  opening &&
                  !relationshipsRequested &&
                  item.dependencies.length === 0 &&
                  item.requiredBy.length === 0
                ) {
                  setRelationshipsRequested(true);
                  void onResolveRelationships().catch(() =>
                    setRelationshipsRequested(false),
                  );
                }
              }}
              aria-label={`Show relationships for ${item.displayName}`}
              aria-expanded={relationshipsOpen}
              title={
                item.provider && item.projectId && item.versionId
                  ? item.dependencies.length === 0 &&
                    item.requiredBy.length === 0
                    ? "Check dependencies"
                    : "Show dependencies and dependents"
                  : "Dependencies are not available for this file"
              }
            >
              {pendingAction === "relationships" ? (
                <LoaderCircle
                  size={14}
                  className="animate-spin motion-reduce:animate-none"
                  aria-hidden="true"
                />
              ) : (
                <Network size={14} aria-hidden="true" />
              )}
            </button>
            <button
              type="button"
              className={`inline-flex size-8 items-center justify-center rounded-control border bg-app-bg hover:border-app-accent/45 hover:text-app-accent disabled:opacity-45 ${versionOpen ? "border-app-accent/40 text-app-accent" : "border-app-separator text-app-secondary"}`}
              disabled={disabled || !item.provider || !item.projectId}
              onClick={() => {
                setRelationshipsOpen(false);
                setSelectedVersionId(item.versionId ?? "");
                setVersionOpen((open) => !open);
              }}
              aria-label={`Change version of ${item.displayName}`}
              aria-expanded={versionOpen}
              title={
                item.provider
                  ? "Change installed version"
                  : "Versions are not available for this file"
              }
            >
              {pendingAction === "update" ? (
                <LoaderCircle
                  size={14}
                  className="animate-spin motion-reduce:animate-none"
                  aria-hidden="true"
                />
              ) : (
                <RefreshCw size={14} aria-hidden="true" />
              )}
            </button>
            <button
              type="button"
              className={`inline-flex size-8 items-center justify-center rounded-control border bg-app-bg hover:border-app-accent/45 hover:text-app-accent disabled:opacity-45 ${item.pinned ? "border-app-accent/40 text-app-accent" : "border-app-separator text-app-secondary"}`}
              disabled={disabled || !item.provider || !item.projectId}
              onClick={onPin}
              aria-label={`${item.pinned ? "Unpin" : "Pin"} ${item.displayName}`}
              title={
                item.provider
                  ? item.pinned
                    ? "Allow compatible updates"
                    : "Keep this version"
                  : "Updates are not available for this file"
              }
            >
              {pendingAction === "pin" ? (
                <LoaderCircle
                  size={14}
                  className="animate-spin motion-reduce:animate-none"
                />
              ) : item.pinned ? (
                <PinOff size={14} />
              ) : (
                <Pin size={14} />
              )}
            </button>
            <button
              type="button"
              className="inline-flex size-8 items-center justify-center rounded-control border border-app-separator bg-app-bg text-app-secondary hover:border-app-accent/45 hover:text-app-text disabled:opacity-45"
              disabled={disabled}
              onClick={onToggle}
              aria-label={`${item.enabled ? "Disable" : "Enable"} ${item.displayName}`}
              title={item.enabled ? "Disable mod" : "Enable mod"}
            >
              {pendingAction === "toggle" ? (
                <LoaderCircle
                  size={14}
                  className="animate-spin motion-reduce:animate-none"
                  aria-hidden="true"
                />
              ) : (
                <Power size={14} aria-hidden="true" />
              )}
            </button>
            <button
              type="button"
              className="inline-flex size-8 items-center justify-center rounded-control border border-app-separator bg-app-bg text-app-muted hover:border-app-danger/45 hover:text-app-danger disabled:opacity-45"
              disabled={disabled}
              onClick={onRequestRemove}
              aria-label={`Remove ${item.displayName}`}
              title="Remove mod"
            >
              <Trash2 size={14} aria-hidden="true" />
            </button>
          </div>
        </td>
      </tr>
      {relationshipsOpen ? (
        <tr className="border-b border-app-separator/50 bg-app-raised/25">
          <td colSpan={7} className="px-4 py-4">
            {pendingAction === "relationships" ? (
              <p className="mt-0 mb-3 inline-flex items-center gap-2 text-[10px] text-app-muted">
                <LoaderCircle
                  size={12}
                  className="animate-spin motion-reduce:animate-none"
                  aria-hidden="true"
                />
                Checking this mod’s dependencies…
              </p>
            ) : null}
            <div className="grid grid-cols-2 gap-8 max-[840px]:grid-cols-1">
              <ModRelationshipList
                label="Depends on"
                items={item.dependencies}
                empty="No recorded dependencies"
              />
              <ModRelationshipList
                label="Required by"
                items={item.requiredBy}
                empty="No installed mods depend on this"
              />
            </div>
          </td>
        </tr>
      ) : null}
      {versionOpen ? (
        <tr className="border-b border-app-accent/20 bg-app-accent/5">
          <td colSpan={7} className="px-4 py-4">
            <div className="grid grid-cols-[minmax(260px,420px)_minmax(0,1fr)_auto] items-end gap-4 max-[980px]:grid-cols-1">
              <ComboBox
                label={`Version for ${item.displayName}`}
                value={activeVersionId}
                options={versionOptions}
                onValueChange={setSelectedVersionId}
                placeholder={
                  versionQuery.isPending
                    ? "Loading versions…"
                    : "Choose a version"
                }
                emptyText={
                  versionQuery.isPending
                    ? "Loading versions…"
                    : "No versions match this instance"
                }
                disabled={
                  disabled || versionQuery.isPending || versionQuery.isError
                }
              />
              <div className="min-w-0 pb-1 text-[10px]/[15px] text-app-muted">
                {versionQuery.isError
                  ? contentErrorMessage(versionQuery.error)
                  : item.versionId
                    ? `Installed: ${item.versionId}`
                    : "Choose a release built for this Minecraft version and loader."}
              </div>
              <div className="flex items-center justify-end gap-2">
                <button
                  type="button"
                  className="h-9 rounded-control border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-secondary hover:text-app-text disabled:opacity-45"
                  disabled={disabled}
                  onClick={() => setVersionOpen(false)}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="inline-flex h-9 items-center gap-2 rounded-control border-0 bg-app-accent px-4 text-[11px] font-bold text-app-on-accent hover:brightness-105 disabled:cursor-not-allowed disabled:opacity-45"
                  disabled={
                    disabled ||
                    versionQuery.isPending ||
                    !activeVersionId ||
                    activeVersionId === item.versionId
                  }
                  onClick={() => onUpdate(activeVersionId)}
                >
                  {pendingAction === "update" ? (
                    <LoaderCircle
                      size={13}
                      className="animate-spin motion-reduce:animate-none"
                      aria-hidden="true"
                    />
                  ) : (
                    <RefreshCw size={13} aria-hidden="true" />
                  )}
                  Apply version
                </button>
              </div>
            </div>
            <div className="mt-4 border-t border-app-separator/45 pt-3">
              <p className="m-0 text-[9px] font-bold tracking-[.08em] text-app-muted uppercase">
                Recent versions
              </p>
              {historyQuery.isPending ? (
                <p className="mt-2 mb-0 text-[10px] text-app-muted">
                  Loading version history…
                </p>
              ) : historyQuery.isError ? (
                <p className="mt-2 mb-0 text-[10px] text-app-danger">
                  {contentErrorMessage(historyQuery.error)}
                </p>
              ) : recentVersions.length > 0 ? (
                <div className="mt-2 flex flex-wrap gap-2">
                  {recentVersions.map((history) => {
                    const versionName =
                      versionQuery.data?.find(
                        (version) => version.id === history.versionId,
                      )?.name ?? history.versionId;
                    return (
                      <button
                        key={history.versionId}
                        type="button"
                        className="inline-flex min-w-0 items-center gap-2 rounded-control border border-app-separator bg-app-bg px-3 py-2 text-left text-app-secondary hover:border-app-accent/45 hover:text-app-text disabled:opacity-45"
                        disabled={disabled || pendingAction === "update"}
                        onClick={() => onUpdate(history.versionId)}
                        title={`Restore ${versionName}`}
                      >
                        <RotateCcw
                          size={12}
                          className="shrink-0 text-app-accent"
                          aria-hidden="true"
                        />
                        <span className="min-w-0">
                          <span className="block max-w-64 truncate text-[10px] font-bold">
                            {versionName}
                          </span>
                          <span className="mt-0.5 block text-[8px] text-app-muted">
                            Used until {formatDate(history.changedAt)}
                          </span>
                        </span>
                      </button>
                    );
                  })}
                </div>
              ) : (
                <p className="mt-2 mb-0 text-[10px] text-app-muted">
                  No earlier versions recorded yet.
                </p>
              )}
            </div>
          </td>
        </tr>
      ) : null}
      {confirmingRemove ? (
        <tr className="border-b border-app-danger/25 bg-app-danger/5">
          <td colSpan={7} className="px-4 py-3">
            <div className="flex items-center justify-between gap-5">
              <p className="m-0 text-[10px]/[15px] text-app-warning">
                Remove <strong>{item.displayName}</strong>? This may break
                dependent mods.{" "}
                {item.origin === "modpack"
                  ? "Reinstalling the modpack will restore it."
                  : "The file will move to slate’s trash."}
              </p>
              <div className="flex shrink-0 items-center gap-2">
                <button
                  type="button"
                  className="h-8 rounded-control border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-secondary"
                  disabled={disabled}
                  onClick={onCancelRemove}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="inline-flex h-8 items-center gap-1.5 rounded-control border border-app-danger/45 bg-app-danger/10 px-3 text-[11px] font-bold text-app-danger disabled:opacity-45"
                  disabled={disabled}
                  onClick={onConfirmRemove}
                >
                  {pendingAction === "remove" ? (
                    <LoaderCircle
                      size={13}
                      className="animate-spin motion-reduce:animate-none"
                      aria-hidden="true"
                    />
                  ) : (
                    <Trash2 size={13} aria-hidden="true" />
                  )}
                  Remove
                </button>
              </div>
            </div>
          </td>
        </tr>
      ) : null}
    </>
  );
}

function ModRelationshipList({
  label,
  items,
  empty,
}: {
  label: string;
  items: InstanceMod["dependencies"];
  empty: string;
}) {
  return (
    <div className="min-w-0">
      <p className="m-0 text-[10px] font-bold tracking-[.06em] text-app-muted uppercase">
        {label}
      </p>
      {items.length > 0 ? (
        <ul className="mt-2 mb-0 flex list-none flex-wrap gap-2 p-0">
          {items.map((item) => (
            <li
              key={`${item.provider}:${item.projectId}`}
              className="max-w-full rounded-control border border-app-separator bg-app-bg px-2.5 py-1.5"
              title={`${modProviderName(item.provider)} · ${item.projectId}`}
            >
              <span className="block max-w-64 truncate text-[10px] font-bold text-app-secondary">
                {item.displayName ?? item.projectId}
              </span>
              <span className="mt-0.5 block font-mono text-[8px] text-app-muted">
                {modProviderName(item.provider)}
              </span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="mt-2 mb-0 text-[10px] text-app-muted">{empty}</p>
      )}
    </div>
  );
}

function modOriginLabel(origin: InstanceMod["origin"]) {
  if (origin === "modpack") return "From modpack";
  if (origin === "added") return "Added in slate";
  return "Local file";
}

export function InstalledModTableHeader({
  sort,
  onSort,
}: {
  sort: InstalledModSort;
  onSort: (sort: InstalledModSort) => void;
}) {
  return (
    <thead className="bg-app-bg/35">
      <tr className="border-b border-app-separator/55">
        <ModTableSortHeader
          label="Mod"
          sortKey="name"
          sort={sort}
          onSort={onSort}
        />
        <ModTableSortHeader
          label="Source"
          sortKey="source"
          sort={sort}
          onSort={onSort}
        />
        <ModTableSortHeader
          label="Version"
          sortKey="version"
          sort={sort}
          onSort={onSort}
        />
        <ModTableSortHeader
          label="Status"
          sortKey="status"
          sort={sort}
          onSort={onSort}
        />
        <ModTableSortHeader
          label="Size"
          sortKey="size"
          sort={sort}
          onSort={onSort}
          align="right"
        />
        <ModTableSortHeader
          label="Installed"
          sortKey="installed"
          sort={sort}
          onSort={onSort}
        />
        <th
          scope="col"
          className="px-4 py-2.5 text-right text-[9px] font-bold tracking-[.08em] text-app-muted uppercase"
        >
          Actions
        </th>
      </tr>
    </thead>
  );
}

export function ModTableSortHeader({
  label,
  sortKey,
  sort,
  onSort,
  align = "left",
}: {
  label: string;
  sortKey: InstalledModSortKey;
  sort: InstalledModSort;
  onSort: (sort: InstalledModSort) => void;
  align?: "left" | "right";
}) {
  const active = sort.key === sortKey;
  return (
    <th
      scope="col"
      aria-sort={active ? sort.direction : "none"}
      className="px-4 py-0"
    >
      <button
        type="button"
        className={`flex h-9 w-full items-center gap-1.5 text-[9px] font-bold tracking-[.08em] uppercase ${
          align === "right" ? "justify-end" : "justify-start"
        } ${active ? "text-app-text" : "text-app-muted hover:text-app-secondary"}`}
        onClick={() =>
          onSort({
            key: sortKey,
            direction:
              active && sort.direction === "ascending"
                ? "descending"
                : "ascending",
          })
        }
      >
        {label}
        {active ? (
          <ChevronLeft
            size={11}
            className={`text-app-accent ${sort.direction === "ascending" ? "rotate-90" : "-rotate-90"}`}
            aria-hidden="true"
          />
        ) : (
          <ArrowUpDown size={11} aria-hidden="true" />
        )}
      </button>
    </th>
  );
}
