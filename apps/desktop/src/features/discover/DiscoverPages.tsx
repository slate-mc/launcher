import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import {
  ArrowLeft,
  ArrowRight,
  CalendarDays,
  Check,
  ChevronLeft,
  Download,
  HardDriveDownload,
  LoaderCircle,
  Search,
  ShieldCheck,
} from "lucide-react";
import { useState } from "react";
import { ContentArtwork } from "../../components/ContentArtwork";
import { ContentBanner } from "../../components/ContentBanner";
import {
  EmptyState,
  InlineNotice,
  PageHeader,
} from "../../components/PageScaffold";
import { ProviderMarkdown } from "../../components/ProviderMarkdown";
import {
  getMinecraftVersionCatalog,
  getModpack,
  getModpackProviders,
  getModpackVersion,
  installModpack,
  listModpackVersions,
  searchModpacks,
} from "../../lib/bridge";
import type {
  ContentLoader,
  ModpackSummary,
  Provider,
} from "../../types/launcher";

const supportedLoaders = new Set<ContentLoader>([
  "vanilla",
  "fabric",
  "neoforge",
]);

export function DiscoverPage() {
  const [draftQuery, setDraftQuery] = useState("");
  const [query, setQuery] = useState("");
  const [provider, setProvider] = useState<Provider | "all">("all");
  const [minecraftVersion, setMinecraftVersion] = useState("");
  const [loader, setLoader] = useState<ContentLoader | "all">("all");
  const [sort, setSort] = useState<
    "relevance" | "downloads" | "updated" | "newest"
  >("relevance");
  const [page, setPage] = useState(1);
  const providersQuery = useQuery({
    queryKey: ["modpack-providers"],
    queryFn: getModpackProviders,
    staleTime: 30_000,
  });
  const minecraftQuery = useQuery({
    queryKey: ["minecraft-version-catalog"],
    queryFn: getMinecraftVersionCatalog,
    staleTime: 5 * 60_000,
  });
  const searchQuery = useQuery({
    queryKey: [
      "modpack-search",
      query,
      provider,
      minecraftVersion,
      loader,
      sort,
      page,
    ],
    queryFn: () =>
      searchModpacks({
        query: query || undefined,
        provider: provider === "all" ? undefined : provider,
        minecraftVersion: minecraftVersion || undefined,
        loader: loader === "all" ? undefined : loader,
        sort,
        page,
        limit: 20,
      }),
    placeholderData: (previous) => previous,
  });
  const unavailableProviders = Object.entries(
    searchQuery.data?.provider_status ?? {},
  )
    .filter(([, status]) => status === "unavailable")
    .map(([id]) => providerName(id as Provider));

  const updateFilter = (change: () => void) => {
    change();
    setPage(1);
  };

  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Discover"
        title="Find your next modpack"
        description="Search CurseForge, Modrinth, and FTB through slate’s normalized content service. Provider formats and credentials stay outside the launcher."
      />
      <main className="px-8 py-7">
        <form
          className="grid grid-cols-[minmax(260px,1fr)_170px_170px_150px_auto] gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            setQuery(draftQuery.trim());
            setPage(1);
          }}
        >
          <label className="relative block">
            <span className="sr-only">Search modpacks</span>
            <Search
              size={16}
              className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-app-muted"
              aria-hidden="true"
            />
            <input
              value={draftQuery}
              onChange={(event) => setDraftQuery(event.target.value)}
              placeholder="Search modpacks"
              className="h-10 w-full rounded-control border border-app-separator bg-app-surface pr-3 pl-10 text-[13px] text-app-text outline-none placeholder:text-app-muted focus:border-app-accent"
            />
          </label>
          <FilterSelect
            label="Provider"
            value={provider}
            onChange={(value) =>
              updateFilter(() => setProvider(value as Provider | "all"))
            }
            options={[
              ["all", "All providers"],
              ...(providersQuery.data?.providers ?? []).map(
                (item) =>
                  [
                    item.id,
                    item.available ? item.name : `${item.name} unavailable`,
                  ] as const,
              ),
            ]}
          />
          <FilterSelect
            label="Minecraft version"
            value={minecraftVersion}
            onChange={(value) => updateFilter(() => setMinecraftVersion(value))}
            options={[
              ["", "Any Minecraft"],
              ...(minecraftQuery.data?.versions
                .slice(0, 80)
                .map((item) => [item.id, item.id] as const) ?? []),
            ]}
          />
          <FilterSelect
            label="Loader"
            value={loader}
            onChange={(value) =>
              updateFilter(() => setLoader(value as ContentLoader | "all"))
            }
            options={[
              ["all", "Any loader"],
              ["fabric", "Fabric"],
              ["neoforge", "NeoForge"],
              ["vanilla", "Vanilla"],
              ["forge", "Forge"],
              ["quilt", "Quilt"],
            ]}
          />
          <button
            type="submit"
            className="inline-flex h-10 items-center justify-center gap-2 rounded-control bg-app-accent px-5 text-xs font-bold text-app-on-accent hover:brightness-105"
          >
            <Search size={15} aria-hidden="true" />
            Search
          </button>
        </form>

        <div className="mt-5 flex items-center justify-between border-b border-app-separator/55 pb-3">
          <p className="m-0 text-xs text-app-secondary">
            {searchQuery.isPending
              ? "Loading provider catalogs"
              : `${searchQuery.data?.items.length ?? 0} results on page ${page}`}
          </p>
          <FilterSelect
            label="Sort results"
            compact
            value={sort}
            onChange={(value) =>
              updateFilter(() => setSort(value as typeof sort))
            }
            options={[
              ["relevance", "Relevance"],
              ["downloads", "Downloads"],
              ["updated", "Recently updated"],
              ["newest", "Newest"],
            ]}
          />
        </div>

        {unavailableProviders.length > 0 ? (
          <div className="mt-4">
            <InlineNotice tone="warning" title="Some providers did not respond">
              Results from {unavailableProviders.join(", ")} are temporarily
              missing. Other providers are still shown.
            </InlineNotice>
          </div>
        ) : null}

        {searchQuery.isPending ? (
          <ResultSkeletons />
        ) : searchQuery.isError ? (
          <EmptyState
            error
            title="Modpacks could not be loaded"
            description={errorMessage(searchQuery.error)}
            action={
              <button
                type="button"
                className="h-9 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent"
                onClick={() => void searchQuery.refetch()}
              >
                Try again
              </button>
            }
          />
        ) : searchQuery.data.items.length === 0 ? (
          <EmptyState
            title="No matching modpacks"
            description="Try a broader search or remove a Minecraft version, loader, or provider filter."
          />
        ) : (
          <div
            className="divide-y divide-app-separator/45"
            aria-busy={searchQuery.isFetching}
          >
            {searchQuery.data.items.map((item) => (
              <ModpackResult key={`${item.provider}:${item.id}`} item={item} />
            ))}
          </div>
        )}

        {searchQuery.data?.items.length ? (
          <nav
            className="mt-5 flex items-center justify-between"
            aria-label="Modpack result pages"
          >
            <button
              type="button"
              disabled={page === 1 || searchQuery.isFetching}
              className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-surface px-3 text-xs font-bold text-app-secondary disabled:opacity-40"
              onClick={() => setPage((current) => Math.max(1, current - 1))}
            >
              <ArrowLeft size={15} aria-hidden="true" /> Previous
            </button>
            <span className="font-mono text-[11px] text-app-muted">
              Page {page}
            </span>
            <button
              type="button"
              disabled={!searchQuery.data.has_more || searchQuery.isFetching}
              className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-surface px-3 text-xs font-bold text-app-secondary disabled:opacity-40"
              onClick={() => setPage((current) => current + 1)}
            >
              Next <ArrowRight size={15} aria-hidden="true" />
            </button>
          </nav>
        ) : null}
      </main>
    </div>
  );
}

function ModpackResult({ item }: { item: ModpackSummary }) {
  return (
    <Link
      to="/discover/$provider/$projectId"
      params={{ provider: item.provider, projectId: item.id }}
      className="group grid grid-cols-[64px_minmax(0,1fr)_auto] items-center gap-4 px-2 py-4 text-app-text no-underline hover:bg-app-surface/65"
    >
      <PackImage
        src={item.icon_url}
        name={item.name}
        stableKey={`${item.provider}:${item.id}`}
      />
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <h2 className="m-0 truncate text-[15px] font-bold tracking-[-.015em]">
            {item.name}
          </h2>
          <ProviderBadge provider={item.provider} />
        </div>
        <p className="mt-1 mb-0 line-clamp-2 max-w-[780px] text-xs/[18px] text-app-secondary">
          {item.summary || "No description supplied by the provider."}
        </p>
        <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 font-mono text-[10px] text-app-muted">
          <span>{formatDownloads(item.downloads)} downloads</span>
          <span>
            {item.minecraft_versions.slice(0, 3).join(", ") ||
              "Version not declared"}
          </span>
          <span>
            {item.loaders.map(loaderLabel).join(", ") || "Loader not declared"}
          </span>
        </div>
      </div>
      <ArrowRight
        size={18}
        className="text-app-muted group-hover:text-app-accent"
        aria-hidden="true"
      />
    </Link>
  );
}

export function ModpackDetailPage() {
  const { provider: rawProvider, projectId } = useParams({ strict: false });
  const provider = rawProvider as Provider;
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [selectedVersionId, setSelectedVersionId] = useState("");
  const [instanceName, setInstanceName] = useState<string | null>(null);
  const [optionalFilesByVersion, setOptionalFilesByVersion] = useState<
    Record<string, string[]>
  >({});
  const projectQuery = useQuery({
    queryKey: ["modpack", provider, projectId],
    queryFn: () => getModpack(provider, projectId ?? ""),
    enabled: Boolean(projectId),
  });
  const versionsQuery = useQuery({
    queryKey: ["modpack-versions", provider, projectId],
    queryFn: () =>
      listModpackVersions({
        provider,
        projectId: projectId ?? "",
        page: 1,
        limit: 50,
      }),
    enabled: Boolean(projectId),
  });
  const effectiveVersionId =
    selectedVersionId ||
    projectQuery.data?.latest_version?.id ||
    versionsQuery.data?.items[0]?.id ||
    "";
  const versionQuery = useQuery({
    queryKey: ["modpack-version", provider, projectId, effectiveVersionId],
    queryFn: () =>
      getModpackVersion({
        provider,
        projectId: projectId ?? "",
        versionId: effectiveVersionId,
      }),
    enabled: Boolean(projectId && effectiveVersionId),
  });
  const installMutation = useMutation({
    mutationFn: installModpack,
    onSuccess: async (started) => {
      queryClient.setQueryData(
        ["instance", started.instance.id],
        started.instance,
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["instances"] }),
        queryClient.invalidateQueries({ queryKey: ["install-jobs"] }),
      ]);
      await navigate({
        to: "/instances/$instanceId/overview",
        params: { instanceId: started.instance.id },
      });
    },
  });

  if (projectQuery.isPending) return <DetailSkeleton />;
  if (projectQuery.isError || !projectQuery.data) {
    return (
      <EmptyState
        error
        title="That modpack is unavailable"
        description={errorMessage(projectQuery.error)}
        action={<BackToDiscover />}
      />
    );
  }
  const project = projectQuery.data;
  const version = versionQuery.data;
  const versionSummary = versionsQuery.data?.items.find(
    (item) => item.id === effectiveVersionId,
  );
  const releaseName =
    version?.name ??
    versionSummary?.name ??
    project.latest_version?.name ??
    "Select a release";
  const releaseMinecraft =
    version?.minecraft.version ?? versionSummary?.minecraft_version;
  const releaseLoader = version?.loader ?? versionSummary?.loader;
  const resolvingRelease = Boolean(
    effectiveVersionId && versionQuery.isPending,
  );
  const resolvedInstanceName = instanceName ?? project.name;
  const optionalFiles = optionalFilesByVersion[effectiveVersionId] ?? [];
  const installable = version
    ? supportedLoaders.has(version.loader.type)
    : false;
  const missingLoaderVersion =
    version?.loader.type !== "vanilla" && !version?.loader.version;
  const deferredIntegrityFiles =
    version?.files.filter(
      (file) => !file.hashes.sha512 && !file.hashes.sha256 && !file.hashes.sha1,
    ).length ?? 0;
  const optionalOptions =
    version?.files
      .filter((file) => file.optional && file.option)
      .map((file) => file.option!) ?? [];

  return (
    <div className="min-h-full bg-app-bg">
      <header className="relative isolate overflow-hidden border-b border-app-separator/55 px-8 py-7">
        <ContentBanner
          bannerSrc={project.banner_url}
          iconSrc={project.icon_url}
          name={`${project.name} banner`}
          eager
          className="absolute inset-0 -z-30 size-full rounded-none opacity-45"
          imageClassName="scale-[1.02]"
        />
        <span
          className="absolute inset-0 -z-20 bg-[linear-gradient(90deg,var(--slate-bg)_0%,rgb(16_20_19_/_88%)_45%,rgb(16_20_19_/_62%)_100%),linear-gradient(0deg,var(--slate-bg)_0%,transparent_80%)]"
          aria-hidden="true"
        />
        <BackToDiscover />
        <div className="mt-5 grid grid-cols-[88px_minmax(0,1fr)_auto] items-start gap-5">
          <PackImage
            src={project.icon_url}
            name={project.name}
            stableKey={`${project.provider}:${project.id}`}
            large
          />
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <ProviderBadge provider={project.provider} />
              <span className="font-mono text-[10px] text-app-muted">
                {project.id}
              </span>
            </div>
            <h1 className="mt-2 mb-0 text-[30px]/[36px] font-bold tracking-[-.035em]">
              {project.name}
            </h1>
            <p className="mt-2 mb-0 max-w-[760px] text-[13px]/[20px] text-app-secondary">
              {project.summary || "No summary supplied by the provider."}
            </p>
          </div>
          <div className="flex gap-5 text-right">
            <Metric
              icon={Download}
              label="Downloads"
              value={formatDownloads(project.downloads)}
            />
            <Metric
              icon={CalendarDays}
              label="Updated"
              value={formatShortDate(project.updated_at)}
            />
          </div>
        </div>
      </header>

      <main className="grid grid-cols-[minmax(0,1fr)_360px] gap-7 px-8 py-7">
        <div className="min-w-0">
          <section>
            <h2 className="m-0 text-[15px] font-bold">About this pack</h2>
            <ProviderMarkdown
              value={
                project.description ||
                project.summary ||
                "No description supplied by the provider."
              }
              className="mt-3"
            />
            <div className="mt-4 flex flex-wrap gap-2">
              {project.categories.slice(0, 10).map((category) => (
                <span
                  key={category}
                  className="rounded-full border border-app-separator px-2.5 py-1 text-[10px] text-app-secondary"
                >
                  {category}
                </span>
              ))}
            </div>
          </section>

          <section className="mt-7 border-t border-app-separator/55 pt-6">
            <div className="flex items-end justify-between gap-4">
              <div>
                <h2 className="m-0 text-[15px] font-bold">Choose a version</h2>
                <p className="mt-1 mb-0 text-[11px] text-app-secondary">
                  The selected release determines Minecraft, loader, loader
                  version, and files.
                </p>
              </div>
              <span className="font-mono text-[10px] text-app-muted">
                {versionsQuery.data?.items.length ?? 0} releases
              </span>
            </div>
            {versionsQuery.isPending ? (
              <div className="mt-4 h-36 animate-pulse rounded-control bg-app-surface" />
            ) : versionsQuery.isError ? (
              <div className="mt-4">
                <InlineNotice
                  tone="danger"
                  title="Versions could not be loaded"
                >
                  {errorMessage(versionsQuery.error)}
                </InlineNotice>
              </div>
            ) : (
              <div className="mt-4 max-h-[320px] overflow-y-auto border-y border-app-separator/55">
                {versionsQuery.data?.items.map((item) => {
                  const selected = item.id === effectiveVersionId;
                  const supported = supportedLoaders.has(item.loader.type);
                  return (
                    <button
                      key={item.id}
                      type="button"
                      className={`grid min-h-16 w-full grid-cols-[minmax(0,1fr)_170px_110px_24px] items-center gap-4 border-0 border-b border-app-separator/40 px-3 text-left last:border-b-0 ${selected ? "bg-app-accent/8" : "bg-transparent hover:bg-app-surface"}`}
                      onClick={() => setSelectedVersionId(item.id)}
                    >
                      <span className="min-w-0">
                        <strong className="block truncate text-xs text-app-text">
                          {item.name}
                        </strong>
                        <span className="mt-1 block font-mono text-[10px] text-app-muted">
                          {item.release_type}
                        </span>
                      </span>
                      <span className="font-mono text-[10px] text-app-secondary">
                        Minecraft {item.minecraft_version}
                      </span>
                      <span
                        className={
                          supported
                            ? "text-[11px] text-app-secondary"
                            : "text-[11px] text-app-warning"
                        }
                      >
                        {loaderLabel(item.loader.type)}{" "}
                        {item.loader.version ?? "· resolves on selection"}
                      </span>
                      {selected ? (
                        <Check
                          size={16}
                          className="text-app-accent"
                          aria-hidden="true"
                        />
                      ) : null}
                    </button>
                  );
                })}
              </div>
            )}
          </section>
        </div>

        <aside className="self-start rounded-control border border-app-separator/70 bg-app-surface p-5">
          <p className="m-0 text-[10px] font-bold tracking-[.08em] text-app-muted uppercase">
            Install instance
          </p>
          <h2 className="mt-2 mb-0 text-lg font-bold">{releaseName}</h2>
          {releaseMinecraft && releaseLoader ? (
            <dl className="mt-4 grid grid-cols-[110px_1fr] gap-y-2 text-xs">
              <dt className="text-app-muted">Minecraft</dt>
              <dd className="m-0 font-mono text-app-text">
                {releaseMinecraft}
              </dd>
              <dt className="text-app-muted">Loader</dt>
              <dd className="m-0 font-mono text-app-text">
                {loaderLabel(releaseLoader.type)} {releaseLoader.version}
              </dd>
              <dt className="text-app-muted">Download</dt>
              <dd className="m-0 font-mono text-app-text">
                {version ? (
                  `${formatBytes(version.total_download_size)}${deferredIntegrityFiles > 0 ? "+" : ""}`
                ) : (
                  <ResolvingValue />
                )}
              </dd>
              <dt className="text-app-muted">Memory</dt>
              <dd className="m-0 font-mono text-app-text">
                {version ? (
                  formatMemory(version.memory.recommended_mb)
                ) : (
                  <ResolvingValue />
                )}
              </dd>
            </dl>
          ) : resolvingRelease || versionsQuery.isPending ? (
            <p className="mt-4 flex items-center gap-2 text-xs text-app-secondary">
              <LoaderCircle
                size={14}
                className="animate-spin text-app-accent"
                aria-hidden="true"
              />
              Loading release details
            </p>
          ) : (
            <p className="mt-4 text-xs text-app-secondary">
              Select an available release to inspect its install target.
            </p>
          )}
          {deferredIntegrityFiles > 0 ? (
            <p className="mt-3 mb-0 text-[10px]/[16px] text-app-muted">
              {deferredIntegrityFiles} provider file
              {deferredIntegrityFiles === 1 ? "" : "s"} will be sized and
              verified when installation starts.
            </p>
          ) : null}
          {effectiveVersionId ? (
            <label className="mt-5 block text-xs font-bold text-app-text">
              Instance name
              <input
                value={resolvedInstanceName}
                maxLength={80}
                onChange={(event) => setInstanceName(event.target.value)}
                className="mt-2 h-10 w-full rounded-control border border-app-separator bg-app-bg px-3 text-[13px] font-normal text-app-text outline-none focus:border-app-accent"
              />
            </label>
          ) : null}
          {versionQuery.isError ? (
            <div className="mt-5">
              <InlineNotice tone="danger" title="Release details unavailable">
                {errorMessage(versionQuery.error)}
                <button
                  type="button"
                  className="mt-3 block text-[11px] font-bold text-app-accent hover:underline"
                  onClick={() => void versionQuery.refetch()}
                >
                  Retry release
                </button>
              </InlineNotice>
            </div>
          ) : null}
          {optionalOptions.length > 0 ? (
            <fieldset className="mt-5 border-0 p-0">
              <legend className="text-xs font-bold text-app-text">
                Optional content
              </legend>
              <div className="mt-2 grid gap-2">
                {optionalOptions.map((option) => (
                  <label
                    key={option.id}
                    className="flex items-center gap-2 text-[11px] text-app-secondary"
                  >
                    <input
                      type="checkbox"
                      checked={
                        option.default || optionalFiles.includes(option.id)
                      }
                      disabled={option.default}
                      onChange={(event) =>
                        setOptionalFilesByVersion((current) => ({
                          ...current,
                          [effectiveVersionId]: event.target.checked
                            ? [
                                ...(current[effectiveVersionId] ?? []),
                                option.id,
                              ]
                            : (current[effectiveVersionId] ?? []).filter(
                                (id) => id !== option.id,
                              ),
                        }))
                      }
                    />
                    {option.name}
                    {option.default ? " (included)" : ""}
                  </label>
                ))}
              </div>
            </fieldset>
          ) : null}
          {version && (!installable || missingLoaderVersion) ? (
            <div className="mt-5">
              <InlineNotice
                tone="warning"
                title="This release cannot be installed yet"
              >
                {!installable
                  ? `${loaderLabel(version.loader.type)} support is not available in slate yet.`
                  : "The provider did not resolve an exact loader version, so slate will not guess one."}
              </InlineNotice>
            </div>
          ) : null}
          {installMutation.isError ? (
            <div className="mt-5">
              <InlineNotice tone="danger" title="Installation could not start">
                {errorMessage(installMutation.error)}
              </InlineNotice>
            </div>
          ) : null}
          {effectiveVersionId ? (
            <>
              <button
                type="button"
                disabled={
                  !version ||
                  !installable ||
                  missingLoaderVersion ||
                  !resolvedInstanceName.trim() ||
                  installMutation.isPending
                }
                className="mt-5 inline-flex h-11 w-full items-center justify-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:bg-app-raised disabled:text-app-muted"
                onClick={() => {
                  if (!version) return;
                  installMutation.mutate({
                    provider,
                    projectId: project.id,
                    versionId: version.id,
                    instanceName: resolvedInstanceName.trim(),
                    includeOptional: optionalFiles,
                  });
                }}
              >
                {installMutation.isPending || resolvingRelease ? (
                  <LoaderCircle
                    size={17}
                    className="animate-spin"
                    aria-hidden="true"
                  />
                ) : (
                  <HardDriveDownload size={17} aria-hidden="true" />
                )}
                {installMutation.isPending
                  ? "Verifying pack files"
                  : resolvingRelease
                    ? "Resolving release"
                    : "Install modpack"}
              </button>
              <p className="mt-3 mb-0 flex items-start gap-2 text-[10px]/[16px] text-app-muted">
                <ShieldCheck
                  size={14}
                  className="mt-px shrink-0 text-app-accent"
                  aria-hidden="true"
                />
                Files are staged, hashed, and applied transactionally. The API
                is re-queried on retry.
              </p>
            </>
          ) : null}
        </aside>
      </main>
    </div>
  );
}

function ResolvingValue() {
  return (
    <span className="inline-flex items-center gap-1.5 font-sans text-[11px] text-app-muted">
      <LoaderCircle size={12} className="animate-spin" aria-hidden="true" />
      Resolving
    </span>
  );
}

function FilterSelect({
  label,
  value,
  onChange,
  options,
  compact = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: ReadonlyArray<readonly [string, string]>;
  compact?: boolean;
}) {
  return (
    <label className={compact ? "flex items-center gap-2" : "block"}>
      <span className={compact ? "text-[11px] text-app-muted" : "sr-only"}>
        {label}
      </span>
      <select
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className={`${compact ? "h-8" : "h-10 w-full"} rounded-control border border-app-separator bg-app-surface px-3 text-xs text-app-secondary outline-none focus:border-app-accent`}
      >
        {options.map(([optionValue, optionLabel]) => (
          <option key={optionValue || "any"} value={optionValue}>
            {optionLabel}
          </option>
        ))}
      </select>
    </label>
  );
}

function ProviderBadge({ provider }: { provider: Provider }) {
  return (
    <span className="rounded-full border border-app-separator px-2 py-0.5 font-mono text-[9px] text-app-muted">
      {providerName(provider)}
    </span>
  );
}

function PackImage({
  src,
  name,
  stableKey,
  large = false,
}: {
  src?: string | null;
  name: string;
  stableKey: string;
  large?: boolean;
}) {
  return (
    <ContentArtwork
      src={src}
      name={name}
      stableKey={stableKey}
      className={`${large ? "size-[88px]" : "size-16"} rounded-control border border-app-separator`}
    />
  );
}

function Metric({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof Download;
  label: string;
  value: string;
}) {
  return (
    <span>
      <span className="flex items-center justify-end gap-1.5 text-[10px] text-app-muted">
        <Icon size={13} aria-hidden="true" />
        {label}
      </span>
      <strong className="mt-1 block font-mono text-[11px] text-app-text">
        {value}
      </strong>
    </span>
  );
}

function BackToDiscover() {
  return (
    <Link
      to="/discover"
      className="inline-flex items-center gap-2 text-[11px] font-bold text-app-secondary no-underline hover:text-app-text"
    >
      <ChevronLeft size={15} aria-hidden="true" />
      Back to discover
    </Link>
  );
}

function ResultSkeletons() {
  return (
    <div
      className="divide-y divide-app-separator/45"
      aria-label="Loading modpacks"
    >
      {Array.from({ length: 7 }, (_, index) => (
        <div key={index} className="grid grid-cols-[64px_1fr] gap-4 py-4">
          <span className="size-16 animate-pulse rounded-control bg-app-surface" />
          <span className="grid content-center gap-2">
            <span className="h-4 w-52 animate-pulse rounded bg-app-surface" />
            <span className="h-3 w-4/5 animate-pulse rounded bg-app-surface" />
            <span className="h-3 w-64 animate-pulse rounded bg-app-surface" />
          </span>
        </div>
      ))}
    </div>
  );
}

function DetailSkeleton() {
  return (
    <div className="p-8" aria-label="Loading modpack">
      <div className="h-36 animate-pulse rounded-control bg-app-surface" />
      <div className="mt-7 grid grid-cols-[1fr_360px] gap-7">
        <div className="h-96 animate-pulse rounded-control bg-app-surface" />
        <div className="h-96 animate-pulse rounded-control bg-app-surface" />
      </div>
    </div>
  );
}

function providerName(provider: Provider) {
  if (provider === "curseforge") return "CurseForge";
  if (provider === "modrinth") return "Modrinth";
  return "FTB";
}

function loaderLabel(loader: ContentLoader) {
  if (loader === "neoforge") return "NeoForge";
  return loader.charAt(0).toUpperCase() + loader.slice(1);
}

function formatDownloads(value: number) {
  return new Intl.NumberFormat("en-US", {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}

function formatBytes(value: number) {
  if (value < 1024 * 1024) return `${Math.max(1, Math.round(value / 1024))} KB`;
  if (value < 1024 * 1024 * 1024)
    return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function formatMemory(value: number) {
  return value >= 1024
    ? `${(value / 1024).toFixed(value % 1024 === 0 ? 0 : 1)} GB`
    : `${value} MB`;
}

function formatShortDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.valueOf())
    ? "Unknown"
    : new Intl.DateTimeFormat("en-US", {
        month: "short",
        day: "numeric",
        year: "numeric",
      }).format(date);
}

function errorMessage(error: unknown) {
  if (error instanceof Error && error.message) return error.message;
  if (
    typeof error === "object" &&
    error &&
    "message" in error &&
    typeof error.message === "string"
  )
    return error.message;
  return "The content service did not complete the request. Try again shortly.";
}
