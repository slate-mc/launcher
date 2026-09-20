import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import {
  ArrowLeft,
  ArrowRight,
  ArrowUpDown,
  Box,
  Check,
  ChevronDown,
  ChevronLeft,
  Copy,
  Cpu,
  Download,
  FolderArchive,
  FolderOpen,
  Heart,
  Layers3,
  LoaderCircle,
  Monitor,
  Image as ImageIcon,
  Plus,
  Pin,
  PinOff,
  Play,
  Power,
  RotateCcw,
  Save,
  Search,
  Settings2,
  ShieldCheck,
  Square,
  Trash2,
  UserRound,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { ComboBox } from "../../components/ComboBox";
import { ContentArtwork } from "../../components/ContentArtwork";
import { InstanceArtwork as InstanceProfileArtwork } from "../../components/InstanceArtwork";
import { InstallProgressIndicator } from "../../components/InstallProgressIndicator";
import { SessionLogPanel } from "../../components/SessionLogPanel";
import {
  EmptyState,
  InlineNotice,
  StatusPill,
} from "../../components/PageScaffold";
import {
  forceStopGameSession,
  createInstanceSnapshot,
  deleteInstanceSnapshot,
  duplicateInstance,
  exportInstance,
  getInstanceGameOptions,
  getInstance,
  getLoaderVersionCatalog,
  getMinecraftVersionCatalog,
  installInstance,
  installMods,
  importInstance,
  launchInstance,
  listAccounts,
  listGameSessions,
  listInstanceContentFiles,
  listInstallJobs,
  listInstanceMods,
  listInstanceSnapshots,
  moveInstanceStorage,
  openInstanceDirectory,
  removeInstanceContentFile,
  removeInstanceMod,
  renameInstance,
  resetInstanceArtwork,
  restoreInstanceSnapshot,
  resolveInstanceMods,
  selectInstanceArtwork,
  selectInstanceJava,
  setInstanceSnapshotPinned,
  setInstanceContentFileEnabled,
  setInstanceModEnabled,
  setInstanceModPinned,
  setInstanceFavorite,
  searchMods,
  trashInstance,
  updateInstanceConfiguration,
  updateInstanceGameOptions,
  updateInstanceSettings,
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
  InstanceContentFile,
  InstanceContentKind,
  InstanceMod,
  LauncherInstance,
  LoaderKind,
  ModpackSummary,
  Provider,
} from "../../types/launcher";

type InstanceSection = "overview" | "content" | "settings";

type InstalledModSortKey =
  "name" | "source" | "version" | "status" | "size" | "installed";

type InstalledModSort = {
  key: InstalledModSortKey;
  direction: "ascending" | "descending";
};

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
          <InstanceSettings
            key={`${query.data.id}:${query.data.revision}`}
            instance={query.data}
          />
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
        <InstanceProfileArtwork
          instance={instance}
          className="size-14 rounded-control border border-app-separator"
        />
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
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["game-sessions"] }),
        queryClient.invalidateQueries({ queryKey: ["instance", instance.id] }),
        queryClient.invalidateQueries({ queryKey: ["instances"] }),
      ]);
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
  const queryClient = useQueryClient();
  const [contentKind, setContentKind] = useState<"mods" | InstanceContentKind>("mods");
  const [browserOpen, setBrowserOpen] = useState(false);
  const [installedFilter, setInstalledFilter] = useState("");
  const [installedStatus, setInstalledStatus] = useState<
    "all" | "enabled" | "disabled"
  >("all");
  const [installedOrigin, setInstalledOrigin] = useState<
    "all" | InstanceMod["origin"]
  >("all");
  const [installedSort, setInstalledSort] = useState<InstalledModSort>({
    key: "name",
    direction: "ascending",
  });
  const [installedPage, setInstalledPage] = useState(1);
  const [installedPageSize, setInstalledPageSize] = useState(25);
  const [draftQuery, setDraftQuery] = useState("");
  const [query, setQuery] = useState("");
  const [provider, setProvider] = useState<"all" | "curseforge" | "modrinth">(
    "all",
  );
  const [sort, setSort] = useState<
    "relevance" | "downloads" | "updated" | "newest"
  >("relevance");
  const [page, setPage] = useState(1);
  const [selectedMods, setSelectedMods] = useState<
    Record<string, ModpackSummary>
  >({});
  const [installJobId, setInstallJobId] = useState<string>();
  const [removeTargetPath, setRemoveTargetPath] = useState<string>();
  const [notice, setNotice] = useState<{
    tone: "positive" | "danger";
    title: string;
    message: string;
  }>();
  const isVanilla = instance.loaderKind === "vanilla";
  const installedQuery = useQuery({
    queryKey: ["instance-mods", instance.id],
    queryFn: () => listInstanceMods(instance.id),
  });
  const resolutionQuery = useQuery({
    queryKey: ["instance-mod-resolutions", instance.id],
    queryFn: () => resolveInstanceMods(instance.id),
    enabled: Boolean(installedQuery.data?.length),
    staleTime: 30 * 60_000,
  });
  const searchQuery = useQuery({
    queryKey: ["mod-search", instance.id, query, provider, sort, page],
    queryFn: () =>
      searchMods({
        instanceId: instance.id,
        query: query || undefined,
        provider: provider === "all" ? undefined : provider,
        sort,
        page,
        limit: 20,
      }),
    enabled: browserOpen && !isVanilla,
    placeholderData: (previous) => previous,
  });
  const jobsQuery = useQuery({
    queryKey: ["install-jobs"],
    queryFn: listInstallJobs,
    enabled: Boolean(installJobId),
    refetchInterval: (jobs) => {
      const tracked = jobs.state.data?.find((job) => job.id === installJobId);
      return tracked &&
        ["succeeded", "failed", "cancelled"].includes(tracked.state)
        ? false
        : 750;
    },
  });
  const installJob = jobsQuery.data?.find((job) => job.id === installJobId);
  const installMutation = useMutation({
    mutationFn: (items: ModpackSummary[]) => {
      if (items.length === 0) {
        throw new Error("Select at least one mod to install.");
      }
      if (items.some((item) => item.provider === "ftb")) {
        throw new Error("FTB does not provide individual mod downloads.");
      }
      return installMods({
        instanceId: instance.id,
        expectedRevision: instance.revision,
        mods: items.map((item) => ({
          provider: item.provider as Exclude<Provider, "ftb">,
          projectId: item.id,
          displayName: item.name,
        })),
      });
    },
    onMutate: () => setNotice(undefined),
    onSuccess: async (job) => {
      setSelectedMods({});
      setInstallJobId(job.id);
      queryClient.setQueryData(["install-jobs"], (current: unknown) =>
        Array.isArray(current) ? [job, ...current] : [job],
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["instance", instance.id] }),
        queryClient.invalidateQueries({ queryKey: ["instances"] }),
      ]);
    },
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Mods were not queued",
        message: contentErrorMessage(error),
      }),
  });
  const refreshContentAfterMutation = async (
    updated: LauncherInstance,
    title: string,
    message: string,
  ) => {
    queryClient.setQueryData(["instance", instance.id], updated);
    setNotice({ tone: "positive", title, message });
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: ["instance-mods", instance.id],
      }),
      queryClient.invalidateQueries({
        queryKey: ["instance-mod-resolutions", instance.id],
      }),
      queryClient.invalidateQueries({ queryKey: ["instances"] }),
    ]);
  };
  const toggleMutation = useMutation({
    mutationFn: ({ item, enabled }: { item: InstanceMod; enabled: boolean }) =>
      setInstanceModEnabled({
        instanceId: instance.id,
        expectedRevision: instance.revision,
        filePath: item.filePath,
        provider: item.provider ?? undefined,
        projectId: item.projectId ?? undefined,
        enabled,
      }),
    onMutate: () => setNotice(undefined),
    onSuccess: (updated, variables) =>
      refreshContentAfterMutation(
        updated,
        variables.enabled ? "Mod enabled" : "Mod disabled",
        `${variables.item.displayName} will ${variables.enabled ? "load" : "stay disabled"} the next time Minecraft starts.`,
      ),
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Mod was not changed",
        message: contentErrorMessage(error),
      }),
  });
  const pinModMutation = useMutation({
    mutationFn: ({ item, pinned }: { item: InstanceMod; pinned: boolean }) =>
      setInstanceModPinned({
        instanceId: instance.id,
        expectedRevision: instance.revision,
        filePath: item.filePath,
        provider: item.provider ?? undefined,
        projectId: item.projectId ?? undefined,
        pinned,
      }),
    onMutate: () => setNotice(undefined),
    onSuccess: (updated, variables) =>
      refreshContentAfterMutation(
        updated,
        variables.pinned ? "Mod version pinned" : "Mod version unpinned",
        `${variables.item.displayName} ${variables.pinned ? "will stay on this version during updates" : "can receive compatible updates"}.`,
      ),
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Mod pin was not changed",
        message: contentErrorMessage(error),
      }),
  });
  const removeMutation = useMutation({
    mutationFn: (item: InstanceMod) =>
      removeInstanceMod({
        instanceId: instance.id,
        expectedRevision: instance.revision,
        filePath: item.filePath,
        provider: item.provider ?? undefined,
        projectId: item.projectId ?? undefined,
      }),
    onMutate: () => setNotice(undefined),
    onSuccess: (updated, item) => {
      setRemoveTargetPath(undefined);
      return refreshContentAfterMutation(
        updated,
        "Mod moved to trash",
        `${item.displayName} was removed from this instance and can be recovered from slate’s trash.`,
      );
    },
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Mod was not removed",
        message: contentErrorMessage(error),
      }),
  });

  useEffect(() => {
    if (
      !installJob ||
      !["succeeded", "failed", "cancelled"].includes(installJob.state)
    ) {
      return;
    }
    void Promise.all([
      queryClient.invalidateQueries({
        queryKey: ["instance-mods", instance.id],
      }),
      queryClient.invalidateQueries({
        queryKey: ["instance-mod-resolutions", instance.id],
      }),
      queryClient.invalidateQueries({ queryKey: ["instance", instance.id] }),
      queryClient.invalidateQueries({ queryKey: ["instances"] }),
    ]);
  }, [installJob, instance.id, queryClient]);

  const completionNotice =
    installJob &&
    ["succeeded", "failed", "cancelled"].includes(installJob.state)
      ? {
          tone:
            installJob.state === "succeeded"
              ? ("positive" as const)
              : ("danger" as const),
          title:
            installJob.state === "succeeded"
              ? "Mods installed"
              : "Mod installation stopped",
          message: installJob.message,
        }
      : undefined;
  const visibleNotice = completionNotice ?? notice;

  const resolutionsByPath = new Map(
    (resolutionQuery.data ?? []).map((resolution) => [
      normalizedModPath(resolution.filePath),
      resolution,
    ]),
  );
  const installed = (installedQuery.data ?? []).map((item) => {
    const resolution = resolutionsByPath.get(normalizedModPath(item.filePath));
    return resolution
      ? {
          ...item,
          provider: resolution.provider,
          projectId: resolution.projectId,
          versionId: resolution.versionId ?? item.versionId,
          displayName: resolution.displayName ?? item.displayName,
          iconUrl: resolution.iconUrl ?? item.iconUrl,
        }
      : item;
  });
  const normalizedInstalledFilter = installedFilter.trim().toLocaleLowerCase();
  const visibleInstalled = installed
    .filter(
      (item) =>
        !normalizedInstalledFilter ||
        `${item.displayName} ${item.filePath} ${item.provider ?? ""} ${item.versionId ?? ""}`
          .toLocaleLowerCase()
          .includes(normalizedInstalledFilter),
    )
    .filter(
      (item) =>
        installedStatus === "all" ||
        (installedStatus === "enabled" ? item.enabled : !item.enabled),
    )
    .filter(
      (item) => installedOrigin === "all" || item.origin === installedOrigin,
    )
    .sort((left, right) => compareInstalledMods(left, right, installedSort));
  const installedIds = new Set(
    installed.flatMap((item) =>
      item.provider && item.projectId
        ? [`${item.provider}:${item.projectId}`]
        : [],
    ),
  );
  const selectedModItems = Object.values(selectedMods);
  const unavailableProviders = Object.entries(
    searchQuery.data?.provider_status ?? {},
  )
    .filter(([, status]) => status === "unavailable")
    .map(([id]) => modProviderName(id as Provider));
  const installing =
    installMutation.isPending ||
    installJob?.state === "queued" ||
    installJob?.state === "running";
  const contentMutationPending =
    toggleMutation.isPending || pinModMutation.isPending || removeMutation.isPending;
  const installedIdentityPending =
    installedQuery.isPending ||
    (Boolean(installedQuery.data?.length) && resolutionQuery.isFetching);
  const enabledModCount = installed.filter((item) => item.enabled).length;
  const installedPageCount = Math.max(
    1,
    Math.ceil(visibleInstalled.length / installedPageSize),
  );
  const activeInstalledPage = Math.min(installedPage, installedPageCount);
  const installedPageStart = (activeInstalledPage - 1) * installedPageSize;
  const pagedInstalled = visibleInstalled.slice(
    installedPageStart,
    installedPageStart + installedPageSize,
  );
  const updateInstalledSort = (next: InstalledModSort) => {
    setInstalledSort(next);
    setInstalledPage(1);
  };

  if (contentKind !== "mods") {
    return (
      <InstanceFileContent
        instance={instance}
        kind={contentKind}
        modCount={installed.length}
        onKindChange={setContentKind}
      />
    );
  }

  return (
    <div className="grid grid-cols-[220px_minmax(0,1fr)] gap-6">
      <ContentNavigation
        active="mods"
        counts={{ mods: installed.length }}
        onChange={setContentKind}
      />
      <section className="min-w-0 rounded-control border border-app-separator/70 bg-app-surface">
        <div className="flex items-center justify-between border-b border-app-separator/55 px-5 py-4">
          <div>
            <h2 className="m-0 text-[15px] font-bold">Installed mods</h2>
            <p className="mt-1 mb-0 text-[11px] text-app-secondary">
              {isVanilla
                ? "A mod loader is required before mods can be added."
                : `${instance.minecraftVersion} · ${loaderLabel(instance.loaderKind)} ${instance.loaderVersion ?? ""}`}
            </p>
          </div>
          <button
            type="button"
            className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:cursor-not-allowed disabled:opacity-45"
            disabled={isVanilla || installing}
            onClick={() => {
              if (browserOpen) setSelectedMods({});
              setBrowserOpen((open) => !open);
              setNotice(undefined);
            }}
          >
            <Plus size={15} aria-hidden="true" />
            {browserOpen ? "Close browser" : "Add mods"}
          </button>
        </div>

        {isVanilla ? (
          <div className="p-5">
            <InlineNotice tone="neutral" title="This is a Vanilla instance">
              Change the instance to Fabric or NeoForge in Settings before
              adding loader mods.
            </InlineNotice>
          </div>
        ) : null}

        {visibleNotice ? (
          <div className="px-5 pt-5">
            <InlineNotice tone={visibleNotice.tone} title={visibleNotice.title}>
              {visibleNotice.message}
            </InlineNotice>
          </div>
        ) : null}

        {installJob && ["queued", "running"].includes(installJob.state) ? (
          <div className="border-b border-app-separator/55 px-5 py-4">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="m-0 text-xs font-bold text-app-text">
                  Installing content
                </p>
                <p className="mt-1 mb-0 text-[11px] text-app-secondary">
                  {installJob.message}
                </p>
              </div>
              <LoaderCircle
                size={18}
                className="animate-spin text-app-accent motion-reduce:animate-none"
                aria-hidden="true"
              />
            </div>
            <InstallProgressIndicator job={installJob} />
          </div>
        ) : null}

        {browserOpen && !isVanilla ? (
          <div className="border-b border-app-separator/55">
            <div className="border-b border-app-separator/45 bg-app-bg/35 px-5 py-4">
              <div className="mb-4 flex items-start justify-between gap-5">
                <div className="flex items-start gap-3">
                  <ShieldCheck
                    size={18}
                    className="mt-0.5 text-app-accent"
                    aria-hidden="true"
                  />
                  <div>
                    <p className="m-0 text-xs font-bold text-app-text">
                      Compatibility locked to this instance
                    </p>
                    <p className="mt-1 mb-0 font-mono text-[10px] text-app-secondary">
                      Minecraft {instance.minecraftVersion} ·{" "}
                      {loaderLabel(instance.loaderKind)}{" "}
                      {instance.loaderVersion}
                    </p>
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-3">
                  <span className="font-mono text-[10px] text-app-muted">
                    {selectedModItems.length} selected
                  </span>
                  <button
                    type="button"
                    disabled={
                      selectedModItems.length === 0 ||
                      installing ||
                      installedIdentityPending
                    }
                    className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:bg-app-raised disabled:text-app-muted"
                    onClick={() => installMutation.mutate(selectedModItems)}
                  >
                    {installMutation.isPending ? (
                      <LoaderCircle
                        size={14}
                        className="animate-spin motion-reduce:animate-none"
                        aria-hidden="true"
                      />
                    ) : (
                      <Download size={14} aria-hidden="true" />
                    )}
                    {installMutation.isPending
                      ? "Resolving dependencies"
                      : selectedModItems.length > 0
                        ? `Install ${selectedModItems.length} selected`
                        : "Install selected"}
                  </button>
                </div>
              </div>
              <form
                className="grid grid-cols-[minmax(220px,1fr)_150px_150px_auto] gap-2"
                onSubmit={(event) => {
                  event.preventDefault();
                  setQuery(draftQuery.trim());
                  setPage(1);
                }}
              >
                <label className="relative block">
                  <span className="sr-only">Search compatible mods</span>
                  <Search
                    size={15}
                    className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-app-muted"
                    aria-hidden="true"
                  />
                  <input
                    value={draftQuery}
                    onChange={(event) => setDraftQuery(event.target.value)}
                    placeholder="Search compatible mods"
                    className="h-9 w-full rounded-control border border-app-separator bg-app-bg pr-3 pl-9 text-xs text-app-text outline-none placeholder:text-app-muted focus:border-app-accent"
                  />
                </label>
                <ContentSelect
                  label="Provider"
                  value={provider}
                  options={[
                    ["all", "All providers"],
                    ["modrinth", "Modrinth"],
                    ["curseforge", "CurseForge"],
                  ]}
                  onChange={(value) => {
                    setProvider(value as typeof provider);
                    setPage(1);
                  }}
                />
                <ContentSelect
                  label="Sort"
                  value={sort}
                  options={[
                    ["relevance", "Relevance"],
                    ["downloads", "Downloads"],
                    ["updated", "Recently updated"],
                    ["newest", "Newest"],
                  ]}
                  onChange={(value) => {
                    setSort(value as typeof sort);
                    setPage(1);
                  }}
                />
                <button
                  type="submit"
                  className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent"
                >
                  <Search size={14} aria-hidden="true" /> Search
                </button>
              </form>
            </div>

            {unavailableProviders.length > 0 ? (
              <div className="px-5 pt-4">
                <InlineNotice
                  tone="warning"
                  title="Some providers did not respond"
                >
                  Results from {unavailableProviders.join(", ")} are temporarily
                  unavailable.
                </InlineNotice>
              </div>
            ) : null}

            {searchQuery.isPending ? (
              <ModResultSkeletons />
            ) : searchQuery.isError ? (
              <div className="p-5">
                <InlineNotice
                  tone="danger"
                  title="Compatible mods could not be loaded"
                >
                  {contentErrorMessage(searchQuery.error)}
                </InlineNotice>
              </div>
            ) : searchQuery.data?.items.length ? (
              <div
                className="divide-y divide-app-separator/45 px-5"
                aria-busy={searchQuery.isFetching}
              >
                {searchQuery.data.items.map((item) => {
                  const identity = `${item.provider}:${item.id}`;
                  const installedAlready = installedIds.has(identity);
                  const selected = Boolean(selectedMods[identity]);
                  return (
                    <ModSearchResult
                      key={identity}
                      item={item}
                      installed={installedAlready}
                      selected={selected}
                      checkingInstalled={installedIdentityPending}
                      disabled={
                        installing ||
                        (selectedModItems.length >= 50 && !selected)
                      }
                      onToggleSelected={() => {
                        if (installedAlready) return;
                        setSelectedMods((current) => {
                          const next = { ...current };
                          if (next[identity]) delete next[identity];
                          else next[identity] = item;
                          return next;
                        });
                      }}
                    />
                  );
                })}
              </div>
            ) : (
              <EmptyState
                title="No compatible mods found"
                description="Try a different search. slate only returns files matching this instance’s exact Minecraft version and loader."
              />
            )}

            {searchQuery.data?.items.length ? (
              <nav
                className="flex items-center justify-between border-t border-app-separator/45 px-5 py-3"
                aria-label="Mod result pages"
              >
                <button
                  type="button"
                  disabled={page === 1 || searchQuery.isFetching}
                  className="inline-flex h-8 items-center gap-1.5 rounded-control border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-secondary disabled:opacity-40"
                  onClick={() => setPage((current) => Math.max(1, current - 1))}
                >
                  <ChevronLeft size={14} aria-hidden="true" /> Previous
                </button>
                <span className="font-mono text-[10px] text-app-muted">
                  Page {page}
                </span>
                <button
                  type="button"
                  disabled={
                    !searchQuery.data.has_more || searchQuery.isFetching
                  }
                  className="inline-flex h-8 items-center gap-1.5 rounded-control border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-secondary disabled:opacity-40"
                  onClick={() => setPage((current) => current + 1)}
                >
                  Next <ArrowRight size={14} aria-hidden="true" />
                </button>
              </nav>
            ) : null}
          </div>
        ) : null}

        {installedQuery.isPending ? (
          <InstalledModSkeletons />
        ) : installedQuery.isError ? (
          <div className="p-5">
            <InlineNotice
              tone="danger"
              title="Installed mods could not be loaded"
            >
              Reload this page to try again.
            </InlineNotice>
          </div>
        ) : installed.length ? (
          <>
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-app-separator/45 bg-app-bg/20 px-5 py-3">
              <div className="flex min-w-0 flex-1 items-center gap-2">
                <label className="relative block min-w-52 flex-1 max-w-sm">
                  <span className="sr-only">Filter installed mods</span>
                  <Search
                    size={14}
                    className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-app-muted"
                    aria-hidden="true"
                  />
                  <input
                    value={installedFilter}
                    onChange={(event) => {
                      setInstalledFilter(event.target.value);
                      setInstalledPage(1);
                    }}
                    placeholder="Filter by name, file, provider, or version"
                    className="h-8 w-full rounded-control border border-app-separator bg-app-bg pr-3 pl-9 text-[11px] text-app-text outline-none placeholder:text-app-muted focus:border-app-accent"
                  />
                </label>
                <div className="w-32">
                  <ContentSelect
                    label="Mod status"
                    value={installedStatus}
                    options={[
                      ["all", "All statuses"],
                      ["enabled", "Enabled"],
                      ["disabled", "Disabled"],
                    ]}
                    onChange={(value) => {
                      setInstalledStatus(value as typeof installedStatus);
                      setInstalledPage(1);
                    }}
                    compact
                  />
                </div>
                <div className="w-32">
                  <ContentSelect
                    label="Mod origin"
                    value={installedOrigin}
                    options={[
                      ["all", "All origins"],
                      ["modpack", "Modpack"],
                      ["added", "Added"],
                      ["local", "Local file"],
                    ]}
                    onChange={(value) => {
                      setInstalledOrigin(value as typeof installedOrigin);
                      setInstalledPage(1);
                    }}
                    compact
                  />
                </div>
              </div>
              <span className="shrink-0 font-mono text-[10px] text-app-muted">
                {resolutionQuery.isFetching ? (
                  <span className="inline-flex items-center gap-1.5">
                    <LoaderCircle
                      size={11}
                      className="animate-spin motion-reduce:animate-none"
                      aria-hidden="true"
                    />
                    Matching providers
                  </span>
                ) : (
                  `${visibleInstalled.length} shown · ${enabledModCount} enabled · ${installed.length} total`
                )}
              </span>
            </div>
            {visibleInstalled.length ? (
              <div className="overflow-x-auto">
                <table className="w-full min-w-[900px] table-fixed border-collapse text-left">
                  <caption className="sr-only">
                    Installed mods for {instance.name}
                  </caption>
                  <colgroup>
                    <col className="w-[29%]" />
                    <col className="w-[14%]" />
                    <col className="w-[12%]" />
                    <col className="w-[11%]" />
                    <col className="w-[8%]" />
                    <col className="w-[14%]" />
                    <col className="w-[12%]" />
                  </colgroup>
                  <thead className="bg-app-bg/35">
                    <tr className="border-b border-app-separator/55">
                      <ModTableSortHeader
                        label="Mod"
                        sortKey="name"
                        sort={installedSort}
                        onSort={updateInstalledSort}
                      />
                      <ModTableSortHeader
                        label="Source"
                        sortKey="source"
                        sort={installedSort}
                        onSort={updateInstalledSort}
                      />
                      <ModTableSortHeader
                        label="Version"
                        sortKey="version"
                        sort={installedSort}
                        onSort={updateInstalledSort}
                      />
                      <ModTableSortHeader
                        label="Status"
                        sortKey="status"
                        sort={installedSort}
                        onSort={updateInstalledSort}
                      />
                      <ModTableSortHeader
                        label="Size"
                        sortKey="size"
                        sort={installedSort}
                        onSort={updateInstalledSort}
                        align="right"
                      />
                      <ModTableSortHeader
                        label="Installed"
                        sortKey="installed"
                        sort={installedSort}
                        onSort={updateInstalledSort}
                      />
                      <th
                        scope="col"
                        className="px-4 py-2.5 text-right text-[9px] font-bold tracking-[.08em] text-app-muted uppercase"
                      >
                        Actions
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {pagedInstalled.map((item) => (
                      <InstalledModRow
                        key={item.filePath}
                        item={item}
                        disabled={installing || contentMutationPending}
                        confirmingRemove={removeTargetPath === item.filePath}
                        pendingAction={
                          toggleMutation.isPending &&
                          toggleMutation.variables?.item.filePath ===
                            item.filePath
                            ? "toggle"
                            : pinModMutation.isPending &&
                                pinModMutation.variables?.item.filePath === item.filePath
                              ? "pin"
                            : removeMutation.isPending &&
                                removeMutation.variables?.filePath ===
                                  item.filePath
                              ? "remove"
                              : undefined
                        }
                        onToggle={() =>
                          toggleMutation.mutate({
                            item,
                            enabled: !item.enabled,
                          })
                        }
                        onPin={() =>
                          pinModMutation.mutate({ item, pinned: !item.pinned })
                        }
                        onRequestRemove={() =>
                          setRemoveTargetPath(item.filePath)
                        }
                        onCancelRemove={() => setRemoveTargetPath(undefined)}
                        onConfirmRemove={() => removeMutation.mutate(item)}
                      />
                    ))}
                  </tbody>
                </table>
                <div className="flex min-w-[900px] items-center justify-between border-t border-app-separator/45 bg-app-bg/20 px-4 py-2.5">
                  <span className="font-mono text-[9px] text-app-muted">
                    Rows {installedPageStart + 1}–
                    {Math.min(
                      installedPageStart + installedPageSize,
                      visibleInstalled.length,
                    )}{" "}
                    of {visibleInstalled.length}
                  </span>
                  <div className="flex items-center gap-2">
                    <span className="text-[10px] text-app-muted">Rows</span>
                    <div className="w-20">
                      <ContentSelect
                        label="Rows per page"
                        value={String(installedPageSize)}
                        options={[
                          ["25", "25"],
                          ["50", "50"],
                          ["100", "100"],
                        ]}
                        onChange={(value) => {
                          setInstalledPageSize(Number(value));
                          setInstalledPage(1);
                        }}
                        compact
                      />
                    </div>
                    <span className="min-w-20 text-center font-mono text-[9px] text-app-muted">
                      Page {activeInstalledPage} of {installedPageCount}
                    </span>
                    <button
                      type="button"
                      className="inline-flex size-8 items-center justify-center rounded-control border border-app-separator bg-app-bg text-app-secondary disabled:opacity-35"
                      disabled={activeInstalledPage === 1}
                      onClick={() =>
                        setInstalledPage((current) => Math.max(1, current - 1))
                      }
                      aria-label="Previous installed mod page"
                    >
                      <ChevronLeft size={14} aria-hidden="true" />
                    </button>
                    <button
                      type="button"
                      className="inline-flex size-8 items-center justify-center rounded-control border border-app-separator bg-app-bg text-app-secondary disabled:opacity-35"
                      disabled={activeInstalledPage === installedPageCount}
                      onClick={() =>
                        setInstalledPage((current) =>
                          Math.min(installedPageCount, current + 1),
                        )
                      }
                      aria-label="Next installed mod page"
                    >
                      <ArrowRight size={14} aria-hidden="true" />
                    </button>
                  </div>
                </div>
              </div>
            ) : (
              <div className="px-5 py-14 text-center">
                <p className="m-0 text-sm font-bold text-app-text">
                  No installed mods match
                </p>
                <p className="mt-1 mb-0 text-[11px] text-app-secondary">
                  Clear the search or filters to show the complete table.
                </p>
                <button
                  type="button"
                  className="mt-3 h-8 rounded-control border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-secondary hover:text-app-text"
                  onClick={() => {
                    setInstalledFilter("");
                    setInstalledStatus("all");
                    setInstalledOrigin("all");
                    setInstalledPage(1);
                  }}
                >
                  Clear filters
                </button>
              </div>
            )}
          </>
        ) : !browserOpen && !isVanilla ? (
          <EmptyState
            title="No mod JARs detected"
            description="Add a compatible mod from CurseForge or Modrinth, or install a modpack from Discover."
          />
        ) : null}
      </section>
    </div>
  );
}

type ContentSectionKind = "mods" | InstanceContentKind;

function ContentNavigation({
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

function InstanceFileContent({
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

function ModSearchResult({
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

function InstalledModRow({
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

function ModTableSortHeader({
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

function compareInstalledMods(
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

function modOriginLabel(origin: InstanceMod["origin"]) {
  if (origin === "modpack") return "From modpack";
  if (origin === "added") return "Added in slate";
  return "Unmanaged file";
}

function formatContentFileSize(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function normalizedModPath(value: string) {
  return value
    .replaceAll("\\", "/")
    .toLocaleLowerCase()
    .replace(/\.disabled$/, "");
}

function ContentSelect({
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

function ModResultSkeletons() {
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

function InstalledModSkeletons() {
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

function modProviderName(provider: Provider) {
  if (provider === "curseforge") return "CurseForge";
  if (provider === "modrinth") return "Modrinth";
  return "FTB";
}

function formatCompactNumber(value: number) {
  return new Intl.NumberFormat(undefined, { notation: "compact" }).format(
    value,
  );
}

function contentErrorMessage(error: unknown) {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (
    typeof error === "object" &&
    error !== null &&
    "userMessage" in error &&
    typeof error.userMessage === "string" &&
    error.userMessage.trim()
  )
    return error.userMessage;
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string" &&
    error.message.trim()
  )
    return error.message;
  return "slate could not complete that content request. Try again.";
}

function InstanceSettings({ instance }: { instance: LauncherInstance }) {
  const queryClient = useQueryClient();
  const [section, setSection] = useState<
    "profile" | "launch" | "runtime" | "game" | "safety"
  >("profile");
  const [draft, setDraft] = useState(() => instanceSettingsDraft(instance));
  const [minecraftVersion, setMinecraftVersion] = useState(
    instance.minecraftVersion,
  );
  const [loaderKind, setLoaderKind] = useState<LoaderKind>(instance.loaderKind);
  const [loaderSelection, setLoaderSelection] = useState({
    catalogKey: `${instance.minecraftVersion}|${instance.loaderKind}`,
    value: instance.loaderVersion ?? "",
  });
  const [message, setMessage] = useState<string>();
  const accountsQuery = useQuery({
    queryKey: ["accounts"],
    queryFn: listAccounts,
  });

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

  const runtimeMutation = useMutation({
    mutationFn: updateInstanceConfiguration,
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      const runtimeChanged =
        updated.minecraftVersion !== instance.minecraftVersion ||
        updated.loaderKind !== instance.loaderKind ||
        updated.loaderVersion !== instance.loaderVersion;
      setMessage(
        runtimeChanged
          ? "Runtime changed. Reinstall the instance before launching."
          : "Runtime configuration saved. Existing game files were kept.",
      );
    },
  });

  const settingsMutation = useMutation({
    mutationFn: async () => {
      let current = instance;
      if (draft.name.trim() !== instance.name) {
        current = await renameInstance({
          id: instance.id,
          name: draft.name,
          expectedRevision: current.revision,
        });
      }
      return updateInstanceSettings({
        id: instance.id,
        description: draft.description,
        notes: draft.notes,
        groupName: cleanOptional(draft.groupName),
        tags: draft.tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
        preferredAccountId: cleanOptional(draft.preferredAccountId),
        bannerPositionX: draft.bannerPositionX,
        bannerPositionY: draft.bannerPositionY,
        windowMode: draft.windowMode,
        resolutionWidth:
          draft.windowMode === "windowed"
            ? numberOrUndefined(draft.resolutionWidth)
            : undefined,
        resolutionHeight:
          draft.windowMode === "windowed"
            ? numberOrUndefined(draft.resolutionHeight)
            : undefined,
        launcherBehavior: draft.launcherBehavior,
        gameLanguage: draft.gameLanguage,
        quickPlayServer: cleanOptional(draft.quickPlayServer),
        processPriority: draft.processPriority,
        memoryMode: draft.memoryMode,
        initialMemoryMb: draft.initialMemoryMb,
        maximumMemoryMb: draft.maximumMemoryMb,
        javaMode: draft.javaMode,
        performancePreset: draft.performancePreset,
        jvmArguments: draft.jvmArguments
          .split("\n")
          .map((argument) => argument.trim())
          .filter(Boolean),
        environment: parseEnvironmentOverrides(draft.environment),
        backupBeforeChanges: draft.backupBeforeChanges,
        backupRetention: draft.backupRetention,
        logRetentionDays: draft.logRetentionDays,
        expectedRevision: current.revision,
      });
    },
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setMessage("Instance settings saved. They apply on the next launch.");
    },
  });

  const javaMutation = useMutation({
    mutationFn: (mode: "managed" | "detected" | "custom") =>
      selectInstanceJava({
        id: instance.id,
        mode,
        expectedRevision: instance.revision,
      }),
    onSuccess: (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      setMessage("Java selection validated and saved.");
    },
  });

  const artworkMutation = useMutation({
    mutationFn: ({
      kind,
      reset,
    }: {
      kind: "icon" | "banner";
      reset?: boolean;
    }) =>
      (reset ? resetInstanceArtwork : selectInstanceArtwork)({
        id: instance.id,
        kind,
        expectedRevision: instance.revision,
      }),
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setMessage("Instance artwork updated.");
    },
  });

  const saveRuntime = () => {
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
    runtimeMutation.mutate({
      id: instance.id,
      minecraftVersion,
      loaderKind,
      loaderVersion: selectedLoaderVersion,
      memoryMb: instance.memoryMb,
      expectedRevision: instance.revision,
    });
  };

  const saveSettings = () => {
    setMessage(undefined);
    try {
      parseEnvironmentOverrides(draft.environment);
    } catch (error) {
      setMessage(
        error instanceof Error ? error.message : "Check environment overrides.",
      );
      return;
    }
    settingsMutation.mutate();
  };

  const error =
    settingsMutation.error ??
    runtimeMutation.error ??
    javaMutation.error ??
    artworkMutation.error;
  const busy =
    settingsMutation.isPending ||
    runtimeMutation.isPending ||
    javaMutation.isPending ||
    artworkMutation.isPending;

  return (
    <div className="grid grid-cols-[190px_minmax(0,1fr)] overflow-hidden rounded-control border border-app-separator/70 bg-app-surface">
      <nav className="border-r border-app-separator/60 bg-app-bg/35 p-3" aria-label="Instance settings">
        <p className="px-3 pt-1 pb-2 font-mono text-[10px] uppercase tracking-[0.12em] text-app-muted">
          Instance settings
        </p>
        {([
          ["profile", "Profile", UserRound],
          ["launch", "Launch", Monitor],
          ["runtime", "Java & performance", Cpu],
          ["game", "Game configuration", Settings2],
          ["safety", "Lifecycle & safety", FolderArchive],
        ] as const).map(([value, label, Icon]) => (
          <button
            key={value}
            type="button"
            className={`flex h-10 w-full items-center gap-2 rounded-control px-3 text-left text-xs font-semibold transition-colors ${section === value ? "bg-app-accent/12 text-app-accent" : "text-app-secondary hover:bg-app-raised hover:text-app-text"}`}
            onClick={() => setSection(value)}
          >
            <Icon size={15} aria-hidden="true" />
            {label}
          </button>
        ))}
        <div className="mt-5 border-t border-app-separator/55 px-3 pt-4">
          <span className="font-mono text-[10px] text-app-muted">
            REVISION {instance.revision}
          </span>
          <p className="mt-2 mb-0 text-[11px]/[16px] text-app-secondary">
            Every write is revision guarded. Stale views cannot overwrite newer changes.
          </p>
        </div>
      </nav>

      <section className="min-w-0">
        <div className="min-h-[520px] p-6">
          {section === "profile" ? (
            <SettingsPanel
              title="Profile"
              description="Make this instance easy to recognize without changing its Minecraft files."
            >
              <div className="grid grid-cols-2 gap-5">
                <Field label="Instance name">
                  <input
                    className={inputClass}
                    value={draft.name}
                    maxLength={80}
                    onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                  />
                </Field>
                <Field label="Group">
                  <input
                    className={inputClass}
                    value={draft.groupName}
                    placeholder="Ungrouped"
                    maxLength={80}
                    onChange={(event) => setDraft({ ...draft, groupName: event.target.value })}
                  />
                </Field>
              </div>
              <Field label="Description">
                <input
                  className={inputClass}
                  value={draft.description}
                  maxLength={500}
                  placeholder="A short description shown in your library"
                  onChange={(event) => setDraft({ ...draft, description: event.target.value })}
                />
              </Field>
              <Field label="Tags">
                <input
                  className={inputClass}
                  value={draft.tags}
                  placeholder="building, multiplayer, challenge"
                  onChange={(event) => setDraft({ ...draft, tags: event.target.value })}
                />
              </Field>
              <Field label="Instance notes">
                <textarea
                  className={`${inputClass} h-28 resize-y py-3`}
                  value={draft.notes}
                  maxLength={4000}
                  placeholder="Private notes, server rules, goals, or reminders"
                  onChange={(event) => setDraft({ ...draft, notes: event.target.value })}
                />
              </Field>
              <div className="grid grid-cols-2 gap-4 border-t border-app-separator/55 pt-5">
                <ArtworkControl
                  title="Instance icon"
                  description="PNG, JPEG, or WebP up to 2 MB"
                  customized={instance.settings.hasCustomIcon}
                  disabled={artworkMutation.isPending}
                  onChoose={() => artworkMutation.mutate({ kind: "icon" })}
                  onReset={() => artworkMutation.mutate({ kind: "icon", reset: true })}
                />
                <ArtworkControl
                  title="Library banner"
                  description="PNG, JPEG, or WebP up to 8 MB"
                  customized={instance.settings.hasCustomBanner}
                  disabled={artworkMutation.isPending}
                  onChoose={() => artworkMutation.mutate({ kind: "banner" })}
                  onReset={() => artworkMutation.mutate({ kind: "banner", reset: true })}
                />
              </div>
              {instance.settings.hasCustomBanner ? (
                <div className="grid grid-cols-2 gap-5">
                  <RangeField label="Banner horizontal position" value={draft.bannerPositionX} onChange={(value) => setDraft({ ...draft, bannerPositionX: value })} />
                  <RangeField label="Banner vertical position" value={draft.bannerPositionY} onChange={(value) => setDraft({ ...draft, bannerPositionY: value })} />
                </div>
              ) : null}
            </SettingsPanel>
          ) : null}

          {section === "launch" ? (
            <SettingsPanel
              title="Launch behavior"
              description="Choose the account, window, server, and operating-system behavior used for this instance."
            >
              <div className="grid grid-cols-2 gap-5">
                <Field label="Preferred Minecraft account">
                  <select className={inputClass} value={draft.preferredAccountId} onChange={(event) => setDraft({ ...draft, preferredAccountId: event.target.value })}>
                    <option value="">Use default account</option>
                    {(accountsQuery.data ?? []).map((account) => (
                      <option key={account.id} value={account.id}>{account.displayName}</option>
                    ))}
                  </select>
                </Field>
                <Field label="Window mode">
                  <select className={inputClass} value={draft.windowMode} onChange={(event) => setDraft({ ...draft, windowMode: event.target.value as typeof draft.windowMode })}>
                    <option value="windowed">Windowed</option>
                    <option value="maximized">Maximized</option>
                    <option value="fullscreen">Fullscreen</option>
                  </select>
                </Field>
                <Field label="Window width">
                  <input className={inputClass} type="number" min={320} max={16384} disabled={draft.windowMode !== "windowed"} value={draft.resolutionWidth} placeholder="Default" onChange={(event) => setDraft({ ...draft, resolutionWidth: event.target.value })} />
                </Field>
                <Field label="Window height">
                  <input className={inputClass} type="number" min={240} max={16384} disabled={draft.windowMode !== "windowed"} value={draft.resolutionHeight} placeholder="Default" onChange={(event) => setDraft({ ...draft, resolutionHeight: event.target.value })} />
                </Field>
                <Field label="After Minecraft starts">
                  <select className={inputClass} value={draft.launcherBehavior} onChange={(event) => setDraft({ ...draft, launcherBehavior: event.target.value as typeof draft.launcherBehavior })}>
                    <option value="keepOpen">Keep slate open</option>
                    <option value="minimize">Minimize slate</option>
                    <option value="hide">Close the slate window while playing</option>
                  </select>
                </Field>
                <Field label="Game language">
                  <input className={inputClass} value={draft.gameLanguage} placeholder="en_us" onChange={(event) => setDraft({ ...draft, gameLanguage: event.target.value })} />
                </Field>
                <Field label="Quick-join server">
                  <input className={inputClass} value={draft.quickPlayServer} placeholder="play.example.net:25565" onChange={(event) => setDraft({ ...draft, quickPlayServer: event.target.value })} />
                </Field>
                <Field label="Game process priority">
                  <select className={inputClass} value={draft.processPriority} onChange={(event) => setDraft({ ...draft, processPriority: event.target.value as typeof draft.processPriority })}>
                    <option value="low">Low</option>
                    <option value="belowNormal">Below normal</option>
                    <option value="normal">Normal</option>
                    <option value="aboveNormal">Above normal</option>
                    <option value="high">High</option>
                  </select>
                </Field>
              </div>
            </SettingsPanel>
          ) : null}

          {section === "runtime" ? (
            <SettingsPanel
              title="Java & performance"
              description="slate validates runtime compatibility and keeps launch-critical arguments under native control."
            >
              <h3 className="m-0 text-xs font-bold">Minecraft runtime</h3>
              <p className="mt-1 mb-4 text-[11px] text-app-secondary">Changing Minecraft or its loader requires a verified reinstall. Memory-only changes do not.</p>
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
                <div className="col-span-2 flex justify-end">
                  <button type="button" className={secondaryButtonClass} disabled={runtimeMutation.isPending} onClick={saveRuntime}>
                    {runtimeMutation.isPending ? <RotateCcw className="animate-spin" size={14} /> : <Save size={14} />}
                    Save runtime target
                  </button>
                </div>
              </div>

              <div className="border-t border-app-separator/55 pt-5">
                <div className="grid grid-cols-3 gap-3">
                  <ChoiceButton active={draft.memoryMode === "auto"} title="Automatic memory" description={`${instance.modCount} installed mods`} onClick={() => setDraft({ ...draft, memoryMode: "auto" })} />
                  <ChoiceButton active={draft.memoryMode === "custom"} title="Manual memory" description="Set exact limits" onClick={() => setDraft({ ...draft, memoryMode: "custom" })} />
                  <div className="rounded-control border border-app-separator/70 bg-app-bg/40 px-4 py-3">
                    <span className="block text-xs font-bold">Current recommendation</span>
                    <span className="mt-1 block font-mono text-[11px] text-app-accent">{formatMemory(draft.memoryMode === "auto" ? instance.settings.effectiveMemoryMb : draft.maximumMemoryMb)}</span>
                  </div>
                </div>
                <div className="mt-5 grid grid-cols-2 gap-5">
                  <NumberField label="Initial memory (MB)" value={draft.initialMemoryMb} min={256} max={32768} step={256} onChange={(value) => setDraft({ ...draft, initialMemoryMb: value })} />
                  <NumberField label="Maximum memory (MB)" value={draft.maximumMemoryMb} min={1024} max={32768} step={256} disabled={draft.memoryMode === "auto"} onChange={(value) => setDraft({ ...draft, maximumMemoryMb: value })} />
                </div>
              </div>

              <div className="border-t border-app-separator/55 pt-5">
                <h3 className="m-0 text-xs font-bold">Java runtime</h3>
                <p className="mt-1 mb-4 text-[11px] text-app-secondary">Minecraft {instance.minecraftVersion} requires Java {requiredJavaLabel(instance.minecraftVersion)}. A selection is saved only after slate probes it.</p>
                <div className="grid grid-cols-3 gap-3">
                  <ChoiceButton active={instance.settings.javaMode === "managed"} title="Managed" description="Installed by slate" disabled={javaMutation.isPending} onClick={() => javaMutation.mutate("managed")} />
                  <ChoiceButton active={instance.settings.javaMode === "detected"} title="System Java" description="Detect from PATH" disabled={javaMutation.isPending} onClick={() => javaMutation.mutate("detected")} />
                  <ChoiceButton active={instance.settings.javaMode === "custom"} title="Custom executable" description="Choose java or javaw" disabled={javaMutation.isPending} onClick={() => javaMutation.mutate("custom")} />
                </div>
                {instance.settings.customJavaLabel ? <p className="mt-3 mb-0 truncate font-mono text-[10px] text-app-muted">{instance.settings.customJavaLabel}</p> : null}
              </div>

              <div className="border-t border-app-separator/55 pt-5">
                <Field label="Performance preset">
                  <select className={inputClass} value={draft.performancePreset} onChange={(event) => setDraft({ ...draft, performancePreset: event.target.value as typeof draft.performancePreset })}>
                    <option value="balanced">Balanced</option>
                    <option value="throughput">Throughput</option>
                    <option value="lowLatency">Low latency</option>
                    <option value="custom">Custom</option>
                  </select>
                </Field>
                <details className="mt-5 border-t border-app-separator/55 pt-4">
                  <summary className="flex cursor-pointer list-none items-center gap-2 text-xs font-bold"><ChevronDown size={14} />Advanced launch overrides</summary>
                  <p className="mt-2 text-[11px] text-app-secondary">One argument or KEY=value environment override per line. slate rejects memory, classpath, agent, path, and Java control variables.</p>
                  <div className="grid grid-cols-2 gap-5">
                    <Field label="Additional JVM arguments">
                      <textarea className={`${inputClass} h-32 resize-y py-3 font-mono text-[11px]`} value={draft.jvmArguments} placeholder="-Dexample=value" onChange={(event) => setDraft({ ...draft, jvmArguments: event.target.value })} />
                    </Field>
                    <Field label="Environment overrides">
                      <textarea className={`${inputClass} h-32 resize-y py-3 font-mono text-[11px]`} value={draft.environment} placeholder="EXAMPLE=value" onChange={(event) => setDraft({ ...draft, environment: event.target.value })} />
                    </Field>
                  </div>
                </details>
              </div>
            </SettingsPanel>
          ) : null}

          {section === "game" ? (
            <GameOptionsEditor instance={instance} />
          ) : null}

          {section === "safety" ? (
            <SettingsPanel title="Lifecycle & safety" description="Keep recoverable history around content and runtime changes.">
              <label className="flex items-start justify-between gap-6 border-b border-app-separator/55 pb-5">
                <span><strong className="block text-xs">Snapshot before managed changes</strong><span className="mt-1 block text-[11px] text-app-secondary">Create a restore point before pack, runtime, or bulk content changes.</span></span>
                <input type="checkbox" checked={draft.backupBeforeChanges} onChange={(event) => setDraft({ ...draft, backupBeforeChanges: event.target.checked })} />
              </label>
              <div className="grid grid-cols-2 gap-5">
                <NumberField label="Snapshots to keep" value={draft.backupRetention} min={1} max={50} onChange={(value) => setDraft({ ...draft, backupRetention: value })} />
                <NumberField label="Log retention (days)" value={draft.logRetentionDays} min={1} max={365} onChange={(value) => setDraft({ ...draft, logRetentionDays: value })} />
              </div>
              <LifecycleActions instance={instance} />
              <div className="border-t border-app-separator/55 pt-5">
                <h3 className="m-0 text-xs font-bold">Repair</h3>
                <p className="mt-1 mb-4 text-[11px] text-app-secondary">Verify the selected runtime revision and download only missing or changed files.</p>
                <button type="button" className={secondaryButtonClass} onClick={() => void installInstance({ id: instance.id, expectedRevision: instance.revision })}><RotateCcw size={14} />Repair and verify</button>
              </div>
            </SettingsPanel>
          ) : null}
        </div>

        <footer className="flex min-h-16 items-center justify-between border-t border-app-separator/60 bg-app-bg/30 px-6 py-3">
          <p className={`m-0 text-xs ${error ? "text-app-danger" : "text-app-secondary"}`} role={error ? "alert" : "status"}>
            {error ? contentErrorMessage(error) : message}
          </p>
          {section === "game" ? (
            <span className="text-[11px] text-app-muted">Game configuration has its own guarded save action.</span>
          ) : (
            <button type="button" className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-50" disabled={busy} onClick={saveSettings}>
              {settingsMutation.isPending ? <RotateCcw className="animate-spin" size={16} /> : <Check size={16} />}
              Save settings
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}

function GameOptionsEditor({ instance }: { instance: LauncherInstance }) {
  const query = useQuery({
    queryKey: ["instance-game-options", instance.id],
    queryFn: () => getInstanceGameOptions(instance.id),
  });
  if (query.isPending) {
    return <div className="h-80 animate-pulse rounded-control bg-app-raised" aria-label="Loading game configuration" />;
  }
  if (query.isError) {
    return <InlineNotice tone="warning" title="Game configuration unavailable">slate could not read options.txt. Close Minecraft and try again.</InlineNotice>;
  }
  return (
    <GameOptionsForm
      key={`${instance.revision}:${JSON.stringify(query.data.values)}`}
      instance={instance}
      fileExists={query.data.fileExists}
      initialValues={query.data.values}
    />
  );
}

function LifecycleActions({ instance }: { instance: LauncherInstance }) {
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
  const importMutation = useMutation({
    mutationFn: importInstance,
    onSuccess: async (imported) => {
      if (!imported) return;
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setMessage(`${imported.name} was imported as a new instance. Repair it once before playing.`);
    },
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
      setMessage("Snapshot restored. Personal game files now match that restore point.");
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
    importMutation.error ??
    createMutation.error ??
    restoreMutation.error ??
    deleteMutation.error ??
    pinMutation.error;

  return (
    <>
      <section className="border-t border-app-separator/55 pt-5">
        <h3 className="m-0 text-xs font-bold">Open instance folders</h3>
        <p className="mt-1 mb-4 text-[11px] text-app-secondary">Paths stay behind the native boundary. slate creates a missing standard folder before opening it.</p>
        <div className="flex flex-wrap gap-2">
          {([
            ["game", "Game"], ["mods", "Mods"], ["saves", "Worlds"],
            ["screenshots", "Screenshots"], ["logs", "Logs"], ["crashReports", "Crash reports"],
          ] as const).map(([kind, label]) => (
            <button key={kind} type="button" className={secondaryButtonClass} onClick={() => void openInstanceDirectory(instance.id, kind)}>
              <FolderOpen size={14} />{label}
            </button>
          ))}
        </div>
        <div className="mt-4 flex items-center justify-between gap-5 rounded-control border border-app-separator/60 bg-app-bg/35 px-4 py-3">
          <span className="min-w-0">
            <strong className="block text-[11px]">Storage location</strong>
            <span className="mt-1 block truncate font-mono text-[10px] text-app-muted" title={instance.storagePath}>{instance.storagePath}</span>
          </span>
          <button type="button" className={secondaryButtonClass} disabled={moveMutation.isPending} onClick={() => moveMutation.mutate()}>
            {moveMutation.isPending ? <RotateCcw className="animate-spin" size={14} /> : <ArrowRight size={14} />}Move…
          </button>
        </div>
      </section>

      <section className="border-t border-app-separator/55 pt-5">
        <h3 className="m-0 text-xs font-bold">Duplicate instance</h3>
        <p className="mt-1 mb-4 text-[11px] text-app-secondary">Create a clean copy of this runtime definition, then choose which personal files follow it.</p>
        <div className="grid grid-cols-[minmax(0,1fr)_auto] gap-3">
          <input className="h-9 rounded-control border border-app-separator bg-app-bg px-3 text-xs outline-none focus:border-app-accent" value={duplicateName} maxLength={80} onChange={(event) => setDuplicateName(event.target.value)} />
          <button type="button" className={secondaryButtonClass} disabled={!duplicateName.trim() || duplicateMutation.isPending} onClick={() => duplicateMutation.mutate()}>
            {duplicateMutation.isPending ? <RotateCcw className="animate-spin" size={14} /> : <Copy size={14} />}Duplicate
          </button>
        </div>
        <div className="mt-3 flex flex-wrap gap-5 text-[11px] text-app-secondary">
          <label className="flex items-center gap-2"><input type="checkbox" checked={copyWorlds} onChange={(event) => setCopyWorlds(event.target.checked)} />Worlds</label>
          <label className="flex items-center gap-2"><input type="checkbox" checked={copyScreenshots} onChange={(event) => setCopyScreenshots(event.target.checked)} />Screenshots</label>
          <label className="flex items-center gap-2"><input type="checkbox" checked={copySettings} onChange={(event) => setCopySettings(event.target.checked)} />Settings and artwork</label>
        </div>
      </section>

      <section className="border-t border-app-separator/55 pt-5">
        <div className="flex items-start justify-between gap-6">
          <span>
            <h3 className="m-0 text-xs font-bold">Portable archive</h3>
            <p className="mt-1 mb-0 text-[11px] text-app-secondary">Move a profile, worlds, content, configuration, and artwork between computers. Java paths and cached revisions stay local.</p>
          </span>
          <span className="flex shrink-0 gap-2">
            <button type="button" className={secondaryButtonClass} disabled={importMutation.isPending} onClick={() => importMutation.mutate()}>
              {importMutation.isPending ? <RotateCcw className="animate-spin" size={14} /> : <FolderOpen size={14} />}Import…
            </button>
            <button type="button" className={secondaryButtonClass} disabled={exportMutation.isPending} onClick={() => exportMutation.mutate()}>
              {exportMutation.isPending ? <RotateCcw className="animate-spin" size={14} /> : <Download size={14} />}Export…
            </button>
          </span>
        </div>
      </section>

      <section className="border-t border-app-separator/55 pt-5">
        <div className="flex items-start justify-between gap-5">
          <span><h3 className="m-0 text-xs font-bold">Snapshots</h3><p className="mt-1 mb-0 text-[11px] text-app-secondary">Stopped-state copies of the game directory. Pinned snapshots do not count toward retention.</p></span>
          <button type="button" className={secondaryButtonClass} disabled={createMutation.isPending} onClick={() => createMutation.mutate()}>
            {createMutation.isPending ? <RotateCcw className="animate-spin" size={14} /> : <FolderArchive size={14} />}Create snapshot
          </button>
        </div>
        <div className="mt-4 divide-y divide-app-separator/50 border-y border-app-separator/50">
          {snapshotsQuery.isPending ? <p className="py-4 text-xs text-app-secondary">Loading snapshots…</p> : null}
          {snapshotsQuery.data?.length === 0 ? <p className="py-4 text-xs text-app-secondary">No snapshots yet.</p> : null}
          {snapshotsQuery.data?.map((snapshot) => (
            <div key={snapshot.id} className="flex min-h-12 items-center gap-3 py-2">
              <span className="grid size-8 place-items-center rounded-control bg-app-raised text-app-secondary"><FolderArchive size={14} /></span>
              <span className="min-w-0 flex-1"><strong className="block text-[11px]">{formatDate(snapshot.createdAt)}</strong><span className="font-mono text-[10px] text-app-muted">{formatContentFileSize(snapshot.sizeBytes)}</span></span>
              <button type="button" className="p-2 text-app-secondary hover:text-app-text" title={snapshot.pinned ? "Unpin snapshot" : "Pin snapshot"} onClick={() => pinMutation.mutate({ id: snapshot.id, pinned: !snapshot.pinned })}>{snapshot.pinned ? <PinOff size={14} /> : <Pin size={14} />}</button>
              {restoreTarget === snapshot.id ? (
                <span className="flex items-center gap-2 rounded-control border border-app-warning/45 bg-app-warning/5 px-2 py-1">
                  <span className="text-[10px] text-app-secondary">Replace current files?</span>
                  <button type="button" className="text-[10px] font-bold text-app-warning" disabled={restoreMutation.isPending} onClick={() => restoreMutation.mutate(snapshot.id)}>Confirm</button>
                  <button type="button" className="text-[10px] font-bold text-app-secondary" disabled={restoreMutation.isPending} onClick={() => setRestoreTarget(undefined)}>Cancel</button>
                </span>
              ) : (
                <button type="button" className="text-[11px] font-bold text-app-accent" disabled={restoreMutation.isPending} onClick={() => { setDeleteTarget(undefined); setRestoreTarget(snapshot.id); }}>Restore</button>
              )}
              {deleteTarget === snapshot.id ? (
                <span className="flex items-center gap-2 rounded-control border border-app-danger/45 bg-app-danger/5 px-2 py-1">
                  <span className="text-[10px] text-app-secondary">Delete permanently?</span>
                  <button type="button" className="text-[10px] font-bold text-app-danger" disabled={deleteMutation.isPending} onClick={() => deleteMutation.mutate(snapshot.id)}>Delete</button>
                  <button type="button" className="text-[10px] font-bold text-app-secondary" disabled={deleteMutation.isPending} onClick={() => setDeleteTarget(undefined)}>Cancel</button>
                </span>
              ) : (
                <button type="button" className="p-2 text-app-muted hover:text-app-danger" title="Delete snapshot" disabled={deleteMutation.isPending} onClick={() => { setRestoreTarget(undefined); setDeleteTarget(snapshot.id); }}><Trash2 size={14} /></button>
              )}
            </div>
          ))}
        </div>
      </section>

      {error || message ? <p className={`m-0 text-xs ${error ? "text-app-danger" : "text-app-secondary"}`} role={error ? "alert" : "status"}>{error ? contentErrorMessage(error) : message}</p> : null}
    </>
  );
}

const defaultGameOptionValues: Record<string, string> = {
  graphicsMode: "1",
  renderDistance: "12",
  simulationDistance: "12",
  guiScale: "0",
  entityDistanceScaling: "1.0",
  particles: "0",
  mipmapLevels: "4",
  enableVsync: "true",
  maxFps: "120",
  bobView: "true",
  mouseSensitivity: "0.5",
  invertYMouse: "false",
  autoJump: "false",
  toggleCrouch: "false",
  toggleSprint: "false",
  showSubtitles: "false",
  narrator: "0",
  chatVisibility: "0",
  chatOpacity: "1.0",
  textBackgroundOpacity: "0.5",
  darknessEffectScale: "1.0",
  damageTiltStrength: "1.0",
  directionalAudio: "false",
  realmsNotifications: "true",
  allowServerListing: "true",
  chatLinks: "true",
  chatLinksPrompt: "true",
  soundCategory_master: "1.0",
  soundCategory_music: "1.0",
  soundCategory_weather: "1.0",
  soundCategory_hostile: "1.0",
  soundCategory_player: "1.0",
};

function GameOptionsForm({
  instance,
  fileExists,
  initialValues,
}: {
  instance: LauncherInstance;
  fileExists: boolean;
  initialValues: Record<string, string>;
}) {
  const queryClient = useQueryClient();
  const [values, setValues] = useState(() => ({
    ...defaultGameOptionValues,
    ...initialValues,
  }));
  const mutation = useMutation({
    mutationFn: () =>
      updateInstanceGameOptions({
        id: instance.id,
        values,
        expectedRevision: instance.revision,
      }),
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({
        queryKey: ["instance-game-options", instance.id],
      });
    },
  });
  const update = (key: string, value: string) =>
    setValues((current) => ({ ...current, [key]: value }));
  const resetCategory = (keys: string[]) =>
    setValues((current) => ({
      ...current,
      ...Object.fromEntries(keys.map((key) => [key, defaultGameOptionValues[key]])),
    }));

  return (
    <SettingsPanel
      title="Game configuration"
      description="Edit common Minecraft options without discarding settings added by the game or mods."
    >
      <InlineNotice title="Unknown keys are preserved">
        {fileExists
          ? "slate updates only the recognized keys below. Mod-specific and future Minecraft options stay byte-for-byte unchanged."
          : "Minecraft has not created options.txt yet. Saving creates it with only the recognized choices below."}
      </InlineNotice>

      <GameOptionSection title="Video" onReset={() => resetCategory(["graphicsMode", "renderDistance", "simulationDistance", "maxFps", "guiScale", "entityDistanceScaling", "enableVsync", "bobView"])}>
        <Field label="Graphics quality">
          <select className={inputClass} value={values.graphicsMode} onChange={(event) => update("graphicsMode", event.target.value)}>
            <option value="0">Fast</option><option value="1">Fancy</option><option value="2">Fabulous</option>
          </select>
        </Field>
        <GameOptionNumber label="Render distance" value={values.renderDistance} min={2} max={64} suffix="chunks" onChange={(value) => update("renderDistance", value)} />
        <GameOptionNumber label="Simulation distance" value={values.simulationDistance} min={2} max={32} suffix="chunks" onChange={(value) => update("simulationDistance", value)} />
        <GameOptionNumber label="Maximum frame rate" value={values.maxFps} min={10} max={260} suffix="FPS" onChange={(value) => update("maxFps", value)} />
        <GameOptionNumber label="GUI scale" value={values.guiScale} min={0} max={8} onChange={(value) => update("guiScale", value)} />
        <GameOptionRange label="Entity distance" value={values.entityDistanceScaling} min={0.5} max={5} step={0.1} onChange={(value) => update("entityDistanceScaling", value)} />
        <GameOptionCheckbox label="Vertical sync" checked={values.enableVsync === "true"} onChange={(checked) => update("enableVsync", String(checked))} />
        <GameOptionCheckbox label="View bobbing" checked={values.bobView === "true"} onChange={(checked) => update("bobView", String(checked))} />
      </GameOptionSection>

      <GameOptionSection title="Audio" onReset={() => resetCategory(["soundCategory_master", "soundCategory_music", "soundCategory_weather", "soundCategory_hostile", "soundCategory_player", "directionalAudio"])}>
        <GameOptionRange label="Master volume" value={values.soundCategory_master} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_master", value)} />
        <GameOptionRange label="Music" value={values.soundCategory_music} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_music", value)} />
        <GameOptionRange label="Weather" value={values.soundCategory_weather} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_weather", value)} />
        <GameOptionRange label="Hostile creatures" value={values.soundCategory_hostile} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_hostile", value)} />
        <GameOptionRange label="Players" value={values.soundCategory_player} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_player", value)} />
        <GameOptionCheckbox label="Directional audio" checked={values.directionalAudio === "true"} onChange={(checked) => update("directionalAudio", String(checked))} />
      </GameOptionSection>

      <GameOptionSection title="Controls & accessibility" onReset={() => resetCategory(["mouseSensitivity", "chatOpacity", "darknessEffectScale", "damageTiltStrength", "autoJump", "invertYMouse", "toggleCrouch", "toggleSprint", "showSubtitles"])}>
        <GameOptionRange label="Mouse sensitivity" value={values.mouseSensitivity} min={0} max={1} step={0.01} percentage onChange={(value) => update("mouseSensitivity", value)} />
        <GameOptionRange label="Chat opacity" value={values.chatOpacity} min={0} max={1} step={0.01} percentage onChange={(value) => update("chatOpacity", value)} />
        <GameOptionRange label="Darkness pulse strength" value={values.darknessEffectScale} min={0} max={1} step={0.01} percentage onChange={(value) => update("darknessEffectScale", value)} />
        <GameOptionRange label="Damage tilt strength" value={values.damageTiltStrength} min={0} max={1} step={0.01} percentage onChange={(value) => update("damageTiltStrength", value)} />
        <GameOptionCheckbox label="Auto jump" checked={values.autoJump === "true"} onChange={(checked) => update("autoJump", String(checked))} />
        <GameOptionCheckbox label="Invert mouse" checked={values.invertYMouse === "true"} onChange={(checked) => update("invertYMouse", String(checked))} />
        <GameOptionCheckbox label="Toggle crouch" checked={values.toggleCrouch === "true"} onChange={(checked) => update("toggleCrouch", String(checked))} />
        <GameOptionCheckbox label="Toggle sprint" checked={values.toggleSprint === "true"} onChange={(checked) => update("toggleSprint", String(checked))} />
        <GameOptionCheckbox label="Show subtitles" checked={values.showSubtitles === "true"} onChange={(checked) => update("showSubtitles", String(checked))} />
      </GameOptionSection>

      <GameOptionSection title="Multiplayer" onReset={() => resetCategory(["allowServerListing", "realmsNotifications", "chatLinks", "chatLinksPrompt"])}>
        <GameOptionCheckbox label="Allow server listing" checked={values.allowServerListing === "true"} onChange={(checked) => update("allowServerListing", String(checked))} />
        <GameOptionCheckbox label="Realms notifications" checked={values.realmsNotifications === "true"} onChange={(checked) => update("realmsNotifications", String(checked))} />
        <GameOptionCheckbox label="Open links in chat" checked={values.chatLinks === "true"} onChange={(checked) => update("chatLinks", String(checked))} />
        <GameOptionCheckbox label="Prompt before opening links" checked={values.chatLinksPrompt === "true"} onChange={(checked) => update("chatLinksPrompt", String(checked))} />
      </GameOptionSection>

      <div className="flex items-center justify-between border-t border-app-separator/55 pt-5">
        <p className={`m-0 text-xs ${mutation.isError ? "text-app-danger" : "text-app-secondary"}`} role={mutation.isError ? "alert" : "status"}>
          {mutation.isError ? contentErrorMessage(mutation.error) : mutation.isSuccess ? "Game configuration saved." : "Changes apply on the next launch."}
        </p>
        <button type="button" className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-50" disabled={mutation.isPending} onClick={() => mutation.mutate()}>
          {mutation.isPending ? <RotateCcw className="animate-spin" size={15} /> : <Save size={15} />}
          Save game configuration
        </button>
      </div>
    </SettingsPanel>
  );
}

function GameOptionSection({ title, onReset, children }: { title: string; onReset: () => void; children: ReactNode }) {
  return (
    <section className="grid grid-cols-2 gap-x-5 gap-y-4 border-t border-app-separator/55 pt-5">
      <div className="col-span-2 flex items-center justify-between">
        <h3 className="m-0 text-xs font-bold">{title}</h3>
        <button type="button" className="text-[10px] font-bold text-app-secondary hover:text-app-text" onClick={onReset}>Reset category</button>
      </div>
      {children}
    </section>
  );
}

function GameOptionCheckbox({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="flex h-10 items-center justify-between rounded-control border border-app-separator/70 bg-app-bg/35 px-3 text-xs font-semibold">
      {label}
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
    </label>
  );
}

function GameOptionNumber({ label, value, min, max, suffix, onChange }: { label: string; value: string; min: number; max: number; suffix?: string; onChange: (value: string) => void }) {
  return (
    <Field label={label}>
      <span className="relative block">
        <input className={`${inputClass} ${suffix ? "pr-16" : ""}`} type="number" min={min} max={max} value={value} onChange={(event) => onChange(event.target.value)} />
        {suffix ? <span className="absolute right-3 bottom-3 text-[10px] text-app-muted">{suffix}</span> : null}
      </span>
    </Field>
  );
}

function GameOptionRange({ label, value, min, max, step, percentage = false, onChange }: { label: string; value: string; min: number; max: number; step: number; percentage?: boolean; onChange: (value: string) => void }) {
  const numeric = Number(value);
  return (
    <label className="block text-xs font-bold">
      <span className="flex justify-between"><span>{label}</span><span className="font-mono text-[10px] text-app-muted">{percentage ? `${Math.round(numeric * 100)}%` : numeric.toFixed(1)}</span></span>
      <input className="mt-3 w-full accent-app-accent" type="range" min={min} max={max} step={step} value={value} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}

type InstanceSettingsDraft = {
  name: string;
  description: string;
  notes: string;
  groupName: string;
  tags: string;
  bannerPositionX: number;
  bannerPositionY: number;
  preferredAccountId: string;
  windowMode: LauncherInstance["settings"]["windowMode"];
  resolutionWidth: string;
  resolutionHeight: string;
  launcherBehavior: LauncherInstance["settings"]["launcherBehavior"];
  gameLanguage: string;
  quickPlayServer: string;
  processPriority: LauncherInstance["settings"]["processPriority"];
  memoryMode: LauncherInstance["settings"]["memoryMode"];
  initialMemoryMb: number;
  maximumMemoryMb: number;
  javaMode: LauncherInstance["settings"]["javaMode"];
  performancePreset: LauncherInstance["settings"]["performancePreset"];
  jvmArguments: string;
  environment: string;
  backupBeforeChanges: boolean;
  backupRetention: number;
  logRetentionDays: number;
};

function instanceSettingsDraft(instance: LauncherInstance): InstanceSettingsDraft {
  const settings = instance.settings;
  return {
    name: instance.name,
    description: settings.description,
    notes: settings.notes,
    groupName: settings.groupName ?? "",
    tags: settings.tags.join(", "),
    bannerPositionX: settings.bannerPositionX,
    bannerPositionY: settings.bannerPositionY,
    preferredAccountId: settings.preferredAccountId ?? "",
    windowMode: settings.windowMode,
    resolutionWidth: settings.resolutionWidth?.toString() ?? "",
    resolutionHeight: settings.resolutionHeight?.toString() ?? "",
    launcherBehavior: settings.launcherBehavior,
    gameLanguage: settings.gameLanguage,
    quickPlayServer: settings.quickPlayServer ?? "",
    processPriority: settings.processPriority,
    memoryMode: settings.memoryMode,
    initialMemoryMb: settings.initialMemoryMb,
    maximumMemoryMb: instance.memoryMb,
    javaMode: settings.javaMode,
    performancePreset: settings.performancePreset,
    jvmArguments: settings.jvmArguments.join("\n"),
    environment: Object.entries(settings.environment)
      .map(([key, value]) => `${key}=${value}`)
      .join("\n"),
    backupBeforeChanges: settings.backupBeforeChanges,
    backupRetention: settings.backupRetention,
    logRetentionDays: settings.logRetentionDays,
  };
}

function cleanOptional(value: string) {
  const cleaned = value.trim();
  return cleaned ? cleaned : undefined;
}

function numberOrUndefined(value: string) {
  if (!value.trim()) return undefined;
  const number = Number(value);
  return Number.isInteger(number) ? number : undefined;
}

function parseEnvironmentOverrides(value: string) {
  const result: Record<string, string> = {};
  for (const [index, rawLine] of value.split("\n").entries()) {
    const line = rawLine.trim();
    if (!line) continue;
    const separator = line.indexOf("=");
    if (separator <= 0) {
      throw new Error(`Environment line ${index + 1} must use KEY=value.`);
    }
    const key = line.slice(0, separator).trim();
    const itemValue = line.slice(separator + 1);
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
      throw new Error(`Environment line ${index + 1} has an invalid variable name.`);
    }
    result[key] = itemValue;
  }
  return result;
}

function requiredJavaLabel(version: string) {
  const [major = 0, minor = 0, patch = 0] = version
    .split(/[.-]/)
    .slice(0, 3)
    .map((part) => Number(part));
  if (major === 1 && minor <= 16) return 8;
  if (major === 1 && minor === 17) return 16;
  if (major === 1 && (minor < 20 || (minor === 20 && patch <= 4))) return 17;
  return 21;
}

function formatMemory(value: number) {
  return value % 1024 === 0 ? `${value / 1024} GB` : `${value} MB`;
}

function SettingsPanel({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-5">
      <header>
        <h2 className="m-0 text-[16px] font-bold">{title}</h2>
        <p className="mt-1 mb-0 text-xs text-app-secondary">{description}</p>
      </header>
      {children}
    </div>
  );
}

function ArtworkControl({
  title,
  description,
  customized,
  disabled,
  onChoose,
  onReset,
}: {
  title: string;
  description: string;
  customized: boolean;
  disabled: boolean;
  onChoose: () => void;
  onReset: () => void;
}) {
  return (
    <div className="flex items-center gap-3 rounded-control border border-app-separator/70 bg-app-bg/35 p-3">
      <span className="grid size-10 shrink-0 place-items-center rounded-control bg-app-raised text-app-secondary">
        <ImageIcon size={17} aria-hidden="true" />
      </span>
      <span className="min-w-0 flex-1">
        <strong className="block text-xs">{title}</strong>
        <span className="block truncate text-[10px] text-app-muted">
          {customized ? "Custom image selected" : description}
        </span>
      </span>
      {customized ? (
        <button type="button" className="text-[11px] font-semibold text-app-secondary hover:text-app-text" disabled={disabled} onClick={onReset}>
          Reset
        </button>
      ) : null}
      <button type="button" className="text-[11px] font-bold text-app-accent" disabled={disabled} onClick={onChoose}>
        Choose
      </button>
    </div>
  );
}

function RangeField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="block text-xs font-bold">
      <span className="flex justify-between"><span>{label}</span><span className="font-mono text-[10px] text-app-muted">{value}%</span></span>
      <input className="mt-3 w-full accent-app-accent" type="range" min={0} max={100} value={value} onChange={(event) => onChange(Number(event.target.value))} />
    </label>
  );
}

function ChoiceButton({
  active,
  title,
  description,
  disabled = false,
  onClick,
}: {
  active: boolean;
  title: string;
  description: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button type="button" className={`rounded-control border px-4 py-3 text-left transition-colors ${active ? "border-app-accent bg-app-accent/10" : "border-app-separator/70 bg-app-bg/35 hover:border-app-secondary"}`} disabled={disabled} onClick={onClick}>
      <span className="flex items-center justify-between text-xs font-bold">{title}{active ? <Check size={14} className="text-app-accent" /> : null}</span>
      <span className="mt-1 block text-[10px] text-app-muted">{description}</span>
    </button>
  );
}

function NumberField({
  label,
  value,
  min,
  max,
  step = 1,
  disabled = false,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  disabled?: boolean;
  onChange: (value: number) => void;
}) {
  return (
    <Field label={label}>
      <input className={inputClass} type="number" value={value} min={min} max={max} step={step} disabled={disabled} onChange={(event) => onChange(Number(event.target.value))} />
    </Field>
  );
}

const secondaryButtonClass =
  "inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text hover:border-app-secondary disabled:opacity-50";

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
