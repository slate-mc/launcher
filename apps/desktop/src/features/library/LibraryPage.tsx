import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import {
  ChevronRight,
  FolderInput,
  Plus,
  RotateCcw,
  Search,
  Star,
} from "lucide-react";
import { useMemo, useState } from "react";
import { InstanceArtwork } from "../../components/InstanceArtwork";
import {
  EmptyState,
  InlineNotice,
  PageHeader,
  StatusPill,
} from "../../components/PageScaffold";
import {
  importInstance,
  listInstances,
  setInstanceFavorite,
} from "../../lib/bridge";
import {
  formatDate,
  instanceVersionLine,
  setupStateLabel,
  setupStateTone,
} from "../../lib/format";
import { getUserFacingError } from "../../lib/userFacingError";
import type { LoaderKind } from "../../types/launcher";

type LoaderFilter = "all" | LoaderKind;

export function LibraryPage() {
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [loader, setLoader] = useState<LoaderFilter>("all");
  const [importedMessage, setImportedMessage] = useState<string>();
  const instancesQuery = useQuery({
    queryKey: ["instances"],
    queryFn: listInstances,
  });
  const favoriteMutation = useMutation({
    mutationFn: setInstanceFavorite,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
    },
  });
  const importMutation = useMutation({
    mutationFn: importInstance,
    onMutate: () => setImportedMessage(undefined),
    onSuccess: async (instance) => {
      if (!instance) return;
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setImportedMessage(
        instance.setupState === "preparing"
          ? `${instance.name} was added. Installation is running.`
          : `${instance.name} was added to your library.`,
      );
    },
  });

  const instances = useMemo(() => {
    const term = search.trim().toLocaleLowerCase();
    return (instancesQuery.data ?? []).filter((instance) => {
      const matchesSearch =
        !term ||
        instance.name.toLocaleLowerCase().includes(term) ||
        instance.minecraftVersion.toLocaleLowerCase().includes(term);
      return (
        matchesSearch && (loader === "all" || instance.loaderKind === loader)
      );
    });
  }, [instancesQuery.data, loader, search]);

  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Library"
        title="Your instances"
        description="Keep each Minecraft setup separate, organized, and ready to play."
        actions={
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="inline-flex h-10 items-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text hover:border-app-secondary disabled:opacity-50"
              disabled={importMutation.isPending}
              onClick={() => importMutation.mutate()}
            >
              {importMutation.isPending ? (
                <RotateCcw
                  className="animate-spin"
                  size={16}
                  aria-hidden="true"
                />
              ) : (
                <FolderInput size={16} aria-hidden="true" />
              )}
              Import…
            </button>
            <Link
              to="/library/new"
              className="inline-flex h-10 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent no-underline hover:brightness-105"
            >
              <Plus size={17} aria-hidden="true" />
              New instance
            </Link>
          </div>
        }
      />

      <div className="px-8 py-6">
        {importMutation.isError ? (
          <div className="mb-5">
            <InlineNotice tone="danger" title="Import did not complete">
              {getUserFacingError(
                importMutation.error,
                "Choose a valid CurseForge ZIP, Modrinth .mrpack, or slate instance archive.",
              )}
            </InlineNotice>
          </div>
        ) : importedMessage ? (
          <div className="mb-5">
            <InlineNotice tone="positive" title="Import started">
              {importedMessage}
            </InlineNotice>
          </div>
        ) : null}
        <div className="flex items-center justify-between gap-4">
          <label className="relative block w-full max-w-[430px]">
            <span className="sr-only">Search instances</span>
            <Search
              className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-app-muted"
              size={17}
              aria-hidden="true"
            />
            <input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              className="h-10 w-full rounded-control border border-app-separator bg-app-surface pr-3 pl-10 text-xs text-app-text placeholder:text-app-muted focus:border-app-accent focus:outline-none"
              placeholder="Search by name or version"
            />
          </label>
          <div className="flex items-center rounded-control border border-app-separator bg-app-surface p-1">
            {(["all", "vanilla", "fabric", "neoForge"] as const).map(
              (value) => (
                <button
                  type="button"
                  key={value}
                  className={`h-8 rounded-compact border-0 px-3 text-[11px] font-bold ${
                    loader === value
                      ? "bg-app-raised text-app-text"
                      : "bg-transparent text-app-muted hover:text-app-text"
                  }`}
                  aria-pressed={loader === value}
                  onClick={() => setLoader(value)}
                >
                  {value === "all"
                    ? "All loaders"
                    : value === "neoForge"
                      ? "NeoForge"
                      : value[0].toUpperCase() + value.slice(1)}
                </button>
              ),
            )}
          </div>
        </div>

        {instancesQuery.isPending ? (
          <div className="mt-6 grid gap-2" aria-label="Loading instances">
            {[0, 1, 2].map((value) => (
              <div
                key={value}
                className="h-[78px] animate-pulse rounded-control bg-app-surface"
              />
            ))}
          </div>
        ) : instancesQuery.isError ? (
          <EmptyState
            error
            title="The library is unavailable"
            description="Your local data was not changed. Try again."
            action={
              <button
                type="button"
                className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text"
                onClick={() => void instancesQuery.refetch()}
              >
                <RotateCcw size={16} aria-hidden="true" />
                Retry
              </button>
            }
          />
        ) : instances.length === 0 ? (
          <EmptyState
            title={
              search || loader !== "all"
                ? "No matching instances"
                : "No instances yet"
            }
            description={
              search || loader !== "all"
                ? "Clear the search or choose another loader filter."
                : "Create a Vanilla, Fabric, or NeoForge setup to start your library."
            }
            action={
              search || loader !== "all" ? (
                <button
                  className="h-9 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text"
                  type="button"
                  onClick={() => {
                    setSearch("");
                    setLoader("all");
                  }}
                >
                  Clear filters
                </button>
              ) : (
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text disabled:opacity-50"
                    disabled={importMutation.isPending}
                    onClick={() => importMutation.mutate()}
                  >
                    <FolderInput size={16} aria-hidden="true" />
                    Import
                  </button>
                  <Link
                    to="/library/new"
                    className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent no-underline"
                  >
                    <Plus size={16} aria-hidden="true" />
                    Create instance
                  </Link>
                </div>
              )
            }
          />
        ) : (
          <div className="mt-6 overflow-hidden rounded-control border border-app-separator/70">
            <div className="grid h-9 grid-cols-[minmax(250px,1.4fr)_minmax(230px,1fr)_130px_120px_42px] items-center gap-4 bg-app-sidebar px-4 text-[10px] font-bold tracking-[.07em] text-app-muted uppercase">
              <span>Name</span>
              <span>Version and loader</span>
              <span>State</span>
              <span>Created</span>
              <span className="sr-only">Favorite</span>
            </div>
            {instances.map((instance) => (
              <div
                key={instance.id}
                className="grid min-h-[76px] grid-cols-[minmax(250px,1.4fr)_minmax(230px,1fr)_130px_120px_42px] items-center gap-4 border-t border-app-separator/50 bg-app-bg px-4 hover:bg-app-hover/25"
              >
                <Link
                  to="/instances/$instanceId/overview"
                  params={{ instanceId: instance.id }}
                  className="flex min-w-0 items-center gap-3 text-app-text no-underline"
                >
                  <InstanceArtwork
                    instance={instance}
                    className="size-10 rounded-lg border border-app-separator/70"
                  />
                  <span className="min-w-0">
                    <strong className="block overflow-hidden text-[13px] font-bold text-ellipsis whitespace-nowrap">
                      {instance.name}
                    </strong>
                    <small className="mt-0.5 block text-[11px] text-app-muted capitalize">
                      {instance.mode} setup
                    </small>
                  </span>
                </Link>
                <span className="min-w-0 overflow-hidden font-mono text-[11px] text-app-secondary text-ellipsis whitespace-nowrap">
                  {instanceVersionLine(instance)}
                </span>
                <StatusPill tone={setupStateTone(instance.setupState)}>
                  {setupStateLabel(instance.setupState)}
                </StatusPill>
                <span className="text-[11px] text-app-secondary">
                  {formatDate(instance.createdAt)}
                </span>
                <button
                  type="button"
                  className="inline-flex size-8 items-center justify-center rounded-compact border-0 bg-transparent text-app-muted hover:bg-app-raised hover:text-app-accent disabled:opacity-50"
                  aria-label={
                    instance.favorite
                      ? `Remove ${instance.name} from favorites`
                      : `Add ${instance.name} to favorites`
                  }
                  disabled={favoriteMutation.isPending}
                  onClick={() =>
                    favoriteMutation.mutate({
                      id: instance.id,
                      favorite: !instance.favorite,
                      expectedRevision: instance.revision,
                    })
                  }
                >
                  <Star
                    size={17}
                    fill={instance.favorite ? "currentColor" : "none"}
                    aria-hidden="true"
                  />
                </button>
              </div>
            ))}
          </div>
        )}

        {favoriteMutation.isError ? (
          <p className="mt-3 text-xs text-app-danger" role="alert">
            The favorite could not be updated. Reload the library and try again.
          </p>
        ) : null}
        {instances.length > 0 ? (
          <p className="mt-3 flex items-center justify-end gap-1.5 text-[11px] text-app-muted">
            Open an instance to review its configuration
            <ChevronRight size={14} aria-hidden="true" />
          </p>
        ) : null}
      </div>
    </div>
  );
}
