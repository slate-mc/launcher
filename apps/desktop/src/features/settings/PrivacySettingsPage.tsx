import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { BarChart3, Check, LockKeyhole, Save, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { InlineNotice, PageHeader } from "../../components/PageScaffold";
import { getPreferences, updatePreferences } from "../../lib/bridge";
import { SettingsNavigation } from "./SettingsNavigation";

export function PrivacySettingsPage() {
  const queryClient = useQueryClient();
  const preferencesQuery = useQuery({
    queryKey: ["preferences"],
    queryFn: getPreferences,
  });
  const [draftEnabled, setDraftEnabled] = useState<boolean>();
  const [saved, setSaved] = useState(false);
  const enabled =
    draftEnabled ?? preferencesQuery.data?.telemetryEnabled ?? false;
  const mutation = useMutation({
    mutationFn: async () => {
      const preferences = preferencesQuery.data;
      if (!preferences) throw new Error("Preferences are unavailable.");
      return updatePreferences({
        ...preferences,
        telemetryEnabled: enabled,
      });
    },
    onSuccess: (preferences) => {
      queryClient.setQueryData(["preferences"], preferences);
      setDraftEnabled(undefined);
      setSaved(true);
    },
  });
  const dirty =
    preferencesQuery.data !== undefined &&
    enabled !== preferencesQuery.data.telemetryEnabled;

  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="Settings"
        title="Privacy"
        description="Choose whether slate can send anonymous usage information."
      />
      <SettingsNavigation />
      <div className="grid grid-cols-[minmax(0,1fr)_360px] gap-6 px-8 py-7">
        <section className="rounded-control border border-app-separator/70 bg-app-surface p-6">
          <div className="flex items-start justify-between gap-8">
            <div>
              <div className="flex items-center gap-2">
                <BarChart3 size={18} className="text-app-accent" aria-hidden="true" />
                <h2 className="m-0 text-base font-bold">Help improve slate</h2>
              </div>
              <p className="mt-2 mb-0 max-w-2xl text-xs/5 text-app-secondary">
                Share anonymous events about setup, installs, content changes,
                and game launches. This is off until you turn it on.
              </p>
            </div>
            <label className="inline-flex shrink-0 cursor-pointer items-center gap-3 text-xs font-bold">
              <input
                type="checkbox"
                className="h-4 w-4 accent-[var(--color-app-accent)]"
                checked={enabled}
                disabled={preferencesQuery.isPending || preferencesQuery.isError}
                onChange={(event) => {
                  setDraftEnabled(event.target.checked);
                  setSaved(false);
                }}
              />
              Share usage
            </label>
          </div>

          <div className="mt-6 grid grid-cols-2 gap-4 border-t border-app-separator/55 pt-6">
            <PrivacyCard
              icon={Check}
              title="What is shared"
              items={[
                "Launcher version and operating system",
                "Whether a setup, install, or launch succeeded",
                "Loader and provider type when relevant",
                "A random identifier unique to this installation",
              ]}
            />
            <PrivacyCard
              icon={LockKeyhole}
              title="What stays private"
              items={[
                "Microsoft and Minecraft account details",
                "Worlds, servers, messages, and screenshots",
                "File names, folder paths, and game logs",
                "Modpack names and personal instance settings",
              ]}
            />
          </div>

          {mutation.isError ? (
            <InlineNotice tone="danger" title="Privacy choice not saved">
              Restart slate and try again. Your previous choice is still in use.
            </InlineNotice>
          ) : null}

          <div className="mt-7 flex items-center justify-between border-t border-app-separator/55 pt-5">
            <p className="m-0 text-xs text-app-secondary" role="status">
              {saved
                ? "Privacy choice saved."
                : dirty
                  ? "Your new choice is not saved yet."
                  : enabled
                    ? "Anonymous usage sharing is on."
                    : "Anonymous usage sharing is off."}
            </p>
            <button
              type="button"
              className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-50"
              disabled={!dirty || mutation.isPending}
              onClick={() => mutation.mutate()}
            >
              {saved ? <Check size={16} /> : <Save size={16} />}
              {mutation.isPending ? "Saving…" : "Save privacy choice"}
            </button>
          </div>
        </section>

        <aside className="grid content-start gap-5">
          <InlineNotice title="Private by default">
            No anonymous usage events are sent unless you choose to share them.
          </InlineNotice>
          <section className="rounded-control border border-app-separator/70 bg-app-surface p-5">
            <div className="flex items-center gap-2">
              <ShieldCheck size={18} className="text-app-accent" aria-hidden="true" />
              <h2 className="m-0 text-[15px] font-bold">Your choice applies immediately</h2>
            </div>
            <p className="mt-2 mb-0 text-[11px]/[18px] text-app-secondary">
              Turning sharing off also removes anonymous events waiting to be sent.
            </p>
          </section>
        </aside>
      </div>
    </div>
  );
}

function PrivacyCard({
  icon: Icon,
  title,
  items,
}: {
  icon: typeof Check;
  title: string;
  items: string[];
}) {
  return (
    <div className="rounded-control bg-app-bg/55 p-4">
      <div className="flex items-center gap-2">
        <Icon size={15} className="text-app-accent" aria-hidden="true" />
        <h3 className="m-0 text-xs font-bold">{title}</h3>
      </div>
      <ul className="mt-3 mb-0 grid gap-2 pl-4 text-[11px]/[17px] text-app-secondary">
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  );
}
