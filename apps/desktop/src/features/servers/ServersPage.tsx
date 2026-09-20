import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Check,
  Gauge,
  LoaderCircle,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Trash2,
  Users,
  X,
} from "lucide-react";
import { useMemo, useState, type CSSProperties, type FormEvent } from "react";
import {
  EmptyState,
  InlineNotice,
  PageHeader,
  StatusPill,
} from "../../components/PageScaffold";
import {
  createSavedServer,
  launchInstance,
  listAccounts,
  listGameSessions,
  listInstances,
  listSavedServers,
  pingServer,
  removeSavedServer,
  updateSavedServer,
} from "../../lib/bridge";
import { loaderLabel } from "../../lib/format";
import { getUserFacingError } from "../../lib/userFacingError";
import type {
  LauncherInstance,
  MinecraftAccount,
  SavedServer,
  ServerStatus,
  ServerTextSegment,
} from "../../types/launcher";

const primaryButtonClass =
  "inline-flex h-9 items-center justify-center gap-2 rounded-control border-0 bg-app-accent px-4 text-xs font-bold text-app-on-accent hover:brightness-105 disabled:cursor-not-allowed disabled:opacity-55";
const secondaryButtonClass =
  "inline-flex h-9 items-center justify-center gap-2 rounded-control border border-app-separator bg-app-raised px-3.5 text-xs font-bold text-app-text hover:border-app-secondary disabled:cursor-not-allowed disabled:opacity-50";
const iconButtonClass =
  "inline-flex size-9 items-center justify-center rounded-control border border-app-separator bg-app-raised text-app-secondary hover:border-app-secondary hover:text-app-text disabled:cursor-not-allowed disabled:opacity-45";
const inputClass =
  "mt-2 h-10 w-full rounded-control border border-app-separator bg-app-bg px-3 text-[13px] text-app-text placeholder:text-app-muted focus:border-app-accent focus:outline-none";

export function ServersPage() {
  const queryClient = useQueryClient();
  const [editingId, setEditingId] = useState<string | "new">();
  const serversQuery = useQuery({
    queryKey: ["saved-servers"],
    queryFn: listSavedServers,
  });
  const instancesQuery = useQuery({
    queryKey: ["instances"],
    queryFn: listInstances,
  });
  const accountsQuery = useQuery({
    queryKey: ["minecraft-accounts"],
    queryFn: listAccounts,
  });
  const sessionsQuery = useQuery({
    queryKey: ["game-sessions"],
    queryFn: listGameSessions,
    refetchInterval: 1_000,
  });
  const servers = serversQuery.data ?? [];
  const editingServer =
    editingId && editingId !== "new"
      ? servers.find((server) => server.id === editingId)
      : undefined;

  const saveMutation = useMutation({
    mutationFn: async (input: ServerDraft) => {
      if (editingServer) {
        return updateSavedServer({ id: editingServer.id, ...input });
      }
      return createSavedServer(input);
    },
    onSuccess: async () => {
      setEditingId(undefined);
      await queryClient.invalidateQueries({ queryKey: ["saved-servers"] });
    },
  });

  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Servers"
        title="Saved servers"
        description="Keep favorite servers close and open them with the right Minecraft instance."
        actions={
          <button
            type="button"
            className={editingId ? secondaryButtonClass : primaryButtonClass}
            onClick={() => {
              saveMutation.reset();
              setEditingId((current) => (current ? undefined : "new"));
            }}
          >
            {editingId ? <X size={15} /> : <Plus size={16} />}
            {editingId ? "Close" : "Add server"}
          </button>
        }
      />

      <div className="mx-auto max-w-[1040px] px-8 py-7">
        {editingId ? (
          <ServerForm
            key={editingServer?.id ?? "new"}
            server={editingServer}
            instances={instancesQuery.data ?? []}
            pending={saveMutation.isPending}
            error={
              saveMutation.isError
                ? getUserFacingError(
                    saveMutation.error,
                    "That server could not be saved. Check the details and try again.",
                  )
                : undefined
            }
            onCancel={() => setEditingId(undefined)}
            onSubmit={(input) => saveMutation.mutate(input)}
          />
        ) : null}

        {serversQuery.isPending ? (
          <div className="grid gap-3" aria-label="Loading saved servers">
            <div className="h-28 animate-pulse rounded-control bg-app-surface" />
            <div className="h-28 animate-pulse rounded-control bg-app-surface" />
          </div>
        ) : serversQuery.isError ? (
          <EmptyState
            error
            title="Saved servers are unavailable"
            description="Restart slate and try again. Your instances were not changed."
          />
        ) : servers.length === 0 && !editingId ? (
          <EmptyState
            title="No saved servers"
            description="Add a server once, then choose which instance to use whenever you join."
            action={
              <button
                type="button"
                className={primaryButtonClass}
                onClick={() => setEditingId("new")}
              >
                <Plus size={16} /> Add your first server
              </button>
            }
          />
        ) : (
          <div className="grid gap-3">
            {servers.map((server) => (
              <SavedServerRow
                key={server.id}
                server={server}
                instances={instancesQuery.data ?? []}
                defaultAccount={
                  accountsQuery.data?.find((account) => account.isDefault) ??
                  accountsQuery.data?.[0]
                }
                runningInstanceIds={
                  new Set(
                    (sessionsQuery.data ?? []).map(
                      (session) => session.instanceId,
                    ),
                  )
                }
                onEdit={() => {
                  saveMutation.reset();
                  setEditingId(server.id);
                }}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

type ServerDraft = {
  name: string;
  address: string;
  preferredInstanceId?: string;
};

function ServerForm({
  server,
  instances,
  pending,
  error,
  onCancel,
  onSubmit,
}: {
  server?: SavedServer;
  instances: LauncherInstance[];
  pending: boolean;
  error?: string;
  onCancel: () => void;
  onSubmit: (input: ServerDraft) => void;
}) {
  const [name, setName] = useState(server?.name ?? "");
  const [address, setAddress] = useState(server?.address ?? "");
  const [preferredInstanceId, setPreferredInstanceId] = useState(
    server?.preferredInstanceId ?? "",
  );
  const valid = name.trim().length > 0 && address.trim().length > 0;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!valid) return;
    onSubmit({
      name: name.trim(),
      address: address.trim(),
      preferredInstanceId: preferredInstanceId || undefined,
    });
  };

  return (
    <form
      className="mb-6 border-b border-app-separator/60 pb-6"
      onSubmit={submit}
    >
      <div className="mb-4 flex items-center justify-between gap-4">
        <div>
          <h2 className="m-0 text-lg font-bold">
            {server ? "Edit server" : "Add a server"}
          </h2>
          <p className="mt-1 mb-0 text-xs text-app-secondary">
            The instance choice can be changed before joining.
          </p>
        </div>
      </div>
      <div className="grid grid-cols-[minmax(180px,.8fr)_minmax(240px,1.2fr)_minmax(220px,1fr)] gap-4 max-[900px]:grid-cols-1">
        <label className="text-xs font-bold text-app-text">
          Name
          <input
            className={inputClass}
            value={name}
            maxLength={80}
            autoFocus
            placeholder="My server"
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="text-xs font-bold text-app-text">
          Address
          <input
            className={inputClass}
            value={address}
            maxLength={255}
            spellCheck={false}
            placeholder="play.example.com"
            onChange={(event) => setAddress(event.target.value)}
          />
        </label>
        <label className="text-xs font-bold text-app-text">
          Preferred instance
          <select
            className={inputClass}
            value={preferredInstanceId}
            onChange={(event) => setPreferredInstanceId(event.target.value)}
          >
            <option value="">Choose when joining</option>
            {instances.map((instance) => (
              <option key={instance.id} value={instance.id}>
                {instance.name} · {instance.minecraftVersion}
              </option>
            ))}
          </select>
        </label>
      </div>
      {error ? (
        <div className="mt-4">
          <InlineNotice tone="danger" title="Server was not saved">
            {error}
          </InlineNotice>
        </div>
      ) : null}
      <div className="mt-5 flex justify-end gap-2">
        <button
          type="button"
          className={secondaryButtonClass}
          onClick={onCancel}
        >
          Cancel
        </button>
        <button
          type="submit"
          className={primaryButtonClass}
          disabled={!valid || pending}
        >
          {pending ? (
            <LoaderCircle className="animate-spin" size={15} />
          ) : (
            <Check size={15} />
          )}
          {pending ? "Saving…" : "Save server"}
        </button>
      </div>
    </form>
  );
}

function SavedServerRow({
  server,
  instances,
  defaultAccount,
  runningInstanceIds,
  onEdit,
}: {
  server: SavedServer;
  instances: LauncherInstance[];
  defaultAccount?: MinecraftAccount;
  runningInstanceIds: Set<string>;
  onEdit: () => void;
}) {
  const queryClient = useQueryClient();
  const [confirmRemove, setConfirmRemove] = useState(false);
  const statusQuery = useQuery({
    queryKey: ["server-status", server.address],
    queryFn: () => pingServer(server.address),
    retry: false,
    refetchInterval: 30_000,
  });
  const readyInstances = useMemo(
    () => instances.filter((instance) => instance.setupState === "ready"),
    [instances],
  );
  const automaticInstance = readyInstances[0];
  const [selectedOverride, setSelectedOverride] = useState(
    server.preferredInstanceId ?? "",
  );
  const selectedInstanceId = selectedOverride || automaticInstance?.id || "";

  const preferenceMutation = useMutation({
    mutationFn: (preferredInstanceId: string) =>
      updateSavedServer({
        id: server.id,
        name: server.name,
        address: server.address,
        preferredInstanceId: preferredInstanceId || undefined,
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["saved-servers"] });
    },
  });
  const removeMutation = useMutation({
    mutationFn: () => removeSavedServer(server.id),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["saved-servers"] });
    },
  });
  const launchMutation = useMutation({
    mutationFn: () => {
      if (!selectedInstanceId || !defaultAccount) {
        throw new Error("Choose an instance and connect a Minecraft account.");
      }
      return launchInstance(
        selectedInstanceId,
        defaultAccount.id,
        server.address,
      );
    },
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["game-sessions"] }),
        queryClient.invalidateQueries({ queryKey: ["instances"] }),
      ]);
    },
  });
  const selectedInstance = instances.find(
    (instance) => instance.id === selectedInstanceId,
  );
  const running = selectedInstanceId
    ? runningInstanceIds.has(selectedInstanceId)
    : false;
  const joinDisabled =
    !statusQuery.data?.online ||
    !selectedInstance ||
    !defaultAccount ||
    running ||
    launchMutation.isPending;

  return (
    <article className="overflow-hidden rounded-control border border-app-separator/70 bg-app-surface">
      <div className="grid grid-cols-[72px_minmax(0,1fr)_minmax(270px,320px)] gap-5 p-5 max-[860px]:grid-cols-[72px_minmax(0,1fr)]">
        <ServerIcon
          status={statusQuery.data}
          name={server.name}
          loading={statusQuery.isPending}
        />
        <div className="min-w-0 py-0.5">
          <div className="flex flex-wrap items-center gap-2">
            <strong className="overflow-hidden text-[15px] font-bold text-ellipsis whitespace-nowrap">
              {server.name}
            </strong>
            <ServerStatusPill query={statusQuery} />
          </div>
          <small className="mt-1 block overflow-hidden font-mono text-[10px] text-app-muted text-ellipsis whitespace-nowrap">
            {server.address}
          </small>
          <ServerMotd status={statusQuery.data} />
          <ServerFacts status={statusQuery.data} />
        </div>

        <div className="border-s border-app-separator/60 ps-5 max-[860px]:col-span-2 max-[860px]:border-s-0 max-[860px]:border-t max-[860px]:pt-4 max-[860px]:ps-0">
          <div className="mb-3 flex items-center justify-between gap-3">
            <span className="text-[10px] font-bold tracking-[.06em] text-app-muted uppercase">
              Join this server
            </span>
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                className={iconButtonClass}
                aria-label={`Refresh ${server.name}`}
                title="Refresh status"
                disabled={statusQuery.isFetching}
                onClick={() => void statusQuery.refetch()}
              >
                <RefreshCw
                  className={statusQuery.isFetching ? "animate-spin" : ""}
                  size={15}
                />
              </button>
              <button
                type="button"
                className={iconButtonClass}
                aria-label={`Edit ${server.name}`}
                title="Edit server"
                onClick={onEdit}
              >
                <Pencil size={15} />
              </button>
              <button
                type="button"
                className={iconButtonClass}
                aria-label={`Remove ${server.name}`}
                title="Remove server"
                onClick={() => setConfirmRemove(true)}
              >
                <Trash2 size={15} />
              </button>
            </div>
          </div>

          <label className="sr-only" htmlFor={`server-instance-${server.id}`}>
            Instance for {server.name}
          </label>
          <select
            id={`server-instance-${server.id}`}
            className="h-9 w-full rounded-control border border-app-separator bg-app-bg px-3 text-xs text-app-text focus:border-app-accent focus:outline-none disabled:opacity-50"
            value={selectedInstanceId}
            disabled={
              readyInstances.length === 0 || preferenceMutation.isPending
            }
            onChange={(event) => {
              const value = event.target.value;
              setSelectedOverride(value);
              preferenceMutation.mutate(value);
            }}
          >
            {readyInstances.length === 0 ? (
              <option value="">No playable instances</option>
            ) : null}
            {readyInstances.map((instance) => (
              <option key={instance.id} value={instance.id}>
                {instance.name} · {instance.minecraftVersion}
              </option>
            ))}
          </select>

          <div className="mt-2 flex min-h-4 items-center justify-between gap-3">
            <span className="text-[10px] text-app-muted">
              Any playable instance can be used
            </span>
            {selectedInstance ? (
              <span className="truncate font-mono text-[10px] text-app-muted">
                {loaderLabel(selectedInstance.loaderKind)}
              </span>
            ) : null}
          </div>

          <button
            type="button"
            className={`${primaryButtonClass} mt-3 w-full`}
            disabled={joinDisabled}
            title={joinReason({
              online: statusQuery.data?.online,
              selectedInstance,
              defaultAccount,
              running,
            })}
            onClick={() => launchMutation.mutate()}
          >
            {launchMutation.isPending ? (
              <LoaderCircle className="animate-spin" size={15} />
            ) : (
              <Play size={15} fill="currentColor" />
            )}
            {launchMutation.isPending ? "Starting…" : "Join server"}
          </button>
        </div>
      </div>

      {confirmRemove ? (
        <div className="mt-4 flex items-center justify-between gap-4 border-t border-app-separator/55 pt-4">
          <p className="m-0 text-xs text-app-secondary">
            Remove {server.name} from your saved servers?
          </p>
          <div className="flex gap-2">
            <button
              type="button"
              className={secondaryButtonClass}
              onClick={() => setConfirmRemove(false)}
            >
              Keep
            </button>
            <button
              type="button"
              className="inline-flex h-9 items-center gap-2 rounded-control border-0 bg-app-danger px-4 text-xs font-bold text-app-bg disabled:opacity-50"
              disabled={removeMutation.isPending}
              onClick={() => removeMutation.mutate()}
            >
              <Trash2 size={14} /> Remove
            </button>
          </div>
        </div>
      ) : null}

      {launchMutation.isError ||
      preferenceMutation.isError ||
      removeMutation.isError ? (
        <div className="mt-4">
          <InlineNotice tone="danger" title="Server action did not complete">
            {getUserFacingError(
              launchMutation.error ??
                preferenceMutation.error ??
                removeMutation.error,
              "Try again. If the problem continues, restart slate.",
            )}
          </InlineNotice>
        </div>
      ) : null}
    </article>
  );
}

function ServerIcon({
  status,
  name,
  loading,
}: {
  status?: ServerStatus;
  name: string;
  loading: boolean;
}) {
  if (loading) {
    return (
      <span
        className="inline-flex size-[72px] animate-pulse rounded-control border border-app-separator bg-app-raised"
        aria-label={`Loading ${name} server icon`}
      />
    );
  }
  if (status?.favicon) {
    return (
      <img
        src={status.favicon}
        alt=""
        className="size-[72px] rounded-control border border-app-separator bg-app-raised object-cover [image-rendering:pixelated]"
      />
    );
  }
  const initials = name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0])
    .join("")
    .toLocaleUpperCase();
  return (
    <span
      className="inline-flex size-[72px] items-center justify-center rounded-control border border-app-separator bg-app-accent/10 font-mono text-base font-bold tracking-[.08em] text-app-accent"
      aria-label={`${name} server icon`}
    >
      <span aria-hidden="true">{initials || "MC"}</span>
    </span>
  );
}

function ServerMotd({ status }: { status?: ServerStatus }) {
  if (!status?.description) {
    return (
      <p className="mt-4 mb-0 min-h-[34px] text-[11px]/[17px] text-app-muted">
        {status?.online === false
          ? "Server did not respond."
          : "Checking server message…"}
      </p>
    );
  }
  if (status.descriptionSegments.length === 0) {
    return (
      <p className="mt-4 mb-0 line-clamp-2 min-h-[34px] font-mono text-[11px]/[17px] text-app-secondary">
        {status.description}
      </p>
    );
  }
  return (
    <p
      className="mt-4 mb-0 line-clamp-2 min-h-[34px] whitespace-pre-wrap font-mono text-[11px]/[17px]"
      aria-label={status.description}
    >
      {status.descriptionSegments.map((segment, index) => (
        <span
          key={`${index}:${segment.text}`}
          aria-hidden="true"
          style={segmentStyle(segment)}
        >
          {segment.obfuscated ? segment.text.replace(/\S/g, "█") : segment.text}
        </span>
      ))}
    </p>
  );
}

function ServerFacts({ status }: { status?: ServerStatus }) {
  if (!status?.online) return null;
  const players =
    status.playersOnline !== undefined
      ? status.playersMax !== undefined
        ? `${formatCount(status.playersOnline)} / ${formatCount(status.playersMax)}`
        : formatCount(status.playersOnline)
      : undefined;

  return (
    <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-[10px] text-app-muted">
      {players ? (
        <span className="inline-flex items-center gap-1.5 font-mono">
          <Users size={12} aria-hidden="true" />
          {players} players
        </span>
      ) : null}
      {status.latencyMs !== undefined ? (
        <span className="inline-flex items-center gap-1.5 font-mono">
          <Gauge size={12} aria-hidden="true" />
          {status.latencyMs} ms
        </span>
      ) : null}
      {status.versionName ? (
        <span
          className="max-w-full truncate font-mono"
          title={status.versionName}
        >
          {status.versionName}
        </span>
      ) : null}
    </div>
  );
}

function formatCount(value: number) {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 }).format(
    value,
  );
}

function segmentStyle(segment: ServerTextSegment): CSSProperties {
  const decorations = [
    segment.underlined ? "underline" : "",
    segment.strikethrough ? "line-through" : "",
  ].filter(Boolean);
  return {
    color: segment.color,
    fontWeight: segment.bold ? 700 : undefined,
    fontStyle: segment.italic ? "italic" : undefined,
    textDecoration: decorations.length > 0 ? decorations.join(" ") : undefined,
    textShadow: segment.color ? "0 1px 1px rgba(0, 0, 0, 0.72)" : undefined,
  };
}

function ServerStatusPill({
  query,
}: {
  query: ReturnType<typeof useQuery<ServerStatus>>;
}) {
  if (query.isPending) {
    return <StatusPill tone="neutral">Checking</StatusPill>;
  }
  if (!query.data?.online) {
    return <StatusPill tone="danger">Offline</StatusPill>;
  }
  return <StatusPill tone="positive">Online</StatusPill>;
}

function joinReason({
  online,
  selectedInstance,
  defaultAccount,
  running,
}: {
  online?: boolean;
  selectedInstance?: LauncherInstance;
  defaultAccount?: MinecraftAccount;
  running: boolean;
}) {
  if (!online) return "The server is offline.";
  if (!selectedInstance) return "Install an instance before joining.";
  if (!defaultAccount) return "Connect a Minecraft account before joining.";
  if (running) return "This instance is already running.";
  return `Join with ${selectedInstance.name}`;
}
