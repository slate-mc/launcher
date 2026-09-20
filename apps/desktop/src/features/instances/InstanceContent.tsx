import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowRight,
  ChevronLeft,
  Download,
  FileUp,
  LoaderCircle,
  Plus,
  Search,
  ShieldCheck,
} from "lucide-react";
import { useEffect, useState } from "react";
import { InstallProgressIndicator } from "../../components/InstallProgressIndicator";
import { EmptyState, InlineNotice } from "../../components/PageScaffold";
import {
  installMods,
  importLocalMod,
  listInstallJobs,
  listInstanceMods,
  removeInstanceMod,
  resolveInstanceMods,
  searchMods,
  setInstanceModEnabled,
  setInstanceModPinned,
} from "../../lib/bridge";
import { loaderLabel } from "../../lib/format";
import { installJobMessage } from "../../lib/installJobPresentation";
import { UserFacingError } from "../../lib/userFacingError";
import type {
  InstanceContentKind,
  InstanceMod,
  LauncherInstance,
  ModpackSummary,
  Provider,
} from "../../types/launcher";
import { contentErrorMessage } from "./instanceContentFormat";
import {
  ContentNavigation,
  ContentSelect,
  InstalledModRow,
  InstalledModSkeletons,
  InstanceFileContent,
  ModResultSkeletons,
  ModSearchResult,
  ModTableSortHeader,
} from "./InstanceContentComponents";
import {
  compareInstalledMods,
  modProviderName,
  normalizedModPath,
  type InstalledModSort,
} from "./instanceContentModel";

export function InstanceContent({ instance }: { instance: LauncherInstance }) {
  const queryClient = useQueryClient();
  const [contentKind, setContentKind] = useState<"mods" | InstanceContentKind>(
    "mods",
  );
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
        throw new UserFacingError("Select at least one mod to install.");
      }
      if (items.some((item) => item.provider === "ftb")) {
        throw new UserFacingError(
          "FTB does not provide individual mod downloads.",
        );
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
  const importMutation = useMutation({
    mutationFn: () =>
      importLocalMod({
        instanceId: instance.id,
        expectedRevision: instance.revision,
      }),
    onMutate: () => setNotice(undefined),
    onSuccess: (updated) => {
      if (updated.revision === instance.revision) return;
      return refreshContentAfterMutation(
        updated,
        "Mod imported",
        "The local mod JAR is ready for the next launch.",
      );
    },
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Mod was not imported",
        message: contentErrorMessage(error),
      }),
  });
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
          message: installJobMessage(installJob),
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
    importMutation.isPending ||
    toggleMutation.isPending ||
    pinModMutation.isPending ||
    removeMutation.isPending;
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
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-bg px-4 text-xs font-bold text-app-secondary hover:text-app-text disabled:cursor-not-allowed disabled:opacity-45"
              disabled={isVanilla || installing || contentMutationPending}
              onClick={() => importMutation.mutate()}
            >
              {importMutation.isPending ? (
                <LoaderCircle
                  size={15}
                  className="animate-spin motion-reduce:animate-none"
                  aria-hidden="true"
                />
              ) : (
                <FileUp size={15} aria-hidden="true" />
              )}
              {importMutation.isPending ? "Importing" : "Import JAR"}
            </button>
            <button
              type="button"
              className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:cursor-not-allowed disabled:opacity-45"
              disabled={isVanilla || installing || contentMutationPending}
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
                  {installJobMessage(installJob)}
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
                      Showing compatible mods
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
                description="Try a different search. Results must support this instance’s Minecraft version and loader."
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
                    Checking installed mods
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
                                pinModMutation.variables?.item.filePath ===
                                  item.filePath
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
            description="Add a compatible mod from CurseForge or Modrinth, import a local JAR, or install a modpack from Discover."
          />
        ) : null}
      </section>
    </div>
  );
}
