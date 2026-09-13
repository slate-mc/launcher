import { CircleAlert, Inbox } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "../lib/cn";

export function PageHeader({
  eyebrow,
  title,
  description,
  actions,
}: {
  eyebrow: string;
  title: string;
  description: string;
  actions?: ReactNode;
}) {
  return (
    <header className="flex items-end justify-between gap-8 border-b border-app-separator/55 px-8 py-7">
      <div className="max-w-[720px]">
        <p className="mb-2 text-[11px]/[15px] font-bold tracking-[.1em] text-app-muted uppercase">
          {eyebrow}
        </p>
        <h1 className="m-0 text-[30px]/[36px] font-bold tracking-[-.035em] text-app-text">
          {title}
        </h1>
        <p className="mt-2 mb-0 text-[13px]/[20px] text-app-secondary">
          {description}
        </p>
      </div>
      {actions ? <div className="shrink-0">{actions}</div> : null}
    </header>
  );
}

export function StatusPill({
  tone,
  children,
}: {
  tone: "positive" | "neutral" | "warning" | "danger";
  children: ReactNode;
}) {
  return (
    <span
      className={cn(
        "inline-flex min-h-6 items-center gap-1.5 rounded-full border px-2.5 text-[10px] font-bold tracking-[.02em]",
        tone === "positive" &&
          "border-app-accent/35 bg-app-accent/10 text-app-accent",
        tone === "neutral" &&
          "border-app-separator bg-app-raised text-app-secondary",
        tone === "warning" &&
          "border-app-warning/35 bg-app-warning/10 text-app-warning",
        tone === "danger" &&
          "border-app-danger/35 bg-app-danger/10 text-app-danger",
      )}
    >
      <span
        className={cn(
          "size-1.5 rounded-full bg-current",
          tone === "neutral" && "bg-app-muted",
        )}
        aria-hidden="true"
      />
      {children}
    </span>
  );
}

export function EmptyState({
  title,
  description,
  action,
  error = false,
}: {
  title: string;
  description: string;
  action?: ReactNode;
  error?: boolean;
}) {
  const Icon = error ? CircleAlert : Inbox;
  return (
    <section
      className="flex min-h-[360px] flex-col items-center justify-center px-8 py-16 text-center"
      role={error ? "alert" : undefined}
    >
      <span className="mb-5 inline-flex size-12 items-center justify-center rounded-control border border-app-separator bg-app-surface text-app-secondary">
        <Icon size={22} aria-hidden="true" />
      </span>
      <h2 className="m-0 text-xl/[28px] font-bold tracking-[-.025em] text-app-text">
        {title}
      </h2>
      <p className="mt-2 mb-0 max-w-[520px] text-[13px]/[20px] text-app-secondary">
        {description}
      </p>
      {action ? <div className="mt-6">{action}</div> : null}
    </section>
  );
}

export function InlineNotice({
  tone = "neutral",
  title,
  children,
}: {
  tone?: "neutral" | "positive" | "success" | "warning" | "danger";
  title: string;
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        "rounded-control border border-app-separator bg-app-surface px-4 py-3",
        tone === "warning" && "border-app-warning/40 bg-app-warning/5",
        tone === "danger" && "border-app-danger/40 bg-app-danger/5",
        (tone === "positive" || tone === "success") &&
          "border-app-accent/40 bg-app-accent/5",
      )}
    >
      <strong className="block text-xs font-bold text-app-text">{title}</strong>
      <div className="mt-1 text-xs/[18px] text-app-secondary">{children}</div>
    </div>
  );
}
