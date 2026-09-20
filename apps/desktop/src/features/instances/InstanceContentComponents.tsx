import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Check,
  ChevronLeft,
  Download,
  FileUp,
  LoaderCircle,
  Plus,
  RotateCcw,
  Search,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import { ContentArtwork } from "../../components/ContentArtwork";
import { InstallProgressIndicator } from "../../components/InstallProgressIndicator";
import { EmptyState, InlineNotice } from "../../components/PageScaffold";
import {
  importLocalContentFile,
  installContent,
  installInstance,
  listInstallJobs,
  listInstanceContentFiles,
  listInstanceWorlds,
  removeInstanceContentFile,
  searchContent,
  setInstanceContentFileEnabled,
} from "../../lib/bridge";
import { installJobMessage } from "../../lib/installJobPresentation";
import type {
  InstanceContentFile,
  InstanceContentKind,
  LauncherInstance,
  ModpackSummary,
} from "../../types/launcher";
import {
  contentErrorMessage,
  formatContentFileSize,
} from "./instanceContentFormat";
import {
  modProviderName,
  type ContentSectionKind,
} from "./instanceContentModel";

const secondaryButtonClass =
  "inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text hover:border-app-secondary disabled:opacity-50";

export function ContentNavigation({
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

export function InstanceFileContent({
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
  const [worldName, setWorldName] = useState("");
  const [browserOpen, setBrowserOpen] = useState(false);
  const [draftQuery, setDraftQuery] = useState("");
  const [searchTerm, setSearchTerm] = useState("");
  const [sort, setSort] = useState<
    "relevance" | "downloads" | "updated" | "newest"
  >("relevance");
  const [page, setPage] = useState(1);
  const [selectedContent, setSelectedContent] = useState<
    Record<string, ModpackSummary>
  >({});
  const [installJobId, setInstallJobId] = useState<string>();
  const [notice, setNotice] = useState<{
    tone: "positive" | "danger";
    title: string;
    message: string;
  }>();
  const query = useQuery({
    queryKey: ["instance-content-files", instance.id, kind],
    queryFn: () => listInstanceContentFiles(instance.id, kind),
  });
  const worldsQuery = useQuery({
    queryKey: ["instance-worlds", instance.id],
    queryFn: () => listInstanceWorlds(instance.id),
    enabled: kind === "dataPack",
  });
  const worlds = worldsQuery.data ?? [];
  const selectedWorldName = worlds.includes(worldName) ? worldName : worlds[0];
  const searchQuery = useQuery({
    queryKey: ["content-search", instance.id, kind, searchTerm, sort, page],
    queryFn: () =>
      searchContent({
        instanceId: instance.id,
        kind,
        query: searchTerm || undefined,
        sort,
        page,
        limit: 20,
      }),
    enabled: browserOpen && (kind !== "dataPack" || Boolean(selectedWorldName)),
    placeholderData: (previous) => previous,
  });
  const jobsQuery = useQuery({
    queryKey: ["install-jobs"],
    queryFn: async () => {
      const jobs = await listInstallJobs();
      const tracked = jobs.find((job) => job.id === installJobId);
      if (tracked?.state === "succeeded") {
        await Promise.all([
          queryClient.invalidateQueries({
            queryKey: ["instance-content-files", instance.id, kind],
          }),
          queryClient.invalidateQueries({
            queryKey: ["instance", instance.id],
          }),
          queryClient.invalidateQueries({ queryKey: ["instances"] }),
        ]);
      }
      return jobs;
    },
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
    mutationFn: ({
      file,
      enabled,
    }: {
      file: InstanceContentFile;
      enabled: boolean;
    }) =>
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
  const importMutation = useMutation({
    mutationFn: () =>
      importLocalContentFile({
        instanceId: instance.id,
        kind,
        worldName: kind === "dataPack" ? selectedWorldName : undefined,
        expectedRevision: instance.revision,
      }),
    onMutate: () => setNotice(undefined),
    onSuccess: (updated) => {
      if (updated.revision === instance.revision) return;
      return refresh(
        updated,
        `${contentKindSingular(kind)} imported and ready to use.`,
      );
    },
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Content was not imported",
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
      return refresh(
        updated,
        `${file.displayName} was moved to slate’s recoverable trash.`,
      );
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
          "slate will restore files included with the modpack and leave files you added in place.",
      });
    },
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: "Pack restore was not queued",
        message: contentErrorMessage(error),
      }),
  });
  const installMutation = useMutation({
    mutationFn: (items: ModpackSummary[]) =>
      installContent({
        instanceId: instance.id,
        expectedRevision: instance.revision,
        kind,
        worldName: kind === "dataPack" ? selectedWorldName : undefined,
        content: items.map((item) => ({
          provider: "modrinth",
          projectId: item.id,
          displayName: item.name,
          iconUrl: item.icon_url ?? undefined,
        })),
      }),
    onMutate: () => setNotice(undefined),
    onSuccess: async (job) => {
      setSelectedContent({});
      setInstallJobId(job.id);
      queryClient.setQueryData(["install-jobs"], (current: unknown) =>
        Array.isArray(current) ? [job, ...current] : [job],
      );
      await queryClient.invalidateQueries({ queryKey: ["install-jobs"] });
    },
    onError: (error) =>
      setNotice({
        tone: "danger",
        title: `${contentKindLabel(kind)} were not queued`,
        message: contentErrorMessage(error),
      }),
  });
  const files = query.data ?? [];
  const selectedItems = Object.values(selectedContent);
  const installedIdentities = new Set(
    files.flatMap((file) =>
      file.provider && file.projectId
        ? [`${file.provider}:${file.projectId}`]
        : [],
    ),
  );
  const label = contentKindLabel(kind);
  const singular = contentKindSingular(kind);
  const installNotice =
    installJob?.state === "succeeded"
      ? {
          tone: "positive" as const,
          title: `${label} installed`,
          message: "The selected content is ready in this instance.",
        }
      : installJob && ["failed", "cancelled"].includes(installJob.state)
        ? {
            tone: "danger" as const,
            title: `${label} were not installed`,
            message: installJobMessage(installJob),
          }
        : undefined;
  const visibleNotice = notice ?? installNotice;
  const installing =
    installMutation.isPending ||
    installJob?.state === "queued" ||
    installJob?.state === "running" ||
    installJob?.state === "paused";
  const busy =
    importMutation.isPending ||
    toggleMutation.isPending ||
    removeMutation.isPending ||
    installing;

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
          <div className="flex items-center gap-2">
            {kind === "dataPack" ? (
              worldsQuery.isPending ? (
                <span className="inline-flex items-center gap-2 text-[11px] text-app-secondary">
                  <LoaderCircle
                    className="animate-spin motion-reduce:animate-none"
                    size={14}
                  />
                  Loading worlds
                </span>
              ) : worlds.length ? (
                <div className="w-44">
                  <ContentSelect
                    label="Target world"
                    value={selectedWorldName ?? ""}
                    options={worlds.map((world) => [world, world] as const)}
                    onChange={setWorldName}
                    compact
                  />
                </div>
              ) : (
                <span className="text-[11px] text-app-muted">
                  No worlds found
                </span>
              )
            ) : null}
            <button
              type="button"
              className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:cursor-not-allowed disabled:opacity-45"
              disabled={busy || (kind === "dataPack" && !selectedWorldName)}
              onClick={() => {
                if (browserOpen) setSelectedContent({});
                setBrowserOpen((open) => !open);
                setNotice(undefined);
              }}
            >
              <Plus size={14} aria-hidden="true" />
              {browserOpen ? "Close browser" : `Add ${label.toLowerCase()}`}
            </button>
            <button
              type="button"
              className={secondaryButtonClass}
              disabled={
                busy ||
                restoreMutation.isPending ||
                (kind === "dataPack" && !selectedWorldName)
              }
              onClick={() => importMutation.mutate()}
            >
              {importMutation.isPending ? (
                <LoaderCircle
                  className="animate-spin motion-reduce:animate-none"
                  size={14}
                />
              ) : (
                <FileUp size={14} />
              )}
              {importMutation.isPending ? "Importing" : "Import ZIP"}
            </button>
            {instance.modpackSource ? (
              <button
                type="button"
                className={secondaryButtonClass}
                disabled={busy || restoreMutation.isPending}
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
          </div>
        </header>
        {visibleNotice ? (
          <div className="px-5 pt-5">
            <InlineNotice tone={visibleNotice.tone} title={visibleNotice.title}>
              {visibleNotice.message}
            </InlineNotice>
          </div>
        ) : null}
        {installJob &&
        ["queued", "running", "paused"].includes(installJob.state) ? (
          <div className="border-b border-app-separator/55 px-5 py-4">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="m-0 text-xs font-bold text-app-text">
                  Installing {label.toLowerCase()}
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
        {browserOpen ? (
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
                      Showing compatible {label.toLowerCase()}
                    </p>
                    <p className="mt-1 mb-0 font-mono text-[10px] text-app-secondary">
                      Minecraft {instance.minecraftVersion}
                      {kind === "dataPack" && selectedWorldName
                        ? ` · ${selectedWorldName}`
                        : ""}
                    </p>
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-3">
                  <span className="font-mono text-[10px] text-app-muted">
                    {selectedItems.length} selected
                  </span>
                  <button
                    type="button"
                    disabled={selectedItems.length === 0 || installing}
                    className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:bg-app-raised disabled:text-app-muted"
                    onClick={() => installMutation.mutate(selectedItems)}
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
                      ? "Preparing install"
                      : selectedItems.length
                        ? `Install ${selectedItems.length} selected`
                        : "Install selected"}
                  </button>
                </div>
              </div>
              <form
                className="grid grid-cols-[minmax(220px,1fr)_150px_auto] gap-2"
                onSubmit={(event) => {
                  event.preventDefault();
                  setSearchTerm(draftQuery.trim());
                  setPage(1);
                }}
              >
                <label className="relative block">
                  <span className="sr-only">Search {label.toLowerCase()}</span>
                  <Search
                    size={15}
                    className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-app-muted"
                    aria-hidden="true"
                  />
                  <input
                    value={draftQuery}
                    onChange={(event) => setDraftQuery(event.target.value)}
                    placeholder={`Search ${label.toLowerCase()}`}
                    className="h-9 w-full rounded-control border border-app-separator bg-app-bg pr-3 pl-9 text-xs text-app-text outline-none placeholder:text-app-muted focus:border-app-accent"
                  />
                </label>
                <ContentSelect
                  label="Sort results"
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
                <button type="submit" className={secondaryButtonClass}>
                  Search
                </button>
              </form>
            </div>
            {searchQuery.isPending ? (
              <ModResultSkeletons />
            ) : searchQuery.isError ? (
              <div className="p-5">
                <InlineNotice tone="danger" title="Content could not be loaded">
                  {contentErrorMessage(searchQuery.error)}
                </InlineNotice>
              </div>
            ) : searchQuery.data?.items.length ? (
              <>
                <div className="divide-y divide-app-separator/45 px-5">
                  {searchQuery.data.items.map((item) => {
                    const identity = `${item.provider}:${item.id}`;
                    return (
                      <ModSearchResult
                        key={identity}
                        item={item}
                        installed={installedIdentities.has(identity)}
                        selected={Boolean(selectedContent[identity])}
                        checkingInstalled={query.isPending}
                        disabled={installing}
                        onToggleSelected={() =>
                          setSelectedContent((current) => {
                            const next = { ...current };
                            if (next[identity]) delete next[identity];
                            else next[identity] = item;
                            return next;
                          })
                        }
                      />
                    );
                  })}
                </div>
                <div className="flex items-center justify-between border-t border-app-separator/45 px-5 py-3">
                  <span className="font-mono text-[10px] text-app-muted">
                    Page {page}
                  </span>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      className={secondaryButtonClass}
                      disabled={page === 1 || searchQuery.isFetching}
                      onClick={() => setPage((current) => current - 1)}
                    >
                      Previous
                    </button>
                    <button
                      type="button"
                      className={secondaryButtonClass}
                      disabled={
                        !searchQuery.data.has_more || searchQuery.isFetching
                      }
                      onClick={() => setPage((current) => current + 1)}
                    >
                      Next
                    </button>
                  </div>
                </div>
              </>
            ) : (
              <EmptyState
                title={`No compatible ${label.toLowerCase()} found`}
                description={`Try a different search. Results are limited to ${singular.toLowerCase()} versions that support Minecraft ${instance.minecraftVersion}.`}
              />
            )}
          </div>
        ) : null}
        {query.isPending ? (
          <div className="grid min-h-64 place-items-center text-xs text-app-secondary">
            <span className="inline-flex items-center gap-2">
              <LoaderCircle className="animate-spin" size={15} />
              Scanning instance files…
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
                      <div className="flex min-w-0 items-center gap-3">
                        {file.iconUrl ? (
                          <ContentImage
                            src={file.iconUrl}
                            name={file.displayName}
                          />
                        ) : null}
                        <span className="min-w-0">
                          <strong className="block truncate text-xs text-app-text">
                            {file.displayName}
                          </strong>
                          <span
                            className="mt-0.5 block truncate font-mono text-[9px] text-app-muted"
                            title={file.filePath}
                          >
                            {file.filePath}
                          </span>
                        </span>
                      </div>
                    </td>
                    <td className="px-3 py-3 text-app-secondary">
                      {file.worldName ?? "Instance"}
                    </td>
                    <td className="px-3 py-3">
                      <span
                        className={`rounded-full border px-2 py-1 font-mono text-[9px] ${file.origin === "modpack" ? "border-app-accent/35 text-app-accent" : "border-app-separator text-app-secondary"}`}
                      >
                        {file.origin === "modpack"
                          ? "Included with pack"
                          : "Added by you"}
                      </span>
                    </td>
                    <td className="px-3 py-3 font-mono text-[10px] text-app-secondary">
                      {file.fileSize
                        ? formatContentFileSize(file.fileSize)
                        : "Folder"}
                    </td>
                    <td className="px-3 py-3">
                      <div className="flex items-center justify-end gap-2">
                        {file.canToggle ? (
                          <button
                            type="button"
                            className="h-7 rounded-control border border-app-separator px-2.5 text-[10px] font-bold text-app-secondary hover:text-app-text disabled:opacity-45"
                            disabled={busy}
                            onClick={() =>
                              toggleMutation.mutate({
                                file,
                                enabled: !file.enabled,
                              })
                            }
                          >
                            {file.enabled ? "Hide" : "Restore"}
                          </button>
                        ) : (
                          <span
                            className="text-[9px] text-app-muted"
                            title="Folder packs are enabled and ordered inside Minecraft."
                          >
                            In-game
                          </span>
                        )}
                        {removeTarget === file.filePath ? (
                          <span className="flex items-center gap-2">
                            <button
                              type="button"
                              className="text-[10px] font-bold text-app-danger"
                              disabled={busy}
                              onClick={() => removeMutation.mutate(file)}
                            >
                              Confirm
                            </button>
                            <button
                              type="button"
                              className="text-[10px] text-app-secondary"
                              onClick={() => setRemoveTarget(undefined)}
                            >
                              Cancel
                            </button>
                          </span>
                        ) : (
                          <button
                            type="button"
                            className="p-1.5 text-app-muted hover:text-app-danger"
                            title={`Remove ${file.displayName}`}
                            disabled={busy}
                            onClick={() => setRemoveTarget(file.filePath)}
                          >
                            <Trash2 size={14} />
                          </button>
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
            description={
              kind === "dataPack"
                ? worlds.length
                  ? "Choose a world above to import a compatible data-pack ZIP."
                  : "Create and save a world in Minecraft before adding a data pack."
                : `Import a compatible ZIP into this instance’s ${label.toLocaleLowerCase()} folder.`
            }
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

function contentKindSingular(kind: InstanceContentKind) {
  switch (kind) {
    case "resourcePack":
      return "Resource pack";
    case "shaderPack":
      return "Shader pack";
    case "dataPack":
      return "Data pack";
  }
}

export function ModSearchResult({
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

export function ContentSelect({
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

export function ModResultSkeletons() {
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

export function InstalledModSkeletons() {
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

function formatCompactNumber(value: number) {
  return new Intl.NumberFormat(undefined, { notation: "compact" }).format(
    value,
  );
}
