# Installing and launching Minecraft

slate installs Vanilla, Fabric, and NeoForge into an isolated instance layout backed by shared,
content-verified Minecraft artifacts. Installation and launch are native Rust operations; the
renderer sends stable IDs and never constructs paths, Java arguments, or credentials.

## Install pipeline

1. Resolve the selected release from Mojang's signed-hash metadata chain.
2. Evaluate operating-system and architecture rules for libraries and natives.
3. Download the client, libraries, logging configuration, asset index, and asset objects from an
   explicit HTTPS origin allowlist. Existing files are reused only after integrity verification.
4. Retry only transient HTTP failures, with bounded attempts and jitter. Permanent HTTP failures,
   invalid metadata, and integrity mismatches fail immediately.
5. Install the required Eclipse Temurin runtime under
   `<storage-root>/runtimes/java/<major>/<platform-arch>/<release>` and probe its actual version and
   architecture before binding it to the revision.
6. For Fabric, resolve the exact compatible loader profile and its Maven libraries. For NeoForge,
   verify the official installer JAR, run its client processors with the managed Java runtime, and
   validate the generated version metadata and artifacts.
7. Extract natives through a traversal- and link-resistant staging directory, then atomically
   publish the immutable installed-revision manifest. Schema version 2 records a SHA-256 digest for
   every planned launch artifact; the manifest's own digest is committed with the database
   revision.

The installer reports durable stages for metadata, base-game files, assets, Java, loader work,
launch files, natives, verification, and commit. File-backed stages include bounded completed/total
counters. The desktop polls the durable snapshot and renders determinate progress without coupling
the job lifetime to a React component. On startup, jobs left queued or running by a previous
launcher process become interrupted failures that can be retried; jobs belonging to trashed
instances become cancelled.

## Launch pipeline

Launch requires a ready instance and a connected Minecraft account. Rust refreshes the Microsoft,
Xbox, XSTS, and Minecraft session chain, reloads the immutable installed revision, probes its bound
Java runtime, creates a shell-free argument vector, verifies the manifest against its database
digest, and re-hashes every planned launch artifact before starting Java. It then persists the
session and starts the supervised Java child process. Access tokens remain secret-marked native
values and never cross IPC or appear in redacted plans. Revisions installed with an older manifest
schema must be reinstalled before launch.

## Active sessions

The native process supervisor owns the active-session snapshot and enforces one Minecraft process
per instance while allowing different instances to run concurrently. Home, instance details, the
activity page, and the global footer poll that snapshot. A running instance replaces Play with Stop;
the available stop operation is currently an explicit force-close and warns that unsaved world
progress may be lost. Exits are detected with the owned child handle and persisted as exited,
crashed, or user-cancelled. Graceful companion-assisted shutdown and safe process reattachment after
a full launcher-process restart remain later lifecycle work.

## Reproducible smoke validation

The ignored target directory may be reused so large assets and managed runtimes are not downloaded
for every matrix:

```powershell
cargo run -p slate-installer --example install_smoke -- `
  "C:\absolute\path\to\slate-install-smoke" `
  <minecraft-version> <fabric-version> <neoforge-version>
```

On September 13, 2026, the complete install and launch-plan boundary passed on Windows x64 for:

| Minecraft | Vanilla | Fabric | NeoForge | Managed Java |
| --- | --- | --- | --- | --- |
| 26.2 | built-in | 0.19.5 | 26.2.0.88 | 25 |
| 1.21.1 | built-in | 0.19.5 | 21.1.250 | 21 |
| 1.20.2 | built-in | 0.19.5 | 20.2.93 | 17 |

The smoke command performs real downloads and NeoForge processor execution, reloads each installed
manifest, constructs its complete authenticated launch plan with a nonfunctional test identity, and
checks every planned launch artifact against the persisted SHA-256 index. It deliberately does not
start Minecraft.
