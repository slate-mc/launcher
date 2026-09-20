import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Check,
  CircleAlert,
  Database,
  HardDrive,
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
  getPreferences,
  getPreflight,
  updatePreferences,
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
          <PreflightPanel query={preflightQuery} />
          <InlineNotice title="Privacy default">
            Optional telemetry stays off unless you explicitly enable it.
            Account and launch capabilities are independent from local library
            management.
          </InlineNotice>
        </aside>
      </div>
    </div>
  );
}

function PreferencesForm({ initial }: { initial: AppPreferences }) {
  const queryClient = useQueryClient();
  const [values, setValues] = useState({
    ...initial,
    telemetryEnabled: false,
  });
  const [saved, setSaved] = useState(false);
  const baseline = { ...initial, telemetryEnabled: false };
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
