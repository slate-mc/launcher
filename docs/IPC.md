# IPC contracts

IPC schema major version is `1`. JSON fields use `camelCase`; UUIDs serialize as strings and UTC
timestamps use RFC 3339. Rust commands return typed data or a serializable `AppError`; failures are
never embedded in success payloads. The renderer validates native results with Zod before they enter
React state.

`slate-contracts` currently registers 115 deterministic JSON Schema documents under `schemas/ipc`.
They cover:

- bootstrap, preflight, onboarding, preferences, and remote launcher features;
- Microsoft authentication and Minecraft account management;
- instances, settings, artwork, game options, snapshots, import/export, and recoverable trash;
- installation jobs, queue control, resolved runtimes, and update checks;
- running and retained sessions plus bounded live-log events;
- saved servers and status results;
- modpack discovery, versions, install/update requests, installed mods, other content types,
  dependencies, pinning, history, and local imports;
- storage summaries and cleanup requests;
- privacy controls, sanitized support reports, and product telemetry; and
- stable `AppError` and event-envelope shapes.

The Tauri composition root exposes roughly 100 commands, split by account, catalog, instance,
installation, launch, content, server, storage, settings, and support modules. Long-running installs
return durable job IDs. Live session logs use a bounded Tauri channel subscription; retained logs
use bounded snapshots. Mutating requests carry expected instance revisions where concurrent changes
could otherwise overwrite newer state.

Secrets are marked in native launch structures and redacted from display/export forms. Microsoft,
Xbox, and Minecraft tokens never cross IPC. Filesystem commands accept backend-owned instance IDs
and constrained action enums rather than renderer-provided arbitrary paths.

Generate schemas with:

```powershell
cargo run -p xtask -- contracts-generate
```

Verify the checked-in contract set with:

```powershell
cargo run -p xtask -- contracts-check
```

Any field, enum, command behavior, or schema-version change requires compatibility review. Additive
fields should remain optional until every supported desktop version can consume them.

