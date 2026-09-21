import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import {
  ArrowRight,
  Check,
  ChevronDown,
  CircleAlert,
  Layers3,
  LoaderCircle,
  Newspaper,
  Play,
  Plus,
  RotateCcw,
  Server,
  Square,
} from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import {
  bridgeMode,
  forceStopGameSession,
  getBootstrap,
  getModpack,
  launchInstance,
  listAccounts,
  listGameSessions,
  listInstances,
  listSavedServers,
  previewUpdates,
} from "../../lib/bridge";
import { cn } from "../../lib/cn";
import {
  InstanceArtwork as InstanceProfileArtwork,
  InstanceBanner,
} from "../../components/InstanceArtwork";
import { MinecraftHead } from "../../components/MinecraftHead";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import type {
  LauncherInstance,
  LauncherUpdate,
  SavedServer,
} from "../../types/launcher";

type Filter = "all" | "modded" | "vanilla";
const emptyInstances: LauncherInstance[] = [];

const eyebrowClass =
  "mb-2 text-xs/[16px] font-bold tracking-[.09em] text-app-secondary uppercase";
const controlButtonClass =
  "inline-flex items-center justify-center gap-2 rounded-control border-0 px-[18px] font-bold transition-[background-color,color,transform] duration-[120ms]";
const instanceGridClass =
  "grid grid-cols-[minmax(220px,1.45fr)_minmax(140px,.9fr)_minmax(100px,.65fr)_minmax(90px,.58fr)] items-center gap-4 max-[1180px]:grid-cols-[minmax(190px,1.3fr)_minmax(120px,.75fr)_minmax(90px,.55fr)]";

export function HomePage() {
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string>();
  const [filter, setFilter] = useState<Filter>("all");
  const [confirmStopId, setConfirmStopId] = useState<string>();
  const bootstrapQuery = useQuery({
    queryKey: ["bootstrap"],
    queryFn: getBootstrap,
    refetchInterval: 5 * 60_000,
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
    refetchInterval: 750,
  });
  const savedServersQuery = useQuery({
    queryKey: ["saved-servers"],
    queryFn: listSavedServers,
  });
  const launchMutation = useMutation({
    mutationFn: ({
      instanceId,
      accountId,
    }: {
      instanceId: string;
      accountId: string;
    }) => launchInstance(instanceId, accountId),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["game-sessions"] }),
        queryClient.invalidateQueries({ queryKey: ["instances"] }),
      ]);
    },
  });
  const stopMutation = useMutation({
    mutationFn: forceStopGameSession,
    onSuccess: async () => {
      setConfirmStopId(undefined);
      await queryClient.invalidateQueries({ queryKey: ["game-sessions"] });
    },
  });

  const instances = instancesQuery.data ?? emptyInstances;
  const selected =
    instances.find((instance) => instance.id === selectedId) ?? instances[0];
  const selectedPackSource = selected?.modpackSource;
  const selectedPackArtworkQuery = useQuery({
    queryKey: [
      "modpack",
      selectedPackSource?.provider,
      selectedPackSource?.projectId,
    ],
    queryFn: () =>
      getModpack(
        selectedPackSource?.provider ?? "modrinth",
        selectedPackSource?.projectId ?? "",
      ),
    enabled: Boolean(selectedPackSource && !selectedPackSource.bannerUrl),
    staleTime: 6 * 60 * 60_000,
  });
  const filteredInstances = useMemo(
    () =>
      instances.filter((instance) => {
        if (filter === "all") return true;
        if (filter === "vanilla") return instance.mode === "vanilla";
        return instance.mode !== "vanilla";
      }),
    [filter, instances],
  );

  const launchCapability = bootstrapQuery.data?.capabilities.find(
    (capability) => capability.id === "minecraft.launch",
  );
  const defaultAccount =
    accountsQuery.data?.find(
      (account) => account.isDefault && account.status === "ready",
    ) ?? accountsQuery.data?.find((account) => account.status === "ready");
  const activeSession = sessionsQuery.data?.find(
    (session) => session.instanceId === selected?.id,
  );
  const playReason = activeSession
    ? activeSession.state === "stopping"
      ? "Minecraft is stopping."
      : "Review the warning before force-closing Minecraft."
    : !launchCapability?.available
      ? (launchCapability?.unavailableReason ??
        "Minecraft cannot be launched right now.")
      : selected?.setupState !== "ready"
        ? "Install this instance before launching."
        : !defaultAccount
          ? "Connect a Minecraft account before launching."
          : "Start Minecraft with the default account.";
  const canPlay =
    launchCapability?.available === true &&
    selected?.setupState === "ready" &&
    Boolean(defaultAccount) &&
    !activeSession &&
    !launchMutation.isPending;

  if (instancesQuery.isPending) {
    return <HomeSkeleton />;
  }

  if (instancesQuery.isError) {
    return (
      <section
        className="flex min-h-full flex-col items-center justify-center px-8 py-[10vh] text-center"
        role="alert"
      >
        <CircleAlert
          className="mb-4 text-app-danger"
          size={28}
          aria-hidden="true"
        />
        <p className={eyebrowClass}>Library unavailable</p>
        <h1 className="m-0 text-[34px]/[40px] font-bold tracking-[-.035em]">
          slate could not load your instances.
        </h1>
        <p className="mt-3 mb-6 max-w-[560px] text-app-secondary">
          Your files were not changed. Restart slate and try again.
        </p>
        <button
          type="button"
          className={cn(
            controlButtonClass,
            "h-[34px] border border-app-separator bg-app-raised text-[11px] text-app-text hover:bg-app-hover",
          )}
          onClick={() => void instancesQuery.refetch()}
        >
          <RotateCcw size={17} aria-hidden="true" />
          Retry
        </button>
      </section>
    );
  }

  if (!selected) {
    return <EmptyHome />;
  }

  const playButton = (
    <button
      className={cn(
        controlButtonClass,
        "h-11 w-full text-[17px] active:translate-y-px disabled:bg-app-raised disabled:text-app-muted disabled:opacity-80",
        activeSession
          ? "border border-app-danger/50 bg-app-sidebar text-app-danger hover:bg-app-danger/10"
          : "bg-app-accent text-app-on-accent hover:brightness-105",
      )}
      type="button"
      disabled={
        activeSession?.state === "stopping" ||
        (!activeSession && !canPlay)
      }
      title={playReason}
      onClick={() => {
        if (!activeSession && defaultAccount) {
          launchMutation.mutate({
            instanceId: selected.id,
            accountId: defaultAccount.id,
          });
        }
      }}
    >
      {activeSession ? (
        activeSession.state === "stopping" ? (
          <LoaderCircle
            className="animate-spin"
            size={20}
            aria-hidden="true"
          />
        ) : (
          <Square size={17} fill="currentColor" aria-hidden="true" />
        )
      ) : (
        <Play size={21} fill="currentColor" aria-hidden="true" />
      )}
      {activeSession?.state === "stopping"
        ? "Stopping…"
        : activeSession
          ? "Stop game"
          : launchMutation.isPending
            ? "Starting…"
            : "Play"}
    </button>
  );

  return (
    <div className="min-h-full">
      <section
        className="relative isolate min-h-[360px] overflow-hidden border-b border-app-separator/55 bg-[#18221d]"
        aria-labelledby="continue-heading"
      >
        <InstanceBanner
          instance={selected}
          fallbackBanner={
            selected.modpackSource?.bannerUrl ??
            selectedPackArtworkQuery.data?.banner_url
          }
          fallbackIcon={
            selected.modpackSource?.iconUrl ??
            selectedPackArtworkQuery.data?.icon_url
          }
          eager
          className="absolute inset-0 -z-30 size-full rounded-none opacity-[.86]"
        />
        <div className="hero-shade absolute inset-0 -z-20" aria-hidden="true" />
        <div className="absolute top-[68px] left-8 flex max-w-[620px] items-start gap-5">
          <InstanceProfileArtwork
            instance={selected}
            fallbackSrc={selectedPackArtworkQuery.data?.icon_url}
            className="mt-1 size-[72px] rounded-control border border-app-separator/80 shadow-[0_12px_30px_rgb(0_0_0_/_28%)]"
            eager
          />
          <div className="home-hero-copy min-w-0">
            <p className={eyebrowClass}>Continue playing</p>
            <h1
              id="continue-heading"
              className="m-0 text-[34px]/[40px] font-bold tracking-[-.035em]"
            >
              {selected.name}
            </h1>
            <p className="mt-2.5 mb-0 text-base/[22px] text-app-text">
              {instanceTechnicalLine(selected)}
            </p>
            <p className="mt-1 mb-0 text-[13px] text-app-secondary">
              Last played {formatLastPlayed(selected.lastPlayed, "not yet")}
            </p>
          </div>
        </div>

        <div className="absolute top-14 right-8 grid w-60 gap-2">
          <Link
            className="grid min-h-14 grid-cols-[38px_minmax(0,1fr)_18px] items-center gap-2.5 rounded-control border border-app-separator/70 bg-app-sidebar/95 p-2 text-left text-app-text disabled:opacity-90"
            to="/accounts"
            title="Manage Minecraft accounts"
          >
            <MinecraftHead
              skinUrl={defaultAccount?.skinUrl}
              playerName={defaultAccount?.displayName ?? "slate player"}
              className="size-[38px] text-[13px]"
            />
            <span className="min-w-0">
              <strong className="block overflow-hidden text-[13px] font-bold text-ellipsis whitespace-nowrap">
                {defaultAccount?.displayName ?? "Connect account"}
              </strong>
              <small className="mt-px block overflow-hidden text-[11px] text-app-muted text-ellipsis whitespace-nowrap">
                {defaultAccount
                  ? "Minecraft account"
                  : "Microsoft sign-in required"}
              </small>
            </span>
            <ChevronDown size={17} aria-hidden="true" />
          </Link>
          {activeSession ? (
            <ConfirmDialog
              open={confirmStopId === activeSession.id}
              onOpenChange={(open) =>
                setConfirmStopId(open ? activeSession.id : undefined)
              }
              trigger={playButton}
              title="Force-close Minecraft?"
              description="Minecraft will be stopped immediately. Unsaved world progress may be lost."
              confirmLabel="Force close"
              pendingLabel="Stopping…"
              cancelLabel="Keep running"
              pending={stopMutation.isPending}
              error={stopMutation.isError ? "Minecraft did not stop. Open the instance for details." : undefined}
              destructive
              onConfirm={() => stopMutation.mutate(activeSession.id)}
            />
          ) : playButton}
          {activeSession ? (
            <p className="m-0 rounded-compact bg-app-sidebar/95 px-3 py-2 font-mono text-[10px] text-app-accent">
              Minecraft is running
            </p>
          ) : launchMutation.isSuccess ? (
            <p className="m-0 rounded-compact bg-app-sidebar/95 px-3 py-2 text-[10px] text-app-accent">
              Minecraft started
            </p>
          ) : null}
          {launchMutation.isError || stopMutation.isError ? (
            <p className="m-0 rounded-compact bg-app-danger/10 px-3 py-2 text-[10px] text-app-danger">
              {stopMutation.isError
                ? "Minecraft did not stop. Open the instance for details."
                : "Minecraft did not start. Open the instance for details."}
            </p>
          ) : null}
        </div>

        <div
          className="absolute right-7 bottom-5 left-7 grid grid-cols-4 gap-2.5"
          aria-label="Quick instance switcher"
        >
          {instances.slice(0, 4).map((instance) => (
            <button
              type="button"
              key={instance.id}
              className={cn(
                "relative grid h-[72px] min-w-0 grid-cols-[58px_minmax(0,1fr)] items-center rounded-control border border-app-separator/70 bg-app-sidebar/90 p-[7px] text-left text-app-text transition-colors duration-[120ms] hover:bg-app-hover/95 max-[1180px]:grid-cols-[48px_minmax(0,1fr)]",
                instance.id === selected.id &&
                  "border-app-accent shadow-[inset_0_0_0_1px_var(--slate-accent)]",
              )}
              onClick={() => {
                setSelectedId(instance.id);
                setConfirmStopId(undefined);
              }}
            >
              <InstanceArtwork instance={instance} compact />
              <span className="min-w-0 px-2.5">
                <strong className="block overflow-hidden text-[13px] font-bold text-ellipsis whitespace-nowrap">
                  {instance.name}
                </strong>
                <small className="mt-0.5 block overflow-hidden font-mono text-[10px] text-app-secondary text-ellipsis whitespace-nowrap">
                  {instanceTechnicalLine(instance)}
                </small>
              </span>
              {instance.id === selected.id ? (
                <Check
                  className="absolute top-[7px] right-[7px] rounded-full bg-app-accent p-0.5 text-app-on-accent"
                  size={16}
                  aria-hidden="true"
                />
              ) : null}
            </button>
          ))}
        </div>
      </section>

      <div className="grid min-h-[380px] grid-cols-[minmax(0,1.7fr)_minmax(340px,.9fr)] bg-app-bg max-[1180px]:grid-cols-[minmax(0,1.6fr)_340px]">
        <section
          className="min-w-0 border-r border-app-separator/50 px-7 pt-6 pb-7"
          aria-labelledby="instances-heading"
        >
          <div className="flex items-start justify-between gap-5 max-[1180px]:flex-col max-[1180px]:items-stretch max-[1180px]:gap-3">
            <div>
              <h2
                id="instances-heading"
                className="m-0 text-[21px]/[28px] font-bold tracking-[-.025em]"
              >
                Your instances
              </h2>
              <p className="mt-0.5 mb-0 text-[13px] text-app-secondary">
                Manage your Minecraft setups and pick up where you left off.
              </p>
            </div>
            <div className="flex items-center gap-2.5 max-[1180px]:justify-between">
              <div
                className="flex items-center gap-0.5 rounded-lg bg-app-surface p-[3px]"
                aria-label="Filter instances"
              >
                {(["all", "modded", "vanilla"] as const).map((value) => (
                  <button
                    key={value}
                    className={cn(
                      "h-[30px] rounded-compact border-0 bg-transparent px-3 text-xs font-semibold text-app-secondary transition-colors duration-[120ms] hover:bg-app-hover hover:text-app-text",
                      filter === value &&
                        "bg-app-accent text-app-on-accent hover:bg-app-accent hover:text-app-on-accent",
                    )}
                    type="button"
                    aria-pressed={filter === value}
                    onClick={() => setFilter(value)}
                  >
                    {value[0].toUpperCase() + value.slice(1)}
                  </button>
                ))}
              </div>
              <Link
                className={cn(
                  controlButtonClass,
                  "h-9 rounded-lg bg-app-accent px-3.5 text-xs text-app-on-accent no-underline hover:brightness-105",
                )}
                to="/library/new"
              >
                <Plus size={18} aria-hidden="true" />
                New instance
              </Link>
            </div>
          </div>

          <div className="mt-[18px]">
            <div
              className={cn(
                instanceGridClass,
                "h-8 px-2.5 text-[10px] font-bold tracking-[.06em] text-app-muted uppercase",
              )}
              aria-hidden="true"
            >
              <span>Name</span>
              <span>Version / loader</span>
              <span>Status</span>
              <span className="max-[1180px]:hidden">Last played</span>
            </div>
            {filteredInstances.map((instance) => (
              <button
                type="button"
                className={cn(
                  instanceGridClass,
                  "min-h-[62px] w-full border-0 border-t border-app-separator/45 bg-transparent px-2.5 py-2 text-left text-app-text transition-colors duration-[120ms] hover:bg-app-hover/40",
                  instance.id === selected.id &&
                    "bg-app-hover/40 shadow-[inset_2px_0_0_var(--slate-accent)]",
                )}
                key={instance.id}
                onClick={() => setSelectedId(instance.id)}
              >
                <span className="flex min-w-0 items-center gap-3">
                  <InstanceArtwork instance={instance} />
                  <span className="min-w-0">
                    <strong className="block overflow-hidden text-[13px] font-bold text-ellipsis whitespace-nowrap">
                      {instance.name}
                    </strong>
                    <small className="mt-px block overflow-hidden text-[11px] text-app-muted text-ellipsis whitespace-nowrap">
                      {instance.settings.description || "Custom Minecraft setup"}
                    </small>
                  </span>
                </span>
                <span className="min-w-0">
                  <span className="block overflow-hidden font-mono text-[11px] text-ellipsis whitespace-nowrap">
                    {instance.minecraftVersion ?? "Not installed"}
                  </span>
                  <small className="mt-px block overflow-hidden text-[11px] text-app-muted text-ellipsis whitespace-nowrap">
                    {loaderDetail(instance)}
                  </small>
                </span>
                <span className="flex items-center gap-2 text-xs text-app-secondary">
                  <span
                    className={cn(
                      "size-2 shrink-0 rounded-full bg-app-muted",
                      sessionsQuery.data?.some(
                        (session) => session.instanceId === instance.id,
                      )
                        ? "bg-app-warning"
                        : instance.setupState === "ready" && "bg-app-accent",
                    )}
                  />
                  {sessionsQuery.data?.some(
                    (session) => session.instanceId === instance.id,
                  )
                    ? "Running"
                    : setupStateLabel(instance.setupState)}
                </span>
                <span className="text-xs text-app-secondary max-[1180px]:hidden">
                  {formatLastPlayed(instance.lastPlayed)}
                </span>
              </button>
            ))}
          </div>
        </section>

        <aside
          className="grid content-start gap-7 bg-app-sidebar/45 px-7 pt-6 pb-7"
          aria-label="Server and update summary"
        >
          <SummarySection
            title="Ready to join"
            subtitle="Your saved servers, one click away."
            to="/servers"
          >
            {savedServersQuery.data?.length ? (
              savedServersQuery.data.slice(0, 2).map((server) => (
                <ServerRow key={server.id} server={server} />
              ))
            ) : (
              <p className="mt-2 mb-0 text-xs text-app-muted">
                No saved servers yet.
              </p>
            )}
          </SummarySection>

          <SummarySection
            title="Latest changes"
            subtitle="News from your Minecraft ecosystem."
            to="/discover"
            divided
          >
            {bridgeMode === "preview" ? (
              previewUpdates.map((update) => (
                <UpdateRow key={update.id} update={update} />
              ))
            ) : (
              <p className="mt-2 mb-0 text-xs text-app-muted">
                News will appear here when available.
              </p>
            )}
          </SummarySection>
        </aside>
      </div>
    </div>
  );
}

function formatLastPlayed(value?: string, empty = "Never") {
  if (!value) return empty;
  const playedAt = new Date(value);
  if (Number.isNaN(playedAt.valueOf())) return value;
  const elapsedSeconds = Math.max(
    0,
    Math.floor((Date.now() - playedAt.valueOf()) / 1000),
  );
  if (elapsedSeconds < 60) return "Just now";
  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60)
    return `${elapsedMinutes} ${elapsedMinutes === 1 ? "minute" : "minutes"} ago`;
  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24)
    return `${elapsedHours} ${elapsedHours === 1 ? "hour" : "hours"} ago`;
  const elapsedDays = Math.floor(elapsedHours / 24);
  if (elapsedDays < 7)
    return `${elapsedDays} ${elapsedDays === 1 ? "day" : "days"} ago`;
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    year: playedAt.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
  }).format(playedAt);
}

function HomeSkeleton() {
  return (
    <div className="p-7" aria-label="Loading launcher home">
      <div className="h-[310px] animate-pulse rounded-control bg-app-surface" />
      <div className="mt-6 grid grid-cols-[1.7fr_.9fr] gap-6">
        <div className="h-[300px] animate-pulse rounded-control bg-app-surface" />
        <div className="h-[300px] animate-pulse rounded-control bg-app-surface" />
      </div>
    </div>
  );
}

function EmptyHome() {
  return (
    <section className="flex min-h-full flex-col items-center justify-center px-8 py-[10vh] text-center">
      <img src="/brand/slate-symbol-jade.svg" alt="" className="mb-6 w-20" />
      <p className={eyebrowClass}>Your library</p>
      <h1 className="m-0 text-[34px]/[40px] font-bold tracking-[-.035em]">
        Your game. Your setup.
      </h1>
      <p className="mt-3 mb-6 max-w-[560px] text-app-secondary">
        Create an instance for Vanilla, Fabric, or NeoForge. slate keeps each
        setup isolated and verifies its files before launch.
      </p>
      <Link
        className={cn(
          controlButtonClass,
          "h-11 bg-app-accent text-app-on-accent no-underline",
        )}
        to="/library/new"
      >
        <Plus size={18} aria-hidden="true" />
        New instance
      </Link>
    </section>
  );
}

function InstanceArtwork({
  instance,
  compact = false,
}: {
  instance: LauncherInstance;
  compact?: boolean;
}) {
  return (
    <InstanceProfileArtwork
      instance={instance}
      className={cn(
        "size-11 rounded-lg border border-app-separator/70",
        compact && "h-14 w-[58px] rounded-[7px] max-[1180px]:w-12",
      )}
    />
  );
}

function SummarySection({
  title,
  subtitle,
  to,
  children,
  divided = false,
}: {
  title: string;
  subtitle: string;
  to: "/servers" | "/discover";
  children: ReactNode;
  divided?: boolean;
}) {
  return (
    <section className={cn(divided && "border-t border-app-separator/55 pt-6")}>
      <div className="flex items-start justify-between gap-5">
        <div>
          <h2 className="m-0 text-lg/[24px] font-bold tracking-[-.025em]">
            {title}
          </h2>
          <p className="mt-0.5 mb-0 text-xs text-app-secondary">{subtitle}</p>
        </div>
        <Link
          to={to}
          className="inline-flex min-h-8 items-center gap-1.5 text-xs font-bold text-app-accent no-underline hover:underline hover:underline-offset-[3px]"
        >
          View all
          <ArrowRight size={15} aria-hidden="true" />
        </Link>
      </div>
      <div className="mt-3 grid gap-1">{children}</div>
    </section>
  );
}

function ServerRow({ server }: { server: SavedServer }) {
  return (
    <div className="grid min-h-[52px] grid-cols-[36px_minmax(0,1fr)_auto] items-center gap-[9px] py-[5px]">
      <span className="inline-flex size-9 items-center justify-center rounded-lg bg-[#2c4939] text-[#b5e5c9]">
        <Server size={19} aria-hidden="true" />
      </span>
      <span className="min-w-0">
        <strong className="block overflow-hidden text-xs font-bold text-ellipsis whitespace-nowrap">
          {server.name}
        </strong>
        <small className="block overflow-hidden text-[11px] text-app-muted text-ellipsis whitespace-nowrap">
          {server.address}
        </small>
      </span>
      <Link
        to="/servers"
        className="inline-flex h-[34px] items-center justify-center rounded-[7px] border border-app-separator bg-app-raised px-3 text-[11px] font-bold text-app-secondary no-underline hover:border-app-secondary hover:text-app-text"
      >
        Open
      </Link>
    </div>
  );
}

function UpdateRow({ update }: { update: LauncherUpdate }) {
  return (
    <div className="grid min-h-[52px] grid-cols-[36px_minmax(0,1fr)_auto] items-center gap-[9px] py-[5px]">
      <span className="inline-flex size-9 items-center justify-center rounded-lg bg-[#344b3e] text-[#d5e9dc]">
        {update.kind === "game" ? (
          <Layers3 size={19} aria-hidden="true" />
        ) : (
          <Newspaper size={19} aria-hidden="true" />
        )}
      </span>
      <span className="min-w-0">
        <strong className="block overflow-hidden text-xs font-bold text-ellipsis whitespace-nowrap">
          {update.title}
        </strong>
        <small className="block overflow-hidden text-[11px] text-app-muted text-ellipsis whitespace-nowrap">
          {update.description}
        </small>
      </span>
      <span className="text-[10px] text-app-secondary">{update.date}</span>
    </div>
  );
}

function instanceTechnicalLine(instance: LauncherInstance) {
  const details = [
    instance.minecraftVersion
      ? "Minecraft " + instance.minecraftVersion
      : undefined,
    loaderLabel(instance.loaderKind),
    instance.modCount ? String(instance.modCount) + " mods" : undefined,
  ].filter(Boolean);
  return details.length > 0 ? details.join(" · ") : "Setup details unavailable";
}

function loaderDetail(instance: LauncherInstance) {
  const loader = loaderLabel(instance.loaderKind);
  const version = instance.loaderVersion ? " " + instance.loaderVersion : "";
  if (!instance.modCount) return loader + version;
  return loader + version + " · " + String(instance.modCount) + " mods";
}

function loaderLabel(loader: LauncherInstance["loaderKind"]) {
  if (loader === "neoForge") return "NeoForge";
  return loader[0].toUpperCase() + loader.slice(1);
}

function setupStateLabel(state: LauncherInstance["setupState"]) {
  if (state === "configured") return "Configured";
  if (state === "preparing") return "Preparing";
  if (state === "ready") return "Ready";
  return "Needs attention";
}
