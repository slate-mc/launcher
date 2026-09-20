import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "@tanstack/react-router";
import {
  ArrowLeft,
  Box,
  Layers3,
  Settings2,
} from "lucide-react";
import { InstanceArtwork as InstanceProfileArtwork } from "../../components/InstanceArtwork";
import { EmptyState, StatusPill } from "../../components/PageScaffold";
import { getInstance } from "../../lib/bridge";
import {
  instanceVersionLine,
  setupStateLabel,
  setupStateTone,
} from "../../lib/format";
import type { LauncherInstance } from "../../types/launcher";
import { InstanceOverview } from "./InstanceOverview";
import { InstanceSettings } from "./InstanceSettings";
import { InstanceContent } from "./InstanceContent";

type InstanceSection = "overview" | "content" | "settings";

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
          <InstanceOverview instance={query.data} />
        ) : section === "content" ? (
          <InstanceContent instance={query.data} />
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
