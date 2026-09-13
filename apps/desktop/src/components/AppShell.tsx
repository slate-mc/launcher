import { useQuery } from "@tanstack/react-query";
import { Link, Outlet } from "@tanstack/react-router";
import {
  Activity,
  CircleHelp,
  Compass,
  Download,
  Home,
  Library,
  Server,
  Settings,
  UserRound,
} from "lucide-react";
import { useEffect, type ComponentType } from "react";
import { bridgeMode, getPreferences, getPreflight } from "../lib/bridge";

type NavigationItem = {
  label: string;
  to: "/home" | "/library" | "/discover" | "/servers";
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
};

const navigation: NavigationItem[] = [
  { label: "Home", to: "/home", icon: Home },
  { label: "Library", to: "/library", icon: Library },
  { label: "Discover", to: "/discover", icon: Compass },
  { label: "Servers", to: "/servers", icon: Server },
];

const quietAction =
  "inline-flex size-9 items-center justify-center rounded-compact border-0 bg-transparent p-0 text-app-secondary no-underline transition-colors duration-[120ms] hover:bg-app-hover hover:text-app-text";

export function AppShell() {
  const preferencesQuery = useQuery({
    queryKey: ["preferences"],
    queryFn: getPreferences,
  });
  const preflightQuery = useQuery({
    queryKey: ["preflight"],
    queryFn: getPreflight,
    staleTime: 60_000,
  });

  useEffect(() => {
    const theme = preferencesQuery.data?.theme ?? "dark";
    const resolved =
      theme === "system"
        ? window.matchMedia("(prefers-color-scheme: light)").matches
          ? "light"
          : "dark"
        : theme;
    document.documentElement.dataset.theme = resolved;
  }, [preferencesQuery.data?.theme]);

  return (
    <div className="flex h-full min-h-[640px] min-w-[960px] flex-col bg-app-bg text-app-text">
      <header className="relative z-20 grid h-[72px] shrink-0 grid-cols-[185px_minmax(430px,1fr)_auto] items-stretch gap-4 border-b border-app-separator/50 bg-app-bg/98 px-6 max-[1180px]:grid-cols-[150px_minmax(380px,1fr)_auto] max-[1180px]:px-[18px]">
        <Link className="flex w-fit items-center" to="/home" aria-label="slate home">
          <img
            className="block w-[126px] max-[1180px]:w-28"
            src="/brand/slate-lockup-paper.svg"
            alt="slate"
          />
        </Link>

        <nav className="flex min-w-0 justify-center" aria-label="Primary">
          {navigation.map((item) => {
            const Icon = item.icon;
            return (
              <Link
                key={item.to}
                to={item.to}
                className="relative inline-flex h-full min-w-[104px] items-center justify-center gap-[9px] px-4 text-[13px] font-semibold text-app-secondary no-underline transition-colors duration-[120ms] after:absolute after:right-[18px] after:bottom-0 after:left-[18px] after:h-0.5 after:scale-x-[.65] after:bg-app-accent after:opacity-0 after:transition-all after:duration-[120ms] hover:bg-app-hover/30 hover:text-app-text max-[1180px]:min-w-[88px] max-[1180px]:px-2.5"
                activeProps={{
                  className: "text-app-text after:scale-x-100 after:opacity-100",
                }}
              >
                <Icon size={19} strokeWidth={1.9} aria-hidden="true" />
                <span>{item.label}</span>
              </Link>
            );
          })}
        </nav>

        <div className="flex items-center gap-1">
          <Link className={quietAction} to="/activity" aria-label="Downloads" title="Downloads">
            <Download size={19} aria-hidden="true" />
          </Link>
          <Link className={quietAction} to="/activity" aria-label="Activity" title="Activity">
            <Activity size={19} aria-hidden="true" />
          </Link>
          <Link
            className={quietAction}
            to="/settings/general"
            aria-label="Settings"
            title="Settings"
          >
            <Settings size={19} aria-hidden="true" />
          </Link>
          <Link
            className="ml-1.5 flex h-[42px] items-center gap-2.5 border-0 border-l border-app-separator bg-transparent py-0 pr-2.5 pl-1.5 text-app-text no-underline hover:bg-app-hover/35"
            to="/accounts"
            aria-label="Account"
            title="Minecraft accounts"
          >
            <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-compact bg-app-accent text-[13px] font-extrabold text-app-on-accent">
              <UserRound size={17} aria-hidden="true" />
            </span>
            <span className="max-w-[110px] overflow-hidden text-[13px] font-semibold text-ellipsis whitespace-nowrap max-[1180px]:hidden">
              Local player
            </span>
          </Link>
        </div>
      </header>

      {bridgeMode === "preview" ? (
        <div
          className="relative z-10 h-7 shrink-0 bg-app-accent px-7 py-[5px] text-center font-mono text-[11px]/[18px] font-medium text-app-on-accent"
          role="status"
        >
          Development preview · sample data · no game actions are executed
        </div>
      ) : null}

      <main className="min-h-0 flex-1 overflow-y-auto [scrollbar-color:var(--slate-separator)_transparent]">
        <Outlet />
      </main>

      <footer className="relative z-20 flex min-h-10 shrink-0 items-center gap-[18px] border-t border-app-separator/65 bg-app-sidebar px-6 text-[11px] text-app-secondary">
        <span className="inline-flex items-center gap-2 font-semibold text-app-text">
          <span
            className={`size-2 shrink-0 rounded-full ${
              preflightQuery.data?.java.available ? "bg-app-accent" : "bg-app-warning"
            }`}
          />
          {preflightQuery.isPending
            ? "Checking Java"
            : preflightQuery.data?.java.available
              ? "Java detected"
              : "Java needs attention"}
        </span>
        <span className="h-[18px] w-px bg-app-separator" />
        <span>No active jobs</span>
        <span className="ml-auto">
          {bridgeMode === "native" ? "Native data" : "Preview environment"}
        </span>
        <Link
          className="inline-flex h-9 items-center justify-center gap-1.5 rounded-compact px-2 text-[11px] text-app-secondary no-underline hover:bg-app-hover hover:text-app-text"
          to="/help"
        >
          <CircleHelp size={16} aria-hidden="true" />
          Help
        </Link>
      </footer>
    </div>
  );
}
