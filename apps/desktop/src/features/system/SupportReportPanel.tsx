import { useMutation, useQuery } from "@tanstack/react-query";
import {
  Check,
  Download,
  FileArchive,
  LoaderCircle,
  Send,
  ShieldCheck,
} from "lucide-react";
import { useState } from "react";
import { InlineNotice } from "../../components/PageScaffold";
import {
  bridgeMode,
  exportSupportReport,
  getSupportReportPreview,
  submitSupportReport,
} from "../../lib/bridge";
import { getUserFacingError } from "../../lib/userFacingError";

export function SupportReportPanel() {
  const [options, setOptions] = useState({
    includeLauncherLogs: true,
    includeInstallActivity: true,
    includeInstanceSummary: true,
  });
  const previewQuery = useQuery({
    queryKey: ["support-report-preview"],
    queryFn: getSupportReportPreview,
  });
  const exportMutation = useMutation({ mutationFn: exportSupportReport });
  const submitMutation = useMutation({ mutationFn: submitSupportReport });
  const selected = Object.values(options).filter(Boolean).length;

  return (
    <section className="col-span-2 overflow-hidden rounded-control border border-app-separator/70 bg-app-surface">
      <div className="flex items-start gap-4 border-b border-app-separator/55 px-6 py-5">
        <span className="inline-flex size-10 shrink-0 items-center justify-center rounded-control bg-app-raised text-app-accent">
          <FileArchive size={20} aria-hidden="true" />
        </span>
        <span className="min-w-0 flex-1">
          <h2 className="m-0 text-[15px] font-bold">Create a support report</h2>
          <p className="mt-1 mb-0 max-w-[650px] text-xs/[19px] text-app-secondary">
            Choose what to include, then save a sanitized report you can review
            before sharing.
          </p>
        </span>
        <span className="inline-flex items-center gap-1.5 rounded-full bg-app-accent/10 px-2.5 py-1 text-[10px] font-bold text-app-accent">
          <ShieldCheck size={13} aria-hidden="true" /> Private by default
        </span>
      </div>

      <div className="grid grid-cols-[minmax(0,1fr)_280px] gap-6 p-6">
        <div className="grid content-start gap-2">
          <ReportOption
            checked={options.includeLauncherLogs}
            title="Launcher diagnostics and crash findings"
            description={
              previewQuery.isPending
                ? "Checking available diagnostics…"
                : `${formatCount(previewQuery.data?.diagnosticFileCount ?? 0, "launcher log file")} · ${formatBytes(previewQuery.data?.diagnosticBytes ?? 0)} before compression · recent crash findings without game log text`
            }
            onChange={(checked) =>
              setOptions((current) => ({
                ...current,
                includeLauncherLogs: checked,
              }))
            }
          />
          <ReportOption
            checked={options.includeInstallActivity}
            title="Installation activity"
            description="Recent states, steps, timestamps, and progress counts without instance names or identifiers."
            onChange={(checked) =>
              setOptions((current) => ({
                ...current,
                includeInstallActivity: checked,
              }))
            }
          />
          <ReportOption
            checked={options.includeInstanceSummary}
            title="Instance compatibility summary"
            description="Minecraft, loader, setup state, and mod counts without names or storage locations."
            onChange={(checked) =>
              setOptions((current) => ({
                ...current,
                includeInstanceSummary: checked,
              }))
            }
          />
        </div>

        <aside className="rounded-control border border-app-separator/65 bg-app-bg p-4">
          <p className="m-0 font-mono text-[10px] font-semibold tracking-[.08em] text-app-muted uppercase">
            Never included
          </p>
          <ul className="mt-3 mb-5 grid list-none gap-2 p-0 text-[11px] text-app-secondary">
            {[
              "Account credentials or player identity",
              "Worlds, screenshots, or Minecraft chat",
              "Absolute storage paths",
            ].map((item) => (
              <li key={item} className="flex items-start gap-2">
                <Check
                  className="mt-0.5 shrink-0 text-app-accent"
                  size={13}
                  aria-hidden="true"
                />
                {item}
              </li>
            ))}
          </ul>
          <div className="grid gap-2">
            <button
              type="button"
              className="inline-flex h-10 w-full items-center justify-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-45"
              disabled={
                bridgeMode !== "native" ||
                selected === 0 ||
                submitMutation.isPending ||
                exportMutation.isPending
              }
              onClick={() => submitMutation.mutate(options)}
            >
              {submitMutation.isPending ? (
                <LoaderCircle className="animate-spin" size={15} aria-hidden="true" />
              ) : (
                <Send size={15} aria-hidden="true" />
              )}
              {submitMutation.isPending ? "Sending report…" : "Send report"}
            </button>
            <button
              type="button"
              className="inline-flex h-10 w-full items-center justify-center gap-2 rounded-control border border-app-separator bg-app-raised px-4 text-xs font-bold text-app-text disabled:opacity-45"
              disabled={
                bridgeMode !== "native" ||
                selected === 0 ||
                submitMutation.isPending ||
                exportMutation.isPending
              }
              onClick={() => exportMutation.mutate(options)}
            >
              {exportMutation.isPending ? (
                <LoaderCircle className="animate-spin" size={15} aria-hidden="true" />
              ) : (
                <Download size={15} aria-hidden="true" />
              )}
              {exportMutation.isPending ? "Preparing copy…" : "Save a copy"}
            </button>
          </div>
          {bridgeMode !== "native" ? (
            <p className="mt-2 mb-0 text-center text-[10px] text-app-muted">
              Available in the desktop app
            </p>
          ) : null}
        </aside>
      </div>

      {submitMutation.isError || exportMutation.isError ? (
        <div className="px-6 pb-6">
          <InlineNotice tone="danger" title="Report could not be completed">
            {getUserFacingError(
              submitMutation.error ?? exportMutation.error,
              "Try again in a moment.",
            )}
          </InlineNotice>
        </div>
      ) : null}
      {submitMutation.data ? (
        <div className="px-6 pb-6">
          <InlineNotice tone="positive" title="Support report sent">
            Report ID {submitMutation.data.reportId} · {formatBytes(submitMutation.data.bytes)}
          </InlineNotice>
        </div>
      ) : null}
      {exportMutation.data ? (
        <div className="px-6 pb-6">
          <InlineNotice tone="positive" title="Support report saved">
            {exportMutation.data.fileName} · report{" "}
            {exportMutation.data.reportId} ·{" "}
            {formatBytes(exportMutation.data.bytes)}
          </InlineNotice>
        </div>
      ) : null}
    </section>
  );
}

function ReportOption({
  checked,
  title,
  description,
  onChange,
}: {
  checked: boolean;
  title: string;
  description: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label
      className={`flex cursor-pointer items-start gap-3 rounded-control border p-4 ${checked ? "border-app-accent/45 bg-app-accent/5" : "border-app-separator bg-app-bg"}`}
    >
      <input
        type="checkbox"
        className="mt-0.5 size-4 accent-app-accent"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span>
        <strong className="block text-xs font-bold">{title}</strong>
        <span className="mt-1 block text-[11px]/[17px] text-app-secondary">
          {description}
        </span>
      </span>
    </label>
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatCount(count: number, noun: string) {
  return `${count.toLocaleString()} ${noun}${count === 1 ? "" : "s"}`;
}
