import { Check, ChevronLeft, LoaderCircle } from "lucide-react";
import { ContentArtwork } from "../../components/ContentArtwork";
import type { ModpackSummary } from "../../types/launcher";
import {
  modProviderName,
  type ContentSectionKind,
} from "./instanceContentModel";

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
      <ContentArtwork
        src={item.icon_url}
        name={item.name}
        stableKey={`${item.provider}:${item.id}`}
        className="size-11 rounded-control border border-app-separator"
      />
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
          {item.summary || "No description available."}
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
                ? "Checking installed content"
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

export function ModResultSkeletons() {
  return (
    <div
      className="divide-y divide-app-separator/45 px-5"
      aria-label="Loading compatible content"
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
