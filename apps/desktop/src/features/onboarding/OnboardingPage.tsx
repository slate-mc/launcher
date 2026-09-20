import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { ArrowLeft, ArrowRight, Check, LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";
import {
  completeOnboarding,
  createInstance,
  getLoaderVersionCatalog,
  getMinecraftAuthStatus,
  getMinecraftVersionCatalog,
  getOnboardingState,
  getPreflight,
  listAccounts,
  listInstances,
  selectOnboardingStorage,
  startMinecraftAuth,
} from "../../lib/bridge";
import { getUserFacingError, UserFacingError } from "../../lib/userFacingError";
import { createInstanceSchema, type LoaderKind } from "../../types/launcher";
import {
  AccountStep,
  InstanceStep,
  JavaStep,
  StorageStep,
} from "./OnboardingSteps";

const steps = ["Account", "Storage", "Java", "First instance"] as const;

export function OnboardingPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [step, setStep] = useState(0);
  const [flowId, setFlowId] = useState<string>();
  const [name, setName] = useState("My Minecraft");
  const [minecraftVersion, setMinecraftVersion] = useState("");
  const [loaderKind, setLoaderKind] = useState<LoaderKind>("vanilla");
  const [loaderSelection, setLoaderSelection] = useState({
    key: "",
    value: "",
  });
  const [validationError, setValidationError] = useState<string>();

  const onboardingQuery = useQuery({
    queryKey: ["onboarding"],
    queryFn: getOnboardingState,
  });
  const accountsQuery = useQuery({
    queryKey: ["minecraft-accounts"],
    queryFn: listAccounts,
  });
  const preflightQuery = useQuery({
    queryKey: ["preflight"],
    queryFn: getPreflight,
  });
  const instancesQuery = useQuery({
    queryKey: ["instances"],
    queryFn: listInstances,
  });
  const versionsQuery = useQuery({
    queryKey: ["minecraft-version-catalog"],
    queryFn: getMinecraftVersionCatalog,
    staleTime: 15 * 60_000,
  });
  const selectedMinecraftVersion =
    minecraftVersion || versionsQuery.data?.latestRelease || "";
  const loaderQuery = useQuery({
    queryKey: ["loader-version-catalog", selectedMinecraftVersion, loaderKind],
    queryFn: () =>
      getLoaderVersionCatalog({
        minecraftVersion: selectedMinecraftVersion,
        loaderKind,
      }),
    enabled: Boolean(selectedMinecraftVersion) && loaderKind !== "vanilla",
    staleTime: 15 * 60_000,
  });
  const authStatusQuery = useQuery({
    queryKey: ["minecraft-auth", flowId],
    queryFn: () => getMinecraftAuthStatus(flowId ?? ""),
    enabled: Boolean(flowId),
    refetchInterval: (query) => {
      const state = query.state.data?.state;
      return state === "succeeded" ||
        state === "failed" ||
        state === "cancelled"
        ? false
        : 800;
    },
  });

  useEffect(() => {
    if (authStatusQuery.data?.state === "succeeded") {
      void Promise.all([
        queryClient.invalidateQueries({ queryKey: ["minecraft-accounts"] }),
        queryClient.invalidateQueries({ queryKey: ["preflight"] }),
      ]);
    }
  }, [authStatusQuery.data?.state, queryClient]);

  const connectMutation = useMutation({
    mutationFn: startMinecraftAuth,
    onSuccess: (flow) => setFlowId(flow.flowId),
  });
  const storageMutation = useMutation({
    mutationFn: selectOnboardingStorage,
    onSuccess: (state) => queryClient.setQueryData(["onboarding"], state),
  });
  const finishMutation = useMutation({
    mutationFn: async () => {
      const existing = instancesQuery.data?.[0];
      const instance = existing ?? (await createFirstInstance());
      const onboarding = await completeOnboarding();
      return { instance, onboarding };
    },
    onSuccess: async ({ instance, onboarding }) => {
      queryClient.setQueryData(["onboarding"], onboarding);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["instances"] }),
        queryClient.invalidateQueries({ queryKey: ["preflight"] }),
      ]);
      await navigate({
        to: "/instances/$instanceId/overview",
        params: { instanceId: instance.id },
        replace: true,
      });
    },
  });

  const readyAccount = accountsQuery.data?.find(
    (account) => account.status === "ready",
  );
  const catalogKey = `${selectedMinecraftVersion}|${loaderKind}`;
  const loaderVersions = loaderQuery.data?.versions ?? [];
  const loaderVersion =
    loaderKind === "vanilla"
      ? ""
      : loaderSelection.key === catalogKey &&
          loaderVersions.includes(loaderSelection.value)
        ? loaderSelection.value
        : (loaderQuery.data?.recommendedVersion ?? loaderVersions[0] ?? "");
  const error = getStepError();

  function getStepError() {
    if (step === 0) {
      if (connectMutation.isError)
        return getUserFacingError(
          connectMutation.error,
          "Sign-in could not start. Try again.",
        );
      if (authStatusQuery.data?.state === "failed")
        return (
          authStatusQuery.data.userMessage ??
          "Sign-in did not finish. Try again."
        );
    }
    if (step === 1 && storageMutation.isError)
      return getUserFacingError(
        storageMutation.error,
        "That folder could not be used. Choose another one.",
      );
    if (step === 3 && finishMutation.isError)
      return getUserFacingError(
        finishMutation.error,
        "Your instance could not be created. Check the selections and try again.",
      );
    return validationError;
  }

  async function createFirstInstance() {
    const parsed = createInstanceSchema.safeParse({
      name,
      mode: loaderKind === "vanilla" ? "vanilla" : "modded",
      minecraftVersion: selectedMinecraftVersion,
      loaderKind,
      loaderVersion: loaderKind === "vanilla" ? undefined : loaderVersion,
      memoryMb: 4096,
    });
    if (!parsed.success)
      throw new UserFacingError(
        parsed.error.issues[0]?.message ?? "Review the instance details.",
      );
    return createInstance(parsed.data);
  }

  function continueForward() {
    setValidationError(undefined);
    if (step === 0 && !readyAccount) {
      setValidationError("Connect a Minecraft account before continuing.");
      return;
    }
    if (step === 3) {
      if (
        !instancesQuery.data?.length &&
        loaderKind !== "vanilla" &&
        !loaderVersion
      ) {
        setValidationError(
          loaderQuery.data?.unavailableReason ??
            "Choose a compatible loader version.",
        );
        return;
      }
      finishMutation.mutate();
      return;
    }
    setStep((current) => Math.min(3, current + 1));
  }

  return (
    <div className="grid h-full min-h-[640px] min-w-[960px] grid-cols-[300px_minmax(0,1fr)] bg-app-bg text-app-text">
      <aside className="relative flex flex-col overflow-hidden border-r border-app-separator/65 bg-app-sidebar p-9">
        <div className="absolute -top-24 -left-32 size-[420px] rounded-full bg-app-accent/10 blur-3xl" />
        <img
          className="brand-lockup-paper relative w-[126px]"
          src="/brand/slate-lockup-paper.svg"
          alt="slate"
        />
        <img
          className="brand-lockup-graphite relative hidden w-[126px]"
          src="/brand/slate-lockup-graphite.svg"
          alt="slate"
        />
        <div className="relative mt-auto mb-auto">
          <p className="font-mono text-[10px] font-semibold tracking-[.14em] text-app-accent uppercase">
            Welcome to slate
          </p>
          <h2 className="mt-3 mb-8 text-[25px]/[31px] font-bold tracking-[-.035em]">
            A few choices, then Minecraft.
          </h2>
          <ol
            className="m-0 grid list-none gap-1 p-0"
            aria-label="Setup progress"
          >
            {steps.map((label, index) => (
              <li
                key={label}
                className={`flex min-h-11 items-center gap-3 border-l-2 px-4 text-xs font-bold ${index === step ? "border-app-accent bg-app-accent/6 text-app-text" : index < step ? "border-transparent text-app-secondary" : "border-transparent text-app-muted"}`}
              >
                <span
                  className={`inline-flex size-6 items-center justify-center rounded-full border font-mono text-[10px] ${index <= step ? "border-app-accent/60" : "border-app-separator"}`}
                >
                  {index < step ? <Check size={12} /> : index + 1}
                </span>
                {label}
              </li>
            ))}
          </ol>
        </div>
        <p className="relative m-0 text-[10px]/[16px] text-app-muted">
          You can change these choices later in Settings.
        </p>
      </aside>

      <main className="flex min-w-0 flex-col overflow-y-auto">
        <div className="mx-auto flex w-full max-w-[820px] flex-1 items-center px-14 py-12">
          <section className="w-full rounded-dialog border border-app-separator/65 bg-app-surface p-8 shadow-[0_24px_80px_rgba(0,0,0,.14)]">
            {step === 0 ? (
              <AccountStep
                account={readyAccount}
                authState={authStatusQuery.data?.state}
                pending={connectMutation.isPending}
                error={error}
                onConnect={() => connectMutation.mutate()}
              />
            ) : step === 1 ? (
              <StorageStep
                customSelected={
                  onboardingQuery.data?.customStorageSelected ?? false
                }
                pending={storageMutation.isPending}
                error={error}
                onChoose={() => storageMutation.mutate()}
              />
            ) : step === 2 ? (
              <JavaStep preflight={preflightQuery.data} />
            ) : (
              <InstanceStep
                existingName={instancesQuery.data?.[0]?.name}
                name={name}
                minecraftVersion={selectedMinecraftVersion}
                minecraftVersions={(versionsQuery.data?.versions ?? [])
                  .filter((version) => version.kind === "release")
                  .map((version) => version.id)}
                loaderKind={loaderKind}
                loaderVersion={loaderVersion}
                loaderVersions={loaderVersions}
                loaderPending={loaderQuery.isPending || loaderQuery.isFetching}
                error={error}
                onNameChange={setName}
                onMinecraftVersionChange={(value) => {
                  setMinecraftVersion(value);
                  setLoaderSelection({ key: "", value: "" });
                }}
                onLoaderKindChange={(value) => {
                  setLoaderKind(value);
                  setLoaderSelection({ key: "", value: "" });
                }}
                onLoaderVersionChange={(value) =>
                  setLoaderSelection({ key: catalogKey, value })
                }
              />
            )}

            <div className="mt-8 flex items-center justify-between border-t border-app-separator/55 pt-5">
              <button
                type="button"
                className="inline-flex h-10 items-center gap-2 rounded-control border border-app-separator bg-app-bg px-4 text-xs font-bold text-app-secondary disabled:opacity-35"
                disabled={step === 0 || finishMutation.isPending}
                onClick={() => {
                  setValidationError(undefined);
                  setStep((current) => Math.max(0, current - 1));
                }}
              >
                <ArrowLeft size={15} /> Back
              </button>
              <button
                type="button"
                className="inline-flex h-10 items-center gap-2 rounded-control bg-app-accent px-5 text-xs font-bold text-app-on-accent disabled:opacity-50"
                disabled={
                  finishMutation.isPending ||
                  (step === 0 && accountsQuery.isPending)
                }
                onClick={continueForward}
              >
                {finishMutation.isPending ? (
                  <LoaderCircle className="animate-spin" size={15} />
                ) : null}
                {step === 3
                  ? instancesQuery.data?.length
                    ? "Finish setup"
                    : "Create and continue"
                  : "Continue"}
                {!finishMutation.isPending ? <ArrowRight size={15} /> : null}
              </button>
            </div>
          </section>
        </div>
      </main>
    </div>
  );
}
