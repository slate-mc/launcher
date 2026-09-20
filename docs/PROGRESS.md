# Progress

Last updated: September 20, 2026

## Working now

- Yarn 4/Corepack workspace with a Tauri 2 desktop app, React 19, strict TypeScript, Vite,
  Tailwind CSS v4, TanStack Query/Router, Zod boundaries, and a multi-crate Rust workspace.
- Dark, light, and system themes using the supplied slate brand assets. System theme and reduced
  motion preferences react to operating-system changes.
- Microsoft/Minecraft public-client PKCE authentication with a stable loopback callback, secure
  credential storage, account refresh/removal, multiple accounts, default account selection, and
  Minecraft skin heads.
- Vanilla, Fabric, and NeoForge version selection, verified installation, compatible managed Java
  acquisition, authenticated launch, duplicate-launch prevention, force stop, last-played tracking,
  and live/retained Minecraft logs.
- Durable installation records with detailed bounded progress, parallel downloads, verified shared
  artifacts, restart recovery for interrupted jobs, and sanitized user-facing failure messages.
- Instance library, favorites, profile text/tags/notes, icon/banner selection and positioning,
  per-instance account/window/language/quick-play/performance/Java settings, common Minecraft game
  options, folder shortcuts, relocation, duplication, portable slate import/export, snapshots,
  recoverable trash, permanent deletion, retention, and storage cleanup.
- Axum modpack API and desktop client for CurseForge, Modrinth, and FTB modpack search, project and
  release details, pagination, normalized install plans, dependency resolution, caching, retries,
  rate limits, structured response envelopes, and health/readiness routes. Development uses
  localhost; production uses `https://api.slatelauncher.org`.
- Discover-to-install modpack flow plus compatible CurseForge/Modrinth mod search inside an
  instance, multi-select installation, required dependency installation, duplicate reuse, installed
  mod resolution, icons, enable/disable, removal, and version pinning.
- Instance content inventory for mods, resource packs, shader packs, and world data packs, including
  pack-vs-user ownership, enable/disable behavior where supported, and recoverable removal.
- Separate Downloads and Activity destinations. Normal product copy no longer exposes internal IDs,
  paths, backend terminology, or raw Rust/HTTP errors; technical output remains in the Minecraft log
  view where it is useful.

## Verification evidence

Verified on Windows on September 20, 2026:

- Frontend lint passes with zero warnings.
- 10 Vitest/Testing Library tests pass.
- The Vite/Tailwind production build passes. It still reports a large initial JavaScript chunk.
- `cargo fmt --all -- --check` passes.
- `cargo test --workspace --all-targets` passes: 112 tests passed and one process test is ignored.
- `cargo check -p slate-desktop` passes.
- Workspace Clippy passes for all targets and features with warnings denied.

## F1 release blockers

1. **Onboarding:** there is no first-run flow for account connection, storage choice, Java readiness,
   and first-instance creation.
2. **Servers:** storage primitives exist, but the desktop still lacks real saved-server CRUD, ping,
   compatibility selection, join, and quick-join history. The current page is intentionally inert.
3. **Content updates:** the API exposes modpack update checks, but the desktop has no end-to-end
   modpack updater. Per-mod update, downgrade, dependency/dependent views, and rollback are also not
   complete; pinning alone is not an updater.
4. **Import coverage:** slate portable archives work, but importing CurseForge/Modrinth packs,
   importing from other launchers, and adding a local JAR through a file picker are not complete.
5. **Download controls:** installation progress and restart recovery work, but user cancellation,
   retry from the Downloads page, queue ordering, pause/resume, and bandwidth controls are missing.
6. **Resource content workflows:** resource packs, shaders, and data packs can be inventoried and
   managed after they exist, but browsing, importing, updating, and ordering them are incomplete.
7. **Distribution:** production packaging, signing, release channels, the Tauri updater backed by
   slate's release API, rollback validation, and uninstall/data-retention behavior have not been
   proven end to end.
8. **Native acceptance:** repeatable clean-machine tests must cover auth plus fresh Vanilla, Fabric,
   NeoForge, and representative large modpack installs/launches on Windows. Interrupted-download and
   recovery scenarios need automated native coverage.
9. **Operational readiness:** crash reporting, privacy-aware product analytics, remote feature
   controls, backend telemetry, bounded offline diagnostics, and the in-app support-report pipeline
   still need production implementations.

## Observability, rollout, and support plan

| Need | Selected approach | Remaining work for slate |
| --- | --- | --- |
| Errors and crashes | Sentry | Integrate desktop and API releases, preserve useful stack traces, sanitize context, and group failures by affected version and user impact. |
| Feature flags and remote configuration | PostHog | Add gradual rollouts, experiments, emergency switches, safe defaults, local caching, and failure-safe behavior when PostHog is unavailable. |
| Product analytics | PostHog | Instrument onboarding, installation, content management, and launch funnels with explicit privacy controls and no secrets or raw diagnostic payloads. |
| Rust instrumentation | `tracing` + `tracing-subscriber` | Standardize structured events and spans across the desktop and API, including request, install, download, and launch correlation. |
| Backend observability | OpenTelemetry to Grafana Cloud | Export API traces, latency, dependency failures, logs, and resource metrics with production sampling and retention policies. |
| Local diagnostics | Rotating files plus a bounded disk queue | Retain useful sanitized evidence across offline periods and crashes without allowing logs or queued telemetry to grow without limit. |
| Updates | Tauri updater plus the slate release API | Ship signed launcher updates, controlled channels and rollouts, rollback protection, and clear recovery behavior. |
| Support reports | In-app report flow plus private object storage | Let users review and submit sanitized diagnostics, upload them securely, and receive a report ID without exposing storage details. |

## Quality and documentation gaps

- Add Playwright end-to-end/visual coverage for dark and light themes, onboarding, install, launch,
  content management, storage, and destructive confirmations.
- Split the roughly 1.03 MB initial frontend bundle with route-level lazy loading.
- Add accessible Radix-backed dialogs/popovers/tooltips where custom controls currently provide only
  visual behavior; complete keyboard and screen-reader testing.
- Add localization/message catalogs before user-facing copy grows further.
- Add virtualization for very large mod, activity, and log views.
- Finish rule-based diagnostics and repair explanations, then connect them to the bounded local
  diagnostics and support-report pipeline above.
- Required specification documents still missing: `ROUTES.md`, `GAME_PROTOCOL.md`, `MANIFESTS.md`,
  `RECOVERY.md`, `CLIENT_MODULES.md`, `API.md`, `PRIVACY.md`, `COMPATIBILITY.md`, `RELEASING.md`, and
  `OPERATIONS.md`. Existing architecture, IPC, testing, security, and dependency docs also need a
  post-F1 implementation refresh.

## Later phases

- F2 Java companion projects, module lifecycle, HUD editor, QoL/PvP modules, config reconciliation,
  diagnostics, benchmarks, and tested Fabric/NeoForge client integration are not started.
- F3 product accounts, communities, publishing, signed releases, cloud sync/backups, operator tools,
  server integration, and cloud support bundles are not started.
- F4 social/party features, practice/replay/recording, cosmetics, creator tools, legacy PvP support,
  and hosted servers remain separately gated initiatives.

## Next executable slice

Finish the F1 server workflow (saved-server CRUD, ping, instance compatibility choice, and join), then
implement desktop modpack updates using the existing `/v1/modpacks/:provider/:project_id/update`
contract. In parallel, establish the structured `tracing` foundation and bounded local diagnostics;
those are prerequisites for useful Sentry, OpenTelemetry, analytics, and support-report integrations.
Add native clean-install smoke coverage so release claims are evidence-based rather than inferred
from unit tests.
