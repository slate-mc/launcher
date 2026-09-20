import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowUpDown,
  Check,
  ChevronLeft,
  LoaderCircle,
  Pin,
  PinOff,
  Power,
  RotateCcw,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import { ContentArtwork } from "../../components/ContentArtwork";
import { EmptyState, InlineNotice, StatusPill } from "../../components/PageScaffold";
import {
  installInstance,
  listInstanceContentFiles,
  removeInstanceContentFile,
  setInstanceContentFileEnabled,
} from "../../lib/bridge";
import { formatDate } from "../../lib/format";
import type {
  InstanceContentFile,
  InstanceContentKind,
  InstanceMod,
  LauncherInstance,
  ModpackSummary,
} from "../../types/launcher";
import {
  contentErrorMessage,
  formatContentFileSize,
} from "./instanceContentFormat";
import {
  modProviderName,
  type ContentSectionKind,
  type InstalledModSort,
  type InstalledModSortKey,
} from "./instanceContentModel";

const secondaryButtonClass =
  "inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text hover:border-app-secondary disabled:opacity-50";

export function ContentNavigation({
  active,
  counts,
  onChange,
}: {
  active: ContentSectionKind;
  counts: Partial<Record<ContentSectionKind, number>>;
  onChange: (kind: ContentSectionKind) => void;
}) {
  const items: Array<[ContentSectionKind, string]> = [
    ["mods", "Mods"],
    ["resourcePack", "Resource packs"],
    ["shaderPack", "Shaders"],
    ["dataPack", "Data packs"],
  ];
  return (
    <nav
      className="rounded-control border border-app-separator/70 bg-app-surface p-2"
      aria-label="Content types"
    >
      {items.map(([kind, label]) => (
        <button
          key={kind}
          type="button"
          className={`flex h-10 w-full items-center justify-between rounded-compact border-0 px-3 text-left text-xs font-bold ${active === kind ? "bg-app-raised text-app-text" : "bg-transparent text-app-secondary hover:bg-app-raised/60 hover:text-app-text"}`}
          onClick={() => onChange(kind)}
        >
          {label}
          <span className="font-mono text-[10px] text-app-muted">
            {counts[kind] ?? "—"}
          </span>
        </button>
      ))}
    </nav>
  );
}

export function InstanceFileContent({
  instance,
  kind,
  modCount,
  onKindChange,
}: {
  instance: LauncherInstance;
  kind: InstanceContentKind;
  modCount: number;
  onKindChange: (kind: ContentSectionKind) => void;
}) {
  const queryClient = useQueryClient();
  const [removeTarget, setRemoveTarget] = useState<string>();
  const [notice, setNotice] = useState<{
    tone: "positive" | "danger";
    title: string;
    message: string;
  }>();
  const query = useQuery({
    queryKey: ["instance-content-files", instance.id, kind],
    queryFn: () => listInstanceContentFiles(instance.id, kind),
  });
  const refresh = async (updated: LauncherInstance, message: string) => {
    queryClient.setQueryData(["instance", instance.id], updated);
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: ["instance-content-files", instance.id, kind],
      }),
      queryClient.invalidateQueries({ queryKey: ["instances"] }),
    ]);
    setNotice({ tone: "positive", title: "Content updated", message });
  };
  const toggleMutation = useMutation({
    mutationFn: ({ file, enabled }: { file: InstanceContentFile; enabled: boolean }) =>
      setInstanceContentFileEnabled({
        instanceId: instance.id,
        kind,
        filePath: file.filePath,
        enabled,
        expectedRevision: instance.revision,
      }),
    onMutate: () => setNotice(undefined),
    onSuccess: (updated, variables) =>
      refresh(
        updated,
        `${variables.file.displayName} is ${variables.enabled ? "enabled" : "disabled"}.`,
      ),
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Content was not changed",
        message: contentErrorMessage(error),
      }),
  });
  const removeMutation = useMutation({
    mutationFn: (file: InstanceContentFile) =>
      removeInstanceContentFile({
        instanceId: instance.id,
        kind,
        filePath: file.filePath,
        expectedRevision: instance.revision,
      }),
    onMutate: () => setNotice(undefined),
    onSuccess: (updated, file) => {
      setRemoveTarget(undefined);
      return refresh(updated, `${file.displayName} was moved to slate’s recoverable trash.`);
    },
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Content was not removed",
        message: contentErrorMessage(error),
      }),
  });
  const restoreMutation = useMutation({
    mutationFn: () =>
      installInstance({ id: instance.id, expectedRevision: instance.revision }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["install-jobs"] });
      setNotice({
        tone: "positive",
        title: "Pack restore queued",
        message:
          "slate will restore pack-managed defaults and leave personal additions in place.",
      });
    },
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Pack restore was not queued",
        message: contentErrorMessage(error),
      }),
  });
  const files = query.data ?? [];
  const label = contentKindLabel(kind);
  const busy = toggleMutation.isPending || removeMutation.isPending;

  return (
    <div className="grid grid-cols-[220px_minmax(0,1fr)] gap-6">
      <ContentNavigation
        active={kind}
        counts={{ mods: modCount, [kind]: files.length }}
        onChange={onKindChange}
      />
      <section className="min-w-0 rounded-control border border-app-separator/70 bg-app-surface">
        <header className="flex items-center justify-between gap-5 border-b border-app-separator/55 px-5 py-4">
          <span>
            <h2 className="m-0 text-[15px] font-bold">{label}</h2>
            <p className="mt-1 mb-0 text-[11px] text-app-secondary">
              {kind === "dataPack"
                ? "Data packs are grouped by world. Folder packs stay controlled by Minecraft."
                : "Archive packs can be hidden from Minecraft, restored, or moved to recoverable trash."}
            </p>
          </span>
          {instance.modpackSource ? (
            <button
              type="button"
              className={secondaryButtonClass}
              disabled={restoreMutation.isPending}
              onClick={() => restoreMutation.mutate()}
            >
              {restoreMutation.isPending ? (
                <RotateCcw className="animate-spin" size={14} />
              ) : (
                <RotateCcw size={14} />
              )}
              Restore pack defaults
            </button>
          ) : null}
        </header>
        {notice ? (
          <div className="px-5 pt-5">
            <InlineNotice tone={notice.tone} title={notice.title}>
              {notice.message}
            </InlineNotice>
          </div>
        ) : null}
        {query.isPending ? (
          <div className="grid min-h-64 place-items-center text-xs text-app-secondary">
            <span className="inline-flex items-center gap-2">
              <LoaderCircle className="animate-spin" size={15} />Scanning instance files…
            </span>
          </div>
        ) : query.isError ? (
          <div className="p-5">
            <InlineNotice tone="danger" title={`${label} could not be loaded`}>
              {contentErrorMessage(query.error)}
            </InlineNotice>
          </div>
        ) : files.length ? (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[760px] table-fixed border-collapse text-left">
              <thead className="border-b border-app-separator/55 bg-app-bg/20 font-mono text-[9px] uppercase tracking-[0.08em] text-app-muted">
                <tr>
                  <th className="w-[34%] px-5 py-3 font-medium">Name</th>
                  <th className="w-[16%] px-3 py-3 font-medium">Scope</th>
                  <th className="w-[16%] px-3 py-3 font-medium">Ownership</th>
                  <th className="w-[13%] px-3 py-3 font-medium">Size</th>
                  <th className="w-[21%] px-3 py-3 font-medium">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-app-separator/45">
                {files.map((file) => (
                  <tr key={file.filePath} className="text-[11px]">
                    <td className="px-5 py-3">
                      <strong className="block truncate text-xs text-app-text">{file.displayName}</strong>
                      <span className="mt-0.5 block truncate font-mono text-[9px] text-app-muted" title={file.filePath}>{file.filePath}</span>
                    </td>
                    <td className="px-3 py-3 text-app-secondary">{file.worldName ?? "Instance"}</td>
                    <td className="px-3 py-3">
                      <span className={`rounded-full border px-2 py-1 font-mono text-[9px] ${file.origin === "modpack" ? "border-app-accent/35 text-app-accent" : "border-app-separator text-app-secondary"}`}>
                        {file.origin === "modpack" ? "Pack-managed" : "Personal"}
                      </span>
                    </td>
                    <td className="px-3 py-3 font-mono text-[10px] text-app-secondary">{file.fileSize ? formatContentFileSize(file.fileSize) : "Folder"}</td>
                    <td className="px-3 py-3">
                      <div className="flex items-center justify-end gap-2">
                        {file.canToggle ? (
                          <button
                            type="button"
                            className="h-7 rounded-control border border-app-separator px-2.5 text-[10px] font-bold text-app-secondary hover:text-app-text disabled:opacity-45"
                            disabled={busy}
                            onClick={() => toggleMutation.mutate({ file, enabled: !file.enabled })}
                          >
                            {file.enabled ? "Hide" : "Restore"}
                          </button>
                        ) : (
                          <span className="text-[9px] text-app-muted" title="Folder packs are enabled and ordered inside Minecraft.">In-game</span>
                        )}
                        {removeTarget === file.filePath ? (
                          <span className="flex items-center gap-2">
                            <button type="button" className="text-[10px] font-bold text-app-danger" disabled={busy} onClick={() => removeMutation.mutate(file)}>Confirm</button>
                            <button type="button" className="text-[10px] text-app-secondary" onClick={() => setRemoveTarget(undefined)}>Cancel</button>
                          </span>
                        ) : (
                          <button type="button" className="p-1.5 text-app-muted hover:text-app-danger" title={`Remove ${file.displayName}`} disabled={busy} onClick={() => setRemoveTarget(file.filePath)}><Trash2 size={14} /></button>
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <EmptyState
            title={`No ${label.toLocaleLowerCase()} detected`}
            description={kind === "dataPack" ? "Install or create a data pack inside a world’s datapacks folder." : `Place compatible archives in this instance’s ${label.toLocaleLowerCase()} folder.`}
          />
        )}
      </section>
    </div>
  );
}

function contentKindLabel(kind: InstanceContentKind) {
  switch (kind) {
    case "resourcePack":
      return "Resource packs";
    case "shaderPack":
      return "Shader packs";
    case "dataPack":
      return "Data packs";
  }
}

export function ModSearchResult({
  item,
  installed,
  selected,
  checkingInstalled,
  disabled,
  onToggleSelected,
}: {
  item: ModpackSummary;
  installed: boolean;
  selected: boolean;
  checkingInstalled: boolean;
  disabled: boolean;
  onToggleSelected: () => void;
}) {
  return (
    <article className="grid grid-cols-[44px_minmax(0,1fr)_auto] items-center gap-3 py-3.5">
      <ContentImage src={item.icon_url} name={item.name} />
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <h3 className="m-0 truncate text-[13px] font-bold text-app-text">
            {item.name}
          </h3>
          <span className="rounded-full border border-app-separator px-2 py-0.5 font-mono text-[9px] text-app-muted">
            {modProviderName(item.provider)}
          </span>
        </div>
        <p className="mt-1 mb-0 line-clamp-1 text-[11px] text-app-secondary">
          {item.summary || "No description supplied by the provider."}
        </p>
        <p className="mt-1.5 mb-0 font-mono text-[9px] text-app-muted">
          {formatCompactNumber(item.downloads)} downloads
        </p>
      </div>
      <button
        type="button"
        disabled={
          disabled || installed || checkingInstalled || item.provider === "ftb"
        }
        onClick={onToggleSelected}
        aria-pressed={selected}
        title={
          installed
            ? "Already installed in this instance"
            : selected
              ? "Remove from install selection"
              : checkingInstalled
                ? "Checking installed mods"
                : undefined
        }
        className={`inline-flex h-8 min-w-20 items-center justify-center gap-1.5 rounded-control border px-3 text-[11px] font-bold disabled:cursor-not-allowed ${
          installed || selected
            ? "border-app-accent/30 bg-app-accent/10 text-app-accent"
            : "border-app-accent/55 bg-app-accent/10 text-app-accent hover:bg-app-accent/15 disabled:opacity-45"
        }`}
      >
        {installed ? (
          <>
            <Check size={13} aria-hidden="true" /> Installed
          </>
        ) : selected ? (
          <>
            <Check size={13} aria-hidden="true" /> Selected
          </>
        ) : checkingInstalled ? (
          <>
            <LoaderCircle
              size={13}
              className="animate-spin motion-reduce:animate-none"
              aria-hidden="true"
            />
            Checking
          </>
        ) : (
          "Add"
        )}
      </button>
    </article>
  );
}

export function InstalledModRow({
  item,
  disabled,
  confirmingRemove,
  pendingAction,
  onToggle,
  onPin,
  onRequestRemove,
  onCancelRemove,
  onConfirmRemove,
}: {
  item: InstanceMod;
  disabled: boolean;
  confirmingRemove: boolean;
  pendingAction?: "toggle" | "pin" | "remove";
  onToggle: () => void;
  onPin: () => void;
  onRequestRemove: () => void;
  onCancelRemove: () => void;
  onConfirmRemove: () => void;
}) {
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
          {item.pinned ? <span className="mt-1 block text-[9px] font-bold text-app-accent">Version pinned</span> : null}
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
              className={`inline-flex size-8 items-center justify-center rounded-control border bg-app-bg hover:border-app-accent/45 hover:text-app-accent disabled:opacity-45 ${item.pinned ? "border-app-accent/40 text-app-accent" : "border-app-separator text-app-secondary"}`}
              disabled={disabled || !item.provider || !item.projectId}
              onClick={onPin}
              aria-label={`${item.pinned ? "Unpin" : "Pin"} ${item.displayName}`}
              title={item.provider ? (item.pinned ? "Allow compatible updates" : "Keep this exact version") : "Local files cannot be version pinned"}
            >
              {pendingAction === "pin" ? <LoaderCircle size={14} className="animate-spin motion-reduce:animate-none" /> : item.pinned ? <PinOff size={14} /> : <Pin size={14} />}
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

function modOriginLabel(origin: InstanceMod["origin"]) {
  if (origin === "modpack") return "From modpack";
  if (origin === "added") return "Added in slate";
  return "Unmanaged file";
}

export function ContentSelect({
  label,
  value,
  options,
  onChange,
  compact = false,
}: {
  label: string;
  value: string;
  options: ReadonlyArray<readonly [string, string]>;
  onChange: (value: string) => void;
  compact?: boolean;
}) {
  return (
    <label className="relative block">
      <span className="sr-only">{label}</span>
      <select
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className={`${compact ? "h-8 text-[11px]" : "h-9 text-xs"} w-full appearance-none rounded-control border border-app-separator bg-app-bg px-3 pr-8 text-app-text outline-none focus:border-app-accent`}
      >
        {options.map(([optionValue, optionLabel]) => (
          <option key={optionValue} value={optionValue}>
            {optionLabel}
          </option>
        ))}
      </select>
      <ChevronLeft
        size={13}
        className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 -rotate-90 text-app-muted"
        aria-hidden="true"
      />
    </label>
  );
}

function ContentImage({ src, name }: { src?: string | null; name: string }) {
  return (
    <ContentArtwork
      src={src}
      name={name}
      stableKey={name}
      className="size-11 rounded-control border border-app-separator"
    />
  );
}

export function ModResultSkeletons() {
  return (
    <div
      className="divide-y divide-app-separator/45 px-5"
      aria-label="Loading compatible mods"
    >
      {Array.from({ length: 5 }, (_, index) => (
        <div key={index} className="grid grid-cols-[44px_1fr] gap-3 py-3.5">
          <span className="size-11 animate-pulse rounded-control bg-app-raised" />
          <span className="grid content-center gap-2">
            <span className="h-3 w-44 animate-pulse rounded bg-app-raised" />
            <span className="h-2.5 w-3/4 animate-pulse rounded bg-app-raised" />
          </span>
        </div>
      ))}
    </div>
  );
}

export function InstalledModSkeletons() {
  return (
    <div className="overflow-hidden" aria-label="Loading installed mods">
      <div className="grid h-9 grid-cols-[2fr_1fr_1fr_1fr] items-center gap-4 border-b border-app-separator/55 bg-app-bg/35 px-4">
        {Array.from({ length: 4 }, (_, index) => (
          <span
            key={index}
            className="h-2 w-16 animate-pulse rounded bg-app-raised"
          />
        ))}
      </div>
      {Array.from({ length: 5 }, (_, index) => (
        <div
          key={index}
          className="grid h-[61px] grid-cols-[36px_2fr_1fr_1fr_1fr] items-center gap-3 border-b border-app-separator/40 px-4"
        >
          <span className="size-9 animate-pulse rounded-control bg-app-raised" />
          <span className="grid gap-2">
            <span className="h-2.5 w-36 animate-pulse rounded bg-app-raised" />
            <span className="h-2 w-52 animate-pulse rounded bg-app-raised" />
          </span>
          {Array.from({ length: 3 }, (_, cell) => (
            <span
              key={cell}
              className="h-2.5 w-14 animate-pulse rounded bg-app-raised"
            />
          ))}
        </div>
      ))}
    </div>
  );
}

function formatCompactNumber(value: number) {
  return new Intl.NumberFormat(undefined, { notation: "compact" }).format(
    value,
  );
}
