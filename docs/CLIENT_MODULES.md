# Java companion module boundary

This document reserves the protocol and ownership boundary for F2 Java companion modules. No
companion module is shipped by the current F1 launcher, and the launcher must not represent module
controls as available until a verified module artifact and compatible adapter exist.

## Ownership

The Rust launcher owns module selection, catalog resolution, verified download, hash/signature
validation, per-instance configuration, enablement state, compatibility checks, rollback, and
launch-time handoff. Java owns game hooks, native Minecraft screens, HUD rendering, keybinds, and
loader/version-specific behavior. React may edit typed configuration but never writes a game JAR,
mods directory, or module config directly.

## Planned package shape

```text
module catalog
  -> module identity and semantic version
  -> supported Minecraft versions and loader adapters
  -> signed artifact plus cryptographic hashes
  -> configuration schema and safe defaults
  -> capabilities and required dependencies

instance module state
  -> enabled/pinned version
  -> validated configuration revision
  -> last known compatible adapter
  -> rollback reference
```

A common API artifact must contain no Minecraft implementation classes. Fabric and NeoForge
adapters depend on that API and the matching loader toolchain; they may not be treated as the same
loader. Version-specific hooks live behind adapter interfaces so supporting a new Minecraft release
does not silently alter older instances.

## Installation and launch

Module changes use the same transactional content machinery as other managed content: resolve an
exact compatible artifact, verify signature/hash, stage, snapshot when configured, atomically
commit ownership, and retain rollback metadata. Launch includes only modules enabled for the exact
instance target. A module failure must not expose account tokens or weaken base launch verification.

## Configuration reconciliation

Configuration has a schema version and immutable revision. Unknown fields are preserved when safe;
removed/renamed fields require an explicit migration. Renderer edits use compare-and-swap revision
guards. Java reports applied configuration and bounded diagnostics so Rust can detect drift without
scraping arbitrary log text.

HUD position, scale, anchors, safe-area behavior, and per-screen visibility belong to a typed layout
model. The editor must preview actual coordinate rules, support keyboard operation, and preserve a
recoverable previous revision.

## Compatibility and safety gates

- exact Minecraft and loader adapter match;
- supported module/API protocol version;
- verified signed artifact and cryptographic hash;
- deterministic dependency set without duplicate packages/classes;
- bounded configuration and diagnostics payloads;
- no arbitrary native library, process execution, URL fetch, or filesystem path authority;
- tested startup, shutdown, disconnect, world change, and crash behavior on Fabric and NeoForge.

## F2 completion evidence

F2 is not complete until the common API and both loader adapters build reproducibly, install through
the launcher, exchange a versioned handshake, apply/reconcile settings, render and edit HUD/QoL/PvP
modules, survive update/rollback, and pass performance plus real-game compatibility tests across the
supported matrix. Until then, capability reporting remains unavailable rather than simulated.
