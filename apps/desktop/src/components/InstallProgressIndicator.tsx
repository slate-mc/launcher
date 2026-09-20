import type { InstallJob } from "../types/launcher";
import { cn } from "../lib/cn";
import {
  installJobMessage,
  installPhaseLabel,
} from "../lib/installJobPresentation";

export function InstallProgressIndicator({
  job,
  className,
}: {
  job: InstallJob;
  className?: string;
}) {
  const message = installJobMessage(job);
  const paused = job.state === "paused";
  const determinate =
    job.completedItems !== undefined &&
    job.totalItems !== undefined &&
    job.totalItems > 0;
  const percent = determinate
    ? Math.min(100, Math.max(0, (job.completedItems! / job.totalItems!) * 100))
    : undefined;

  return (
    <div className={cn("mt-3", className)}>
      <div className="mb-1.5 flex items-center justify-between gap-4 font-mono text-[10px] text-app-muted">
        <span>{installPhaseLabel(job.phase)}</span>
        <span className="tabular-nums">
          {determinate
            ? `${job.completedItems!.toLocaleString()} / ${job.totalItems!.toLocaleString()} files`
            : paused
              ? "Paused"
              : "Working"}
        </span>
      </div>
      <div
        className="relative h-1.5 overflow-hidden rounded-full bg-app-raised"
        role="progressbar"
        aria-label={message}
        aria-valuemin={determinate ? 0 : undefined}
        aria-valuemax={determinate ? job.totalItems : undefined}
        aria-valuenow={determinate ? job.completedItems : undefined}
        aria-valuetext={
          determinate
            ? paused
              ? `Paused at ${Math.round(percent!)}%`
              : `${Math.round(percent!)}%`
            : paused
              ? "Paused"
              : "In progress"
        }
      >
        {determinate ? (
          <span
            className="absolute inset-0 origin-left rounded-full bg-app-accent transition-transform duration-200 ease-out"
            style={{ transform: `scaleX(${percent! / 100})` }}
          />
        ) : paused ? null : (
          <span className="install-progress-indeterminate absolute inset-y-0 w-1/3 rounded-full bg-app-accent" />
        )}
      </div>
      <p
        className="mt-2 mb-0 truncate text-[11px] text-app-secondary"
        title={message}
        aria-live="polite"
      >
        {message}
      </p>
    </div>
  );
}
