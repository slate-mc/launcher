import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import {
  Activity,
  Boxes,
  Check,
  CircleHelp,
  Compass,
  ExternalLink,
  LoaderCircle,
  Plus,
  RefreshCw,
  Server,
  ShieldX,
  Trash2,
  UserRound,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { EmptyState, InlineNotice, PageHeader } from "../../components/PageScaffold";
import { MinecraftHead } from "../../components/MinecraftHead";
import {
  bridgeMode,
  cancelMinecraftAuth,
  getMinecraftAuthStatus,
  listAccounts,
  listInstallJobs,
  previewServers,
  refreshMinecraftAccount,
  removeMinecraftAccount,
  setDefaultMinecraftAccount,
  startMinecraftAuth,
} from "../../lib/bridge";

export function DiscoverPage() {
  return (
    <UnavailablePage
      eyebrow="Discover"
      title="Provider search is not connected"
      description="Modrinth and other provider adapters need real API policy, compatibility normalization, and verified download jobs before search results can be installed."
      icon={Compass}
    />
  );
}

export function ActivityPage() {
  const jobsQuery = useQuery({
    queryKey: ["install-jobs"],
    queryFn: listInstallJobs,
    refetchInterval: 1_000,
  });
  const jobs = jobsQuery.data ?? [];

  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Activity"
        title="Jobs and sessions"
        description="Downloads and instance preparation remain visible across navigation and restarts."
      />
      {jobsQuery.isError ? (
        <EmptyState
          error
          title="Activity is unavailable"
          description="slate could not read the durable installation queue. Your files were not changed."
        />
      ) : jobs.length === 0 ? (
        <EmptyState
          title="No installation activity"
          description="Install an instance to see its durable preparation job here."
        />
      ) : (
        <div className="mx-auto max-w-[940px] px-8 py-7">
          <section className="overflow-hidden rounded-control border border-app-separator/70 bg-app-surface">
            <div className="grid grid-cols-[minmax(0,1fr)_130px_150px] gap-4 border-b border-app-separator/55 px-5 py-3 text-[10px] font-bold tracking-[.06em] text-app-muted uppercase">
              <span>Installation</span>
              <span>State</span>
              <span>Updated</span>
            </div>
            <div className="divide-y divide-app-separator/55">
              {jobs.map((job) => (
                <div
                  key={job.id}
                  className="grid min-h-16 grid-cols-[minmax(0,1fr)_130px_150px] items-center gap-4 px-5 py-3"
                >
                  <span className="min-w-0">
                    <strong className="block overflow-hidden text-xs font-bold text-ellipsis whitespace-nowrap">
                      {job.message}
                    </strong>
                    <small className="mt-1 block font-mono text-[10px] text-app-muted">
                      {job.phase} · {job.id.slice(0, 8)}
                    </small>
                  </span>
                  <span className="inline-flex items-center gap-2 text-xs text-app-secondary">
                    <span
                      className={`size-2 rounded-full ${
                        job.state === "succeeded"
                          ? "bg-app-accent"
                          : job.state === "failed"
                            ? "bg-app-danger"
                            : "bg-app-warning"
                      }`}
                    />
                    {job.state[0].toUpperCase() + job.state.slice(1)}
                  </span>
                  <time className="text-[11px] text-app-secondary">
                    {new Date(job.updatedAt).toLocaleString()}
                  </time>
                </div>
              ))}
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

export function AccountsPage() {
  const queryClient = useQueryClient();
  const [flowId, setFlowId] = useState<string>();
  const [confirmRemove, setConfirmRemove] = useState<string>();
  const repairedSkinIds = useRef(new Set<string>());
  const accountsQuery = useQuery({
    queryKey: ["minecraft-accounts"],
    queryFn: listAccounts,
  });
  const statusQuery = useQuery({
    queryKey: ["minecraft-auth", flowId],
    queryFn: () => getMinecraftAuthStatus(flowId ?? ""),
    enabled: Boolean(flowId),
    refetchInterval: (query) => {
      const state = query.state.data?.state;
      return state === "succeeded" || state === "failed" || state === "cancelled"
        ? false
        : 800;
    },
  });
  const startMutation = useMutation({
    mutationFn: startMinecraftAuth,
    onSuccess: (flow) => setFlowId(flow.flowId),
  });
  const cancelMutation = useMutation({
    mutationFn: cancelMinecraftAuth,
    onSuccess: () => setFlowId(undefined),
  });
  const refreshMutation = useMutation({
    mutationFn: refreshMinecraftAccount,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["minecraft-accounts"] });
    },
  });
  const defaultMutation = useMutation({
    mutationFn: setDefaultMinecraftAccount,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["minecraft-accounts"] });
    },
  });
  const removeMutation = useMutation({
    mutationFn: removeMinecraftAccount,
    onSuccess: async () => {
      setConfirmRemove(undefined);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["minecraft-accounts"] }),
        queryClient.invalidateQueries({ queryKey: ["preflight"] }),
      ]);
    },
  });

  useEffect(() => {
    if (statusQuery.data?.state === "succeeded") {
      void Promise.all([
        queryClient.invalidateQueries({ queryKey: ["minecraft-accounts"] }),
        queryClient.invalidateQueries({ queryKey: ["preflight"] }),
      ]);
    }
  }, [queryClient, statusQuery.data?.state]);

  useEffect(() => {
    const missingSkin = accountsQuery.data?.find(
      (account) => account.status === "ready" && !account.skinUrl && !repairedSkinIds.current.has(account.id),
    );
    if (missingSkin && !refreshMutation.isPending) {
      repairedSkinIds.current.add(missingSkin.id);
      refreshMutation.mutate(missingSkin.id);
    }
  }, [accountsQuery.data, refreshMutation]);

  const authActive =
    statusQuery.data?.state === "waitingForBrowser" ||
    statusQuery.data?.state === "verifying";
  const accounts = accountsQuery.data ?? [];

  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Accounts"
        title="Minecraft identity"
        description="Microsoft credentials stay in your operating-system vault. slate stores only account metadata."
        actions={
          <button
            type="button"
            className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent hover:brightness-105 disabled:opacity-50"
            disabled={bridgeMode !== "native" || authActive || startMutation.isPending}
            onClick={() => startMutation.mutate()}
          >
            {startMutation.isPending ? (
              <LoaderCircle className="animate-spin" size={16} aria-hidden="true" />
            ) : (
              <Plus size={16} aria-hidden="true" />
            )}
            Add Microsoft account
          </button>
        }
      />
      <div className="mx-auto max-w-[880px] px-8 py-8">
        {authActive ? (
          <div className="mb-5 flex items-center justify-between gap-5 rounded-control border border-app-accent/35 bg-app-accent/5 px-5 py-4">
            <span className="flex min-w-0 items-center gap-3">
              <LoaderCircle
                className="shrink-0 animate-spin text-app-accent"
                size={19}
                aria-hidden="true"
              />
              <span>
                <strong className="block text-sm font-bold">
                  {statusQuery.data?.state === "verifying"
                    ? "Verifying Minecraft ownership"
                    : "Finish signing in with Microsoft"}
                </strong>
                <small className="text-[11px] text-app-secondary">
                  The secure sign-in page opened in your default browser.
                </small>
              </span>
            </span>
            <button
              type="button"
              className="h-8 rounded-compact border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-secondary hover:text-app-text"
              disabled={cancelMutation.isPending}
              onClick={() => flowId && cancelMutation.mutate(flowId)}
            >
              Cancel
            </button>
          </div>
        ) : null}

        {statusQuery.data?.state === "succeeded" ? (
          <InlineNotice tone="positive" title="Minecraft account connected">
            {statusQuery.data.account?.displayName ?? "Your account"} is ready to launch.
          </InlineNotice>
        ) : null}
        {statusQuery.data?.state === "failed" ? (
          <InlineNotice tone="danger" title="Sign-in did not complete">
            {statusQuery.data.userMessage ?? "Start a new Microsoft sign-in and try again."}
          </InlineNotice>
        ) : null}
        {startMutation.isError ? (
          <InlineNotice tone="danger" title="Could not start sign-in">
            {userFacingError(
              startMutation.error,
              "Confirm that the operating-system credential vault and default browser are available.",
            )}
          </InlineNotice>
        ) : null}

        <section className="mt-5 overflow-hidden rounded-control border border-app-separator/70 bg-app-surface">
          <div className="flex items-center justify-between border-b border-app-separator/55 px-5 py-4">
            <div>
              <h2 className="m-0 text-[15px] font-bold">Connected accounts</h2>
              <p className="mt-1 mb-0 text-[11px] text-app-secondary">
                The default account is used unless an instance selects another one.
              </p>
            </div>
            <span className="font-mono text-[10px] text-app-muted">
              {accounts.length} connected
            </span>
          </div>

          {accountsQuery.isPending ? (
            <div className="grid gap-px bg-app-separator/45" aria-label="Loading accounts">
              {[0, 1].map((index) => (
                <div key={index} className="h-[76px] animate-pulse bg-app-surface" />
              ))}
            </div>
          ) : accountsQuery.isError ? (
            <div className="p-5">
              <InlineNotice tone="danger" title="Accounts unavailable">
                Your credentials were not changed. Reload this page to retry local storage.
              </InlineNotice>
            </div>
          ) : accounts.length === 0 ? (
            <div className="px-6 py-12 text-center">
              <span className="mx-auto inline-flex size-11 items-center justify-center rounded-control bg-app-raised text-app-accent">
                <UserRound size={21} aria-hidden="true" />
              </span>
              <h2 className="mt-4 mb-1 text-base font-bold">Connect Minecraft to play</h2>
              <p className="mx-auto mt-0 mb-0 max-w-[480px] text-xs/[19px] text-app-secondary">
                Sign in through Microsoft, then slate verifies Java Edition ownership and saves the
                refresh credential in your system vault.
              </p>
            </div>
          ) : (
            <div className="divide-y divide-app-separator/55">
              {accounts.map((account) => (
                <div key={account.id} className="px-5 py-4">
                  <div className="flex items-center gap-4">
                    <MinecraftHead
                      skinUrl={account.skinUrl}
                      playerName={account.displayName}
                      className="size-10 text-sm"
                    />
                    <span className="min-w-0 flex-1">
                      <span className="flex items-center gap-2">
                        <strong className="overflow-hidden text-sm font-bold text-ellipsis whitespace-nowrap">
                          {account.displayName}
                        </strong>
                        {account.isDefault ? (
                          <span className="inline-flex items-center gap-1 rounded-full bg-app-accent/10 px-2 py-0.5 text-[9px] font-bold text-app-accent uppercase">
                            <Check size={11} aria-hidden="true" /> Default
                          </span>
                        ) : null}
                      </span>
                      <small className="mt-0.5 block font-mono text-[10px] text-app-muted">
                        {account.status === "ready" ? "Minecraft ready" : "Sign-in required"}
                      </small>
                    </span>
                    <div className="flex items-center gap-1.5">
                      {!account.isDefault ? (
                        <button
                          type="button"
                          className="h-8 rounded-compact border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-secondary hover:text-app-text disabled:opacity-50"
                          disabled={defaultMutation.isPending}
                          onClick={() => defaultMutation.mutate(account.id)}
                        >
                          Make default
                        </button>
                      ) : null}
                      <button
                        type="button"
                        className="inline-flex size-8 items-center justify-center rounded-compact border border-app-separator bg-app-bg text-app-secondary hover:text-app-text disabled:opacity-50"
                        aria-label={`Refresh ${account.displayName}`}
                        title="Refresh and verify account"
                        disabled={refreshMutation.isPending}
                        onClick={() => refreshMutation.mutate(account.id)}
                      >
                        <RefreshCw size={15} aria-hidden="true" />
                      </button>
                      <button
                        type="button"
                        className="inline-flex size-8 items-center justify-center rounded-compact border border-app-separator bg-app-bg text-app-secondary hover:border-app-danger/40 hover:text-app-danger"
                        aria-label={`Remove ${account.displayName}`}
                        title="Remove account"
                        onClick={() => setConfirmRemove(account.id)}
                      >
                        <Trash2 size={15} aria-hidden="true" />
                      </button>
                    </div>
                  </div>
                  {confirmRemove === account.id ? (
                    <div className="mt-3 flex items-center justify-end gap-2 border-t border-app-separator/45 pt-3">
                      <span className="mr-auto text-[11px] text-app-secondary">
                        Remove the local account and its saved vault credential?
                      </span>
                      <button
                        type="button"
                        className="h-8 rounded-compact px-3 text-[11px] font-bold text-app-secondary hover:bg-app-hover"
                        onClick={() => setConfirmRemove(undefined)}
                      >
                        Keep account
                      </button>
                      <button
                        type="button"
                        className="h-8 rounded-compact bg-app-danger px-3 text-[11px] font-bold text-black disabled:opacity-50"
                        disabled={removeMutation.isPending}
                        onClick={() => removeMutation.mutate(account.id)}
                      >
                        Remove
                      </button>
                    </div>
                  ) : null}
                </div>
              ))}
            </div>
          )}
        </section>

        <p className="mt-4 flex items-center gap-2 text-[11px] text-app-muted">
          <ExternalLink size={14} aria-hidden="true" />
          Microsoft credentials never enter the webview or slate’s SQLite database.
        </p>
      </div>
    </div>
  );
}

export function ServersPage() {
  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Servers"
        title="Saved servers"
        description="Favorites will stay local and link to a compatible instance before joining."
        actions={
          <button
            type="button"
            className="inline-flex h-9 items-center gap-2 rounded-control bg-app-raised px-4 text-xs font-bold text-app-muted opacity-60"
            disabled
            title="Saved-server persistence and validation are not implemented yet."
          >
            <Plus size={16} aria-hidden="true" />
            Add server
          </button>
        }
      />
      {bridgeMode === "preview" ? (
        <div className="px-8 py-7">
          <InlineNotice title="Fixture-only server rows">
            These entries demonstrate layout in browser preview. They are never returned by the
            native adapter.
          </InlineNotice>
          <div className="mt-5 overflow-hidden rounded-control border border-app-separator/70">
            {previewServers.map((server) => (
              <div
                key={server.id}
                className="grid min-h-16 grid-cols-[38px_minmax(0,1fr)_120px] items-center gap-3 border-t border-app-separator/45 bg-app-surface px-4 first:border-0"
              >
                <span className="inline-flex size-9 items-center justify-center rounded-lg bg-app-raised text-app-accent">
                  <Server size={18} aria-hidden="true" />
                </span>
                <span className="min-w-0">
                  <strong className="block text-xs font-bold">{server.name}</strong>
                  <small className="font-mono text-[10px] text-app-muted">
                    {server.address}
                  </small>
                </span>
                <span className="text-right font-mono text-[10px] text-app-secondary">
                  {server.players}
                </span>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <EmptyState
          title="No saved servers"
          description="The native saved-server repository is not implemented, so slate will not show sample entries here."
        />
      )}
    </div>
  );
}

export function HelpPage() {
  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Help"
        title="About this build"
        description="slate 0.1.0 foundation preview"
      />
      <div className="mx-auto grid max-w-[900px] grid-cols-2 gap-5 px-8 py-8">
        <InfoBlock
          icon={Boxes}
          title="Current boundary"
          description="Local instances, durable installation, managed Java runtimes, Minecraft account authentication, and supervised launch are connected."
        />
        <InfoBlock
          icon={ShieldX}
          title="Next integrations"
          description="Provider discovery, mod management, saved-server joining, cloud sync, and the in-game companion remain outside this build."
        />
        <InfoBlock
          icon={Activity}
          title="Data adapter"
          description={
            bridgeMode === "native"
              ? "This window is connected to native Tauri commands and SQLite."
              : "This browser preview is using clearly labeled fixture data."
          }
        />
        <InfoBlock
          icon={CircleHelp}
          title="Recovery posture"
          description="Instance removal is soft-trash only. Managed files are preserved by this implementation."
        />
      </div>
    </div>
  );
}

function UnavailablePage({
  eyebrow,
  title,
  description,
  icon: Icon,
}: {
  eyebrow: string;
  title: string;
  description: string;
  icon: typeof Compass;
}) {
  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader eyebrow={eyebrow} title={title} description={description} />
      <EmptyState
        title="Capability unavailable"
        description="The production route stays honest until a validated native adapter is connected."
        action={
          <Link
            to="/library"
            className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text no-underline"
          >
            <Icon size={16} aria-hidden="true" />
            Return to library
          </Link>
        }
      />
    </div>
  );
}

function InfoBlock({
  icon: Icon,
  title,
  description,
}: {
  icon: typeof Boxes;
  title: string;
  description: string;
}) {
  return (
    <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
      <Icon size={19} className="text-app-accent" aria-hidden="true" />
      <h2 className="mt-4 mb-1 text-sm font-bold">{title}</h2>
      <p className="m-0 text-xs/[19px] text-app-secondary">{description}</p>
    </section>
  );
}

function userFacingError(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (
    typeof error === "object" &&
    error !== null &&
    "userMessage" in error &&
    typeof error.userMessage === "string" &&
    error.userMessage.trim()
  ) {
    return error.userMessage;
  }
  return fallback;
}
