import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowRight,
  Copy,
  Download,
  FolderArchive,
  FolderOpen,
  Pin,
  PinOff,
  RotateCcw,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import {
  createInstanceSnapshot,
  deleteInstanceSnapshot,
  duplicateInstance,
  exportInstance,
  listInstanceSnapshots,
  moveInstanceStorage,
  openInstanceDirectory,
  restoreInstanceSnapshot,
  setInstanceSnapshotPinned,
} from "../../lib/bridge";
import { formatDate } from "../../lib/format";
import type { LauncherInstance } from "../../types/launcher";
import {
  contentErrorMessage,
  formatContentFileSize,
} from "./instanceContentFormat";

const lifecycleButtonClass =
  "inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text hover:border-app-secondary disabled:opacity-50";

export function InstanceLifecycleActions({
  instance,
}: {
  instance: LauncherInstance;
}) {
  const queryClient = useQueryClient();
  const [duplicateName, setDuplicateName] = useState(`${instance.name} copy`);
  const [copyWorlds, setCopyWorlds] = useState(true);
  const [copyScreenshots, setCopyScreenshots] = useState(false);
  const [copySettings, setCopySettings] = useState(true);
  const [restoreTarget, setRestoreTarget] = useState<string>();
  const [deleteTarget, setDeleteTarget] = useState<string>();
  const [message, setMessage] = useState<string>();
  const snapshotsQuery = useQuery({
    queryKey: ["instance-snapshots", instance.id],
    queryFn: () => listInstanceSnapshots(instance.id),
  });
  const duplicateMutation = useMutation({
    mutationFn: () =>
      duplicateInstance({
        id: instance.id,
        name: duplicateName,
        includeWorlds: copyWorlds,
        includeScreenshots: copyScreenshots,
        includeSettings: copySettings,
        expectedRevision: instance.revision,
      }),
    onSuccess: async (duplicate) => {
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setMessage(`${duplicate.name} was created and is ready to install.`);
    },
  });
  const moveMutation = useMutation({
    mutationFn: () =>
      moveInstanceStorage({
        id: instance.id,
        expectedRevision: instance.revision,
      }),
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setMessage(`Instance moved to ${updated.storagePath}`);
    },
  });
  const exportMutation = useMutation({
    mutationFn: () =>
      exportInstance({
        id: instance.id,
        expectedRevision: instance.revision,
      }),
    onSuccess: () => setMessage("Portable instance archive exported."),
  });
  const createMutation = useMutation({
    mutationFn: () =>
      createInstanceSnapshot({
        id: instance.id,
        expectedRevision: instance.revision,
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["instance-snapshots", instance.id],
      });
      setMessage("Snapshot created.");
    },
  });
  const restoreMutation = useMutation({
    mutationFn: (snapshotId: string) =>
      restoreInstanceSnapshot({
        id: instance.id,
        snapshotId,
        expectedRevision: instance.revision,
      }),
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setRestoreTarget(undefined);
      setMessage(
        "Snapshot restored. Personal game files now match that restore point.",
      );
    },
  });
  const deleteMutation = useMutation({
    mutationFn: (snapshotId: string) =>
      deleteInstanceSnapshot({ id: instance.id, snapshotId }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["instance-snapshots", instance.id],
      });
      setDeleteTarget(undefined);
    },
  });
  const pinMutation = useMutation({
    mutationFn: ({ id, pinned }: { id: string; pinned: boolean }) =>
      setInstanceSnapshotPinned({
        id: instance.id,
        snapshotId: id,
        pinned,
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["instance-snapshots", instance.id],
      });
    },
  });
  const error =
    duplicateMutation.error ??
    moveMutation.error ??
    exportMutation.error ??
    createMutation.error ??
    restoreMutation.error ??
    deleteMutation.error ??
    pinMutation.error;

  return (
    <>
      <section className="border-t border-app-separator/55 pt-5">
        <h3 className="m-0 text-xs font-bold">Open instance folders</h3>
        <p className="mt-1 mb-4 text-[11px] text-app-secondary">
          Open the folders you may need for worlds, mods, screenshots, or
          troubleshooting.
        </p>
        <div className="flex flex-wrap gap-2">
          {(
            [
              ["game", "Game"],
              ["mods", "Mods"],
              ["saves", "Worlds"],
              ["screenshots", "Screenshots"],
              ["logs", "Logs"],
              ["crashReports", "Crash reports"],
            ] as const
          ).map(([kind, label]) => (
            <button
              key={kind}
              type="button"
              className={lifecycleButtonClass}
              onClick={() => void openInstanceDirectory(instance.id, kind)}
            >
              <FolderOpen size={14} />
              {label}
            </button>
          ))}
        </div>
        <div className="mt-4 flex items-center justify-between gap-5 rounded-control border border-app-separator/60 bg-app-bg/35 px-4 py-3">
          <span className="min-w-0">
            <strong className="block text-[11px]">Storage location</strong>
            <span
              className="mt-1 block truncate font-mono text-[10px] text-app-muted"
              title={instance.storagePath}
            >
              {instance.storagePath}
            </span>
          </span>
          <button
            type="button"
            className={lifecycleButtonClass}
            disabled={moveMutation.isPending}
            onClick={() => moveMutation.mutate()}
          >
            {moveMutation.isPending ? (
              <RotateCcw className="animate-spin" size={14} />
            ) : (
              <ArrowRight size={14} />
            )}
            Move…
          </button>
        </div>
      </section>

      <section className="border-t border-app-separator/55 pt-5">
        <h3 className="m-0 text-xs font-bold">Duplicate instance</h3>
        <p className="mt-1 mb-4 text-[11px] text-app-secondary">
          Create another instance with the same game version and choose which
          personal files to copy.
        </p>
        <div className="grid grid-cols-[minmax(0,1fr)_auto] gap-3">
          <input
            className="h-9 rounded-control border border-app-separator bg-app-bg px-3 text-xs outline-none focus:border-app-accent"
            value={duplicateName}
            maxLength={80}
            onChange={(event) => setDuplicateName(event.target.value)}
          />
          <button
            type="button"
            className={lifecycleButtonClass}
            disabled={!duplicateName.trim() || duplicateMutation.isPending}
            onClick={() => duplicateMutation.mutate()}
          >
            {duplicateMutation.isPending ? (
              <RotateCcw className="animate-spin" size={14} />
            ) : (
              <Copy size={14} />
            )}
            Duplicate
          </button>
        </div>
        <div className="mt-3 flex flex-wrap gap-5 text-[11px] text-app-secondary">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={copyWorlds}
              onChange={(event) => setCopyWorlds(event.target.checked)}
            />
            Worlds
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={copyScreenshots}
              onChange={(event) => setCopyScreenshots(event.target.checked)}
            />
            Screenshots
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={copySettings}
              onChange={(event) => setCopySettings(event.target.checked)}
            />
            Settings and artwork
          </label>
        </div>
      </section>

      <section className="border-t border-app-separator/55 pt-5">
        <div className="flex items-start justify-between gap-6">
          <span>
            <h3 className="m-0 text-xs font-bold">Export instance</h3>
            <p className="mt-1 mb-0 text-[11px] text-app-secondary">
              Create an archive you can import on another computer.
            </p>
          </span>
          <span className="shrink-0">
            <button
              type="button"
              className={lifecycleButtonClass}
              disabled={exportMutation.isPending}
              onClick={() => exportMutation.mutate()}
            >
              {exportMutation.isPending ? (
                <RotateCcw className="animate-spin" size={14} />
              ) : (
                <Download size={14} />
              )}
              Export…
            </button>
          </span>
        </div>
      </section>

      <section className="border-t border-app-separator/55 pt-5">
        <div className="flex items-start justify-between gap-5">
          <span>
            <h3 className="m-0 text-xs font-bold">Snapshots</h3>
            <p className="mt-1 mb-0 text-[11px] text-app-secondary">
              Create a restore point before making major changes. Pinned
              snapshots are kept until you remove them.
            </p>
          </span>
          <button
            type="button"
            className={lifecycleButtonClass}
            disabled={createMutation.isPending}
            onClick={() => createMutation.mutate()}
          >
            {createMutation.isPending ? (
              <RotateCcw className="animate-spin" size={14} />
            ) : (
              <FolderArchive size={14} />
            )}
            Create snapshot
          </button>
        </div>
        <div className="mt-4 divide-y divide-app-separator/50 border-y border-app-separator/50">
          {snapshotsQuery.isPending ? (
            <p className="py-4 text-xs text-app-secondary">
              Loading snapshots…
            </p>
          ) : null}
          {snapshotsQuery.data?.length === 0 ? (
            <p className="py-4 text-xs text-app-secondary">No snapshots yet.</p>
          ) : null}
          {snapshotsQuery.data?.map((snapshot) => (
            <div
              key={snapshot.id}
              className="flex min-h-12 items-center gap-3 py-2"
            >
              <span className="grid size-8 place-items-center rounded-control bg-app-raised text-app-secondary">
                <FolderArchive size={14} />
              </span>
              <span className="min-w-0 flex-1">
                <strong className="block text-[11px]">
                  {formatDate(snapshot.createdAt)}
                </strong>
                <span className="font-mono text-[10px] text-app-muted">
                  {formatContentFileSize(snapshot.sizeBytes)}
                </span>
              </span>
              <button
                type="button"
                className="p-2 text-app-secondary hover:text-app-text"
                title={snapshot.pinned ? "Unpin snapshot" : "Pin snapshot"}
                onClick={() =>
                  pinMutation.mutate({
                    id: snapshot.id,
                    pinned: !snapshot.pinned,
                  })
                }
              >
                {snapshot.pinned ? <PinOff size={14} /> : <Pin size={14} />}
              </button>
              {restoreTarget === snapshot.id ? (
                <span className="flex items-center gap-2 rounded-control border border-app-warning/45 bg-app-warning/5 px-2 py-1">
                  <span className="text-[10px] text-app-secondary">
                    Replace current files?
                  </span>
                  <button
                    type="button"
                    className="text-[10px] font-bold text-app-warning"
                    disabled={restoreMutation.isPending}
                    onClick={() => restoreMutation.mutate(snapshot.id)}
                  >
                    Confirm
                  </button>
                  <button
                    type="button"
                    className="text-[10px] font-bold text-app-secondary"
                    disabled={restoreMutation.isPending}
                    onClick={() => setRestoreTarget(undefined)}
                  >
                    Cancel
                  </button>
                </span>
              ) : (
                <button
                  type="button"
                  className="text-[11px] font-bold text-app-accent"
                  disabled={restoreMutation.isPending}
                  onClick={() => {
                    setDeleteTarget(undefined);
                    setRestoreTarget(snapshot.id);
                  }}
                >
                  Restore
                </button>
              )}
              {deleteTarget === snapshot.id ? (
                <span className="flex items-center gap-2 rounded-control border border-app-danger/45 bg-app-danger/5 px-2 py-1">
                  <span className="text-[10px] text-app-secondary">
                    Delete permanently?
                  </span>
                  <button
                    type="button"
                    className="text-[10px] font-bold text-app-danger"
                    disabled={deleteMutation.isPending}
                    onClick={() => deleteMutation.mutate(snapshot.id)}
                  >
                    Delete
                  </button>
                  <button
                    type="button"
                    className="text-[10px] font-bold text-app-secondary"
                    disabled={deleteMutation.isPending}
                    onClick={() => setDeleteTarget(undefined)}
                  >
                    Cancel
                  </button>
                </span>
              ) : (
                <button
                  type="button"
                  className="p-2 text-app-muted hover:text-app-danger"
                  title="Delete snapshot"
                  disabled={deleteMutation.isPending}
                  onClick={() => {
                    setRestoreTarget(undefined);
                    setDeleteTarget(snapshot.id);
                  }}
                >
                  <Trash2 size={14} />
                </button>
              )}
            </div>
          ))}
        </div>
      </section>

      {error || message ? (
        <p
          className={`m-0 text-xs ${error ? "text-app-danger" : "text-app-secondary"}`}
          role={error ? "alert" : "status"}
        >
          {error ? contentErrorMessage(error) : message}
        </p>
      ) : null}
    </>
  );
}
