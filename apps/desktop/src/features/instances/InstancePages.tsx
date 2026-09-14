import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import {
  ArrowLeft,
  Box,
  Check,
  Download,
  FolderArchive,
  Heart,
  Layers3,
  LoaderCircle,
  Play,
  RotateCcw,
  Save,
  Settings2,
  Square,
  Trash2,
  UserRound,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { ComboBox } from "../../components/ComboBox";
import { InstallProgressIndicator } from "../../components/InstallProgressIndicator";
import { SessionLogPanel } from "../../components/SessionLogPanel";
import {
  EmptyState,
  InlineNotice,
  StatusPill,
} from "../../components/PageScaffold";
import {
  forceStopGameSession,
  getInstance,
  getLoaderVersionCatalog,
  getMinecraftVersionCatalog,
  installInstance,
  launchInstance,
  listAccounts,
  listGameSessions,
  listInstallJobs,
  renameInstance,
  setInstanceFavorite,
  trashInstance,
  updateInstanceConfiguration,
} from "../../lib/bridge";
import {
  formatDate,
  instanceVersionLine,
  loaderLabel,
  setupStateLabel,
  setupStateTone,
} from "../../lib/format";
import type {
  GameSession,
  LauncherInstance,
  LoaderKind,
} from "../../types/launcher";

type InstanceSection = "overview" | "content" | "settings";

export function InstanceOverviewPage() {
  return <InstancePage section="overview" />;
}

export function InstanceContentPage() {
  return <InstancePage section="content" />;
}

export function InstanceSettingsPage() {
  return <InstancePage section="settings" />;
}

function InstancePage({ section }: { section: InstanceSection }) {
  const { instanceId } = useParams({ strict: false });
  const query = useQuery({
    queryKey: ["instance", instanceId],
    queryFn: () => getInstance(instanceId ?? ""),
    enabled: Boolean(instanceId),
  });

  if (query.isPending) {
    return (
      <div className="p-8" aria-label="Loading instance">
        <div className="h-[150px] animate-pulse rounded-control bg-app-surface" />
        <div className="mt-6 h-[320px] animate-pulse rounded-control bg-app-surface" />
      </div>
    );
  }
  if (query.isError || !query.data) {
    return (
      <EmptyState
        error
        title="That instance is unavailable"
        description="It may have been moved to trash or changed outside this view."
        action={
          <Link
            to="/library"
            className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text no-underline"
          >
            <ArrowLeft size={16} aria-hidden="true" />
            Return to library
          </Link>
        }
      />
    );
  }

  return (
    <div className="min-h-full bg-app-bg">
      <InstanceHeader instance={query.data} section={section} />
      <div className="px-8 py-7">
        {section === "overview" ? (
          <Overview instance={query.data} />
        ) : section === "content" ? (
          <Content instance={query.data} />
        ) : (
          <InstanceSettings instance={query.data} />
        )}
      </div>
    </div>
  );
}

function InstanceHeader({
  instance,
  section,
}: {
  instance: LauncherInstance;
  section: InstanceSection;
}) {
  const tabs: Array<{ id: InstanceSection; label: string; icon: typeof Box }> =
    [
      { id: "overview", label: "Overview", icon: Box },
      { id: "content", label: "Content", icon: Layers3 },
      { id: "settings", label: "Settings", icon: Settings2 },
    ];
  return (
    <header className="border-b border-app-separator/55 bg-app-sidebar/45 px-8 pt-7">
      <Link
        to="/library"
        className="mb-5 inline-flex items-center gap-2 text-[11px] font-bold text-app-secondary no-underline hover:text-app-text"
      >
        <ArrowLeft size={15} aria-hidden="true" />
        All instances
      </Link>
      <div className="flex items-center gap-4">
        <span className="inline-flex size-14 items-center justify-center rounded-control border border-app-separator bg-app-raised text-app-accent">
          <Box size={26} aria-hidden="true" />
        </span>
        <div className="min-w-0">
          <div className="flex items-center gap-3">
            <h1 className="m-0 overflow-hidden text-[28px]/[34px] font-bold tracking-[-.035em] text-ellipsis whitespace-nowrap">
              {instance.name}
            </h1>
            <StatusPill tone={setupStateTone(instance.setupState)}>
              {setupStateLabel(instance.setupState)}
            </StatusPill>
          </div>
          <p className="mt-1 mb-0 font-mono text-[11px] text-app-secondary">
            {instanceVersionLine(instance)}
          </p>
        </div>
      </div>
      <nav className="mt-6 flex" aria-label="Instance sections">
        {tabs.map((tab) => {
          const Icon = tab.icon;
          return (
            <Link
              key={tab.id}
              to={`/instances/$instanceId/${tab.id}`}
              params={{ instanceId: instance.id }}
              className={`inline-flex h-11 items-center gap-2 border-b-2 px-4 text-xs font-bold no-underline ${
                section === tab.id
                  ? "border-app-accent text-app-text"
                  : "border-transparent text-app-muted hover:text-app-text"
              }`}
            >
              <Icon size={16} aria-hidden="true" />
              {tab.label}
            </Link>
          );
        })}
      </nav>
    </header>
  );
}

function Overview({ instance }: { instance: LauncherInstance }) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [name, setName] = useState(instance.name);
  const [confirmTrash, setConfirmTrash] = useState(false);
  const [confirmStop, setConfirmStop] = useState(false);
  const [selectedAccountId, setSelectedAccountId] = useState("");
  const [trackedLogSession, setTrackedLogSession] = useState<
    GameSession | undefined
  >();
  const accountsQuery = useQuery({
    queryKey: ["minecraft-accounts"],
    queryFn: listAccounts,
  });
  const jobsQuery = useQuery({
    queryKey: ["install-jobs"],
    queryFn: listInstallJobs,
    refetchInterval: 1_000,
  });
  const sessionsQuery = useQuery({
    queryKey: ["game-sessions"],
    queryFn: listGameSessions,
    refetchInterval: 750,
  });
  const installJob = jobsQuery.data?.find(
    (job) => job.instanceId === instance.id,
  );

  const refresh = async (updated?: LauncherInstance) => {
    if (updated) {
      queryClient.setQueryData(["instance", instance.id], updated);
    }
    await queryClient.invalidateQueries({ queryKey: ["instances"] });
  };
  const renameMutation = useMutation({
    mutationFn: renameInstance,
    onSuccess: refresh,
  });
  const favoriteMutation = useMutation({
    mutationFn: setInstanceFavorite,
    onSuccess: refresh,
  });
  const trashMutation = useMutation({
    mutationFn: trashInstance,
    onSuccess: async () => {
      queryClient.removeQueries({ queryKey: ["instance", instance.id] });
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      await navigate({ to: "/library" });
    },
  });
  const installMutation = useMutation({
    mutationFn: installInstance,
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["instance", instance.id] }),
        queryClient.invalidateQueries({ queryKey: ["instances"] }),
        queryClient.invalidateQueries({ queryKey: ["install-jobs"] }),
      ]);
    },
  });
  const launchMutation = useMutation({
    mutationFn: ({ accountId }: { accountId: string }) =>
      launchInstance(instance.id, accountId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["game-sessions"] });
    },
  });
  const stopMutation = useMutation({
    mutationFn: forceStopGameSession,
    onSuccess: async () => {
      setConfirmStop(false);
      await queryClient.invalidateQueries({ queryKey: ["game-sessions"] });
    },
  });

  const readyAccounts = (accountsQuery.data ?? []).filter(
    (account) => account.status === "ready",
  );
  const effectiveAccountId =
    selectedAccountId ||
    readyAccounts.find((account) => account.isDefault)?.id ||
    readyAccounts[0]?.id ||
    "";
  const activeSession = sessionsQuery.data?.find(
    (session) => session.instanceId === instance.id,
  );
  const retainedLogSession =
    trackedLogSession?.instanceId === instance.id
      ? trackedLogSession
      : undefined;
  const logSession = activeSession ?? retainedLogSession;
  useEffect(() => {
    if (installJob?.state === "succeeded" || installJob?.state === "failed") {
      void queryClient.invalidateQueries({
        queryKey: ["instance", instance.id],
      });
      void queryClient.invalidateQueries({ queryKey: ["instances"] });
    }
  }, [installJob?.state, instance.id, queryClient]);

  const installing =
    instance.setupState === "preparing" ||
    installJob?.state === "queued" ||
    installJob?.state === "running";
  const ready = instance.setupState === "ready";

  return (
    <div className="grid grid-cols-[minmax(0,1.35fr)_minmax(300px,.65fr)] gap-6">
      <div className="grid content-start gap-6">
        <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
          <div className="flex items-start justify-between gap-5">
            <div>
              <p className="m-0 text-[10px] font-bold tracking-[.08em] text-app-muted uppercase">
                Pre-launch status
              </p>
              <h2 className="mt-2 mb-1 text-lg font-bold tracking-[-.02em]">
                {activeSession
                  ? activeSession.state === "stopping"
                    ? "Stopping Minecraft"
                    : "Minecraft is running"
                  : ready
                    ? "Ready to launch"
                    : installing
                      ? "Installing and verifying"
                      : instance.setupState === "blocked"
                        ? "Installation needs attention"
                        : "Ready to install"}
              </h2>
              <p className="m-0 max-w-[600px] text-xs/[19px] text-app-secondary">
                {activeSession
                  ? activeSession.state === "stopping"
                    ? `Waiting for process ${activeSession.pid} to exit.`
                    : `Process ${activeSession.pid} is active. slate will keep checking it while the launcher is open.`
                  : ready
                    ? "Game files, loader files, natives, assets, and the version-specific managed Java runtime are installed."
                    : installing
                      ? (installJob?.message ??
                        "Resolving metadata and preparing downloads.")
                      : instance.loaderKind === "neoForge"
                        ? "Install downloads verified game files and runs NeoForge’s official client installer in slate’s managed directory."
                        : "Install downloads and verifies the base game, assets, libraries, natives, and matching Java runtime."}
              </p>
            </div>
            <button
              type="button"
              className={`inline-flex h-10 min-w-32 items-center justify-center gap-2 rounded-control px-4 text-xs font-bold disabled:bg-app-raised disabled:text-app-muted disabled:opacity-70 ${
                activeSession
                  ? "border border-app-danger/45 bg-transparent text-app-danger hover:bg-app-danger/10"
                  : "bg-app-accent text-app-on-accent hover:brightness-105"
              }`}
              disabled={
                activeSession?.state === "stopping" ||
                installing ||
                installMutation.isPending ||
                launchMutation.isPending ||
                stopMutation.isPending ||
                (ready && !effectiveAccountId)
              }
              title={
                activeSession
                  ? "Review the warning before force-closing Minecraft."
                  : ready
                    ? effectiveAccountId
                      ? "Start Minecraft with the selected account."
                      : "Connect a Minecraft account before launching."
                    : "Install this exact instance revision."
              }
              onClick={() => {
                if (activeSession) {
                  setConfirmStop(true);
                } else if (ready) {
                  launchMutation.mutate({ accountId: effectiveAccountId });
                } else {
                  installMutation.mutate({
                    id: instance.id,
                    expectedRevision: instance.revision,
                  });
                }
              }}
            >
              {activeSession ? (
                activeSession.state === "stopping" ? (
                  <LoaderCircle
                    className="animate-spin"
                    size={17}
                    aria-hidden="true"
                  />
                ) : (
                  <Square size={15} fill="currentColor" aria-hidden="true" />
                )
              ) : ready ? (
                <Play size={17} fill="currentColor" aria-hidden="true" />
              ) : (
                <Download size={17} aria-hidden="true" />
              )}
              {activeSession?.state === "stopping"
                ? "Stopping…"
                : activeSession
                  ? "Stop game"
                  : launchMutation.isPending
                    ? "Starting…"
                    : ready
                      ? "Play"
                      : installing || installMutation.isPending
                        ? "Installing…"
                        : instance.setupState === "blocked"
                          ? "Retry install"
                          : "Install"}
            </button>
          </div>
          {installing && installJob ? (
            <InstallProgressIndicator job={installJob} />
          ) : null}
          {activeSession && confirmStop ? (
            <div
              className="mt-4 flex items-center gap-4 border-t border-app-separator/55 pt-4"
              role="alert"
            >
              <span className="min-w-0 flex-1">
                <strong className="block text-xs font-bold text-app-text">
                  Force-close Minecraft?
                </strong>
                <span className="mt-0.5 block text-[11px]/[17px] text-app-secondary">
                  slate cannot request an in-game save yet. Unsaved world
                  progress may be lost.
                </span>
              </span>
              <button
                type="button"
                className="h-8 rounded-compact border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-secondary hover:text-app-text"
                onClick={() => setConfirmStop(false)}
              >
                Keep running
              </button>
              <button
                type="button"
                className="h-8 rounded-compact bg-app-danger px-3 text-[11px] font-bold text-[#24110f] disabled:opacity-50"
                disabled={stopMutation.isPending}
                onClick={() => stopMutation.mutate(activeSession.id)}
              >
                Force close
              </button>
            </div>
          ) : null}
          {ready && !activeSession ? (
            <div className="mt-4 grid grid-cols-[minmax(0,1fr)_auto] items-end gap-4 border-t border-app-separator/55 pt-4">
              {readyAccounts.length > 0 ? (
                <ComboBox
                  label="Minecraft account"
                  value={effectiveAccountId}
                  options={readyAccounts.map((account) => ({
                    value: account.id,
                    label: account.displayName,
                    description: account.isDefault
                      ? "Default account"
                      : "Minecraft Java Edition",
                    recommended: account.isDefault,
                  }))}
                  onValueChange={setSelectedAccountId}
                />
              ) : (
                <div>
                  <strong className="block text-xs font-bold text-app-text">
                    Minecraft account required
                  </strong>
                  <p className="mt-1 mb-0 text-[11px]/[17px] text-app-muted">
                    Connect and verify a Microsoft account before starting this
                    instance.
                  </p>
                </div>
              )}
              <Link
                to="/accounts"
                className="inline-flex h-10 items-center gap-2 rounded-control border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-secondary no-underline hover:text-app-text"
              >
                <UserRound size={15} aria-hidden="true" />
                Manage accounts
              </Link>
            </div>
          ) : null}
          {installMutation.isError || installJob?.state === "failed" ? (
            <InlineNotice tone="danger" title="Installation did not complete">
              {installJob?.message ??
                "slate could not queue the installation. Reload the instance and try again."}
            </InlineNotice>
          ) : null}
          {launchMutation.isError ? (
            <InlineNotice tone="danger" title="Minecraft did not start">
              The installed files were left intact. Reinstall if verification
              reports a missing or corrupt artifact.
            </InlineNotice>
          ) : null}
          {stopMutation.isError ? (
            <InlineNotice tone="danger" title="Minecraft did not stop">
              The process is still being tracked. Try force-closing it again.
            </InlineNotice>
          ) : null}
        </section>

        {logSession ? (
          <SessionLogPanel
            key={logSession.id}
            session={logSession}
            active={activeSession?.id === logSession.id}
            onAttached={setTrackedLogSession}
          />
        ) : null}

        <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
          <h2 className="m-0 text-[15px] font-bold tracking-[-.015em]">
            Identity
          </h2>
          <p className="mt-1 mb-5 text-xs text-app-secondary">
            The name is presentation only; slate keeps the stable instance ID
            underneath.
          </p>
          <label className="block text-xs font-bold text-app-text">
            Instance name
            <span className="mt-2 flex gap-2">
              <input
                value={name}
                maxLength={80}
                className="h-10 min-w-0 flex-1 rounded-control border border-app-separator bg-app-bg px-3 text-[13px] text-app-text focus:border-app-accent focus:outline-none"
                onChange={(event) => setName(event.target.value)}
              />
              <button
                type="button"
                className="inline-flex h-10 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-45"
                disabled={
                  renameMutation.isPending ||
                  !name.trim() ||
                  name.trim() === instance.name
                }
                onClick={() =>
                  renameMutation.mutate({
                    id: instance.id,
                    name,
                    expectedRevision: instance.revision,
                  })
                }
              >
                <Save size={16} aria-hidden="true" />
                Save
              </button>
            </span>
          </label>
          {renameMutation.isError ? (
            <p className="mt-3 text-xs text-app-danger" role="alert">
              The name could not be saved. Reload and try again.
            </p>
          ) : null}
        </section>
      </div>

      <aside className="grid content-start gap-5">
        <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
          <h2 className="m-0 text-[15px] font-bold tracking-[-.015em]">
            Setup details
          </h2>
          <dl className="mt-4 grid gap-3">
            <Detail label="Minecraft" value={instance.minecraftVersion} mono />
            <Detail
              label="Loader"
              value={`${loaderLabel(instance.loaderKind)}${instance.loaderVersion ? ` ${instance.loaderVersion}` : ""}`}
              mono
            />
            <Detail label="Memory" value={`${instance.memoryMb} MB`} mono />
            <Detail label="Created" value={formatDate(instance.createdAt)} />
          </dl>
          <Link
            to="/instances/$instanceId/settings"
            params={{ instanceId: instance.id }}
            className="mt-5 inline-flex h-9 w-full items-center justify-center gap-2 rounded-control border border-app-separator bg-app-bg text-xs font-bold text-app-text no-underline hover:bg-app-hover"
          >
            <Settings2 size={16} aria-hidden="true" />
            Edit configuration
          </Link>
        </section>

        <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
          <h2 className="m-0 text-[15px] font-bold tracking-[-.015em]">
            Library actions
          </h2>
          <div className="mt-4 grid gap-2">
            <button
              type="button"
              className="inline-flex h-9 items-center justify-center gap-2 rounded-control border border-app-separator bg-app-bg text-xs font-bold text-app-text hover:bg-app-hover disabled:opacity-50"
              disabled={favoriteMutation.isPending}
              onClick={() =>
                favoriteMutation.mutate({
                  id: instance.id,
                  favorite: !instance.favorite,
                  expectedRevision: instance.revision,
                })
              }
            >
              <Heart
                size={16}
                fill={instance.favorite ? "currentColor" : "none"}
                aria-hidden="true"
              />
              {instance.favorite ? "Remove favorite" : "Add favorite"}
            </button>
            <button
              type="button"
              className="inline-flex h-9 items-center justify-center gap-2 rounded-control border border-app-danger/35 bg-transparent text-xs font-bold text-app-danger hover:bg-app-danger/10"
              disabled={Boolean(activeSession)}
              title={
                activeSession
                  ? "Stop Minecraft before moving this instance to trash."
                  : undefined
              }
              onClick={() => setConfirmTrash(true)}
            >
              <Trash2 size={16} aria-hidden="true" />
              Move to trash
            </button>
          </div>
        </section>
      </aside>

      {confirmTrash ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/65 p-6"
          role="presentation"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) setConfirmTrash(false);
          }}
        >
          <section
            className="w-full max-w-[430px] rounded-dialog border border-app-separator bg-app-surface p-6 shadow-2xl"
            role="dialog"
            aria-modal="true"
            aria-labelledby="trash-title"
          >
            <h2 id="trash-title" className="m-0 text-lg font-bold">
              Move {instance.name} to trash?
            </h2>
            <p className="mt-2 mb-0 text-xs/[19px] text-app-secondary">
              The library record will be hidden, but slate will not delete the
              managed instance files in this implementation.
            </p>
            <div className="mt-6 flex justify-end gap-2">
              <button
                type="button"
                className="h-9 rounded-control border border-app-separator bg-app-bg px-4 text-xs font-bold text-app-text"
                onClick={() => setConfirmTrash(false)}
              >
                Cancel
              </button>
              <button
                type="button"
                className="h-9 rounded-control bg-app-danger px-4 text-xs font-bold text-black disabled:opacity-50"
                disabled={trashMutation.isPending}
                onClick={() =>
                  trashMutation.mutate({
                    id: instance.id,
                    expectedRevision: instance.revision,
                  })
                }
              >
                {trashMutation.isPending ? "Moving…" : "Move to trash"}
              </button>
            </div>
            {trashMutation.isError ? (
              <p className="mt-3 text-xs text-app-danger" role="alert">
                The instance could not be moved. Reload and try again.
              </p>
            ) : null}
          </section>
        </div>
      ) : null}
    </div>
  );
}

function Content({ instance }: { instance: LauncherInstance }) {
  return (
    <div className="grid grid-cols-[220px_minmax(0,1fr)] gap-6">
      <nav
        className="rounded-control border border-app-separator/70 bg-app-surface p-2"
        aria-label="Content types"
      >
        {[
          ["Mods", instance.modCount ?? 0],
          ["Resource packs", 0],
          ["Shaders", 0],
          ["Data packs", 0],
        ].map(([label, count], index) => (
          <button
            key={label}
            type="button"
            className={`flex h-10 w-full items-center justify-between rounded-compact border-0 px-3 text-left text-xs font-bold ${
              index === 0
                ? "bg-app-raised text-app-text"
                : "bg-transparent text-app-muted"
            }`}
            disabled={index !== 0}
            title={
              index === 0
                ? undefined
                : "This content inventory is not implemented yet."
            }
          >
            {label}
            <span className="font-mono text-[10px]">{count}</span>
          </button>
        ))}
      </nav>
      <section className="rounded-control border border-app-separator/70 bg-app-surface">
        <div className="flex items-center justify-between border-b border-app-separator/55 px-5 py-4">
          <div>
            <h2 className="m-0 text-[15px] font-bold">Installed mods</h2>
            <p className="mt-1 mb-0 text-[11px] text-app-secondary">
              Content records will be tied to immutable instance revisions.
            </p>
          </div>
          <button
            type="button"
            className="h-9 rounded-control bg-app-raised px-4 text-xs font-bold text-app-muted opacity-60"
            disabled
            title="Provider downloads and local artifact import are not implemented yet."
          >
            Add content
          </button>
        </div>
        <EmptyState
          title="No indexed content"
          description="slate has not scanned or installed artifacts for this configured profile. Existing preview counts are illustrative only."
        />
      </section>
    </div>
  );
}

function InstanceSettings({ instance }: { instance: LauncherInstance }) {
  const queryClient = useQueryClient();
  const [minecraftVersion, setMinecraftVersion] = useState(
    instance.minecraftVersion,
  );
  const [loaderKind, setLoaderKind] = useState<LoaderKind>(instance.loaderKind);
  const [loaderSelection, setLoaderSelection] = useState({
    catalogKey: `${instance.minecraftVersion}|${instance.loaderKind}`,
    value: instance.loaderVersion ?? "",
  });
  const [memoryMb, setMemoryMb] = useState(instance.memoryMb);
  const [message, setMessage] = useState<string>();
  const versionsQuery = useQuery({
    queryKey: ["minecraft-version-catalog"],
    queryFn: getMinecraftVersionCatalog,
    staleTime: 15 * 60_000,
  });
  const loaderQuery = useQuery({
    queryKey: ["loader-version-catalog", minecraftVersion, loaderKind],
    queryFn: () => getLoaderVersionCatalog({ minecraftVersion, loaderKind }),
    enabled: Boolean(minecraftVersion) && loaderKind !== "vanilla",
    staleTime: 15 * 60_000,
  });
  const loaderCatalogKey = `${minecraftVersion}|${loaderKind}`;
  const selectedLoaderVersion =
    loaderKind === "vanilla"
      ? undefined
      : loaderSelection.catalogKey === loaderCatalogKey &&
          loaderQuery.data?.versions.includes(loaderSelection.value)
        ? loaderSelection.value
        : (loaderQuery.data?.recommendedVersion ?? loaderSelection.value);

  const mutation = useMutation({
    mutationFn: updateInstanceConfiguration,
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setMessage("Configuration saved. The instance remains uninstalled.");
    },
  });

  const save = () => {
    setMessage(undefined);
    if (!minecraftVersion.trim()) {
      setMessage("Enter a Minecraft version.");
      return;
    }
    if (versionsQuery.isError) {
      setMessage("The Minecraft release catalog is unavailable.");
      return;
    }
    if (
      loaderKind !== "vanilla" &&
      (loaderQuery.isPending || loaderQuery.isFetching)
    ) {
      setMessage("Wait for slate to resolve loader compatibility.");
      return;
    }
    if (loaderKind !== "vanilla" && loaderQuery.isError) {
      setMessage("The compatible loader catalog is unavailable.");
      return;
    }
    if (loaderKind !== "vanilla" && loaderQuery.data?.versions.length === 0) {
      setMessage(
        loaderQuery.data?.unavailableReason ??
          "No compatible loader release is available.",
      );
      return;
    }
    mutation.mutate({
      id: instance.id,
      minecraftVersion,
      loaderKind,
      loaderVersion: selectedLoaderVersion,
      memoryMb,
      expectedRevision: instance.revision,
    });
  };

  return (
    <div className="grid grid-cols-[minmax(0,1fr)_300px] gap-6">
      <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
        <h2 className="m-0 text-[15px] font-bold">Game and runtime</h2>
        <p className="mt-1 mb-5 text-xs text-app-secondary">
          Changing these values returns the profile to Configured until a future
          install job succeeds.
        </p>
        <div className="grid grid-cols-2 gap-5">
          <ComboBox
            label="Minecraft version"
            value={minecraftVersion}
            options={(versionsQuery.data?.versions ?? []).map(
              (version, index) => ({
                value: version.id,
                label: `Minecraft ${version.id}`,
                description: index === 0 ? "Latest release" : "Release",
                recommended: index === 0,
              }),
            )}
            disabled={versionsQuery.isPending || versionsQuery.isError}
            placeholder={
              versionsQuery.isPending ? "Loading releases…" : "Choose a release"
            }
            onValueChange={(value) => {
              setMinecraftVersion(value);
            }}
          />
          <ComboBox
            label="Loader"
            value={loaderKind}
            options={
              instance.mode === "vanilla"
                ? [{ value: "vanilla", label: "Vanilla" }]
                : [
                    { value: "fabric", label: "Fabric" },
                    { value: "neoForge", label: "NeoForge" },
                    ...(instance.mode === "pvp"
                      ? [{ value: "vanilla", label: "Vanilla" }]
                      : []),
                  ]
            }
            disabled={instance.mode === "vanilla"}
            onValueChange={(value) => {
              setLoaderKind(value as LoaderKind);
            }}
          />
          <ComboBox
            label="Loader version"
            value={selectedLoaderVersion ?? ""}
            options={(loaderQuery.data?.versions ?? []).map((version) => ({
              value: version,
              label: version,
              description:
                version === loaderQuery.data?.recommendedVersion
                  ? `${loaderLabel(loaderKind)} recommended`
                  : `${loaderLabel(loaderKind)} loader release`,
              recommended: version === loaderQuery.data?.recommendedVersion,
            }))}
            disabled={
              loaderKind === "vanilla" ||
              loaderQuery.isPending ||
              loaderQuery.isFetching ||
              loaderQuery.data?.versions.length === 0
            }
            placeholder={
              loaderKind === "vanilla"
                ? "Built into Minecraft"
                : loaderQuery.isPending || loaderQuery.isFetching
                  ? "Loading compatible versions…"
                  : "Choose a loader version"
            }
            emptyText={
              loaderQuery.data?.unavailableReason ??
              "No compatible loader versions"
            }
            onValueChange={(value) =>
              setLoaderSelection({ catalogKey: loaderCatalogKey, value })
            }
          />
          <Field label="Memory limit">
            <select
              value={memoryMb}
              className={inputClass}
              onChange={(event) => setMemoryMb(Number(event.target.value))}
            >
              {[2048, 4096, 6144, 8192, 12288, 16384].map((value) => (
                <option key={value} value={value}>
                  {value / 1024} GB ({value} MB)
                </option>
              ))}
            </select>
          </Field>
        </div>
        <div className="mt-6 flex items-center justify-between border-t border-app-separator/55 pt-5">
          <p
            className={`m-0 text-xs ${mutation.isError ? "text-app-danger" : "text-app-secondary"}`}
            role={mutation.isError ? "alert" : "status"}
          >
            {mutation.isError
              ? "The configuration could not be saved. Reload and try again."
              : message}
          </p>
          <button
            type="button"
            className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-50"
            disabled={mutation.isPending}
            onClick={save}
          >
            {mutation.isPending ? (
              <RotateCcw
                className="animate-spin"
                size={16}
                aria-hidden="true"
              />
            ) : (
              <Check size={16} aria-hidden="true" />
            )}
            Save configuration
          </button>
        </div>
      </section>

      <aside className="grid content-start gap-4">
        <InlineNotice title="Revision guarded">
          Every edit includes revision {instance.revision}. A stale screen
          cannot silently overwrite a newer change.
        </InlineNotice>
        <InlineNotice tone="warning" title="Installation unavailable">
          Metadata resolution and verified downloads are the next boundary.
          Saving here does not mark the instance ready.
        </InlineNotice>
        <div className="rounded-control border border-app-separator bg-app-surface p-4">
          <span className="inline-flex items-center gap-2 text-xs font-bold text-app-text">
            <FolderArchive size={16} aria-hidden="true" />
            Managed storage
          </span>
          <p className="mt-2 mb-0 text-[11px]/[17px] text-app-secondary">
            Paths stay behind the native boundary. The renderer receives stable
            IDs only.
          </p>
        </div>
      </aside>
    </div>
  );
}

const inputClass =
  "mt-2 h-10 w-full rounded-control border border-app-separator bg-app-bg px-3 text-[13px] text-app-text placeholder:text-app-muted focus:border-app-accent focus:outline-none disabled:opacity-55";

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block text-xs font-bold text-app-text">
      {label}
      {children}
    </label>
  );
}

function Detail({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="flex items-baseline justify-between gap-5 border-b border-app-separator/45 pb-2 last:border-0 last:pb-0">
      <dt className="text-[11px] text-app-muted">{label}</dt>
      <dd
        className={`m-0 text-right text-xs font-semibold ${mono ? "font-mono text-[11px]" : ""}`}
      >
        {value}
      </dd>
    </div>
  );
}
