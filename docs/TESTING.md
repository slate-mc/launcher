# Testing

The suite covers behavior and invariants across Rust, renderer, browser-preview, and selected live
installation boundaries:

- domain state-machine happy path and property coverage for terminal states
- canonical lowercase product identity and UUID wire representation
- instance-name validation
- camelCase IPC serialization and stable schema registration
- managed-path normalization and escape/device-name rejection
- shell-free launch-plan validation and marked-secret redaction
- SQLite migration, WAL/foreign-key configuration, backup creation, FK rejection, persistence,
  and optimistic revision conflict
- deterministic schema generation
- typed renderer-boundary parsing
- launcher UI behavior with typed preview data
- dark/light home visual snapshots and minimum-window navigation
- automated WCAG A/AA scans across primary routes and keyboard/focus behavior for the shared
  destructive confirmation dialog
- installer interruption cleanup (including an aborted active download scope), startup recovery,
  content transactions, provider normalization, API envelopes, rate limits, and compatibility
  validation

Run:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo run -p xtask -- contracts-check
```

Frontend checks:

    corepack yarn lint
    corepack yarn test --run
    corepack yarn test:e2e
    corepack yarn build

Slate Client checks:

```powershell
.\gradlew.bat clientCheck
```

This gate formats Kotlin/Gradle/Java sources, runs Detekt, compiles with warnings denied, executes
JUnit tests across the protocol/runtime/configuration/HUD/modules/adapters, and compiles the JMH
benchmarks. Dependency versions are locked per module.

The ignored native smoke executable performs real Vanilla, Fabric, NeoForge, and optional modpack
downloads, verifies the installed revision, and constructs the complete authenticated launch plan
with a nonfunctional test identity. It does not start Minecraft. Release evidence must separately
cover operating-system credential storage, interactive Microsoft authentication, real process
launch/exit, signed updates, and recovery from an injected native interruption.
