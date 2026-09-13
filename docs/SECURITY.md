# Security baseline

## Trust boundaries

- Minecraft mods and loader installers are executable JVM code selected by the player; they are
  not represented as sandboxed.
- Renderer input is untrusted. Native operations must resolve backend-owned IDs and grants and
  revalidate paths, revisions, limits, and current state.
- Network metadata, manifests, archives, images, server descriptions, and deep links are
  untrusted inputs with type-specific size and origin limits.
- Local same-user processes are outside a strong isolation guarantee, so companion handshakes
  are capability authentication rather than anti-cheat attestation.

## Implemented controls

- Managed relative paths reject absolute paths, traversal, empty components, control characters,
  drive/alternate-stream syntax, Windows device names, ambiguous trailing dots/spaces, and
  excessive lengths.
- Launch plans never invoke a shell, clear ambient environment variables, require absolute paths,
  and separate marked secret values from display/export values.
- SQLite enforces foreign keys and constrained state values; account records contain credential
  references rather than tokens.
- Minecraft authentication uses system-browser authorization code with PKCE and a loopback callback.
  Microsoft refresh credentials are stored only in the operating-system vault; short-lived Xbox
  and Minecraft tokens remain in native memory and never cross IPC.
- Network acquisition uses explicit HTTPS origin allowlists, response-size limits, declared hashes
  or trusted sidecar hashes, bounded transient retries, and safe staging paths.
- Rust workspace policy forbids `unsafe` and denies `unwrap`, `expect`, `todo`, and
  `unimplemented` in Clippy checks.

Lexical path validation is not a substitute for filesystem-time canonicalization and symlink,
device, case-collision, and ownership checks. Those checks must be added at the file-operation
boundary before archive import or managed-file mutation is enabled.
