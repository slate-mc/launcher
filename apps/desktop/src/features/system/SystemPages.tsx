import { Link } from "@tanstack/react-router";
import {
  Activity,
  Boxes,
  CircleHelp,
  Compass,
  Plus,
  Server,
  ShieldX,
  UserRound,
} from "lucide-react";
import { EmptyState, InlineNotice, PageHeader } from "../../components/PageScaffold";
import { bridgeMode, previewServers } from "../../lib/bridge";

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
  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Activity"
        title="Jobs and sessions"
        description="Downloads, preparation, recovery work, and game sessions will be tracked here."
      />
      <EmptyState
        title="No active work"
        description="No durable jobs or sessions have been created. Launching and installation remain unavailable."
      />
    </div>
  );
}

export function AccountsPage() {
  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Accounts"
        title="Minecraft identity"
        description="Local instance management remains available without an account."
      />
      <div className="mx-auto max-w-[760px] px-8 py-8">
        <section className="rounded-control border border-app-separator/70 bg-app-surface p-6">
          <span className="inline-flex size-10 items-center justify-center rounded-control bg-app-raised text-app-accent">
            <UserRound size={20} aria-hidden="true" />
          </span>
          <h2 className="mt-4 mb-1 text-lg font-bold">No Minecraft account configured</h2>
          <p className="mt-0 mb-5 text-xs/[19px] text-app-secondary">
            Microsoft device authorization cannot be shipped until slate has its own approved app
            registration and keychain-backed token service.
          </p>
          <InlineNotice tone="warning" title="Sign-in unavailable">
            This is a capability blocker, not a simulated login. No credentials are requested or
            stored by this build.
          </InlineNotice>
        </section>
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
          description="Local profiles, preferences, storage setup, runtime preflight, and revision-guarded edits are implemented."
        />
        <InfoBlock
          icon={ShieldX}
          title="Unavailable by design"
          description="Account sign-in, content downloads, installation jobs, server joining, and Minecraft process launch are disabled."
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
