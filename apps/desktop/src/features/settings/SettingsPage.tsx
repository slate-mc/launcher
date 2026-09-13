import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, CircleAlert, Database, HardDrive, Save, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { InlineNotice, PageHeader, StatusPill } from "../../components/PageScaffold";
import { getPreferences, getPreflight, updatePreferences } from "../../lib/bridge";
import type { AppPreferences } from "../../types/launcher";

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
        description="Local preferences are acknowledged by the Rust backend before the interface applies them."
      />
      <div className="grid grid-cols-[minmax(0,1fr)_360px] gap-6 px-8 py-7">
        {preferencesQuery.isPending ? (
          <div className="h-[430px] animate-pulse rounded-control bg-app-surface" />
        ) : preferencesQuery.isError ? (
          <InlineNotice tone="danger" title="Preferences unavailable">
            slate could not load the local settings database. No values were changed.
          </InlineNotice>
        ) : (
          <PreferencesForm initial={preferencesQuery.data} />
        )}
        <aside className="grid content-start gap-5">
          <PreflightPanel query={preflightQuery} />
          <InlineNotice title="Privacy default">
            Optional telemetry stays off unless you explicitly enable it. Account and launch
            capabilities are independent from local library management.
          </InlineNotice>
        </aside>
      </div>
    </div>
  );
}

function PreferencesForm({ initial }: { initial: AppPreferences }) {
  const queryClient = useQueryClient();
  const [values, setValues] = useState(initial);
  const [saved, setSaved] = useState(false);
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
        These values persist across restarts in native mode.
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
          description="The future download manager will use this as its global upper bound."
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
          title="Optional telemetry"
          description="Off by default. No telemetry transport is implemented in this build."
        >
          <label className="inline-flex items-center gap-2 text-xs font-bold text-app-text">
            <input
              type="checkbox"
              checked={values.telemetryEnabled}
              className="size-4 accent-[var(--slate-accent)]"
              onChange={(event) => update("telemetryEnabled", event.target.checked)}
            />
            Enabled
          </label>
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
          disabled={mutation.isPending}
          onClick={() => mutation.mutate(values)}
        >
          {saved ? <Check size={16} aria-hidden="true" /> : <Save size={16} aria-hidden="true" />}
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
    return <div className="h-[280px] animate-pulse rounded-control bg-app-surface" />;
  }
  if (query.isError) {
    return (
      <InlineNotice tone="danger" title="Preflight unavailable">
        The native runtime checks could not be completed.
      </InlineNotice>
    );
  }
  const preflight = query.data;
  return (
    <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
      <div className="flex items-center gap-2">
        <ShieldCheck size={18} className="text-app-accent" aria-hidden="true" />
        <h2 className="m-0 text-[15px] font-bold">System preflight</h2>
      </div>
      <p className="mt-1 mb-4 text-[11px]/[17px] text-app-secondary">
        Local checks only. A passing row does not imply the game can launch.
      </p>
      <div className="grid gap-2">
        <CheckRow
          icon={Database}
          label="Local database"
          ready={preflight.databaseReady}
        />
        <CheckRow
          icon={HardDrive}
          label="Managed storage"
          ready={preflight.storageReady}
        />
        <CheckRow
          icon={CircleAlert}
          label="Minecraft account"
          ready={preflight.accountConfigured}
          detail={preflight.accountConfigured ? undefined : "Not configured"}
        />
        <CheckRow
          icon={CircleAlert}
          label="Java runtime"
          ready={preflight.java.available}
          detail={preflight.java.version ?? preflight.java.unavailableReason}
        />
        <CheckRow
          icon={CircleAlert}
          label="Launch feature"
          ready={preflight.launchImplemented}
          detail="Intentionally unavailable"
        />
      </div>
    </section>
  );
}

function CheckRow({
  icon: Icon,
  label,
  ready,
  detail,
}: {
  icon: typeof Database;
  label: string;
  ready: boolean;
  detail?: string;
}) {
  return (
    <div className="flex min-h-11 items-center gap-3 border-b border-app-separator/45 py-2 last:border-0">
      <Icon size={16} className="text-app-muted" aria-hidden="true" />
      <span className="min-w-0 flex-1">
        <strong className="block text-xs font-bold text-app-text">{label}</strong>
        {detail ? (
          <small className="block overflow-hidden text-[10px] text-app-muted text-ellipsis whitespace-nowrap">
            {detail}
          </small>
        ) : null}
      </span>
      <StatusPill tone={ready ? "positive" : "warning"}>
        {ready ? "Ready" : "Pending"}
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
