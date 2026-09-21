import { Link, useRouterState } from "@tanstack/react-router";
import { HardDrive, ShieldCheck, SlidersHorizontal } from "lucide-react";

export function SettingsNavigation() {
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const items = [
    { to: "/settings/general" as const, label: "General", icon: SlidersHorizontal },
    { to: "/settings/storage" as const, label: "Storage", icon: HardDrive },
    { to: "/settings/privacy" as const, label: "Privacy", icon: ShieldCheck },
  ];
  return (
    <nav
      className="flex h-12 items-end gap-1 border-b border-app-separator/55 px-8"
      aria-label="Settings sections"
    >
      {items.map((item) => {
        const active = pathname === item.to;
        const Icon = item.icon;
        return (
          <Link
            key={item.to}
            to={item.to}
            className={`relative inline-flex h-11 items-center gap-2 px-3 text-xs font-bold transition-colors ${active ? "text-app-text" : "text-app-secondary hover:text-app-text"}`}
          >
            <Icon size={15} aria-hidden="true" />
            {item.label}
            {active ? (
              <span className="absolute inset-x-2 bottom-0 h-0.5 rounded-full bg-app-accent" />
            ) : null}
          </Link>
        );
      })}
    </nav>
  );
}
