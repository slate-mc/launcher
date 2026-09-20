import type { ReactNode } from "react";

export function SettingsPanel({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-5">
      <header>
        <h2 className="m-0 text-[16px] font-bold">{title}</h2>
        <p className="mt-1 mb-0 text-xs text-app-secondary">{description}</p>
      </header>
      {children}
    </div>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block text-xs font-bold text-app-text">
      {label}
      {children}
    </label>
  );
}
