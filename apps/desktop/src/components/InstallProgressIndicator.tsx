import type { InstallJob } from "../types/launcher";
import { cn } from "../lib/cn";

export function InstallProgressIndicator({
  job,
  className,
}: {
  job: InstallJob;
  className?: string;
}) {
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
        <span>{phaseLabel(job.phase)}</span>
        <span className="tabular-nums">
          {determinate
            ? `${job.completedItems!.toLocaleString()} / ${job.totalItems!.toLocaleString()} files`
            : "Working"}
        </span>
      </div>
      <div
        className="relative h-1.5 overflow-hidden rounded-full bg-app-raised"
        role="progressbar"
        aria-label={job.message}
        aria-valuemin={determinate ? 0 : undefined}
        aria-valuemax={determinate ? job.totalItems : undefined}
        aria-valuenow={determinate ? job.completedItems : undefined}
        aria-valuetext={
          determinate ? `${Math.round(percent!)}%` : "In progress"
        }
      >
        {determinate ? (
          <span
            className="absolute inset-0 origin-left rounded-full bg-app-accent transition-transform duration-200 ease-out"
            style={{ transform: `scaleX(${percent! / 100})` }}
          />
        ) : (
          <span className="install-progress-indeterminate absolute inset-y-0 w-1/3 rounded-full bg-app-accent" />
        )}
      </div>
    </div>
  );
}

function phaseLabel(phase: string) {
  const labels: Record<string, string> = {
    metadata: "Metadata",
    "base-game": "Base game",
    assets: "Game assets",
    runtime: "Java runtime",
    loader: "Mod loader",
    "launch-files": "Launch files",
    natives: "Native libraries",
    content: "Instance content",
    verification: "Verification",
    commit: "Finalizing",
  };
  return labels[phase] ?? "Installation";
}
