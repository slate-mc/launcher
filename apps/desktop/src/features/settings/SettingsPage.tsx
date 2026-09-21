import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Check,
  CircleAlert,
  Database,
  Download,
  HardDrive,
  RefreshCw,
  Save,
  ShieldCheck,
} from "lucide-react";
import { useState } from "react";
import {
  InlineNotice,
  PageHeader,
  StatusPill,
} from "../../components/PageScaffold";
import {
  bridgeMode,
  checkForLauncherUpdate,
  getBootstrap,
  getPreferences,
  getPreflight,
  installLauncherUpdate,
  updatePreferences,
} from "../../lib/bridge";
import type {
  LauncherUpdateCheck,
  LauncherUpdateProgress,
} from "../../lib/bridge";
import type { AppPreferences } from "../../types/launcher";
import { SettingsNavigation } from "./SettingsNavigation";

export function SettingsPage() {
  const preferencesQuery = useQuery({
    queryKey: ["preferences"],
    queryFn: getPreferences,
  });
  const preflightQuery = useQuery({
    queryKey: ["preflight"],
    queryFn: getPreflight,
  });
  const bootstrapQuery = useQuery({
    queryKey: ["bootstrap"],
    queryFn: getBootstrap,
    refetchInterval: 5 * 60_000,
  });

  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Settings"
        title="Launcher preferences"
        description="Choose how slate looks, behaves, and handles downloads."
      />
      <SettingsNavigation />
      <div className="grid grid-cols-[minmax(0,1fr)_360px] gap-6 px-8 py-7">
        {preferencesQuery.isPending ? (
          <div className="h-[430px] animate-pulse rounded-control bg-app-surface" />
        ) : preferencesQuery.isError ? (
          <InlineNotice tone="danger" title="Preferences unavailable">
            slate could not load your preferences. Nothing was changed.
          </InlineNotice>
        ) : (
          <PreferencesForm initial={preferencesQuery.data} />
        )}
        <aside className="grid content-start gap-5">
          <UpdatePanel bootstrap={bootstrapQuery.data} />
          <PreflightPanel query={preflightQuery} />
        </aside>
      </div>
    </div>
  );
}

function UpdatePanel({
  bootstrap,
}: {
  bootstrap: Awaited<ReturnType<typeof getBootstrap>> | undefined;
}) {
  const [available, setAvailable] = useState<LauncherUpdateCheck>();
  const [checked, setChecked] = useState(false);
  const [progress, setProgress] = useState<LauncherUpdateProgress>();
  const capability = bootstrap?.capabilities.find(
    (entry) => entry.id === "launcher.updates",
  );
  const enabled = bridgeMode === "native" && capability?.available === true;
  const checkMutation = useMutation({
    mutationFn: checkForLauncherUpdate,
    onSuccess: (update) => {
      setChecked(true);
      setAvailable(update);
      setProgress(undefined);
    },
  });
  const installMutation = useMutation({
    mutationFn: () => installLauncherUpdate(setProgress),
  });
  const percent =
    progress?.totalBytes && progress.totalBytes > 0
      ? Math.min(
          100,
          Math.round((progress.downloadedBytes / progress.totalBytes) * 100),
        )
      : undefined;

  return (
    <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
      <div className="flex items-center gap-2">
        <Download size={18} className="text-app-accent" aria-hidden="true" />
        <h2 className="m-0 text-[15px] font-bold">Launcher updates</h2>
      </div>
      <p className="mt-1 mb-4 text-[11px]/[17px] text-app-secondary">
        {enabled
          ? "Check for a signed slate release and install it when you are ready."
          : "Signed release builds can check for launcher updates here."}
      </p>

      {available ? (
        <div className="border-y border-app-separator/55 py-3">
          <strong className="block text-xs">Version {available.version}</strong>
          {available.notes ? (
            <p className="mt-1 mb-0 line-clamp-3 text-[10px]/[16px] text-app-secondary">
              {available.notes}
            </p>
          ) : null}
        </div>
      ) : checked ? (
        <p className="m-0 border-y border-app-separator/55 py-3 text-[11px] text-app-secondary">
          slate is up to date.
        </p>
      ) : null}

      {installMutation.isPending ? (
        <div className="mt-4" aria-live="polite">
          <div className="mb-1.5 flex justify-between text-[10px] text-app-secondary">
            <span>Downloading update</span>
            <span>{percent === undefined ? "Starting" : `${percent}%`}</span>
          </div>
          <div className="h-1.5 overflow-hidden rounded-full bg-app-raised">
            <span
              className="block h-full bg-app-accent transition-[width] duration-150"
              style={{ width: `${percent ?? 8}%` }}
            />
          </div>
        </div>
      ) : null}

      {checkMutation.isError || installMutation.isError ? (
        <p
          className="mt-3 mb-0 text-[10px]/[16px] text-app-danger"
          role="alert"
        >
          {installMutation.isError
            ? "The update was not installed. Restart slate and try again."
            : "slate could not check for updates. Try again later."}
        </p>
      ) : null}

      <button
        type="button"
        className="mt-4 inline-flex h-9 w-full items-center justify-center gap-2 rounded-control border border-app-separator bg-app-bg px-3 text-[11px] font-bold text-app-text hover:border-app-accent/45 disabled:opacity-45"
        disabled={
          !enabled || checkMutation.isPending || installMutation.isPending
        }
        onClick={() =>
          available ? installMutation.mutate() : checkMutation.mutate()
        }
      >
        {available ? (
          <Download size={14} aria-hidden="true" />
        ) : (
          <RefreshCw
            size={14}
            className={checkMutation.isPending ? "animate-spin" : undefined}
            aria-hidden="true"
          />
        )}
        {installMutation.isPending
          ? "Installing…"
          : available
            ? `Install ${available.version}`
            : checkMutation.isPending
              ? "Checking…"
              : "Check for updates"}
      </button>
    </section>
  );
}

function PreferencesForm({ initial }: { initial: AppPreferences }) {
  const queryClient = useQueryClient();
  const [values, setValues] = useState(initial);
  const [saved, setSaved] = useState(false);
  const baseline = initial;
  const dirty = !samePreferences(values, baseline);
  const mutation = useMutation({
    mutationFn: updatePreferences,
    onSuccess: (preferences) => {
      queryClient.setQueryData(["preferences"], preferences);
      setValues(preferences);
      setSaved(true);
    },
  });

  const update = <Key extends keyof AppPreferences>(
    key: Key,
    value: AppPreferences[Key],
  ) => {
    setSaved(false);
    setValues((current) => ({ ...current, [key]: value }));
  };

  return (
    <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
      <h2 className="m-0 text-[15px] font-bold">Appearance and behavior</h2>
      <p className="mt-1 mb-6 text-xs text-app-secondary">
        Saved on this device and applied across restarts.
      </p>
      <div className="grid gap-6">
        <SettingRow
          title="Theme"
          description="Dark is the initial slate experience; System follows the operating system."
        >
          <select
            value={values.theme}
            className={selectClass}
            onChange={(event) =>
              update("theme", event.target.value as AppPreferences["theme"])
            }
          >
            <option value="dark">Dark</option>
            <option value="light">Light</option>
            <option value="system">System</option>
          </select>
        </SettingRow>
        <SettingRow
          title="Reduced motion"
          description="System respects your OS preference; On minimizes interface motion."
        >
          <select
            value={values.reduceMotion}
            className={selectClass}
            onChange={(event) =>
              update(
                "reduceMotion",
                event.target.value as AppPreferences["reduceMotion"],
              )
            }
          >
            <option value="system">System</option>
            <option value="on">On</option>
            <option value="off">Off</option>
          </select>
        </SettingRow>
        <SettingRow
          title="Concurrent downloads"
          description="Maximum number of verified game, loader, modpack, and mod files downloaded in parallel."
        >
          <select
            value={values.downloadConcurrency}
            className={selectClass}
            onChange={(event) =>
              update("downloadConcurrency", Number(event.target.value))
            }
          >
            {[1, 2, 3, 4, 5, 6, 7, 8].map((value) => (
              <option value={value} key={value}>
                {value} {value === 1 ? "file" : "files"}
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow
          title="Download speed"
          description="Limit slate’s combined download speed when installing games, modpacks, and mods."
        >
          <select
            value={values.downloadBandwidthLimitMib}
            className={selectClass}
            onChange={(event) =>
              update("downloadBandwidthLimitMib", Number(event.target.value))
            }
          >
            <option value={0}>No limit</option>
            {[5, 10, 25, 50, 100, 250, 500].map((value) => (
              <option value={value} key={value}>
                {value} MB/s
              </option>
            ))}
          </select>
        </SettingRow>
      </div>

      <div className="mt-7 flex items-center justify-between border-t border-app-separator/55 pt-5">
        <p
          className={`m-0 text-xs ${mutation.isError ? "text-app-danger" : "text-app-secondary"}`}
          role={mutation.isError ? "alert" : "status"}
        >
          {mutation.isError
            ? "Preferences could not be saved."
            : saved
              ? "Preferences saved."
              : "Changes are not saved yet."}
        </p>
        <button
          type="button"
          className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-50"
          disabled={mutation.isPending || !dirty}
          onClick={() => mutation.mutate(values)}
        >
          {saved ? (
            <Check size={16} aria-hidden="true" />
          ) : (
            <Save size={16} aria-hidden="true" />
          )}
          {mutation.isPending ? "Saving…" : "Save preferences"}
        </button>
      </div>
    </section>
  );
}

function PreflightPanel({
  query,
}: {
  query: ReturnType<typeof useQuery<Awaited<ReturnType<typeof getPreflight>>>>;
}) {
  if (query.isPending) {
    return (
      <div className="h-[280px] animate-pulse rounded-control bg-app-surface" />
    );
  }
  if (query.isError) {
    return (
      <InlineNotice tone="danger" title="System check unavailable">
        slate could not check this device. Try again after restarting the
        launcher.
      </InlineNotice>
    );
  }
  const preflight = query.data;
  return (
    <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
      <div className="flex items-center gap-2">
        <ShieldCheck size={18} className="text-app-accent" aria-hidden="true" />
        <h2 className="m-0 text-[15px] font-bold">System check</h2>
      </div>
      <p className="mt-1 mb-4 text-[11px]/[17px] text-app-secondary">
        Check the essentials slate needs before you install or play.
      </p>
      <div className="grid gap-2">
        <CheckRow
          icon={Database}
          label="Launcher data"
          state={preflight.databaseReady ? "ready" : "attention"}
        />
        <CheckRow
          icon={HardDrive}
          label="Game storage"
          state={preflight.storageReady ? "ready" : "attention"}
        />
        <CheckRow
          icon={CircleAlert}
          label="Minecraft account"
          state={preflight.accountConfigured ? "ready" : "attention"}
          detail={preflight.accountConfigured ? undefined : "Not configured"}
        />
        <CheckRow
          icon={CircleAlert}
          label="System Java (optional)"
          state={preflight.java.available ? "ready" : "optional"}
          detail={
            preflight.java.version ??
            "Not detected. slate can download the Java version each instance needs."
          }
        />
        <CheckRow
          icon={CircleAlert}
          label="Game launch"
          state={preflight.launchImplemented ? "ready" : "attention"}
          detail={
            preflight.launchImplemented
              ? "Ready to launch installed instances"
              : "Not available"
          }
        />
      </div>
    </section>
  );
}

function CheckRow({
  icon: Icon,
  label,
  state,
  detail,
}: {
  icon: typeof Database;
  label: string;
  state: "ready" | "attention" | "optional";
  detail?: string;
}) {
  return (
    <div className="flex min-h-11 items-center gap-3 border-b border-app-separator/45 py-2 last:border-0">
      <Icon size={16} className="text-app-muted" aria-hidden="true" />
      <span className="min-w-0 flex-1">
        <strong className="block text-xs font-bold text-app-text">
          {label}
        </strong>
        {detail ? (
          <small className="block overflow-hidden text-[10px] text-app-muted text-ellipsis whitespace-nowrap">
            {detail}
          </small>
        ) : null}
      </span>
      <StatusPill
        tone={
          state === "ready"
            ? "positive"
            : state === "optional"
              ? "neutral"
              : "warning"
        }
      >
        {state === "ready"
          ? "Ready"
          : state === "optional"
            ? "Optional"
            : "Attention"}
      </StatusPill>
    </div>
  );
}

function SettingRow({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_190px] items-center gap-8 border-b border-app-separator/45 pb-5 last:border-0 last:pb-0">
      <div>
        <h3 className="m-0 text-xs font-bold text-app-text">{title}</h3>
        <p className="mt-1 mb-0 text-[11px]/[17px] text-app-secondary">
          {description}
        </p>
      </div>
      <div className="justify-self-stretch">{children}</div>
    </div>
  );
}

const selectClass =
  "h-9 w-full rounded-control border border-app-separator bg-app-bg px-3 text-xs font-semibold text-app-text focus:border-app-accent focus:outline-none";

function samePreferences(left: AppPreferences, right: AppPreferences) {
  return (
    left.theme === right.theme &&
    left.downloadConcurrency === right.downloadConcurrency &&
    left.downloadBandwidthLimitMib === right.downloadBandwidthLimitMib &&
    left.telemetryEnabled === right.telemetryEnabled &&
    left.reduceMotion === right.reduceMotion &&
    left.trashRetentionDays === right.trashRetentionDays
  );
}
