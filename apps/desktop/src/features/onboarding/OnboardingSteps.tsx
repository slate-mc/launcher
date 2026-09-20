import {
  Check,
  CircleUserRound,
  FolderOpen,
  Gamepad2,
  HardDrive,
  LoaderCircle,
} from "lucide-react";
import { ComboBox } from "../../components/ComboBox";
import { MinecraftHead } from "../../components/MinecraftHead";
import type {
  LoaderKind,
  MinecraftAccount,
  Preflight,
} from "../../types/launcher";

export function AccountStep({
  account,
  authState,
  pending,
  error,
  onConnect,
}: {
  account?: MinecraftAccount;
  authState?:
    "waitingForBrowser" | "verifying" | "succeeded" | "failed" | "cancelled";
  pending: boolean;
  error?: string;
  onConnect: () => void;
}) {
  return (
    <StepBody
      icon={<CircleUserRound size={22} />}
      eyebrow="Minecraft account"
      title="Connect the account you play with"
      description="Microsoft sign-in opens in your browser. Once it finishes, you will return here automatically."
    >
      {account ? (
        <div className="flex items-center gap-4 rounded-control border border-app-accent/40 bg-app-accent/5 p-4">
          <MinecraftHead
            skinUrl={account.skinUrl}
            playerName={account.displayName}
            className="size-11 text-sm"
          />
          <span className="min-w-0 flex-1">
            <strong className="block truncate text-sm">
              {account.displayName}
            </strong>
            <span className="mt-1 flex items-center gap-1.5 text-[11px] font-semibold text-app-accent">
              <Check size={13} aria-hidden="true" /> Ready to play
            </span>
          </span>
        </div>
      ) : (
        <div className="rounded-control border border-app-separator bg-app-bg p-5">
          <strong className="block text-sm">Minecraft: Java Edition</strong>
          <p className="mt-1.5 mb-5 max-w-[520px] text-xs/[19px] text-app-secondary">
            Connect a Microsoft account with Minecraft ownership to install and
            launch games.
          </p>
          <button
            type="button"
            className="inline-flex h-10 items-center gap-2 rounded-control bg-app-accent px-4 text-xs font-bold text-app-on-accent disabled:opacity-50"
            disabled={
              pending ||
              authState === "waitingForBrowser" ||
              authState === "verifying"
            }
            onClick={onConnect}
          >
            {pending ||
            authState === "waitingForBrowser" ||
            authState === "verifying" ? (
              <LoaderCircle
                className="animate-spin"
                size={16}
                aria-hidden="true"
              />
            ) : (
              <CircleUserRound size={16} aria-hidden="true" />
            )}
            {authState === "verifying"
              ? "Verifying account…"
              : authState === "waitingForBrowser"
                ? "Waiting for sign-in…"
                : "Sign in with Microsoft"}
          </button>
        </div>
      )}
      {error ? <StepError>{error}</StepError> : null}
    </StepBody>
  );
}

export function StorageStep({
  customSelected,
  pending,
  error,
  onChoose,
}: {
  customSelected: boolean;
  pending: boolean;
  error?: string;
  onChoose: () => void;
}) {
  return (
    <StepBody
      icon={<HardDrive size={22} />}
      eyebrow="Storage"
      title="Choose where your games live"
      description="The recommended location works for most players. You can choose another drive if you need more room."
    >
      <div className="grid grid-cols-2 gap-3">
        <div
          className={`rounded-control border p-4 ${customSelected ? "border-app-separator bg-app-bg" : "border-app-accent bg-app-accent/5"}`}
        >
          <span className="flex items-center justify-between gap-3">
            <strong className="text-sm">Recommended location</strong>
            {!customSelected ? (
              <Check className="text-app-accent" size={17} />
            ) : null}
          </span>
          <p className="mt-2 mb-0 text-[11px]/[17px] text-app-secondary">
            Keeps slate data and Minecraft files together on this device.
          </p>
        </div>
        <div
          className={`rounded-control border p-4 ${customSelected ? "border-app-accent bg-app-accent/5" : "border-app-separator bg-app-bg"}`}
        >
          <span className="flex items-center justify-between gap-3">
            <strong className="text-sm">Another drive or folder</strong>
            {customSelected ? (
              <Check className="text-app-accent" size={17} />
            ) : null}
          </span>
          <p className="mt-2 mb-4 text-[11px]/[17px] text-app-secondary">
            New instances will use the location you choose.
          </p>
          <button
            type="button"
            className="inline-flex h-9 items-center gap-2 rounded-control border border-app-separator bg-app-surface px-3 text-[11px] font-bold text-app-text disabled:opacity-50"
            disabled={pending}
            onClick={onChoose}
          >
            {pending ? (
              <LoaderCircle className="animate-spin" size={15} />
            ) : (
              <FolderOpen size={15} />
            )}
            {customSelected ? "Choose a different folder" : "Choose folder"}
          </button>
        </div>
      </div>
      {error ? <StepError>{error}</StepError> : null}
    </StepBody>
  );
}

export function JavaStep({ preflight }: { preflight?: Preflight }) {
  return (
    <StepBody
      icon={<Gamepad2 size={22} />}
      eyebrow="Game runtime"
      title="Java is handled for you"
      description="slate selects and downloads the right Java version for each Minecraft release, so older and newer instances can coexist."
    >
      <div className="rounded-control border border-app-accent/35 bg-app-accent/5 p-5">
        <span className="flex items-start gap-3">
          <span className="mt-0.5 inline-flex size-8 items-center justify-center rounded-full bg-app-accent text-app-on-accent">
            <Check size={16} aria-hidden="true" />
          </span>
          <span>
            <strong className="block text-sm">Managed automatically</strong>
            <span className="mt-1 block text-xs/[19px] text-app-secondary">
              {preflight?.java.available
                ? `A system Java installation is also available${preflight.java.version ? ` (${preflight.java.version})` : ""}.`
                : "No system Java is required. The first installation will fetch what it needs."}
            </span>
          </span>
        </span>
      </div>
    </StepBody>
  );
}

export function InstanceStep({
  existingName,
  name,
  minecraftVersion,
  minecraftVersions,
  loaderKind,
  loaderVersion,
  loaderVersions,
  loaderPending,
  error,
  onNameChange,
  onMinecraftVersionChange,
  onLoaderKindChange,
  onLoaderVersionChange,
}: {
  existingName?: string;
  name: string;
  minecraftVersion: string;
  minecraftVersions: string[];
  loaderKind: LoaderKind;
  loaderVersion: string;
  loaderVersions: string[];
  loaderPending: boolean;
  error?: string;
  onNameChange: (value: string) => void;
  onMinecraftVersionChange: (value: string) => void;
  onLoaderKindChange: (value: LoaderKind) => void;
  onLoaderVersionChange: (value: string) => void;
}) {
  return (
    <StepBody
      icon={<Gamepad2 size={22} />}
      eyebrow="First instance"
      title={
        existingName
          ? "Your first instance is ready"
          : "Create your first Minecraft setup"
      }
      description={
        existingName
          ? `Continue with ${existingName}, or create more instances from your library later.`
          : "Start clean with vanilla, Fabric, or NeoForge. You can add content and tune performance later."
      }
    >
      {existingName ? (
        <div className="flex items-center gap-3 rounded-control border border-app-accent/35 bg-app-accent/5 p-4">
          <Check className="text-app-accent" size={19} />
          <strong className="text-sm">{existingName}</strong>
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-4">
          <label className="col-span-2 block text-xs font-bold">
            Instance name
            <input
              className="mt-2 h-10 w-full rounded-control border border-app-separator bg-app-bg px-3 text-[13px] text-app-text focus:border-app-accent focus:outline-none"
              value={name}
              maxLength={80}
              onChange={(event) => onNameChange(event.target.value)}
            />
          </label>
          <ComboBox
            label="Minecraft version"
            value={minecraftVersion}
            options={minecraftVersions.map((version) => ({
              value: version,
              label: `Minecraft ${version}`,
            }))}
            onValueChange={onMinecraftVersionChange}
            placeholder="Choose a release"
          />
          <ComboBox
            label="Mod loader"
            value={loaderKind}
            options={[
              {
                value: "vanilla",
                label: "Vanilla",
                description: "No mod loader",
              },
              {
                value: "fabric",
                label: "Fabric",
                description: "Lightweight mod loader",
              },
              {
                value: "neoForge",
                label: "NeoForge",
                description: "Full modding platform",
              },
            ]}
            onValueChange={(value) => onLoaderKindChange(value as LoaderKind)}
          />
          {loaderKind !== "vanilla" ? (
            <div className="col-span-2">
              <ComboBox
                label="Loader version"
                value={loaderVersion}
                options={loaderVersions.map((version, index) => ({
                  value: version,
                  label: version,
                  recommended: index === 0,
                }))}
                onValueChange={onLoaderVersionChange}
                disabled={loaderPending}
                placeholder={
                  loaderPending
                    ? "Finding compatible versions…"
                    : "Choose a loader version"
                }
              />
            </div>
          ) : null}
        </div>
      )}
      {error ? <StepError>{error}</StepError> : null}
    </StepBody>
  );
}

function StepBody({
  icon,
  eyebrow,
  title,
  description,
  children,
}: {
  icon: React.ReactNode;
  eyebrow: string;
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <>
      <span className="inline-flex size-11 items-center justify-center rounded-control bg-app-raised text-app-accent">
        {icon}
      </span>
      <p className="mt-5 mb-0 font-mono text-[10px] font-semibold tracking-[.12em] text-app-accent uppercase">
        {eyebrow}
      </p>
      <h1 className="mt-2 mb-2 max-w-[620px] text-[30px]/[36px] font-bold tracking-[-.035em]">
        {title}
      </h1>
      <p className="mt-0 mb-7 max-w-[650px] text-[13px]/[21px] text-app-secondary">
        {description}
      </p>
      {children}
    </>
  );
}

function StepError({ children }: { children: React.ReactNode }) {
  return (
    <p className="mt-4 mb-0 text-xs font-medium text-app-danger" role="alert">
      {children}
    </p>
  );
}
