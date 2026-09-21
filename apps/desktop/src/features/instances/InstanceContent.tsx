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
  resolveInstanceModRelationships,
  resolveInstanceMods,
  searchMods,
  setInstanceModEnabled,
  setInstanceModPinned,
  updateInstanceMod,
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
  ModResultSkeletons,
  ModSearchResult,
} from "./ContentBrowserComponents";
import { InstanceFileContent } from "./InstanceContentComponents";
import { InstalledModsTable } from "./InstalledModsTable";
import { modProviderName, normalizedModPath } from "./instanceContentModel";

export function InstanceContent({ instance }: { instance: LauncherInstance }) {
  const queryClient = useQueryClient();
  const [contentKind, setContentKind] = useState<"mods" | InstanceContentKind>(
    "mods",
  );
  const [browserOpen, setBrowserOpen] = useState(false);
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
  const updateModMutation = useMutation({
    mutationFn: ({
      item,
      versionId,
    }: {
      item: InstanceMod;
      versionId: string;
    }) => {
      if (!item.provider || !item.projectId) {
        throw new UserFacingError(
          "This mod is not linked to a supported provider.",
        );
      }
      return updateInstanceMod({
        instanceId: instance.id,
        expectedRevision: instance.revision,
        provider: item.provider,
        projectId: item.projectId,
        filePath: item.filePath,
        displayName: item.displayName,
        targetVersionId: versionId,
      });
    },
    onMutate: () => setNotice(undefined),
    onSuccess: async (job) => {
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
        title: "Mod version was not queued",
        message: contentErrorMessage(error),
      }),
  });
  const relationshipMutation = useMutation({
    mutationFn: (item: InstanceMod) => {
      if (!item.provider || !item.projectId || !item.versionId) {
        throw new UserFacingError(
          "Dependencies are not available for this mod file.",
        );
      }
      return resolveInstanceModRelationships({
        instanceId: instance.id,
        provider: item.provider,
        projectId: item.projectId,
        versionId: item.versionId,
        filePath: item.filePath,
      });
    },
    onMutate: () => setNotice(undefined),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["instance-mod-resolutions", instance.id],
      });
    },
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Dependencies could not be loaded",
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
      queryClient.invalidateQueries({
        queryKey: ["instance-mod-history", instance.id],
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
              ? installJob.operation === "modUpdate"
                ? "Mod version changed"
                : "Mods installed"
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
  const resolutionsByIdentity = new Map(
    (resolutionQuery.data ?? []).map((resolution) => [
      `${resolution.provider}:${resolution.projectId}`,
      resolution,
    ]),
  );
  const resolveReferenceNames = (references: InstanceMod["dependencies"]) =>
    references.map((reference) => ({
      ...reference,
      displayName:
        reference.displayName ??
        resolutionsByIdentity.get(
          `${reference.provider}:${reference.projectId}`,
        )?.displayName ??
        null,
    }));
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
          dependencies: resolveReferenceNames(resolution.dependencies),
          requiredBy: resolveReferenceNames(resolution.requiredBy),
        }
      : {
          ...item,
          dependencies: resolveReferenceNames(item.dependencies),
          requiredBy: resolveReferenceNames(item.requiredBy),
        };
  });
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
    updateModMutation.isPending ||
    installJob?.state === "queued" ||
    installJob?.state === "running" ||
    installJob?.state === "paused";
  const contentMutationPending =
    importMutation.isPending ||
    relationshipMutation.isPending ||
    toggleMutation.isPending ||
    pinModMutation.isPending ||
    removeMutation.isPending;
  const installedIdentityPending =
    installedQuery.isPending ||
    (Boolean(installedQuery.data?.length) && resolutionQuery.isFetching);

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

        <InstalledModsTable
          instanceId={instance.id}
          instanceName={instance.name}
          installed={installed}
          loading={installedQuery.isPending}
          failed={installedQuery.isError}
          resolutionFetching={resolutionQuery.isFetching}
          disabled={installing || contentMutationPending}
          pendingAction={(item) =>
            updateModMutation.isPending &&
            updateModMutation.variables?.item.filePath === item.filePath
              ? "update"
              : relationshipMutation.isPending &&
                  relationshipMutation.variables?.filePath === item.filePath
                ? "relationships"
                : toggleMutation.isPending &&
                    toggleMutation.variables?.item.filePath === item.filePath
                  ? "toggle"
                  : pinModMutation.isPending &&
                      pinModMutation.variables?.item.filePath === item.filePath
                    ? "pin"
                    : removeMutation.isPending &&
                        removeMutation.variables?.filePath === item.filePath
                      ? "remove"
                      : undefined
          }
          onUpdate={(item, versionId) =>
            updateModMutation.mutate({ item, versionId })
          }
          onResolveRelationships={(item) =>
            relationshipMutation.mutateAsync(item).then(() => undefined)
          }
          onToggle={(item) =>
            toggleMutation.mutate({ item, enabled: !item.enabled })
          }
          onPin={(item) =>
            pinModMutation.mutate({ item, pinned: !item.pinned })
          }
          onRemove={(item) => removeMutation.mutate(item)}
          browserOpen={browserOpen}
          isVanilla={isVanilla}
        />
      </section>
    </div>
  );
}
