# slate

slate is a Minecraft Java Edition launcher and full in-game client platform focused on recoverable
installations, understandable failures, reproducible setups, and a polished customizable game
experience. Like Lunar Client or Dawn Client, Slate Client spans performance, HUD, accessibility,
visual, quality-of-life, profile, diagnostics, creator, and PvP features; PvP is not its sole
identity.

The repository contains the Rust foundation, loader-aware Minecraft launch planning, and the
first Tauri 2 desktop shell. The launcher renderer is React 19, strict TypeScript, Vite, and
Tailwind CSS v4. Local instance data crosses a small typed Tauri command facade backed by SQLite.

## Workspace

- apps/desktop — React launcher and Tauri composition root
- services/modpack-api — normalized provider, install-plan, release, telemetry, and support API
- crates/domain — stable IDs and pure business/state policies
- crates/contracts — versioned renderer-safe IPC DTOs and schema registry
- crates/client-protocol — bounded, exact-target launcher/Kotlin handshake validation
- crates/modpack-api-contracts — versioned public HTTP contract shared by API and launcher
- crates/platform — application paths and managed relative-path validation
- crates/storage — SQLite connection policy, migrations, and repositories
- crates/auth — Microsoft PKCE, Minecraft identity, and operating-system credential vault
- crates/minecraft — Mojang metadata, inheritance, artifacts, and shell-free launch planning
- crates/loaders — Fabric profile and NeoForge installer metadata adapters
- crates/installer — verified downloads, Java runtimes, loader installation, and recovery
- crates/process — game-process ownership, stop state, and bounded log streaming
- crates/modpack-client — typed launcher client for the public content API
- tools/xtask — deterministic contract schema generation and checks
- client — Kotlin Slate Client protocol, runtime, HUD, modules, diagnostics, profiles, loader
  adapter contracts, and benchmarks
- schemas/ipc — generated JSON Schemas checked into source control
- docs — architecture, design, security, data, testing, decisions, and progress

Minecraft authentication registration and credential-handling details are documented in
[`docs/AUTHENTICATION.md`](docs/AUTHENTICATION.md).

## Development

Rust 1.95.0 and Yarn 4.18.0 are pinned. Corepack selects the repository's Yarn version.

    corepack yarn install
    corepack yarn dev
    corepack yarn tauri dev

Development uses an always-visible preview-data label and cannot claim to execute game actions.
Set `VITE_DATA_ADAPTER=native` to exercise the real Tauri facade during development. Packaged Tauri
builds read the real local instance database.

Run all checks:

    corepack yarn lint
    corepack yarn test
    corepack yarn build
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features
    cargo run -p xtask -- contracts-check
    .\gradlew.bat clientCheck
