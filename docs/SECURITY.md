# Security baseline

## Trust boundaries

- Minecraft mods and loader installers are executable JVM code selected by the player; slate does
  not claim to sandbox them.
- Renderer input is untrusted. Native commands resolve backend-owned IDs and revalidate paths,
  revisions, limits, compatibility, and current state.
- Provider metadata, archives, images, markdown, server responses, and deep links are untrusted.
- The public content API has no meaningful desktop secret. Abuse controls rely on bounded input,
  rate limits, caching, trusted upstreams, and endpoint-specific policy.
- Local same-user processes are outside a strong isolation guarantee. Process/session tracking is
  lifecycle control, not anti-cheat attestation.

## Identity and secrets

Microsoft authentication uses authorization code with PKCE and a fixed loopback callback for the
registered public client. No Microsoft client secret ships in the application. Refresh tokens are
stored in the operating-system credential vault. Xbox and Minecraft access tokens stay in native
memory, are marked secret in launch plans, and never cross IPC, logs, support reports, or analytics.

The desktop contains no CurseForge or provider credential. It calls the slate content API, whose
provider adapters and any future upstream credentials remain server-side. Sentry ingestion DSNs are
public identifiers; upload/auth tokens for source maps, telemetry collectors, support storage, and
release signing exist only in CI or server secret stores.

## Filesystem and process controls

- Managed paths reject absolute paths, traversal, control characters, drive/alternate-stream
  syntax, Windows device names, ambiguous trailing characters, and excessive lengths.
- Imports canonicalize selected roots, reject symlink roots and entries, enforce entry/count/size
  bounds, validate archive paths, and extract only into staged managed destinations.
- Instance content enumeration and mutation reject symlinks and unexpected file types.
- Downloads use explicit HTTPS origin allowlists, response limits, staging files, preferred
  SHA-512/SHA-256/SHA-1 verification, alternate-source retry, and deletion on mismatch.
- Launch planning never invokes a shell. Child environments are rebuilt from an allowlisted base,
  executable paths are absolute, and arguments remain separate values.
- One supervised child is allowed per instance. Stop, affinity, priority, and log paths are owned by
  native state rather than renderer-provided process IDs or paths.

## Network and content controls

The API never accepts an arbitrary fetch URL. Provider URLs must originate in trusted responses and
pass HTTPS host policy; redirect destinations are checked again. Install destinations use normalized
relative paths. Provider HTML/markdown is sanitized before rendering. Server status parsing bounds
packet, text, favicon, and formatting data.

API responses use stable public errors and request correlation without exposing raw Reqwest, Axum,
filesystem, database, or provider failures. Rate limits apply by endpoint class and IP. Cache keys
are normalized and do not contain credentials.

## Diagnostics and privacy

Telemetry and desktop crash reporting are off until explicitly enabled. Event names and properties
are allowlisted and queues are bounded. Crash filters remove request data, identity, paths, local
variables, breadcrumbs, and secret-shaped values. Support reports are generated locally, shown to
the user before submission, bounded, sanitized, and uploaded only by explicit action to private
expiring storage.

Rust forbids `unsafe` and Clippy denies `unwrap`, `expect`, `todo`, and `unimplemented`. Release
artifacts and updater manifests are signed; production readiness still requires operational drills
for signing custody, rollback, symbolication, bucket expiry/access, and alert routing.
