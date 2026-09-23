# Progress

Last updated: September 22, 2026

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
- Production API configuration now fails closed unless Sentry, PostHog, OTLP export, and private
  support-report storage are configured. A secret-safe preflight and manually dispatched endpoint
  drill validate the deployed health, analytics relay, and support upload paths.
- First-run onboarding guides new players through Minecraft account connection, storage choice,
  managed Java readiness, and first-instance creation. Existing libraries bypass it automatically,
  and a chosen storage location becomes the default for future instances.
- Separate Downloads and Activity destinations. Normal product copy no longer exposes internal IDs,
  paths, backend terminology, or raw Rust/HTTP errors; technical output remains in the Minecraft log
  view where it is useful.
- Route-level loading keeps the initial desktop JavaScript entry near 311 KB; larger Discover and
  instance-management features load only when opened.
- The first Kotlin Slate Client foundation now builds as isolated API, runtime, configuration, HUD,
  diagnostics, profile, standard module, Fabric/NeoForge adapter-contract, and JMH benchmark
  projects. Exact-target compatibility, dependency ordering, rollback on module startup failure,
  compare-and-swap configuration, safe-area HUD layout and undo, bounded diagnostic sanitization,
  broad client presets, and performance/QoL/PvP primitives have unit coverage. PvP is one optional
  preset within the broader Lunar/Dawn-style client platform. Loader adapters intentionally report
  contract-only until real game hooks pass exact-version acceptance.
- The Fabric adapter now builds and remaps a self-contained Minecraft 1.21.1/Fabric Loader 0.19.5
  client mod with a Kotlin entrypoint, bundled shared modules, exact manifest constraints, atomic
  process-bound handshake output, and cleanup on orderly shutdown. Its handshake remains
  `contract_only` until the artifact is installed by the launcher and passes a real-game run.
- The old public `pvp` instance mode is now `slateClient` across Rust, IPC schemas, validation, and
  the desktop UI. Existing local records remain readable through a private legacy storage mapping.
  The creation card describes the full client platform and stays unavailable until launcher-owned
  artifact installation is complete; PvP remains an optional module/preset rather than a mode.
- A Rust launcher/Kotlin handshake contract now parses only bounded regular files and validates the
  schema/protocol, exact Minecraft/loader/Java target, process ID, session freshness, module IDs,
  semantic versions, uniqueness, and accepted adapter status. Contract-only adapters cannot satisfy
  the future active-client gate.

## Verification evidence

Verification evidence current through September 22, 2026:

- Frontend lint passes with zero warnings.
- 22 Vitest/Testing Library tests pass.
- 20 Playwright checks pass. They cover dark/light home snapshots, minimum-window navigation,
  automated WCAG A/AA scans across 16 primary routes, and keyboard/focus behavior for destructive
  confirmation dialogs.
- The Vite/Tailwind production build passes with route-level chunks and no size warning.
- `cargo fmt --all -- --check` passes.
- `cargo test --workspace` passes: 174 tests passed and one process test is ignored.
- `cargo check -p slate-desktop` passes.
- Workspace Clippy passes for all targets and features with warnings denied.
- A clean-root native acceptance executable resolves compatible loader versions and verifies fresh
  Vanilla, Fabric, NeoForge, and optional exact modpack installations through launch planning.
- A live Windows clean-root run passed Minecraft 1.21.1 Vanilla, Fabric, NeoForge, and All the Mods
  10 8.1 (CurseForge `925200:8764211`), installing 3,935 pack content files and verifying 109
  NeoForge launch artifacts on managed Java 21.
- The Gradle `clientCheck` gate passes for all Kotlin Slate Client projects: Spotless/ktlint,
  Detekt, warning-free Kotlin/JVM 21 compilation, JUnit tests, adapter contract tests, dependency
  locks, and JMH benchmark compilation.

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
   feature controls, signed updates, consented crash reporting, private support-report submission,
   and environment-gated backend telemetry are active. Production service configuration and
   validation remain.

## Observability, rollout, and support plan

| Need                                   | Selected approach                              | Remaining work for slate                                                                                                                                                                                                                                                                                                     |
| -------------------------------------- | ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Errors and crashes                     | Sentry                                         | API failures, consented native panics, consented renderer failures, release tags, sanitization, source-map upload, native debug-file upload, anonymous affected-installation grouping, and production configuration enforcement are implemented. Configure the actual project and validate symbolication and alert routing.    |
| Feature flags and remote configuration | PostHog                                        | Allowlisted emergency switches, local percentage rollouts, bounded caching, periodic refresh, and failure-safe defaults are implemented. Configure and exercise production flags, then add ownership and stale-flag cleanup policy.                                                                                          |
| Product analytics                      | PostHog                                        | Explicit opt-in, an allowlisted event contract, lifecycle events, a bounded offline queue, and the server-side PostHog relay are implemented. Configure the production project and finish dashboard/retention validation.                                                                                                    |
| Rust instrumentation                   | `tracing` + `tracing-subscriber`               | Structured JSON tracing now covers API request correlation and desktop install/launch/session lifecycle events. Continue extending fields as features are added.                                                                                                                                                             |
| Backend observability                  | OpenTelemetry to Grafana Cloud                 | Environment-gated OTLP export covers API traces, structured logs, HTTP latency, provider failures/latency, cache outcomes, install plans, and service resources while retaining local JSON output. Production now requires an exporter. Configure credentials, dashboards, alerts, sampling, and retention.                     |
| Local diagnostics                      | Rotating files plus a bounded disk queue       | Daily bounded launcher logs, a bounded non-blocking writer, sanitized user-reviewed report export, and a separate bounded product-event delivery queue are implemented. Crash reports avoid local queuing, and failed support uploads deliberately leave no extra archive behind.                                            |
| Updates                                | Tauri updater plus the slate release API       | Signed release builds, Stable/Beta channels, deterministic staged rollout, an emergency stop percentage, downgrade protection, prerelease isolation, bounded local attempt history, update UI, and multi-platform artifacts are implemented. Configure channel manifests and complete the production rollout/recovery drill. |
| Support reports                        | In-app report flow plus private object storage | User-reviewed local export, explicit bounded submission, private S3-compatible storage, server-only credentials, report IDs, rate limiting, user-safe failures, and a synthetic upload drill are implemented. Configure the production bucket lifecycle and validate operator retrieval, access controls, and expiry.           |

## Quality and documentation gaps

- Extend Playwright from route-level accessibility and destructive-dialog coverage into full native
  install, launch, content-management, storage, and recovery workflows.
- Destructive instance, storage, account, saved-server, snapshot, and force-stop actions now use the
  shared Radix-backed alert dialog with trapped/restored focus, Escape handling, pending-state
  protection, and typed-name validation for permanent instance deletion. Remaining
  popovers/tooltips and screen-reader interaction tests still need coverage.
- Add localization/message catalogs before user-facing copy grows further.
- Live and retained Minecraft logs now virtualize rendered rows while preserving the bounded text
  snapshot. Installed mods use filtering plus 25/50/100-row pagination, and active game activity is
  naturally bounded by the process supervisor. Reassess provider search/result virtualization when
  infinite scrolling replaces the current paged views.
- Continue extending rule-based diagnostics as new crash signatures appear. The current assistant
  covers common Java, memory, dependency, duplicate-mod/package, mixin, damaged-archive, graphics,
  disk, and authentication failures; it links to relevant instance controls and includes bounded
  finding codes in support reports without attaching Minecraft log text.
- Route, game-process, manifest, recovery, companion-module, public API, compatibility, release,
  architecture, IPC, data-model, security, dependency, and testing contracts now describe the
  implemented F1 system. Keep them current as command or persistence contracts change.

## Later phases

- F2 Slate Client foundation is implemented in Kotlin with Google Android Kotlin style and enforced
  formatting/static-analysis/test gates. Loader-neutral lifecycle, HUD layout/editor state,
  performance/accessibility/visual/QoL/PvP module foundations, config reconciliation, diagnostics,
  presets, and benchmark sources are present. Real Fabric/NeoForge 1.21.1 hooks, rendered in-game
  screens, launcher artifact installation/handshake, full module behavior, config persistence, and
  real-game performance/compatibility evidence remain.
- F3 product accounts, communities, publishing, signed releases, cloud sync/backups, operator tools,
  server integration, and cloud support bundles are not started.
- F4 social/party features, practice/replay/recording, cosmetics, creator tools, legacy PvP support,
  and hosted servers remain separately gated initiatives.

## Next executable slice

Implement the first exact-version Fabric 1.21.1 Slate Client adapter and launcher handshake without
changing the contract-only NeoForge status, then prove the adapter in a real-game acceptance run.
