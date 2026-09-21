# Game process protocol

This protocol defines how slate turns a ready instance and Minecraft account into one supervised
Java process. The renderer requests operations by stable IDs; Rust owns credentials, paths,
arguments, files, process handles, and durable state.

## Preconditions

A launch is accepted only when the instance exists outside trash, has a committed ready revision,
has no active process, and references a usable Minecraft account. Rust refreshes the Microsoft,
Xbox Live, XSTS, and Minecraft token chain when necessary. OAuth access and refresh tokens remain
in the operating-system credential vault and never cross IPC.

The installed revision manifest must use the current schema, match its database digest, and pass a
fresh hash verification for every launch artifact. The selected managed, detected, or custom Java
runtime is probed for major version and architecture before process creation.

## Launch sequence

```text
renderer launch request
  -> resolve instance and account
  -> reject an existing active session
  -> refresh Minecraft identity
  -> load and verify installed revision
  -> resolve Java and native directory
  -> build shell-free JVM/game argument arrays
  -> persist starting session
  -> spawn owned Java child
  -> publish running session snapshot
  -> capture combined stdout/stderr
  -> classify and persist exit
```

Arguments are constructed as an executable plus arrays, never as a shell command. Secret-marked
values may enter the child environment/arguments but redacted launch plans replace them before
display, export, diagnostics, or logging. Instance settings may add validated JVM arguments and a
small allowlisted environment map; they cannot override classpath, natives, identity, Java, slate
internals, or operating-system path variables.

## State and concurrency

The lifecycle is `queued -> preparing -> downloading? -> verifying -> ready -> starting -> running
-> exited|crashed`. Installation state and game-process state are separate. One Minecraft process
is allowed per instance, while different instances may run concurrently.

The process supervisor is authoritative while slate is running. Home, instance Overview, Activity,
Servers, and the footer consume the same active-session snapshot. Force stop is explicit and may
lose unsaved game progress. A normal zero exit is `exited`; a nonzero unexpected exit is `crashed`;
a slate-requested termination is user-cancelled.

## Logs and events

Each session has a launcher-owned log file under the instance log directory. The renderer
subscribes with only a session ID. Rust resolves the path, sends at most the latest 256 KiB, then
streams bounded 32 KiB append chunks. The renderer retains at most 250,000 characters and may clear
only its local view. Complete retained logs remain subject to the instance log-retention setting.

Session records store timestamps, exit classification, and bounded sequenced events. Last played is
derived from persisted session start time, not renderer state. The crash assistant recognizes a
bounded set of signatures and returns safe diagnostic codes and actions without uploading or
embedding raw Minecraft logs in support reports.

## Quick join

A saved server launch first selects an instance, then follows the normal launch protocol with a
validated quick-play server value. Server compatibility is advisory as described in
[COMPATIBILITY.md](COMPATIBILITY.md); it never bypasses instance/runtime verification.

## Restart behavior

slate currently owns child handles only for its process lifetime. It persists sessions and repairs
stale active records after restart, but does not claim safe process reattachment. Companion-assisted
graceful shutdown and verified cross-restart reattachment are future protocol extensions and must
be versioned before use.
