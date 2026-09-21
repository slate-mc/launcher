import { ArrowRight, ChevronLeft, LoaderCircle, Search } from "lucide-react";
import { useState } from "react";
import { EmptyState, InlineNotice } from "../../components/PageScaffold";
import type { InstanceMod } from "../../types/launcher";
import {
  ContentSelect,
  InstalledModSkeletons,
} from "./ContentBrowserComponents";
import { InstalledModRow, InstalledModTableHeader } from "./InstalledModRow";
import {
  compareInstalledMods,
  type InstalledModSort,
} from "./instanceContentModel";

type PendingModAction =
  "update" | "relationships" | "toggle" | "pin" | "remove";

export function InstalledModsTable({
  instanceId,
  instanceName,
  installed,
  loading,
  failed,
  resolutionFetching,
  disabled,
  pendingAction,
  onUpdate,
  onResolveRelationships,
  onToggle,
  onPin,
  onRemove,
  browserOpen,
  isVanilla,
}: {
  instanceId: string;
  instanceName: string;
  installed: InstanceMod[];
  loading: boolean;
  failed: boolean;
  resolutionFetching: boolean;
  disabled: boolean;
  pendingAction: (item: InstanceMod) => PendingModAction | undefined;
  onUpdate: (item: InstanceMod, versionId: string) => void;
  onResolveRelationships: (item: InstanceMod) => Promise<void>;
  onToggle: (item: InstanceMod) => void;
  onPin: (item: InstanceMod) => void;
  onRemove: (item: InstanceMod) => void;
  browserOpen: boolean;
  isVanilla: boolean;
}) {
  const [filter, setFilter] = useState("");
  const [status, setStatus] = useState<"all" | "enabled" | "disabled">("all");
  const [origin, setOrigin] = useState<"all" | InstanceMod["origin"]>("all");
  const [sort, setSort] = useState<InstalledModSort>({
    key: "name",
    direction: "ascending",
  });
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(25);
  const [removeTarget, setRemoveTarget] = useState<string>();
  const normalizedFilter = filter.trim().toLocaleLowerCase();
  const visible = installed
    .filter(
      (item) =>
        !normalizedFilter ||
        `${item.displayName} ${item.filePath} ${item.provider ?? ""} ${item.versionId ?? ""}`
          .toLocaleLowerCase()
          .includes(normalizedFilter),
    )
    .filter(
      (item) =>
        status === "all" ||
        (status === "enabled" ? item.enabled : !item.enabled),
    )
    .filter((item) => origin === "all" || item.origin === origin)
    .sort((left, right) => compareInstalledMods(left, right, sort));
  const enabledCount = installed.filter((item) => item.enabled).length;
  const pageCount = Math.max(1, Math.ceil(visible.length / pageSize));
  const activePage = Math.min(page, pageCount);
  const pageStart = (activePage - 1) * pageSize;
  const paged = visible.slice(pageStart, pageStart + pageSize);

  if (loading) return <InstalledModSkeletons />;
  if (failed) {
    return (
      <div className="p-5">
        <InlineNotice tone="danger" title="Installed mods could not be loaded">
          Reload this page to try again.
        </InlineNotice>
      </div>
    );
  }
  if (!installed.length) {
    return !browserOpen && !isVanilla ? (
      <EmptyState
        title="No mod JARs detected"
        description="Add a compatible mod from CurseForge or Modrinth, import a local JAR, or install a modpack from Discover."
      />
    ) : null;
  }

  return (
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
              value={filter}
              onChange={(event) => {
                setFilter(event.target.value);
                setPage(1);
              }}
              placeholder="Filter by name, file, provider, or version"
              className="h-8 w-full rounded-control border border-app-separator bg-app-bg pr-3 pl-9 text-[11px] text-app-text outline-none placeholder:text-app-muted focus:border-app-accent"
            />
          </label>
          <div className="w-32">
            <ContentSelect
              label="Mod status"
              value={status}
              options={[
                ["all", "All statuses"],
                ["enabled", "Enabled"],
                ["disabled", "Disabled"],
              ]}
              onChange={(value) => {
                setStatus(value as typeof status);
                setPage(1);
              }}
              compact
            />
          </div>
          <div className="w-32">
            <ContentSelect
              label="Mod origin"
              value={origin}
              options={[
                ["all", "All origins"],
                ["modpack", "Modpack"],
                ["added", "Added"],
                ["local", "Local file"],
              ]}
              onChange={(value) => {
                setOrigin(value as typeof origin);
                setPage(1);
              }}
              compact
            />
          </div>
        </div>
        <span className="shrink-0 font-mono text-[10px] text-app-muted">
          {resolutionFetching ? (
            <span className="inline-flex items-center gap-1.5">
              <LoaderCircle
                size={11}
                className="animate-spin motion-reduce:animate-none"
                aria-hidden="true"
              />
              Checking installed mods
            </span>
          ) : (
            `${visible.length} shown · ${enabledCount} enabled · ${installed.length} total`
          )}
        </span>
      </div>
      {visible.length ? (
        <div className="overflow-x-auto">
          <table className="w-full min-w-[900px] table-fixed border-collapse text-left">
            <caption className="sr-only">
              Installed mods for {instanceName}
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
            <InstalledModTableHeader
              sort={sort}
              onSort={(next) => {
                setSort(next);
                setPage(1);
              }}
            />
            <tbody>
              {paged.map((item) => (
                <InstalledModRow
                  key={item.filePath}
                  item={item}
                  disabled={disabled}
                  confirmingRemove={removeTarget === item.filePath}
                  pendingAction={pendingAction(item)}
                  instanceId={instanceId}
                  onUpdate={(versionId) => onUpdate(item, versionId)}
                  onResolveRelationships={() => onResolveRelationships(item)}
                  onToggle={() => onToggle(item)}
                  onPin={() => onPin(item)}
                  onRequestRemove={() => setRemoveTarget(item.filePath)}
                  onCancelRemove={() => setRemoveTarget(undefined)}
                  onConfirmRemove={() => {
                    setRemoveTarget(undefined);
                    onRemove(item);
                  }}
                />
              ))}
            </tbody>
          </table>
          <div className="flex min-w-[900px] items-center justify-between border-t border-app-separator/45 bg-app-bg/20 px-4 py-2.5">
            <span className="font-mono text-[9px] text-app-muted">
              Rows {pageStart + 1}–
              {Math.min(pageStart + pageSize, visible.length)} of{" "}
              {visible.length}
            </span>
            <div className="flex items-center gap-2">
              <span className="text-[10px] text-app-muted">Rows</span>
              <div className="w-20">
                <ContentSelect
                  label="Rows per page"
                  value={String(pageSize)}
                  options={[
                    ["25", "25"],
                    ["50", "50"],
                    ["100", "100"],
                  ]}
                  onChange={(value) => {
                    setPageSize(Number(value));
                    setPage(1);
                  }}
                  compact
                />
              </div>
              <span className="min-w-20 text-center font-mono text-[9px] text-app-muted">
                Page {activePage} of {pageCount}
              </span>
              <button
                type="button"
                className="inline-flex size-8 items-center justify-center rounded-control border border-app-separator bg-app-bg text-app-secondary disabled:opacity-35"
                disabled={activePage === 1}
                onClick={() => setPage((current) => Math.max(1, current - 1))}
                aria-label="Previous installed mod page"
              >
                <ChevronLeft size={14} aria-hidden="true" />
              </button>
              <button
                type="button"
                className="inline-flex size-8 items-center justify-center rounded-control border border-app-separator bg-app-bg text-app-secondary disabled:opacity-35"
                disabled={activePage === pageCount}
                onClick={() =>
                  setPage((current) => Math.min(pageCount, current + 1))
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
              setFilter("");
              setStatus("all");
              setOrigin("all");
              setPage(1);
            }}
          >
            Clear filters
          </button>
        </div>
      )}
    </>
  );
}
