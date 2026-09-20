import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Check,
  ChevronDown,
  Cpu,
  FolderArchive,
  Image as ImageIcon,
  Monitor,
  RotateCcw,
  Save,
  Settings2,
  UserRound,
} from "lucide-react";
import { useState } from "react";
import { ComboBox } from "../../components/ComboBox";
import {
  getLoaderVersionCatalog,
  getMinecraftVersionCatalog,
  installInstance,
  listAccounts,
  renameInstance,
  resetInstanceArtwork,
  selectInstanceArtwork,
  selectInstanceJava,
  updateInstanceConfiguration,
  updateInstanceSettings,
} from "../../lib/bridge";
import { loaderLabel } from "../../lib/format";
import type { LauncherInstance, LoaderKind } from "../../types/launcher";
import {
  contentErrorMessage,
} from "./instanceContentFormat";
import { InstanceLifecycleActions } from "./InstanceLifecycleActions";
import { InstanceGameOptionsEditor } from "./InstanceGameOptionsEditor";
import { Field, SettingsPanel } from "./InstanceSettingsUi";
import { inputClass, secondaryButtonClass } from "./instanceSettingsStyles";

export function InstanceSettings({ instance }: { instance: LauncherInstance }) {
  const queryClient = useQueryClient();
  const [section, setSection] = useState<
    "profile" | "launch" | "runtime" | "game" | "safety"
  >("profile");
  const [draft, setDraft] = useState(() => instanceSettingsDraft(instance));
  const [minecraftVersion, setMinecraftVersion] = useState(
    instance.minecraftVersion,
  );
  const [loaderKind, setLoaderKind] = useState<LoaderKind>(instance.loaderKind);
  const [loaderSelection, setLoaderSelection] = useState({
    catalogKey: `${instance.minecraftVersion}|${instance.loaderKind}`,
    value: instance.loaderVersion ?? "",
  });
  const [message, setMessage] = useState<string>();
  const accountsQuery = useQuery({
    queryKey: ["accounts"],
    queryFn: listAccounts,
  });

  const versionsQuery = useQuery({
    queryKey: ["minecraft-version-catalog"],
    queryFn: getMinecraftVersionCatalog,
    staleTime: 15 * 60_000,
  });
  const loaderQuery = useQuery({
    queryKey: ["loader-version-catalog", minecraftVersion, loaderKind],
    queryFn: () => getLoaderVersionCatalog({ minecraftVersion, loaderKind }),
    enabled: Boolean(minecraftVersion) && loaderKind !== "vanilla",
    staleTime: 15 * 60_000,
  });
  const loaderCatalogKey = `${minecraftVersion}|${loaderKind}`;
  const selectedLoaderVersion =
    loaderKind === "vanilla"
      ? undefined
      : loaderSelection.catalogKey === loaderCatalogKey &&
          loaderQuery.data?.versions.includes(loaderSelection.value)
        ? loaderSelection.value
        : (loaderQuery.data?.recommendedVersion ?? loaderSelection.value);

  const runtimeMutation = useMutation({
    mutationFn: updateInstanceConfiguration,
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      const runtimeChanged =
        updated.minecraftVersion !== instance.minecraftVersion ||
        updated.loaderKind !== instance.loaderKind ||
        updated.loaderVersion !== instance.loaderVersion;
      setMessage(
        runtimeChanged
          ? "Runtime changed. Reinstall the instance before launching."
          : "Runtime configuration saved. Existing game files were kept.",
      );
    },
  });

  const settingsMutation = useMutation({
    mutationFn: async () => {
      let current = instance;
      if (draft.name.trim() !== instance.name) {
        current = await renameInstance({
          id: instance.id,
          name: draft.name,
          expectedRevision: current.revision,
        });
      }
      return updateInstanceSettings({
        id: instance.id,
        description: draft.description,
        notes: draft.notes,
        groupName: cleanOptional(draft.groupName),
        tags: draft.tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
        preferredAccountId: cleanOptional(draft.preferredAccountId),
        bannerPositionX: draft.bannerPositionX,
        bannerPositionY: draft.bannerPositionY,
        windowMode: draft.windowMode,
        resolutionWidth:
          draft.windowMode === "windowed"
            ? numberOrUndefined(draft.resolutionWidth)
            : undefined,
        resolutionHeight:
          draft.windowMode === "windowed"
            ? numberOrUndefined(draft.resolutionHeight)
            : undefined,
        launcherBehavior: draft.launcherBehavior,
        gameLanguage: draft.gameLanguage,
        quickPlayServer: cleanOptional(draft.quickPlayServer),
        processPriority: draft.processPriority,
        cpuAffinity: parseCpuAffinity(draft.cpuAffinity),
        memoryMode: draft.memoryMode,
        initialMemoryMb: draft.initialMemoryMb,
        maximumMemoryMb: draft.maximumMemoryMb,
        javaMode: draft.javaMode,
        performancePreset: draft.performancePreset,
        jvmArguments: draft.jvmArguments
          .split("\n")
          .map((argument) => argument.trim())
          .filter(Boolean),
        environment: parseEnvironmentOverrides(draft.environment),
        backupBeforeChanges: draft.backupBeforeChanges,
        backupRetention: draft.backupRetention,
        logRetentionDays: draft.logRetentionDays,
        expectedRevision: current.revision,
      });
    },
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setMessage("Instance settings saved. They apply on the next launch.");
    },
  });

  const javaMutation = useMutation({
    mutationFn: (mode: "managed" | "detected" | "custom") =>
      selectInstanceJava({
        id: instance.id,
        mode,
        expectedRevision: instance.revision,
      }),
    onSuccess: (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      setMessage("Java selection validated and saved.");
    },
  });

  const artworkMutation = useMutation({
    mutationFn: ({
      kind,
      reset,
    }: {
      kind: "icon" | "banner";
      reset?: boolean;
    }) =>
      (reset ? resetInstanceArtwork : selectInstanceArtwork)({
        id: instance.id,
        kind,
        expectedRevision: instance.revision,
      }),
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      setMessage("Instance artwork updated.");
    },
  });

  const saveRuntime = () => {
    setMessage(undefined);
    if (!minecraftVersion.trim()) {
      setMessage("Enter a Minecraft version.");
      return;
    }
    if (versionsQuery.isError) {
      setMessage("The Minecraft release catalog is unavailable.");
      return;
    }
    if (
      loaderKind !== "vanilla" &&
      (loaderQuery.isPending || loaderQuery.isFetching)
    ) {
      setMessage("Wait for slate to resolve loader compatibility.");
      return;
    }
    if (loaderKind !== "vanilla" && loaderQuery.isError) {
      setMessage("The compatible loader catalog is unavailable.");
      return;
    }
    if (loaderKind !== "vanilla" && loaderQuery.data?.versions.length === 0) {
      setMessage(
        loaderQuery.data?.unavailableReason ??
          "No compatible loader release is available.",
      );
      return;
    }
    runtimeMutation.mutate({
      id: instance.id,
      minecraftVersion,
      loaderKind,
      loaderVersion: selectedLoaderVersion,
      memoryMb: instance.memoryMb,
      expectedRevision: instance.revision,
    });
  };

  const saveSettings = () => {
    setMessage(undefined);
    try {
      parseEnvironmentOverrides(draft.environment);
    } catch (error) {
      setMessage(
        error instanceof Error ? error.message : "Check environment overrides.",
      );
      return;
    }
    settingsMutation.mutate();
  };

  const error =
    settingsMutation.error ??
    runtimeMutation.error ??
    javaMutation.error ??
    artworkMutation.error;
  const busy =
    settingsMutation.isPending ||
    runtimeMutation.isPending ||
    javaMutation.isPending ||
    artworkMutation.isPending;

  return (
    <div className="grid grid-cols-[190px_minmax(0,1fr)] overflow-hidden rounded-control border border-app-separator/70 bg-app-surface">
      <nav className="border-r border-app-separator/60 bg-app-bg/35 p-3" aria-label="Instance settings">
        <p className="px-3 pt-1 pb-2 font-mono text-[10px] uppercase tracking-[0.12em] text-app-muted">
          Instance settings
        </p>
        {([
          ["profile", "Profile", UserRound],
          ["launch", "Launch", Monitor],
          ["runtime", "Java & performance", Cpu],
          ["game", "Game configuration", Settings2],
          ["safety", "Lifecycle & safety", FolderArchive],
        ] as const).map(([value, label, Icon]) => (
          <button
            key={value}
            type="button"
            className={`flex h-10 w-full items-center gap-2 rounded-control px-3 text-left text-xs font-semibold transition-colors ${section === value ? "bg-app-accent/12 text-app-accent" : "text-app-secondary hover:bg-app-raised hover:text-app-text"}`}
            onClick={() => setSection(value)}
          >
            <Icon size={15} aria-hidden="true" />
            {label}
          </button>
        ))}
        <div className="mt-5 border-t border-app-separator/55 px-3 pt-4">
          <span className="font-mono text-[10px] text-app-muted">
            REVISION {instance.revision}
          </span>
          <p className="mt-2 mb-0 text-[11px]/[16px] text-app-secondary">
            Every write is revision guarded. Stale views cannot overwrite newer changes.
          </p>
        </div>
      </nav>

      <section className="min-w-0">
        <div className="min-h-[520px] p-6">
          {section === "profile" ? (
            <SettingsPanel
              title="Profile"
              description="Make this instance easy to recognize without changing its Minecraft files."
            >
              <div className="grid grid-cols-2 gap-5">
                <Field label="Instance name">
                  <input
                    className={inputClass}
                    value={draft.name}
                    maxLength={80}
                    onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                  />
                </Field>
                <Field label="Group">
                  <input
                    className={inputClass}
                    value={draft.groupName}
                    placeholder="Ungrouped"
                    maxLength={80}
                    onChange={(event) => setDraft({ ...draft, groupName: event.target.value })}
                  />
                </Field>
              </div>
              <Field label="Description">
                <input
                  className={inputClass}
                  value={draft.description}
                  maxLength={500}
                  placeholder="A short description shown in your library"
                  onChange={(event) => setDraft({ ...draft, description: event.target.value })}
                />
              </Field>
              <Field label="Tags">
                <input
                  className={inputClass}
                  value={draft.tags}
                  placeholder="building, multiplayer, challenge"
                  onChange={(event) => setDraft({ ...draft, tags: event.target.value })}
                />
              </Field>
              <Field label="Instance notes">
                <textarea
                  className={`${inputClass} h-28 resize-y py-3`}
                  value={draft.notes}
                  maxLength={4000}
                  placeholder="Private notes, server rules, goals, or reminders"
                  onChange={(event) => setDraft({ ...draft, notes: event.target.value })}
                />
              </Field>
              <div className="grid grid-cols-2 gap-4 border-t border-app-separator/55 pt-5">
                <ArtworkControl
                  title="Instance icon"
                  description="PNG, JPEG, or WebP up to 2 MB"
                  customized={instance.settings.hasCustomIcon}
                  disabled={artworkMutation.isPending}
                  onChoose={() => artworkMutation.mutate({ kind: "icon" })}
                  onReset={() => artworkMutation.mutate({ kind: "icon", reset: true })}
                />
                <ArtworkControl
                  title="Library banner"
                  description="PNG, JPEG, or WebP up to 8 MB"
                  customized={instance.settings.hasCustomBanner}
                  disabled={artworkMutation.isPending}
                  onChoose={() => artworkMutation.mutate({ kind: "banner" })}
                  onReset={() => artworkMutation.mutate({ kind: "banner", reset: true })}
                />
              </div>
              {instance.settings.hasCustomBanner ? (
                <div className="grid grid-cols-2 gap-5">
                  <RangeField label="Banner horizontal position" value={draft.bannerPositionX} onChange={(value) => setDraft({ ...draft, bannerPositionX: value })} />
                  <RangeField label="Banner vertical position" value={draft.bannerPositionY} onChange={(value) => setDraft({ ...draft, bannerPositionY: value })} />
                </div>
              ) : null}
            </SettingsPanel>
          ) : null}

          {section === "launch" ? (
            <SettingsPanel
              title="Launch behavior"
              description="Choose the account, window, server, and operating-system behavior used for this instance."
            >
              <div className="grid grid-cols-2 gap-5">
                <Field label="Preferred Minecraft account">
                  <select className={inputClass} value={draft.preferredAccountId} onChange={(event) => setDraft({ ...draft, preferredAccountId: event.target.value })}>
                    <option value="">Use default account</option>
                    {(accountsQuery.data ?? []).map((account) => (
                      <option key={account.id} value={account.id}>{account.displayName}</option>
                    ))}
                  </select>
                </Field>
                <Field label="Window mode">
                  <select className={inputClass} value={draft.windowMode} onChange={(event) => setDraft({ ...draft, windowMode: event.target.value as typeof draft.windowMode })}>
                    <option value="windowed">Windowed</option>
                    <option value="maximized">Maximized</option>
                    <option value="fullscreen">Fullscreen</option>
                  </select>
                </Field>
                <Field label="Window width">
                  <input className={inputClass} type="number" min={320} max={16384} disabled={draft.windowMode !== "windowed"} value={draft.resolutionWidth} placeholder="Default" onChange={(event) => setDraft({ ...draft, resolutionWidth: event.target.value })} />
                </Field>
                <Field label="Window height">
                  <input className={inputClass} type="number" min={240} max={16384} disabled={draft.windowMode !== "windowed"} value={draft.resolutionHeight} placeholder="Default" onChange={(event) => setDraft({ ...draft, resolutionHeight: event.target.value })} />
                </Field>
                <Field label="After Minecraft starts">
                  <select className={inputClass} value={draft.launcherBehavior} onChange={(event) => setDraft({ ...draft, launcherBehavior: event.target.value as typeof draft.launcherBehavior })}>
                    <option value="keepOpen">Keep slate open</option>
                    <option value="minimize">Minimize slate</option>
                    <option value="hide">Close the slate window while playing</option>
                  </select>
                </Field>
                <Field label="Game language">
                  <input className={inputClass} value={draft.gameLanguage} placeholder="en_us" onChange={(event) => setDraft({ ...draft, gameLanguage: event.target.value })} />
                </Field>
                <Field label="Quick-join server">
                  <input className={inputClass} value={draft.quickPlayServer} placeholder="play.example.net:25565" onChange={(event) => setDraft({ ...draft, quickPlayServer: event.target.value })} />
                </Field>
                <Field label="Game process priority">
                  <select className={inputClass} value={draft.processPriority} onChange={(event) => setDraft({ ...draft, processPriority: event.target.value as typeof draft.processPriority })}>
                    <option value="low">Low</option>
                    <option value="belowNormal">Below normal</option>
                    <option value="normal">Normal</option>
                    <option value="aboveNormal">Above normal</option>
                    <option value="high">High</option>
                  </select>
                </Field>
              </div>
            </SettingsPanel>
          ) : null}

          {section === "runtime" ? (
            <SettingsPanel
              title="Java & performance"
              description="slate validates runtime compatibility and keeps launch-critical arguments under native control."
            >
              <h3 className="m-0 text-xs font-bold">Minecraft runtime</h3>
              <p className="mt-1 mb-4 text-[11px] text-app-secondary">Changing Minecraft or its loader requires a verified reinstall. Memory-only changes do not.</p>
              <div className="grid grid-cols-2 gap-5">
          <ComboBox
            label="Minecraft version"
            value={minecraftVersion}
            options={(versionsQuery.data?.versions ?? []).map(
              (version, index) => ({
                value: version.id,
                label: `Minecraft ${version.id}`,
                description: index === 0 ? "Latest release" : "Release",
                recommended: index === 0,
              }),
            )}
            disabled={versionsQuery.isPending || versionsQuery.isError}
            placeholder={
              versionsQuery.isPending ? "Loading releases…" : "Choose a release"
            }
            onValueChange={(value) => {
              setMinecraftVersion(value);
            }}
          />
          <ComboBox
            label="Loader"
            value={loaderKind}
            options={
              instance.mode === "vanilla"
                ? [{ value: "vanilla", label: "Vanilla" }]
                : [
                    { value: "fabric", label: "Fabric" },
                    { value: "neoForge", label: "NeoForge" },
                    ...(instance.mode === "pvp"
                      ? [{ value: "vanilla", label: "Vanilla" }]
                      : []),
                  ]
            }
            disabled={instance.mode === "vanilla"}
            onValueChange={(value) => {
              setLoaderKind(value as LoaderKind);
            }}
          />
          <ComboBox
            label="Loader version"
            value={selectedLoaderVersion ?? ""}
            options={(loaderQuery.data?.versions ?? []).map((version) => ({
              value: version,
              label: version,
              description:
                version === loaderQuery.data?.recommendedVersion
                  ? `${loaderLabel(loaderKind)} recommended`
                  : `${loaderLabel(loaderKind)} loader release`,
              recommended: version === loaderQuery.data?.recommendedVersion,
            }))}
            disabled={
              loaderKind === "vanilla" ||
              loaderQuery.isPending ||
              loaderQuery.isFetching ||
              loaderQuery.data?.versions.length === 0
            }
            placeholder={
              loaderKind === "vanilla"
                ? "Built into Minecraft"
                : loaderQuery.isPending || loaderQuery.isFetching
                  ? "Loading compatible versions…"
                  : "Choose a loader version"
            }
            emptyText={
              loaderQuery.data?.unavailableReason ??
              "No compatible loader versions"
            }
            onValueChange={(value) =>
              setLoaderSelection({ catalogKey: loaderCatalogKey, value })
            }
          />
                <div className="col-span-2 flex justify-end">
                  <button type="button" className={secondaryButtonClass} disabled={runtimeMutation.isPending} onClick={saveRuntime}>
                    {runtimeMutation.isPending ? <RotateCcw className="animate-spin" size={14} /> : <Save size={14} />}
                    Save runtime target
                  </button>
                </div>
              </div>

              <div className="border-t border-app-separator/55 pt-5">
                <div className="grid grid-cols-3 gap-3">
                  <ChoiceButton active={draft.memoryMode === "auto"} title="Automatic memory" description={`${instance.modCount} installed mods`} onClick={() => setDraft({ ...draft, memoryMode: "auto" })} />
                  <ChoiceButton active={draft.memoryMode === "custom"} title="Manual memory" description="Set exact limits" onClick={() => setDraft({ ...draft, memoryMode: "custom" })} />
                  <div className="rounded-control border border-app-separator/70 bg-app-bg/40 px-4 py-3">
                    <span className="block text-xs font-bold">Current recommendation</span>
                    <span className="mt-1 block font-mono text-[11px] text-app-accent">{formatMemory(draft.memoryMode === "auto" ? instance.settings.effectiveMemoryMb : draft.maximumMemoryMb)}</span>
                  </div>
                </div>
                <div className="mt-5 grid grid-cols-2 gap-5">
                  <NumberField label="Initial memory (MB)" value={draft.initialMemoryMb} min={256} max={32768} step={256} onChange={(value) => setDraft({ ...draft, initialMemoryMb: value })} />
                  <NumberField label="Maximum memory (MB)" value={draft.maximumMemoryMb} min={1024} max={32768} step={256} disabled={draft.memoryMode === "auto"} onChange={(value) => setDraft({ ...draft, maximumMemoryMb: value })} />
                </div>
              </div>

              <div className="border-t border-app-separator/55 pt-5">
                <h3 className="m-0 text-xs font-bold">Java runtime</h3>
                <p className="mt-1 mb-4 text-[11px] text-app-secondary">Minecraft {instance.minecraftVersion} requires Java {requiredJavaLabel(instance.minecraftVersion)}. A selection is saved only after slate probes it.</p>
                <div className="grid grid-cols-3 gap-3">
                  <ChoiceButton active={instance.settings.javaMode === "managed"} title="Managed" description="Installed by slate" disabled={javaMutation.isPending} onClick={() => javaMutation.mutate("managed")} />
                  <ChoiceButton active={instance.settings.javaMode === "detected"} title="System Java" description="Detect from PATH" disabled={javaMutation.isPending} onClick={() => javaMutation.mutate("detected")} />
                  <ChoiceButton active={instance.settings.javaMode === "custom"} title="Custom executable" description="Choose java or javaw" disabled={javaMutation.isPending} onClick={() => javaMutation.mutate("custom")} />
                </div>
                {instance.settings.customJavaLabel ? <p className="mt-3 mb-0 truncate font-mono text-[10px] text-app-muted">{instance.settings.customJavaLabel}</p> : null}
              </div>

              <div className="border-t border-app-separator/55 pt-5">
                <Field label="Performance preset">
                  <select className={inputClass} value={draft.performancePreset} onChange={(event) => setDraft({ ...draft, performancePreset: event.target.value as typeof draft.performancePreset })}>
                    <option value="balanced">Balanced</option>
                    <option value="throughput">Throughput</option>
                    <option value="lowLatency">Low latency</option>
                    <option value="custom">Custom</option>
                  </select>
                </Field>
                <details className="mt-5 border-t border-app-separator/55 pt-4">
                  <summary className="flex cursor-pointer list-none items-center gap-2 text-xs font-bold"><ChevronDown size={14} />Advanced launch overrides</summary>
                  <p className="mt-2 text-[11px] text-app-secondary">One argument or KEY=value environment override per line. slate rejects memory, classpath, agent, path, and Java control variables.</p>
                  <div className="grid grid-cols-2 gap-5">
                    <Field label="CPU affinity">
                      <input className={`${inputClass} font-mono text-[11px]`} value={draft.cpuAffinity} placeholder="0, 1, 2, 3 (empty uses all CPUs)" onChange={(event) => setDraft({ ...draft, cpuAffinity: event.target.value })} />
                    </Field>
                    <div className="flex items-end pb-3 text-[11px] leading-5 text-app-secondary">
                      Restrict Minecraft to selected logical CPU indices. Leave empty unless diagnosing performance or compatibility issues.
                    </div>
                    <Field label="Additional JVM arguments">
                      <textarea className={`${inputClass} h-32 resize-y py-3 font-mono text-[11px]`} value={draft.jvmArguments} placeholder="-Dexample=value" onChange={(event) => setDraft({ ...draft, jvmArguments: event.target.value })} />
                    </Field>
                    <Field label="Environment overrides">
                      <textarea className={`${inputClass} h-32 resize-y py-3 font-mono text-[11px]`} value={draft.environment} placeholder="EXAMPLE=value" onChange={(event) => setDraft({ ...draft, environment: event.target.value })} />
                    </Field>
                  </div>
                </details>
              </div>
            </SettingsPanel>
          ) : null}

          {section === "game" ? (
            <InstanceGameOptionsEditor instance={instance} />
          ) : null}

          {section === "safety" ? (
            <SettingsPanel title="Lifecycle & safety" description="Keep recoverable history around content and runtime changes.">
              <label className="flex items-start justify-between gap-6 border-b border-app-separator/55 pb-5">
                <span><strong className="block text-xs">Snapshot before managed changes</strong><span className="mt-1 block text-[11px] text-app-secondary">Create a restore point before pack, runtime, or bulk content changes.</span></span>
                <input type="checkbox" checked={draft.backupBeforeChanges} onChange={(event) => setDraft({ ...draft, backupBeforeChanges: event.target.checked })} />
              </label>
              <div className="grid grid-cols-2 gap-5">
                <NumberField label="Snapshots to keep" value={draft.backupRetention} min={1} max={50} onChange={(value) => setDraft({ ...draft, backupRetention: value })} />
                <NumberField label="Log retention (days)" value={draft.logRetentionDays} min={1} max={365} onChange={(value) => setDraft({ ...draft, logRetentionDays: value })} />
              </div>
            <InstanceLifecycleActions instance={instance} />
              <div className="border-t border-app-separator/55 pt-5">
                <h3 className="m-0 text-xs font-bold">Repair</h3>
                <p className="mt-1 mb-4 text-[11px] text-app-secondary">Verify the selected runtime revision and download only missing or changed files.</p>
                <button type="button" className={secondaryButtonClass} onClick={() => void installInstance({ id: instance.id, expectedRevision: instance.revision })}><RotateCcw size={14} />Repair and verify</button>
              </div>
            </SettingsPanel>
          ) : null}
        </div>

        <footer className="flex min-h-16 items-center justify-between border-t border-app-separator/60 bg-app-bg/30 px-6 py-3">
          <p className={`m-0 text-xs ${error ? "text-app-danger" : "text-app-secondary"}`} role={error ? "alert" : "status"}>
            {error ? contentErrorMessage(error) : message}
          </p>
          {section === "game" ? (
            <span className="text-[11px] text-app-muted">Game configuration has its own guarded save action.</span>
          ) : (
            <button type="button" className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-50" disabled={busy} onClick={saveSettings}>
              {settingsMutation.isPending ? <RotateCcw className="animate-spin" size={16} /> : <Check size={16} />}
              Save settings
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}

type InstanceSettingsDraft = {
  name: string;
  description: string;
  notes: string;
  groupName: string;
  tags: string;
  bannerPositionX: number;
  bannerPositionY: number;
  preferredAccountId: string;
  windowMode: LauncherInstance["settings"]["windowMode"];
  resolutionWidth: string;
  resolutionHeight: string;
  launcherBehavior: LauncherInstance["settings"]["launcherBehavior"];
  gameLanguage: string;
  quickPlayServer: string;
  processPriority: LauncherInstance["settings"]["processPriority"];
  cpuAffinity: string;
  memoryMode: LauncherInstance["settings"]["memoryMode"];
  initialMemoryMb: number;
  maximumMemoryMb: number;
  javaMode: LauncherInstance["settings"]["javaMode"];
  performancePreset: LauncherInstance["settings"]["performancePreset"];
  jvmArguments: string;
  environment: string;
  backupBeforeChanges: boolean;
  backupRetention: number;
  logRetentionDays: number;
};

function instanceSettingsDraft(instance: LauncherInstance): InstanceSettingsDraft {
  const settings = instance.settings;
  return {
    name: instance.name,
    description: settings.description,
    notes: settings.notes,
    groupName: settings.groupName ?? "",
    tags: settings.tags.join(", "),
    bannerPositionX: settings.bannerPositionX,
    bannerPositionY: settings.bannerPositionY,
    preferredAccountId: settings.preferredAccountId ?? "",
    windowMode: settings.windowMode,
    resolutionWidth: settings.resolutionWidth?.toString() ?? "",
    resolutionHeight: settings.resolutionHeight?.toString() ?? "",
    launcherBehavior: settings.launcherBehavior,
    gameLanguage: settings.gameLanguage,
    quickPlayServer: settings.quickPlayServer ?? "",
    processPriority: settings.processPriority,
    cpuAffinity: settings.cpuAffinity.join(", "),
    memoryMode: settings.memoryMode,
    initialMemoryMb: settings.initialMemoryMb,
    maximumMemoryMb: instance.memoryMb,
    javaMode: settings.javaMode,
    performancePreset: settings.performancePreset,
    jvmArguments: settings.jvmArguments.join("\n"),
    environment: Object.entries(settings.environment)
      .map(([key, value]) => `${key}=${value}`)
      .join("\n"),
    backupBeforeChanges: settings.backupBeforeChanges,
    backupRetention: settings.backupRetention,
    logRetentionDays: settings.logRetentionDays,
  };
}

function cleanOptional(value: string) {
  const cleaned = value.trim();
  return cleaned ? cleaned : undefined;
}

function numberOrUndefined(value: string) {
  if (!value.trim()) return undefined;
  const number = Number(value);
  return Number.isInteger(number) ? number : undefined;
}

function parseEnvironmentOverrides(value: string) {
  const result: Record<string, string> = {};
  for (const [index, rawLine] of value.split("\n").entries()) {
    const line = rawLine.trim();
    if (!line) continue;
    const separator = line.indexOf("=");
    if (separator <= 0) {
      throw new Error(`Environment line ${index + 1} must use KEY=value.`);
    }
    const key = line.slice(0, separator).trim();
    const itemValue = line.slice(separator + 1);
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
      throw new Error(`Environment line ${index + 1} has an invalid variable name.`);
    }
    result[key] = itemValue;
  }
  return result;
}

function parseCpuAffinity(value: string) {
  if (!value.trim()) return [];
  const cpus = value.split(",").map((rawCpu, index) => {
    const cpu = Number(rawCpu.trim());
    if (!Number.isInteger(cpu) || cpu < 0 || cpu > 63) {
      throw new Error(`CPU affinity item ${index + 1} must be an integer from 0 through 63.`);
    }
    return cpu;
  });
  return [...new Set(cpus)].sort((left, right) => left - right);
}

function requiredJavaLabel(version: string) {
  const [major = 0, minor = 0, patch = 0] = version
    .split(/[.-]/)
    .slice(0, 3)
    .map((part) => Number(part));
  if (major === 1 && minor <= 16) return 8;
  if (major === 1 && minor === 17) return 16;
  if (major === 1 && (minor < 20 || (minor === 20 && patch <= 4))) return 17;
  return 21;
}

function formatMemory(value: number) {
  return value % 1024 === 0 ? `${value / 1024} GB` : `${value} MB`;
}

function ArtworkControl({
  title,
  description,
  customized,
  disabled,
  onChoose,
  onReset,
}: {
  title: string;
  description: string;
  customized: boolean;
  disabled: boolean;
  onChoose: () => void;
  onReset: () => void;
}) {
  return (
    <div className="flex items-center gap-3 rounded-control border border-app-separator/70 bg-app-bg/35 p-3">
      <span className="grid size-10 shrink-0 place-items-center rounded-control bg-app-raised text-app-secondary">
        <ImageIcon size={17} aria-hidden="true" />
      </span>
      <span className="min-w-0 flex-1">
        <strong className="block text-xs">{title}</strong>
        <span className="block truncate text-[10px] text-app-muted">
          {customized ? "Custom image selected" : description}
        </span>
      </span>
      {customized ? (
        <button type="button" className="text-[11px] font-semibold text-app-secondary hover:text-app-text" disabled={disabled} onClick={onReset}>
          Reset
        </button>
      ) : null}
      <button type="button" className="text-[11px] font-bold text-app-accent" disabled={disabled} onClick={onChoose}>
        Choose
      </button>
    </div>
  );
}

function RangeField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="block text-xs font-bold">
      <span className="flex justify-between"><span>{label}</span><span className="font-mono text-[10px] text-app-muted">{value}%</span></span>
      <input className="mt-3 w-full accent-app-accent" type="range" min={0} max={100} value={value} onChange={(event) => onChange(Number(event.target.value))} />
    </label>
  );
}

function ChoiceButton({
  active,
  title,
  description,
  disabled = false,
  onClick,
}: {
  active: boolean;
  title: string;
  description: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button type="button" className={`rounded-control border px-4 py-3 text-left transition-colors ${active ? "border-app-accent bg-app-accent/10" : "border-app-separator/70 bg-app-bg/35 hover:border-app-secondary"}`} disabled={disabled} onClick={onClick}>
      <span className="flex items-center justify-between text-xs font-bold">{title}{active ? <Check size={14} className="text-app-accent" /> : null}</span>
      <span className="mt-1 block text-[10px] text-app-muted">{description}</span>
    </button>
  );
}

function NumberField({
  label,
  value,
  min,
  max,
  step = 1,
  disabled = false,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  disabled?: boolean;
  onChange: (value: number) => void;
}) {
  return (
    <Field label={label}>
      <input className={inputClass} type="number" value={value} min={min} max={max} step={step} disabled={disabled} onChange={(event) => onChange(Number(event.target.value))} />
    </Field>
  );
}
