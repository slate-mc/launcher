import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArchiveRestore,
  Boxes,
  Coffee,
  Eraser,
  FileClock,
  HardDrive,
  LoaderCircle,
  PackageOpen,
  RotateCcw,
  ScrollText,
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { useState } from "react";
import { ContentArtwork } from "../../components/ContentArtwork";
import { EmptyState, InlineNotice, PageHeader, StatusPill } from "../../components/PageScaffold";
import {
  clearStorageCategory,
  deleteTrashedInstance,
  emptyInstanceTrash,
  getPreferences,
  getStorageOverview,
  restoreTrashedInstance,
  updatePreferences,
} from "../../lib/bridge";
import { formatDate, loaderLabel } from "../../lib/format";
import { getUserFacingError, UserFacingError } from "../../lib/userFacingError";
import type {
  StorageCategory,
  StorageCategorySummary,
  TrashedInstance,
} from "../../types/launcher";
import { SettingsNavigation } from "./SettingsNavigation";

const categoryDetails: Record<
  StorageCategory,
  {
    title: string;
    description: string;
    icon: typeof HardDrive;
    action?: string;
    managed?: boolean;
  }
> = {
  instances: {
    title: "Installed instances",
    description: "Game directories, worlds, mods, configuration, screenshots, and snapshots.",
    icon: Boxes,
  },
  sharedGameFiles: {
    title: "Shared Minecraft files",
    description: "Shared game and loader files reused between instances.",
    icon: PackageOpen,
    action: "Remove files",
    managed: true,
  },
  managedJava: {
    title: "Downloaded Java versions",
    description: "Java versions slate downloaded for your Minecraft instances.",
    icon: Coffee,
    action: "Remove Java versions",
    managed: true,
  },
  logs: {
    title: "Logs",
    description: "Launcher diagnostics and retained Minecraft session output.",
    icon: ScrollText,
    action: "Clear logs",
  },
  temporaryFiles: {
    title: "Incomplete downloads",
    description: "Files left by interrupted or failed downloads that are safe to clear.",
    icon: FileClock,
    action: "Clear residue",
  },
  removedContent: {
    title: "Removed instance content",
    description: "Mods and packs previously removed from an instance’s Content page.",
    icon: ArchiveRestore,
    action: "Empty content trash",
  },
};

export function StorageSettingsPage() {
  const queryClient = useQueryClient();
  const [confirmCategory, setConfirmCategory] = useState<StorageCategory>();
  const [deleteTarget, setDeleteTarget] = useState<string>();
  const [confirmationName, setConfirmationName] = useState("");
  const [confirmEmpty, setConfirmEmpty] = useState(false);
  const [message, setMessage] = useState<string>();
  const overviewQuery = useQuery({
    queryKey: ["storage-overview"],
    queryFn: getStorageOverview,
    staleTime: 15_000,
  });
  const preferencesQuery = useQuery({
    queryKey: ["preferences"],
    queryFn: getPreferences,
  });

  const refreshStorage = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["storage-overview"] }),
      queryClient.invalidateQueries({ queryKey: ["instances"] }),
    ]);
  };
  const clearMutation = useMutation({
    mutationFn: clearStorageCategory,
    onSuccess: async (result) => {
      setConfirmCategory(undefined);
      setMessage(
        result.reclaimedBytes > 0
          ? `Reclaimed ${formatBytes(result.reclaimedBytes)} from ${formatCount(result.removedFiles, "file")}.`
          : "That storage category was already clear.",
      );
      await refreshStorage();
    },
  });
  const restoreMutation = useMutation({
    mutationFn: restoreTrashedInstance,
    onSuccess: async (instance) => {
      setMessage(`${instance.name} was restored to the library.`);
      await refreshStorage();
    },
  });
  const deleteMutation = useMutation({
    mutationFn: deleteTrashedInstance,
    onSuccess: async (result) => {
      setDeleteTarget(undefined);
      setConfirmationName("");
      setMessage(`Permanently deleted the instance and reclaimed ${formatBytes(result.reclaimedBytes)}.`);
      await refreshStorage();
    },
  });
  const emptyMutation = useMutation({
    mutationFn: emptyInstanceTrash,
    onSuccess: async (result) => {
      setConfirmEmpty(false);
      setMessage(`Emptied instance trash and reclaimed ${formatBytes(result.reclaimedBytes)}.`);
      await refreshStorage();
    },
  });
  const retentionMutation = useMutation({
    mutationFn: async (days: number) => {
      const current = preferencesQuery.data;
      if (!current) throw new UserFacingError("Preferences are still loading.");
      return updatePreferences({ ...current, trashRetentionDays: days });
    },
    onSuccess: (preferences) => {
      queryClient.setQueryData(["preferences"], preferences);
      void queryClient.invalidateQueries({ queryKey: ["storage-overview"] });
      setMessage(
        preferences.trashRetentionDays === 0
          ? "Trashed instances will be kept until you delete them."
          : `Trashed instances will be deleted after ${preferences.trashRetentionDays} days.`,
      );
    },
  });

  const error =
    overviewQuery.error ??
    clearMutation.error ??
    restoreMutation.error ??
    deleteMutation.error ??
    emptyMutation.error ??
    retentionMutation.error;
  const trash = overviewQuery.data?.trashedInstances ?? [];
  const trashSize = trash.reduce((total, item) => total + item.sizeBytes, 0);

  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Settings"
        title="Storage"
        description="Review disk use, clear files you no longer need, and recover removed instances."
        actions={
          <button
            type="button"
            className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-surface px-3 text-xs font-bold text-app-secondary hover:border-app-accent/45 hover:text-app-text disabled:opacity-50"
            disabled={overviewQuery.isFetching}
            onClick={() => void overviewQuery.refetch()}
          >
            <RotateCcw className={overviewQuery.isFetching ? "animate-spin" : ""} size={14} />
            Rescan
          </button>
        }
      />
      <SettingsNavigation />

      <main className="px-8 py-7">
        {error ? (
          <div className="mb-5">
            <InlineNotice tone="danger" title="Storage action did not complete">
              {errorMessage(error)}
            </InlineNotice>
          </div>
        ) : message ? (
          <div className="mb-5">
            <InlineNotice tone="positive" title="Storage updated">{message}</InlineNotice>
          </div>
        ) : null}

        {overviewQuery.isPending ? (
          <StorageSkeleton />
        ) : overviewQuery.isError ? (
          <EmptyState
            error
            title="Storage could not be measured"
            description="No files were changed. Check that every configured storage location is connected, then rescan."
          />
        ) : (
          <div className="grid gap-9">
            <section aria-labelledby="storage-use-title">
              <div className="mb-4 flex items-end justify-between gap-6">
                <div>
                  <h2 id="storage-use-title" className="m-0 text-[17px] font-bold">Storage use</h2>
                  <p className="mt-1 mb-0 text-xs text-app-secondary">
                    {formatBytes(overviewQuery.data.totalSizeBytes)} used on this device. {formatBytes(overviewQuery.data.reclaimableSizeBytes)} can be cleared without deleting active instances.
                  </p>
                </div>
              </div>
              <div className="overflow-hidden rounded-control border border-app-separator/70 bg-app-surface">
                {overviewQuery.data.categories.map((category) => (
                  <StorageCategoryRow
                    key={category.category}
                    category={category}
                    confirming={confirmCategory === category.category}
                    pending={clearMutation.isPending && clearMutation.variables?.category === category.category}
                    onRequest={() => { setMessage(undefined); setConfirmCategory(category.category); }}
                    onCancel={() => setConfirmCategory(undefined)}
                    onConfirm={() => clearMutation.mutate({ category: category.category, confirmManagedData: categoryDetails[category.category].managed })}
                  />
                ))}
              </div>
            </section>

            <section aria-labelledby="instance-trash-title">
              <div className="mb-4 flex items-end justify-between gap-6">
                <div>
                  <h2 id="instance-trash-title" className="m-0 text-[17px] font-bold">Trashed instances</h2>
                  <p className="mt-1 mb-0 text-xs text-app-secondary">
                    {formatCount(trash.length, "instance")}, {formatBytes(trashSize)}. Restore an instance with all of its remaining files.
                  </p>
                </div>
                <div className="flex items-center gap-3">
                  <label className="flex items-center gap-2 text-[11px] text-app-secondary">
                    Delete after
                    <select
                      className="h-9 rounded-control border border-app-separator bg-app-surface px-3 text-xs font-semibold text-app-text focus:border-app-accent focus:outline-none"
                      value={preferencesQuery.data?.trashRetentionDays ?? 30}
                      disabled={preferencesQuery.isPending || retentionMutation.isPending}
                      onChange={(event) => retentionMutation.mutate(Number(event.target.value))}
                    >
                      <option value={7}>7 days</option>
                      <option value={14}>14 days</option>
                      <option value={30}>30 days</option>
                      <option value={60}>60 days</option>
                      <option value={90}>90 days</option>
                      <option value={0}>Never</option>
                    </select>
                  </label>
                  {trash.length > 0 ? (
                    <button
                      type="button"
                      className="inline-flex h-9 items-center gap-2 rounded-control border border-app-danger/35 bg-app-danger/5 px-3 text-xs font-bold text-app-danger hover:bg-app-danger/10"
                      onClick={() => setConfirmEmpty(true)}
                    >
                      <Trash2 size={14} />Empty trash
                    </button>
                  ) : null}
                </div>
              </div>

              {confirmEmpty ? (
                <div className="mb-3 flex items-center gap-4 rounded-control border border-app-danger/40 bg-app-danger/5 px-4 py-3">
                  <TriangleAlert size={18} className="shrink-0 text-app-danger" />
                  <p className="m-0 min-w-0 flex-1 text-xs text-app-secondary">
                    Permanently delete all {formatCount(trash.length, "instance")} and reclaim approximately {formatBytes(trashSize)}? Worlds and personal files cannot be recovered.
                  </p>
                  <button type="button" className="h-8 px-3 text-xs font-bold text-app-secondary" onClick={() => setConfirmEmpty(false)}>Cancel</button>
                  <button
                    type="button"
                    className="inline-flex h-8 items-center gap-2 rounded-control bg-app-danger px-3 text-xs font-bold text-app-bg disabled:opacity-50"
                    disabled={emptyMutation.isPending}
                    onClick={() => emptyMutation.mutate(trash.length)}
                  >
                    {emptyMutation.isPending ? <LoaderCircle size={13} className="animate-spin" /> : <Trash2 size={13} />}
                    Permanently delete
                  </button>
                </div>
              ) : null}

              {trash.length === 0 ? (
                <div className="border-y border-app-separator/55 py-10 text-center">
                  <ArchiveRestore className="mx-auto text-app-muted" size={24} />
                  <h3 className="mt-3 mb-0 text-sm font-bold">Trash is empty</h3>
                  <p className="mt-1 mb-0 text-xs text-app-secondary">Instances moved to trash will appear here before permanent deletion.</p>
                </div>
              ) : (
                <div className="overflow-hidden rounded-control border border-app-separator/70 bg-app-surface">
                  {trash.map((instance) => (
                    <TrashedInstanceRow
                      key={instance.id}
                      instance={instance}
                      deleting={deleteMutation.isPending && deleteMutation.variables?.id === instance.id}
                      restoring={restoreMutation.isPending && restoreMutation.variables?.id === instance.id}
                      confirming={deleteTarget === instance.id}
                      confirmationName={confirmationName}
                      onConfirmationName={setConfirmationName}
                      onRestore={() => restoreMutation.mutate({ id: instance.id, expectedRevision: instance.revision })}
                      onRequestDelete={() => { setDeleteTarget(instance.id); setConfirmationName(""); setMessage(undefined); }}
                      onCancelDelete={() => { setDeleteTarget(undefined); setConfirmationName(""); }}
                      onDelete={() => deleteMutation.mutate({ id: instance.id, expectedRevision: instance.revision, confirmationName })}
                    />
                  ))}
                </div>
              )}
            </section>
          </div>
        )}
      </main>
    </div>
  );
}

function StorageCategoryRow({
  category,
  confirming,
  pending,
  onRequest,
  onCancel,
  onConfirm,
}: {
  category: StorageCategorySummary;
  confirming: boolean;
  pending: boolean;
  onRequest: () => void;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const details = categoryDetails[category.category];
  const Icon = details.icon;
  return (
    <div className="border-b border-app-separator/55 last:border-0">
      <div className="grid min-h-16 grid-cols-[36px_minmax(0,1fr)_140px_150px] items-center gap-4 px-5 py-3">
        <span className="inline-flex size-9 items-center justify-center rounded-control bg-app-raised text-app-secondary"><Icon size={17} /></span>
        <span className="min-w-0">
          <strong className="block text-xs font-bold">{details.title}</strong>
          <span className="mt-0.5 block text-[10px]/[15px] text-app-secondary">{details.description}</span>
        </span>
        <span className="text-right">
          <strong className="block font-mono text-xs">{formatBytes(category.sizeBytes)}</strong>
          <span className="mt-0.5 block font-mono text-[9px] text-app-muted">{formatCount(category.fileCount, "file")}</span>
        </span>
        <span className="flex justify-end">
          {details.action ? (
            <button
              type="button"
              className={`h-8 rounded-control border px-3 text-[11px] font-bold disabled:opacity-40 ${details.managed ? "border-app-warning/35 text-app-warning hover:bg-app-warning/5" : "border-app-separator text-app-secondary hover:border-app-accent/45 hover:text-app-text"}`}
              disabled={category.sizeBytes === 0 || pending}
              onClick={onRequest}
            >
              {pending ? "Clearing…" : details.action}
            </button>
          ) : (
            <StatusPill tone="neutral">Clear from Library</StatusPill>
          )}
        </span>
      </div>
      {confirming ? (
        <div className={`flex items-center gap-4 border-t px-5 py-3 ${details.managed ? "border-app-warning/30 bg-app-warning/5" : "border-app-separator/55 bg-app-bg/35"}`}>
          {details.managed ? <TriangleAlert size={17} className="shrink-0 text-app-warning" /> : <Eraser size={17} className="shrink-0 text-app-secondary" />}
          <p className="m-0 min-w-0 flex-1 text-[11px]/[17px] text-app-secondary">
            {details.managed
              ? "Every affected instance will be marked for repair. slate will download these files again before the next launch. Personal worlds and instance content are not removed."
              : `Permanently remove ${formatCount(category.fileCount, "file")} and reclaim ${formatBytes(category.sizeBytes)}?`}
          </p>
          <button type="button" className="h-8 px-3 text-[11px] font-bold text-app-secondary" onClick={onCancel}>Cancel</button>
          <button type="button" className={`h-8 rounded-control px-3 text-[11px] font-bold ${details.managed ? "bg-app-warning text-app-bg" : "bg-app-accent text-app-on-accent"}`} onClick={onConfirm}>Confirm</button>
        </div>
      ) : null}
    </div>
  );
}

function TrashedInstanceRow({
  instance,
  deleting,
  restoring,
  confirming,
  confirmationName,
  onConfirmationName,
  onRestore,
  onRequestDelete,
  onCancelDelete,
  onDelete,
}: {
  instance: TrashedInstance;
  deleting: boolean;
  restoring: boolean;
  confirming: boolean;
  confirmationName: string;
  onConfirmationName: (value: string) => void;
  onRestore: () => void;
  onRequestDelete: () => void;
  onCancelDelete: () => void;
  onDelete: () => void;
}) {
  const busy = deleting || restoring;
  return (
    <div className="border-b border-app-separator/55 last:border-0">
      <div className="grid min-h-[72px] grid-cols-[44px_minmax(0,1fr)_150px_140px_180px] items-center gap-4 px-5 py-3">
        <ContentArtwork src={instance.iconUrl} name={instance.name} className="size-11 rounded-control" />
        <span className="min-w-0">
          <strong className="block truncate text-xs font-bold">{instance.name}</strong>
          <span className="mt-1 block truncate font-mono text-[9px] text-app-muted">
            Minecraft {instance.minecraftVersion} · {loaderLabel(instance.loaderKind)}{instance.loaderVersion ? ` ${instance.loaderVersion}` : ""}
          </span>
          {instance.sourceName && instance.sourceName !== instance.name ? <span className="mt-1 block truncate text-[9px] text-app-secondary">{instance.sourceName}</span> : null}
        </span>
        <span>
          <strong className="block font-mono text-[11px]">{formatBytes(instance.sizeBytes)}</strong>
          <span className="mt-1 block font-mono text-[9px] text-app-muted">{formatCount(instance.fileCount, "file")}</span>
        </span>
        <span className="text-[10px] text-app-secondary">
          <span className="block">Deleted {formatDate(instance.trashedAt)}</span>
          {!instance.filesPresent ? <span className="mt-1 block text-app-warning">Files missing</span> : null}
        </span>
        <span className="flex justify-end gap-2">
          <button type="button" className="inline-flex h-8 items-center gap-1.5 rounded-control border border-app-separator px-3 text-[11px] font-bold text-app-secondary hover:border-app-accent/45 hover:text-app-text disabled:opacity-45" disabled={busy} onClick={onRestore}>
            {restoring ? <LoaderCircle size={13} className="animate-spin" /> : <RotateCcw size={13} />}Restore
          </button>
          <button type="button" className="inline-flex size-8 items-center justify-center rounded-control border border-app-danger/30 text-app-danger hover:bg-app-danger/10 disabled:opacity-45" disabled={busy} aria-label={`Permanently delete ${instance.name}`} onClick={onRequestDelete}>
            <Trash2 size={14} />
          </button>
        </span>
      </div>
      {confirming ? (
        <div className="flex items-center gap-4 border-t border-app-danger/30 bg-app-danger/5 px-5 py-3">
          <TriangleAlert size={17} className="shrink-0 text-app-danger" />
          <label className="min-w-0 flex-1 text-[10px] text-app-secondary">
            Enter <strong className="text-app-text">{instance.name}</strong> to permanently delete its worlds and files.
            <input
              autoFocus
              className="mt-2 h-9 w-full max-w-[420px] rounded-control border border-app-separator bg-app-bg px-3 text-xs text-app-text focus:border-app-danger focus:outline-none"
              value={confirmationName}
              onChange={(event) => onConfirmationName(event.target.value)}
            />
          </label>
          <button type="button" className="h-8 px-3 text-[11px] font-bold text-app-secondary" disabled={deleting} onClick={onCancelDelete}>Cancel</button>
          <button type="button" className="inline-flex h-8 items-center gap-2 rounded-control bg-app-danger px-3 text-[11px] font-bold text-app-bg disabled:opacity-40" disabled={deleting || confirmationName !== instance.name} onClick={onDelete}>
            {deleting ? <LoaderCircle size={13} className="animate-spin" /> : <Trash2 size={13} />}Delete permanently
          </button>
        </div>
      ) : null}
    </div>
  );
}

function StorageSkeleton() {
  return (
    <div className="grid gap-9" aria-label="Measuring storage">
      <div className="h-[420px] animate-pulse rounded-control bg-app-surface" />
      <div className="h-[220px] animate-pulse rounded-control bg-app-surface" />
    </div>
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 10 ? value.toFixed(1) : value.toFixed(2)} ${units[unit]}`;
}

function formatCount(value: number, noun: string) {
  return `${value.toLocaleString()} ${value === 1 ? noun : `${noun}s`}`;
}

function errorMessage(error: unknown) {
  return getUserFacingError(
    error,
    "slate could not complete that storage action. Refresh and try again.",
  );
}
