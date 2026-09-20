import { useQuery } from "@tanstack/react-query";
import {
  LoaderCircle,
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
import {
  listInstanceContentHistory,
  listInstanceContentVersions,
} from "../../lib/bridge";
import { formatDate } from "../../lib/format";
import type {
  InstanceContentFile,
  InstanceContentKind,
} from "../../types/launcher";
import { formatContentFileSize } from "./instanceContentFormat";

export function InstalledContentRow({
  item,
  instanceId,
  kind,
  disabled,
  confirmingRemove,
  onToggle,
  onPin,
  onUpdate,
  onRequestRemove,
  onCancelRemove,
  onConfirmRemove,
}: {
  item: InstanceContentFile;
  instanceId: string;
  kind: InstanceContentKind;
  disabled: boolean;
  confirmingRemove: boolean;
  onToggle: () => void;
  onPin: () => void;
  onUpdate: (versionId: string) => void;
  onRequestRemove: () => void;
  onCancelRemove: () => void;
  onConfirmRemove: () => void;
}) {
  const [versionOpen, setVersionOpen] = useState(false);
  const [selectedVersionId, setSelectedVersionId] = useState(
    item.versionId ?? "",
  );
  const linked = Boolean(item.provider && item.projectId);
  const versionQuery = useQuery({
    queryKey: [
      "instance-content-versions",
      instanceId,
      kind,
      item.provider,
      item.projectId,
    ],
    queryFn: () => {
      if (!item.provider || !item.projectId) {
        throw new Error("Versions are not available for this content file.");
      }
      return listInstanceContentVersions({
        instanceId,
        kind,
        provider: item.provider,
        projectId: item.projectId,
      });
    },
    enabled: versionOpen && linked,
    staleTime: 5 * 60_000,
  });
  const historyQuery = useQuery({
    queryKey: [
      "instance-content-history",
      instanceId,
      kind,
      item.provider,
      item.projectId,
    ],
    queryFn: () => {
      if (!item.provider || !item.projectId) {
        throw new Error(
          "Version history is not available for this content file.",
        );
      }
      return listInstanceContentHistory({
        instanceId,
        kind,
        provider: item.provider,
        projectId: item.projectId,
      });
    },
    enabled: versionOpen && linked,
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
      <tr className="text-[11px]">
        <td className="px-5 py-3">
          <div className="flex min-w-0 items-center gap-3">
            {item.iconUrl ? (
              <ContentArtwork
                src={item.iconUrl}
                name={item.displayName}
                stableKey={`${item.provider}:${item.projectId}`}
                className="size-9 shrink-0 rounded-control border border-app-separator"
              />
            ) : null}
            <span className="min-w-0">
              <strong className="block truncate text-xs text-app-text">
                {item.displayName}
              </strong>
              <span
                className="mt-0.5 block truncate font-mono text-[9px] text-app-muted"
                title={item.filePath}
              >
                {item.filePath}
              </span>
            </span>
          </div>
        </td>
        <td className="px-3 py-3 text-app-secondary">
          {item.worldName ?? "Instance"}
        </td>
        <td className="px-3 py-3">
          <span
            className={`rounded-full border px-2 py-1 font-mono text-[9px] ${item.origin === "modpack" ? "border-app-accent/35 text-app-accent" : "border-app-separator text-app-secondary"}`}
          >
            {item.origin === "modpack" ? "Included with pack" : "Added by you"}
          </span>
          {item.pinned ? (
            <span className="mt-1 block text-[9px] font-bold text-app-accent">
              Version pinned
            </span>
          ) : null}
        </td>
        <td className="px-3 py-3 font-mono text-[10px] text-app-secondary">
          {item.fileSize ? formatContentFileSize(item.fileSize) : "Folder"}
        </td>
        <td className="px-3 py-3">
          <div className="flex items-center justify-end gap-1.5">
            {linked ? (
              <>
                <button
                  type="button"
                  className={`inline-flex size-7 items-center justify-center rounded-control border bg-app-bg disabled:opacity-45 ${versionOpen ? "border-app-accent/40 text-app-accent" : "border-app-separator text-app-secondary hover:border-app-accent/45 hover:text-app-accent"}`}
                  disabled={disabled}
                  onClick={() => {
                    setSelectedVersionId(item.versionId ?? "");
                    setVersionOpen((open) => !open);
                  }}
                  aria-label={`Change version of ${item.displayName}`}
                  title="Change installed version"
                >
                  <RefreshCw size={13} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className={`inline-flex size-7 items-center justify-center rounded-control border bg-app-bg disabled:opacity-45 ${item.pinned ? "border-app-accent/40 text-app-accent" : "border-app-separator text-app-secondary hover:border-app-accent/45 hover:text-app-accent"}`}
                  disabled={disabled}
                  onClick={onPin}
                  aria-label={`${item.pinned ? "Unpin" : "Pin"} ${item.displayName}`}
                  title={
                    item.pinned
                      ? "Allow compatible updates"
                      : "Keep this version"
                  }
                >
                  {item.pinned ? <PinOff size={13} /> : <Pin size={13} />}
                </button>
              </>
            ) : null}
            {item.canToggle ? (
              <button
                type="button"
                className="inline-flex size-7 items-center justify-center rounded-control border border-app-separator bg-app-bg text-app-secondary hover:text-app-text disabled:opacity-45"
                disabled={disabled}
                onClick={onToggle}
                aria-label={`${item.enabled ? "Hide" : "Restore"} ${item.displayName}`}
                title={item.enabled ? "Hide from Minecraft" : "Restore"}
              >
                <Power size={13} aria-hidden="true" />
              </button>
            ) : (
              <span className="px-1 text-[9px] text-app-muted">In-game</span>
            )}
            {confirmingRemove ? (
              <span className="flex items-center gap-2">
                <button
                  type="button"
                  className="text-[10px] font-bold text-app-danger"
                  disabled={disabled}
                  onClick={onConfirmRemove}
                >
                  Confirm
                </button>
                <button
                  type="button"
                  className="text-[10px] text-app-secondary"
                  onClick={onCancelRemove}
                >
                  Cancel
                </button>
              </span>
            ) : (
              <button
                type="button"
                className="p-1.5 text-app-muted hover:text-app-danger"
                title={`Remove ${item.displayName}`}
                disabled={disabled}
                onClick={onRequestRemove}
              >
                <Trash2 size={14} />
              </button>
            )}
          </div>
        </td>
      </tr>
      {versionOpen ? (
        <tr className="border-b border-app-accent/20 bg-app-accent/5">
          <td colSpan={5} className="px-5 py-4">
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
              <div>
                <p className="m-0 text-[10px] font-bold text-app-secondary">
                  Recent versions
                </p>
                <div className="mt-2 flex flex-wrap gap-2">
                  {historyQuery.isPending ? (
                    <span className="inline-flex items-center gap-1.5 text-[9px] text-app-muted">
                      <LoaderCircle size={11} className="animate-spin" />{" "}
                      Loading
                    </span>
                  ) : recentVersions.length ? (
                    recentVersions.map((history) => (
                      <button
                        key={`${history.versionId}:${history.changedAt}`}
                        type="button"
                        className="inline-flex items-center gap-1.5 rounded-full border border-app-separator px-2 py-1 font-mono text-[9px] text-app-secondary hover:border-app-accent/45 hover:text-app-accent"
                        onClick={() => setSelectedVersionId(history.versionId)}
                        title={`Used ${formatDate(history.changedAt)}`}
                      >
                        <RotateCcw size={10} /> {history.versionId}
                      </button>
                    ))
                  ) : (
                    <span className="text-[9px] text-app-muted">
                      No earlier versions recorded
                    </span>
                  )}
                </div>
              </div>
              <button
                type="button"
                className="inline-flex h-9 items-center justify-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:bg-app-raised disabled:text-app-muted"
                disabled={
                  disabled ||
                  !activeVersionId ||
                  activeVersionId === item.versionId ||
                  versionQuery.isPending
                }
                onClick={() => onUpdate(activeVersionId)}
              >
                <RefreshCw size={13} /> Apply version
              </button>
            </div>
            {versionQuery.isError ? (
              <p className="mt-3 mb-0 text-[10px] text-app-danger">
                Compatible versions could not be loaded. Try again.
              </p>
            ) : null}
          </td>
        </tr>
      ) : null}
    </>
  );
}
