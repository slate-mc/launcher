import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const publicKey = process.env.SLATE_UPDATER_PUBLIC_KEY?.trim();
if (!publicKey || publicKey.length < 32) {
  throw new Error("SLATE_UPDATER_PUBLIC_KEY is required for a signed release.");
}

const endpoint =
  process.env.SLATE_UPDATER_ENDPOINT?.trim() ??
  "https://api.slatelauncher.org/v1/launcher/updates/{{target}}/{{arch}}/{{current_version}}";
const parsedEndpoint = new URL(endpoint.replaceAll(/\{\{[^}]+\}\}/g, "value"));
if (parsedEndpoint.protocol !== "https:") {
  throw new Error("The production updater endpoint must use HTTPS.");
}

const destination = resolve(
  "apps/desktop/src-tauri/tauri.release.generated.json",
);
const configuration = {
  bundle: {
    createUpdaterArtifacts: true,
  },
  plugins: {
    updater: {
      pubkey: publicKey,
      endpoints: [endpoint],
      windows: {
        installMode: "passive",
      },
    },
  },
};

await mkdir(dirname(destination), { recursive: true });
await writeFile(
  destination,
  `${JSON.stringify(configuration, null, 2)}\n`,
  "utf8",
);
console.log(`Wrote signed updater configuration to ${destination}`);
