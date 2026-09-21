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
- Durable installation records with detailed bounded progress, an ordered single-worker queue,
  parallel file downloads, verified shared artifacts, restart recovery for interrupted jobs,
  pause/resume, safe user cancellation, operation-aware retry from Downloads, an aggregate
  download-speed limit covering base-game and content files, and sanitized user-facing failure
  messages. Partial downloads are removed when interrupted, and applied content remains a pending
  transaction until the installation record commits; cancellation or commit failure restores the
  previous files.
- Instance library, favorites, profile text/tags/notes, icon/banner selection and positioning,
  per-instance account/window/language/quick-play/performance/Java settings, common Minecraft game
  options, folder shortcuts, relocation, duplication, portable slate import/export, CurseForge ZIP
  and Modrinth `.mrpack` import, Prism Launcher/MultiMC, CurseForge app, and ATLauncher instance
  folder import, snapshots, recoverable trash, permanent deletion, retention, and storage cleanup.
- Axum modpack API and desktop client for CurseForge, Modrinth, and FTB modpack search, project and
  release details, pagination, normalized install plans, dependency resolution, caching, retries,
  rate limits, structured response envelopes, and health/readiness routes. Development uses
  localhost; production uses `https://api.slatelauncher.org`.
- Discover-to-install modpack flow plus compatible CurseForge/Modrinth mod search inside an
  instance, multi-select installation, required dependency installation, duplicate reuse, installed
  mod resolution, icons, enable/disable, removal, version pinning, exact-version changes, recent
  version rollback, and persisted dependency/dependent views for added and modpack-managed mods.
- Compatible modpack update checks and transactional in-place updates with automatic snapshots,
  obsolete pack-file removal, preservation of user-added mods and atomic pack/loader version
  changes.
- Instance content inventory for mods, resource packs, shader packs, and world data packs, including
  pack-vs-user ownership, enable/disable behavior where supported, recoverable removal, local mod
  JAR import, local resource/shader/data-pack ZIP import, and compatible Modrinth browsing and batch
  installation. Provider content uses exact Minecraft/runtime matching, explicit world selection for
  data packs, verified downloads, duplicate protection, version pinning, exact-version changes,
  bounded downgrade history, transitive required-dependency installation, snapshots, and rollback.
  Resource packs can be activated, deactivated, and reordered through Minecraft's own pack list
  without discarding built-in packs or unrelated game options. Required loader mods are installed
  into the Mods inventory; installed exact versions are reused and pinned or conflicting
  dependencies fail safely.
- Saved-server create, edit, remove, Java status ping, DNS SRV resolution, validated server icons,
  formatted Minecraft messages, compatible-instance guidance, remembered instance selection, and
  authenticated quick join.
- Structured desktop/API tracing with request correlation, install and session lifecycle events,
  daily JSON launcher diagnostics, a bounded non-blocking event buffer, and automatic age/count/size
  retention.
- Live session output is retained in bounded snapshots and can be reopened from an instance after a
  restart. A rule-based crash assistant recognizes common Java, memory, dependency, duplicate-package,
  and mixin failures and gives short recovery guidance without hiding the technical log.
- Signed launcher-update checks and installation are available in release builds. GitHub Actions
  produces updater artifacts for Windows, Linux, and both macOS architectures, while the release API
  safely adapts the published signed manifest for the desktop updater.
- User-reviewed support reports can package selected launcher diagnostics, installation activity,
  and anonymous compatibility details into a local archive. The exporter removes credentials,
  player identity, and absolute paths and never includes worlds, screenshots, or Minecraft chat.
- Anonymous product analytics are explicit opt-in and use a dedicated Privacy page. Only a fixed
  event schema for setup, installation, content, and launch outcomes is accepted. Events use a
  random installation ID, remain bounded in an offline queue, and are removed when sharing is
  disabled. The API relays configured events to PostHog without placing provider configuration in
  the desktop app.
- Remote feature controls support cached emergency switches for authentication, installs, launch,
  and support reports plus a stable local percentage rollout for UI experiments. Failed or expired
  configuration falls back to working defaults, and rollout assignment does not send the local
  installation identifier to the feature service.
- First-run onboarding guides new players through Minecraft account connection, storage choice,
  managed Java readiness, and first-instance creation. Existing libraries bypass it automatically,
  and a chosen storage location becomes the default for future instances.
- Separate Downloads and Activity destinations. Normal product copy no longer exposes internal IDs,
  paths, backend terminology, or raw Rust/HTTP errors; technical output remains in the Minecraft log
  view where it is useful.
- Route-level loading keeps the initial desktop JavaScript entry near 293 KB; larger Discover and
  instance-management features load only when opened.

## Verification evidence

Verified on Windows on September 20, 2026:

- Frontend lint passes with zero warnings.
- 19 Vitest/Testing Library tests pass.
- Three Playwright checks pass at the minimum supported window size, including dark and light visual
  snapshots of the home shell.
- The Vite/Tailwind production build passes with route-level chunks and no size warning.
- `cargo fmt --all -- --check` passes.
- `cargo test --workspace` passes: 164 tests passed and one process test is ignored.
- `cargo check -p slate-desktop` passes.
- Workspace Clippy passes for all targets and features with warnings denied.
- A clean-root native acceptance executable resolves compatible loader versions and verifies fresh
  Vanilla, Fabric, NeoForge, and optional exact modpack installations through launch planning.
- A live Windows clean-root run passed Minecraft 1.21.1 Vanilla, Fabric, NeoForge, and All the Mods
  10 8.1 (CurseForge `925200:8764211`), installing 3,935 pack content files and verifying 109
  NeoForge launch artifacts on managed Java 21.

## F1 release blockers

1. **Distribution:** the signed Tauri updater, release workflow, and release API are implemented.
   Real production signing credentials, a staged update/rollback exercise, release-channel policy,
   and uninstall/data-retention behavior still need end-to-end evidence.
2. **Native acceptance:** the clean-root install harness and a representative ATM10 run cover fresh
   Vanilla, Fabric, NeoForge, and exact large-modpack installation through verified launch planning.
   Authenticated game-process launch still needs recorded release evidence. Interruption safety now
   has deterministic coverage for partial-download cleanup, startup recovery, and content rollback,
   while an injected mid-download native run remains useful release hardening.
3. **Operational readiness:** structured local tracing, privacy-aware product analytics, remote
   feature controls, signed updates, local support-report export, and environment-gated backend
   telemetry are active. Crash reporting and private support-report submission still need
   production implementations.

## Observability, rollout, and support plan

| Need | Selected approach | Remaining work for slate |
| --- | --- | --- |
| Errors and crashes | Sentry | Integrate desktop and API releases, preserve useful stack traces, sanitize context, and group failures by affected version and user impact. |
| Feature flags and remote configuration | PostHog | Allowlisted emergency switches, local percentage rollouts, bounded caching, periodic refresh, and failure-safe defaults are implemented. Configure and exercise production flags, then add ownership and stale-flag cleanup policy. |
| Product analytics | PostHog | Explicit opt-in, an allowlisted event contract, lifecycle events, a bounded offline queue, and the server-side PostHog relay are implemented. Configure the production project and finish dashboard/retention validation. |
| Rust instrumentation | `tracing` + `tracing-subscriber` | Structured JSON tracing now covers API request correlation and desktop install/launch/session lifecycle events. Continue extending fields as features are added. |
| Backend observability | OpenTelemetry to Grafana Cloud | Environment-gated OTLP export now covers API traces, structured logs, HTTP latency, provider failures/latency, cache outcomes, install plans, and service resources while retaining local JSON output. Configure production credentials, dashboards, alerts, sampling, and retention. |
| Local diagnostics | Rotating files plus a bounded disk queue | Daily bounded launcher logs, a bounded non-blocking writer, sanitized user-reviewed report export, and a separate bounded product-event delivery queue are implemented. Crash/support delivery still needs its own consented queue. |
| Updates | Tauri updater plus the slate release API | Signed release builds, update UI, multi-platform artifact workflow, and the release endpoint are implemented. Add controlled channels, staged rollout, rollback protection, and recorded recovery evidence. |
| Support reports | In-app report flow plus private object storage | Local review/export and report IDs are implemented. Add authenticated short-lived uploads to private object storage without exposing storage details. |

## Quality and documentation gaps

- Extend the initial Playwright dark/light and minimum-window coverage to onboarding, install,
  launch, content management, storage, and destructive confirmations.
- Add accessible Radix-backed dialogs/popovers/tooltips where custom controls currently provide only
  visual behavior; complete keyboard and screen-reader testing.
- Add localization/message catalogs before user-facing copy grows further.
- Add virtualization for very large mod, activity, and log views.
- Finish rule-based diagnostics and repair explanations, then include their sanitized results in
  the support-report pipeline above.
- Required specification documents still missing: `ROUTES.md`, `GAME_PROTOCOL.md`, `MANIFESTS.md`,
  `RECOVERY.md`, `CLIENT_MODULES.md`, `API.md`, `PRIVACY.md`, `COMPATIBILITY.md`, and
  `RELEASING.md`. Existing architecture, IPC, testing, security, and dependency docs also need a
  post-F1 implementation refresh.

## Later phases

- F2 Java companion projects, module lifecycle, HUD editor, QoL/PvP modules, config reconciliation,
  diagnostics, benchmarks, and tested Fabric/NeoForge client integration are not started.
- F3 product accounts, communities, publishing, signed releases, cloud sync/backups, operator tools,
  server integration, and cloud support bundles are not started.
- F4 social/party features, practice/replay/recording, cosmetics, creator tools, legacy PvP support,
  and hosted servers remain separately gated initiatives.

## Next executable slice

Add an authenticated Windows process-launch acceptance path, then extend the clean-root harness with
an injected mid-download process interruption for end-to-end recovery evidence.
