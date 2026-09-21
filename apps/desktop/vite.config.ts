import { sentryVitePlugin } from "@sentry/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const sentryBuild = Boolean(
  process.env.SENTRY_AUTH_TOKEN &&
    process.env.SENTRY_ORG &&
    process.env.SENTRY_PROJECT,
);
const release = `slate-desktop@${process.env.npm_package_version ?? "0.1.0"}`;

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    sentryBuild
      ? sentryVitePlugin({
          authToken: process.env.SENTRY_AUTH_TOKEN,
          org: process.env.SENTRY_ORG,
          project: process.env.SENTRY_PROJECT,
          release: { name: release },
          sourcemaps: {
            assets: "./dist/**",
            filesToDeleteAfterUpload: ["./dist/**/*.map"],
          },
          telemetry: false,
        })
      : undefined,
  ],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    sourcemap: sentryBuild ? "hidden" : false,
  },
});
