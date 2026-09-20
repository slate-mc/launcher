import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RotateCcw, Save } from "lucide-react";
import { useState, type ReactNode } from "react";
import { InlineNotice } from "../../components/PageScaffold";
import {
  getInstanceGameOptions,
  updateInstanceGameOptions,
} from "../../lib/bridge";
import type { LauncherInstance } from "../../types/launcher";
import { contentErrorMessage } from "./instanceContentFormat";
import { Field, SettingsPanel } from "./InstanceSettingsUi";
import { inputClass } from "./instanceSettingsStyles";

export function InstanceGameOptionsEditor({ instance }: { instance: LauncherInstance }) {
  const query = useQuery({
    queryKey: ["instance-game-options", instance.id],
    queryFn: () => getInstanceGameOptions(instance.id),
  });
  if (query.isPending) {
    return <div className="h-80 animate-pulse rounded-control bg-app-raised" aria-label="Loading game configuration" />;
  }
  if (query.isError) {
    return <InlineNotice tone="warning" title="Game settings unavailable">Close Minecraft, then try loading these settings again.</InlineNotice>;
  }
  return (
    <GameOptionsForm
      key={`${instance.revision}:${JSON.stringify(query.data.values)}`}
      instance={instance}
      fileExists={query.data.fileExists}
      initialValues={query.data.values}
    />
  );
}

const defaultGameOptionValues: Record<string, string> = {
  graphicsMode: "1",
  renderDistance: "12",
  simulationDistance: "12",
  guiScale: "0",
  entityDistanceScaling: "1.0",
  particles: "0",
  mipmapLevels: "4",
  enableVsync: "true",
  maxFps: "120",
  bobView: "true",
  mouseSensitivity: "0.5",
  invertYMouse: "false",
  autoJump: "false",
  toggleCrouch: "false",
  toggleSprint: "false",
  showSubtitles: "false",
  narrator: "0",
  chatVisibility: "0",
  chatOpacity: "1.0",
  textBackgroundOpacity: "0.5",
  darknessEffectScale: "1.0",
  damageTiltStrength: "1.0",
  directionalAudio: "false",
  realmsNotifications: "true",
  allowServerListing: "true",
  chatLinks: "true",
  chatLinksPrompt: "true",
  soundCategory_master: "1.0",
  soundCategory_music: "1.0",
  soundCategory_weather: "1.0",
  soundCategory_hostile: "1.0",
  soundCategory_player: "1.0",
};

function GameOptionsForm({
  instance,
  fileExists,
  initialValues,
}: {
  instance: LauncherInstance;
  fileExists: boolean;
  initialValues: Record<string, string>;
}) {
  const queryClient = useQueryClient();
  const [values, setValues] = useState(() => ({
    ...defaultGameOptionValues,
    ...initialValues,
  }));
  const mutation = useMutation({
    mutationFn: () =>
      updateInstanceGameOptions({
        id: instance.id,
        values,
        expectedRevision: instance.revision,
      }),
    onSuccess: async (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      await queryClient.invalidateQueries({
        queryKey: ["instance-game-options", instance.id],
      });
    },
  });
  const update = (key: string, value: string) =>
    setValues((current) => ({ ...current, [key]: value }));
  const resetCategory = (keys: string[]) =>
    setValues((current) => ({
      ...current,
      ...Object.fromEntries(keys.map((key) => [key, defaultGameOptionValues[key]])),
    }));

  return (
    <SettingsPanel
      title="Game configuration"
      description="Edit common Minecraft options without discarding settings added by the game or mods."
    >
      <InlineNotice title="Other settings stay unchanged">
        {fileExists
          ? "Only the settings shown below are changed. Other Minecraft and mod settings are left alone."
          : "Minecraft has not created its settings file yet. Save once to create it with the choices below."}
      </InlineNotice>

      <GameOptionSection title="Video" onReset={() => resetCategory(["graphicsMode", "renderDistance", "simulationDistance", "maxFps", "guiScale", "entityDistanceScaling", "enableVsync", "bobView"])}>
        <Field label="Graphics quality">
          <select className={inputClass} value={values.graphicsMode} onChange={(event) => update("graphicsMode", event.target.value)}>
            <option value="0">Fast</option><option value="1">Fancy</option><option value="2">Fabulous</option>
          </select>
        </Field>
        <GameOptionNumber label="Render distance" value={values.renderDistance} min={2} max={64} suffix="chunks" onChange={(value) => update("renderDistance", value)} />
        <GameOptionNumber label="Simulation distance" value={values.simulationDistance} min={2} max={32} suffix="chunks" onChange={(value) => update("simulationDistance", value)} />
        <GameOptionNumber label="Maximum frame rate" value={values.maxFps} min={10} max={260} suffix="FPS" onChange={(value) => update("maxFps", value)} />
        <GameOptionNumber label="GUI scale" value={values.guiScale} min={0} max={8} onChange={(value) => update("guiScale", value)} />
        <GameOptionRange label="Entity distance" value={values.entityDistanceScaling} min={0.5} max={5} step={0.1} onChange={(value) => update("entityDistanceScaling", value)} />
        <GameOptionCheckbox label="Vertical sync" checked={values.enableVsync === "true"} onChange={(checked) => update("enableVsync", String(checked))} />
        <GameOptionCheckbox label="View bobbing" checked={values.bobView === "true"} onChange={(checked) => update("bobView", String(checked))} />
      </GameOptionSection>

      <GameOptionSection title="Audio" onReset={() => resetCategory(["soundCategory_master", "soundCategory_music", "soundCategory_weather", "soundCategory_hostile", "soundCategory_player", "directionalAudio"])}>
        <GameOptionRange label="Master volume" value={values.soundCategory_master} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_master", value)} />
        <GameOptionRange label="Music" value={values.soundCategory_music} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_music", value)} />
        <GameOptionRange label="Weather" value={values.soundCategory_weather} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_weather", value)} />
        <GameOptionRange label="Hostile creatures" value={values.soundCategory_hostile} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_hostile", value)} />
        <GameOptionRange label="Players" value={values.soundCategory_player} min={0} max={1} step={0.01} percentage onChange={(value) => update("soundCategory_player", value)} />
        <GameOptionCheckbox label="Directional audio" checked={values.directionalAudio === "true"} onChange={(checked) => update("directionalAudio", String(checked))} />
      </GameOptionSection>

      <GameOptionSection title="Controls & accessibility" onReset={() => resetCategory(["mouseSensitivity", "chatOpacity", "darknessEffectScale", "damageTiltStrength", "autoJump", "invertYMouse", "toggleCrouch", "toggleSprint", "showSubtitles"])}>
        <GameOptionRange label="Mouse sensitivity" value={values.mouseSensitivity} min={0} max={1} step={0.01} percentage onChange={(value) => update("mouseSensitivity", value)} />
        <GameOptionRange label="Chat opacity" value={values.chatOpacity} min={0} max={1} step={0.01} percentage onChange={(value) => update("chatOpacity", value)} />
        <GameOptionRange label="Darkness pulse strength" value={values.darknessEffectScale} min={0} max={1} step={0.01} percentage onChange={(value) => update("darknessEffectScale", value)} />
        <GameOptionRange label="Damage tilt strength" value={values.damageTiltStrength} min={0} max={1} step={0.01} percentage onChange={(value) => update("damageTiltStrength", value)} />
        <GameOptionCheckbox label="Auto jump" checked={values.autoJump === "true"} onChange={(checked) => update("autoJump", String(checked))} />
        <GameOptionCheckbox label="Invert mouse" checked={values.invertYMouse === "true"} onChange={(checked) => update("invertYMouse", String(checked))} />
        <GameOptionCheckbox label="Toggle crouch" checked={values.toggleCrouch === "true"} onChange={(checked) => update("toggleCrouch", String(checked))} />
        <GameOptionCheckbox label="Toggle sprint" checked={values.toggleSprint === "true"} onChange={(checked) => update("toggleSprint", String(checked))} />
        <GameOptionCheckbox label="Show subtitles" checked={values.showSubtitles === "true"} onChange={(checked) => update("showSubtitles", String(checked))} />
      </GameOptionSection>

      <GameOptionSection title="Multiplayer" onReset={() => resetCategory(["allowServerListing", "realmsNotifications", "chatLinks", "chatLinksPrompt"])}>
        <GameOptionCheckbox label="Allow server listing" checked={values.allowServerListing === "true"} onChange={(checked) => update("allowServerListing", String(checked))} />
        <GameOptionCheckbox label="Realms notifications" checked={values.realmsNotifications === "true"} onChange={(checked) => update("realmsNotifications", String(checked))} />
        <GameOptionCheckbox label="Open links in chat" checked={values.chatLinks === "true"} onChange={(checked) => update("chatLinks", String(checked))} />
        <GameOptionCheckbox label="Prompt before opening links" checked={values.chatLinksPrompt === "true"} onChange={(checked) => update("chatLinksPrompt", String(checked))} />
      </GameOptionSection>

      <div className="flex items-center justify-between border-t border-app-separator/55 pt-5">
        <p className={`m-0 text-xs ${mutation.isError ? "text-app-danger" : "text-app-secondary"}`} role={mutation.isError ? "alert" : "status"}>
          {mutation.isError ? contentErrorMessage(mutation.error) : mutation.isSuccess ? "Game configuration saved." : "Changes apply on the next launch."}
        </p>
        <button type="button" className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-50" disabled={mutation.isPending} onClick={() => mutation.mutate()}>
          {mutation.isPending ? <RotateCcw className="animate-spin" size={15} /> : <Save size={15} />}
          Save game configuration
        </button>
      </div>
    </SettingsPanel>
  );
}

function GameOptionSection({ title, onReset, children }: { title: string; onReset: () => void; children: ReactNode }) {
  return (
    <section className="grid grid-cols-2 gap-x-5 gap-y-4 border-t border-app-separator/55 pt-5">
      <div className="col-span-2 flex items-center justify-between">
        <h3 className="m-0 text-xs font-bold">{title}</h3>
        <button type="button" className="text-[10px] font-bold text-app-secondary hover:text-app-text" onClick={onReset}>Reset category</button>
      </div>
      {children}
    </section>
  );
}

function GameOptionCheckbox({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="flex h-10 items-center justify-between rounded-control border border-app-separator/70 bg-app-bg/35 px-3 text-xs font-semibold">
      {label}
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
    </label>
  );
}

function GameOptionNumber({ label, value, min, max, suffix, onChange }: { label: string; value: string; min: number; max: number; suffix?: string; onChange: (value: string) => void }) {
  return (
    <Field label={label}>
      <span className="relative block">
        <input className={`${inputClass} ${suffix ? "pr-16" : ""}`} type="number" min={min} max={max} value={value} onChange={(event) => onChange(event.target.value)} />
        {suffix ? <span className="absolute right-3 bottom-3 text-[10px] text-app-muted">{suffix}</span> : null}
      </span>
    </Field>
  );
}

function GameOptionRange({ label, value, min, max, step, percentage = false, onChange }: { label: string; value: string; min: number; max: number; step: number; percentage?: boolean; onChange: (value: string) => void }) {
  const numeric = Number(value);
  return (
    <label className="block text-xs font-bold">
      <span className="flex justify-between"><span>{label}</span><span className="font-mono text-[10px] text-app-muted">{percentage ? `${Math.round(numeric * 100)}%` : numeric.toFixed(1)}</span></span>
      <input className="mt-3 w-full accent-app-accent" type="range" min={min} max={max} step={step} value={value} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}
