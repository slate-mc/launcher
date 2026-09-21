# Architecture

## System boundary

slate is split into a native launcher, a sandboxed renderer, a public content API, and the JVM
processes it installs and supervises. Rust owns persistence, authentication, downloads,
installation, Java selection, launch planning, filesystem mutation, recovery, and process
ownership. React owns presentation and sends typed requests through Tauri IPC. Minecraft and its
loaders remain separate Java processes.

The renderer never receives database, arbitrary filesystem, shell, operating-system credential,
or provider-secret authority. SQLite and revision manifests are authoritative for installed state;
React Query is only a view cache. The modpack API decides what content belongs in an install plan,
while the launcher validates and applies that plan to disk.

## Workspace ownership

- `slate-domain`: stable identifiers, names, loader families, and lifecycle policies.
- `slate-platform`: application paths, managed relative paths, Java discovery, and restricted child
  environments.
- `slate-modpack-api-contracts`: versioned HTTP request, response, envelope, content, and install-plan
  types shared by the API and client.
- `slate-contracts`: renderer-safe Tauri DTOs and the JSON Schema registry.
- `slate-auth`: Microsoft PKCE, Xbox/Minecraft token exchange, profile validation, skin lookup, and
  operating-system credential-vault access.
- `slate-minecraft`: Mojang metadata, version inheritance, artifacts, rules, and shell-free launch
  planning.
- `slate-loaders`: Fabric metadata and NeoForge installer/profile adaptation.
- `slate-installer`: staged downloads, hash verification, Java acquisition, loader installation,
  content transactions, immutable installed manifests, and recovery.
- `slate-storage`: SQLite migrations and repositories for accounts, instances, jobs, content,
  settings, snapshots, sessions, servers, telemetry, and remote feature configuration.
- `slate-process`: single-process-per-instance supervision, stop/refresh state, retained logs, and
  bounded log tailing.
- `slate-modpack-client`: typed, timeout-bounded client for the public content API.
- `slate-desktop`: the only Tauri-aware crate; it composes every native capability and exposes the
  IPC facade.
- `slate-modpack-api`: Axum service for normalized provider discovery, resolution, install plans,
  telemetry relay, support reports, and launcher releases.
- `xtask`: deterministic IPC schema generation and checks.

## Dependency direction

```text
domain ───────┬──────────────> platform
              ├──────────────> contracts <──── modpack-api-contracts
              └──────────────> storage  <──── modpack-api-contracts

platform + domain + contracts ────────> minecraft <──── loaders
minecraft + loaders + platform +
modpack-api-contracts ────────────────> installer
minecraft + domain ──────────────────> process
modpack-api-contracts ────────────────> modpack-client

all launcher crates ─────────────────> slate-desktop (composition root)
modpack-api-contracts + minecraft ───> slate-modpack-api
```

Product crates do not depend on Tauri. Provider-specific mapping stays behind API provider
adapters. Tauri command modules may coordinate repositories and product crates, but should move
reusable policies back into the owning crate instead of growing the composition root.

## Runtime flows

Installation is a durable job: create a revision, resolve metadata, acquire verified artifacts and
the matching Java runtime, stage content, verify launch requirements, commit the content
transaction, then atomically make the revision current. Startup recovery reconciles interrupted
jobs and incomplete staging without treating partial data as ready.

Launch refreshes the selected Microsoft credential from the OS vault, validates the installed
manifest and Java binding, builds a secret-aware launch plan, verifies every required artifact,
creates a starting session record, and hands the command to the process supervisor. The supervisor
prevents a second process for the same instance, streams stdout/stderr into a retained session log,
and records exit or forced-stop state.

The desktop is single-instance. A second operating-system launch restores and focuses the existing
window instead of opening another process against the same database, install scheduler, and game
process registry.

Content changes use instance revision guards and content transactions. Modpack-managed and
user-added files remain distinguishable; dependency edges and history support updates, rollback,
and duplicate-dependency suppression.

## Frontend composition

Routes are lazy-loaded into Home, Library, Discover, Servers, system pages, settings, onboarding,
and instance pages. Features call small bridge modules grouped by concern. Runtime validation with
Zod happens at the bridge boundary. Shared primitives own artwork fallback, markdown sanitization,
comboboxes, install progress, virtualized logs, notices, and destructive confirmation behavior.
