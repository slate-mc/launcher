import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ArrowLeft,
  ArrowRight,
  Box,
  Check,
  Layers3,
  Sparkles,
} from "lucide-react";
import { useState } from "react";
import { ComboBox } from "../../components/ComboBox";
import { InlineNotice, PageHeader } from "../../components/PageScaffold";
import {
  createInstance,
  getBootstrap,
  getLoaderVersionCatalog,
  getMinecraftVersionCatalog,
} from "../../lib/bridge";
import { loaderLabel } from "../../lib/format";
import {
  createInstanceSchema,
  type CreateInstanceInput,
  type LoaderKind,
} from "../../types/launcher";

type Step = 1 | 2 | 3;

const fieldClass =
  "mt-2 h-10 w-full rounded-control border border-app-separator bg-app-bg px-3 text-[13px] text-app-text placeholder:text-app-muted focus:border-app-accent focus:outline-none";
const labelClass = "block text-xs font-bold text-app-text";

export function NewInstancePage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [step, setStep] = useState<Step>(1);
  const [mode, setMode] = useState<CreateInstanceInput["mode"]>("vanilla");
  const [name, setName] = useState("New instance");
  const [minecraftVersion, setMinecraftVersion] = useState("");
  const [loaderKind, setLoaderKind] = useState<LoaderKind>("vanilla");
  const [loaderSelection, setLoaderSelection] = useState({
    catalogKey: "",
    value: "",
  });
  const [memoryMb, setMemoryMb] = useState(4096);
  const [validationError, setValidationError] = useState<string>();

  const createMutation = useMutation({
    mutationFn: createInstance,
    onSuccess: async (instance) => {
      await queryClient.invalidateQueries({ queryKey: ["instances"] });
      await navigate({
        to: "/instances/$instanceId/overview",
        params: { instanceId: instance.id },
      });
    },
  });
  const versionsQuery = useQuery({
    queryKey: ["minecraft-version-catalog"],
    queryFn: getMinecraftVersionCatalog,
    staleTime: 15 * 60_000,
  });
  const bootstrapQuery = useQuery({
    queryKey: ["bootstrap"],
    queryFn: getBootstrap,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const slateClientCapability = bootstrapQuery.data?.capabilities.find(
    (capability) => capability.id === "slate.client",
  );
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
        : loaderQuery.data?.recommendedVersion;

  const selectMode = (value: CreateInstanceInput["mode"]) => {
    setMode(value);
    if (value === "vanilla") {
      setLoaderKind("vanilla");
    } else if (value === "slateClient") {
      setMinecraftVersion("1.21.1");
      setLoaderKind("fabric");
      setLoaderSelection({
        catalogKey: "1.21.1|fabric",
        value: "0.19.5",
      });
    } else if (loaderKind === "vanilla") {
      setLoaderKind("fabric");
    }
  };

  const continueToReview = () => {
    const partial = createInstanceSchema.safeParse({
      name,
      mode,
      minecraftVersion,
      loaderKind,
      loaderVersion: selectedLoaderVersion,
      memoryMb,
    });
    if (!partial.success) {
      setValidationError(partial.error.issues[0]?.message ?? "Review the highlighted values.");
      return;
    }
    if (
      loaderKind !== "vanilla" &&
      (loaderQuery.isPending || loaderQuery.isFetching)
    ) {
      setValidationError("Wait for slate to resolve a compatible loader release.");
      return;
    }
    if (
      loaderKind !== "vanilla" &&
      (loaderQuery.isError || loaderQuery.data?.versions.length === 0)
    ) {
      setValidationError(
        loaderQuery.data?.unavailableReason ??
          "No compatible loader release is available for this Minecraft version.",
      );
      return;
    }
    setValidationError(undefined);
    setStep(3);
  };

  const submit = () => {
    const result = createInstanceSchema.safeParse({
      name,
      mode,
      minecraftVersion,
      loaderKind,
      loaderVersion: selectedLoaderVersion,
      memoryMb,
    });
    if (!result.success) {
      setValidationError(result.error.issues[0]?.message ?? "Review the setup.");
      return;
    }
    setValidationError(undefined);
    createMutation.mutate(result.data);
  };

  return (
    <div className="min-h-full bg-app-bg">
      <PageHeader
        eyebrow="New instance"
        title="Build a clean setup"
        description="Choose the Minecraft version, mod loader, and memory for this instance."
        actions={
          <Link
            to="/library"
            className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-surface px-3.5 text-xs font-bold text-app-text no-underline hover:bg-app-hover"
          >
            <ArrowLeft size={16} aria-hidden="true" />
            Library
          </Link>
        }
      />

      <div className="mx-auto grid max-w-[980px] grid-cols-[190px_minmax(0,1fr)] gap-8 px-8 py-8">
        <ol className="m-0 list-none p-0" aria-label="Instance creation progress">
          {[
            [1, "Experience"],
            [2, "Game version"],
            [3, "Review"],
          ].map(([value, label]) => (
            <li
              key={value}
              className={`flex min-h-12 items-center gap-3 border-l px-4 text-xs font-bold ${
                step === value
                  ? "border-app-accent text-app-text"
                  : "border-app-separator text-app-muted"
              }`}
            >
              <span
                className={`inline-flex size-6 items-center justify-center rounded-full border font-mono text-[10px] ${
                  Number(value) < step
                    ? "border-app-accent bg-app-accent text-app-on-accent"
                    : "border-app-separator bg-app-surface"
                }`}
              >
                {Number(value) < step ? (
                  <Check size={13} aria-hidden="true" />
                ) : (
                  value
                )}
              </span>
              {label}
            </li>
          ))}
        </ol>

        <section className="min-w-0 rounded-dialog border border-app-separator/70 bg-app-surface p-6">
          {step === 1 ? (
            <ExperienceStep
              mode={mode}
              slateClientAvailable={slateClientCapability?.available === true}
              onSelect={selectMode}
            />
          ) : step === 2 ? (
            <ConfigurationStep
              name={name}
              minecraftVersion={minecraftVersion}
              loaderKind={loaderKind}
              memoryMb={memoryMb}
              mode={mode}
              versions={versionsQuery.data?.versions.map((version) => version.id) ?? []}
              versionsPending={versionsQuery.isPending}
              versionsError={versionsQuery.isError}
              loaderPending={loaderQuery.isPending || loaderQuery.isFetching}
              loaderVersions={loaderQuery.data?.versions ?? []}
              recommendedLoaderVersion={loaderQuery.data?.recommendedVersion}
              loaderUnavailableReason={loaderQuery.data?.unavailableReason}
              selectedLoaderVersion={selectedLoaderVersion ?? ""}
              onNameChange={setName}
              onMinecraftVersionChange={(value) => {
                setMinecraftVersion(value);
              }}
              onLoaderChange={(value) => {
                setLoaderKind(value);
              }}
              onLoaderVersionChange={(value) =>
                setLoaderSelection({ catalogKey: loaderCatalogKey, value })
              }
              onMemoryChange={setMemoryMb}
              onRetryVersions={() => void versionsQuery.refetch()}
            />
          ) : (
            <ReviewStep
              name={name}
              mode={mode}
              minecraftVersion={minecraftVersion}
              loaderKind={loaderKind}
              loaderVersion={selectedLoaderVersion}
              memoryMb={memoryMb}
            />
          )}

          {validationError ? (
            <p className="mt-5 text-xs text-app-danger" role="alert">
              {validationError}
            </p>
          ) : null}
          {createMutation.isError ? (
            <p className="mt-5 text-xs text-app-danger" role="alert">
              slate could not create this instance. Check the values and retry.
            </p>
          ) : null}

          <div className="mt-7 flex items-center justify-between border-t border-app-separator/55 pt-5">
            <button
              type="button"
              className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-bg px-4 text-xs font-bold text-app-text disabled:opacity-40"
              disabled={step === 1 || createMutation.isPending}
              onClick={() => setStep((step - 1) as Step)}
            >
              <ArrowLeft size={16} aria-hidden="true" />
              Back
            </button>
            {step < 3 ? (
              <button
                type="button"
                className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent"
                onClick={() => {
                  if (step === 1) setStep(2);
                  else continueToReview();
                }}
              >
                Continue
                <ArrowRight size={16} aria-hidden="true" />
              </button>
            ) : (
              <button
                type="button"
                className="inline-flex h-9 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-50"
                disabled={createMutation.isPending}
                onClick={submit}
              >
                {createMutation.isPending ? "Creating…" : "Create instance"}
              </button>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function ExperienceStep({
  mode,
  slateClientAvailable,
  onSelect,
}: {
  mode: CreateInstanceInput["mode"];
  slateClientAvailable: boolean;
  onSelect: (mode: CreateInstanceInput["mode"]) => void;
}) {
  const experiences = [
    {
      id: "vanilla" as const,
      title: "Vanilla",
      description: "Minecraft without a mod loader.",
      icon: Box,
      available: true,
    },
    {
      id: "modded" as const,
      title: "Modded",
      description: "Use Fabric or NeoForge for mods and modpacks.",
      icon: Layers3,
      available: true,
    },
    {
      id: "slateClient" as const,
      title: "Slate Client",
      description: "Performance, HUD, accessibility, QoL, creator, and optional PvP tools.",
      icon: Sparkles,
      available: slateClientAvailable,
    },
  ];
  return (
    <>
      <p className="m-0 text-[11px] font-bold tracking-[.08em] text-app-muted uppercase">
        Step 1 of 3
      </p>
      <h2 className="mt-2 mb-1 text-xl font-bold tracking-[-.025em]">
        Choose an experience
      </h2>
      <p className="mt-0 mb-5 text-xs text-app-secondary">
        This sets safe defaults. You can still edit memory and loader details.
      </p>
      <div className="grid grid-cols-3 gap-3">
        {experiences.map((experience) => {
          const Icon = experience.icon;
          return (
            <button
              key={experience.id}
              type="button"
              aria-pressed={mode === experience.id}
              disabled={!experience.available}
              className={`min-h-[168px] rounded-control border p-4 text-left transition-colors ${
                mode === experience.id
                  ? "border-app-accent bg-app-accent/8"
                  : "border-app-separator bg-app-bg hover:bg-app-hover/45"
              } disabled:cursor-not-allowed disabled:opacity-55 disabled:hover:bg-app-bg`}
              onClick={() => onSelect(experience.id)}
            >
              <span className="inline-flex size-9 items-center justify-center rounded-lg bg-app-raised text-app-accent">
                <Icon size={19} aria-hidden="true" />
              </span>
              <strong className="mt-4 block text-[13px] font-bold text-app-text">
                {experience.title}
              </strong>
              <span className="mt-1.5 block text-[11px]/[17px] text-app-secondary">
                {experience.description}
              </span>
              {!experience.available ? (
                <span className="mt-3 inline-flex rounded-full border border-app-separator px-2 py-1 font-mono text-[9px] tracking-[.08em] text-app-muted uppercase">
                  In development
                </span>
              ) : null}
            </button>
          );
        })}
      </div>
    </>
  );
}

function ConfigurationStep(props: {
  name: string;
  mode: CreateInstanceInput["mode"];
  minecraftVersion: string;
  loaderKind: LoaderKind;
  memoryMb: number;
  onNameChange: (value: string) => void;
  onMinecraftVersionChange: (value: string) => void;
  onLoaderChange: (value: LoaderKind) => void;
  onMemoryChange: (value: number) => void;
  versions: string[];
  versionsPending: boolean;
  versionsError: boolean;
  loaderPending: boolean;
  loaderVersions: string[];
  recommendedLoaderVersion?: string;
  loaderUnavailableReason?: string;
  selectedLoaderVersion: string;
  onRetryVersions: () => void;
  onLoaderVersionChange: (value: string) => void;
}) {
  return (
    <>
      <p className="m-0 text-[11px] font-bold tracking-[.08em] text-app-muted uppercase">
        Step 2 of 3
      </p>
      <h2 className="mt-2 mb-1 text-xl font-bold tracking-[-.025em]">
        Choose your game version
      </h2>
      <p className="mt-0 mb-5 text-xs text-app-secondary">
        {props.mode === "slateClient"
          ? "This build uses Minecraft 1.21.1 with Fabric 0.19.5."
          : "Choose Minecraft first, then pick one of the compatible loader versions."}
      </p>

      <div className="grid grid-cols-2 gap-5">
        <label className={labelClass}>
          Instance name
          <input
            className={fieldClass}
            value={props.name}
            maxLength={80}
            onChange={(event) => props.onNameChange(event.target.value)}
          />
        </label>
        <div>
          <ComboBox
            label="Minecraft version"
            value={props.minecraftVersion}
            options={props.versions.map((version, index) => ({
              value: version,
              label: `Minecraft ${version}`,
              description: index === 0 ? "Latest release" : "Release",
              recommended: index === 0,
            }))}
            placeholder={props.versionsPending ? "Loading releases…" : "Choose a release"}
            emptyText="No matching Minecraft releases"
            disabled={
              props.mode === "slateClient" || props.versionsPending || props.versionsError
            }
            onValueChange={props.onMinecraftVersionChange}
          />
          {props.versionsError ? (
            <button
              type="button"
              className="mt-2 border-0 bg-transparent p-0 text-[11px] font-bold text-app-danger underline underline-offset-2"
              onClick={props.onRetryVersions}
            >
              Minecraft versions unavailable. Retry
            </button>
          ) : null}
        </div>
        <ComboBox
          label="Loader"
          value={props.loaderKind}
          options={
            props.mode === "vanilla"
              ? [{ value: "vanilla", label: "Vanilla" }]
              : [
                  { value: "fabric", label: "Fabric" },
                  { value: "neoForge", label: "NeoForge" },
                ]
          }
          disabled={props.mode === "vanilla" || props.mode === "slateClient"}
          onValueChange={(value) => props.onLoaderChange(value as LoaderKind)}
        />
        <div>
          <ComboBox
            label="Loader version"
            value={props.selectedLoaderVersion}
            options={props.loaderVersions.map((version) => ({
              value: version,
              label: version,
              description:
                version === props.recommendedLoaderVersion
                  ? `${loaderLabel(props.loaderKind)} recommended`
                  : `${loaderLabel(props.loaderKind)} loader release`,
              recommended: version === props.recommendedLoaderVersion,
            }))}
            placeholder={
              !props.minecraftVersion
                ? "Choose Minecraft first"
                : props.loaderKind === "vanilla"
                  ? "Built into Minecraft"
                  : props.loaderPending
                    ? "Loading compatible versions…"
                    : "Choose a loader version"
            }
            emptyText={props.loaderUnavailableReason ?? "No compatible loader versions"}
            disabled={
              !props.minecraftVersion ||
              props.loaderKind === "vanilla" ||
              props.mode === "slateClient" ||
              props.loaderPending ||
              props.loaderVersions.length === 0
            }
            onValueChange={props.onLoaderVersionChange}
          />
          {props.loaderUnavailableReason && !props.loaderPending ? (
            <p className="mt-2 mb-0 text-[11px]/[16px] text-app-danger">
              {props.loaderUnavailableReason}
            </p>
          ) : null}
        </div>
        <label className={`${labelClass} col-span-2`}>
          Memory limit
          <span className="float-right font-mono text-[11px] font-medium text-app-accent">
            {Math.round(props.memoryMb / 1024)} GB
          </span>
          <input
            className="mt-3 w-full accent-[var(--slate-accent)]"
            type="range"
            min={2048}
            max={16384}
            step={1024}
            value={props.memoryMb}
            onChange={(event) => props.onMemoryChange(Number(event.target.value))}
          />
          <span className="mt-1 flex justify-between font-mono text-[10px] font-medium text-app-muted">
            <span>2 GB</span>
            <span>16 GB</span>
          </span>
        </label>
      </div>
    </>
  );
}

function ReviewStep(props: {
  name: string;
  mode: CreateInstanceInput["mode"];
  minecraftVersion: string;
  loaderKind: LoaderKind;
  loaderVersion?: string;
  memoryMb: number;
}) {
  return (
    <>
      <p className="m-0 text-[11px] font-bold tracking-[.08em] text-app-muted uppercase">
        Step 3 of 3
      </p>
      <h2 className="mt-2 mb-1 text-xl font-bold tracking-[-.025em]">
        Review the profile
      </h2>
      <p className="mt-0 mb-5 text-xs text-app-secondary">
        slate creates the instance and its folders. Downloads begin only when you choose Install.
      </p>
      <dl className="grid grid-cols-2 gap-x-8 gap-y-4 rounded-control border border-app-separator bg-app-bg p-5">
        <ReviewValue label="Name" value={props.name} />
        <ReviewValue label="Experience" value={props.mode} capitalize />
        <ReviewValue label="Minecraft" value={props.minecraftVersion} mono />
        <ReviewValue
          label="Loader"
          value={`${loaderLabel(props.loaderKind)}${props.loaderVersion ? ` ${props.loaderVersion}` : ""}`}
          mono
        />
        <ReviewValue label="Memory" value={`${props.memoryMb} MB`} mono />
      </dl>
      <div className="mt-5">
        <InlineNotice title="Ready to create">
          Create the instance now, then install its game files from the overview.
        </InlineNotice>
      </div>
    </>
  );
}

function ReviewValue({
  label,
  value,
  mono = false,
  capitalize = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
  capitalize?: boolean;
}) {
  return (
    <div>
      <dt className="text-[10px] font-bold tracking-[.07em] text-app-muted uppercase">
        {label}
      </dt>
      <dd
        className={`mt-1 mb-0 text-[13px] font-semibold text-app-text ${mono ? "font-mono text-[12px]" : ""} ${capitalize ? "capitalize" : ""}`}
      >
        {value}
      </dd>
    </div>
  );
}
