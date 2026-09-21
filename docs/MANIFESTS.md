# Manifest contracts

Manifests are versioned data contracts, not informal cache files. Readers reject unsupported
schemas and validate all paths, identifiers, hashes, sizes, and target metadata before mutation.

## Installed revision manifest

Written under a launcher-owned immutable revision directory after base-game and loader installation.
Current schema: `2`.

Required content includes instance/revision IDs, Minecraft version, loader kind/version, resolved
version ID, merged Mojang-compatible metadata layers, managed Java runtime, launch artifact SHA-256
digests, and content artifact SHA-256 digests. The serialized manifest digest is committed with the
database revision. Launch reloads the file, verifies the database digest, and re-hashes planned
artifacts. Schema 1 revisions must be repaired/reinstalled before launch.

Publication is atomic: downloads and native extraction occur in confined staging locations;
verification completes before the final manifest and database pointer become visible.

## Content API install plan

Current schema: `1`. The plan is the server/launcher boundary and contains:

- provider-qualified instance/project/version identity;
- exact Minecraft, loader, loader-version, Java, and memory requirements;
- downloads with relative destination, byte size, cryptographic hashes, ordered safe sources, and
  required/optional state;
- controlled extraction actions and deletion paths;
- total download size.

The launcher rejects wrong targets, unsupported loaders, missing hashes, duplicate/unsafe paths,
and size overflow. It downloads directly from validated provider sources, verifies bytes, stages
changes, and commits them transactionally. A plan never grants the API direct filesystem access.

## Imported pack manifests

CurseForge ZIP `manifest.json` and Modrinth `modrinth.index.json` are untrusted input. The desktop
sends a bounded parsed representation to `/v1/import-plan`; the API normalizes it into install-plan
schema 1. Archive entry names are validated again during extraction. Provider-specific fields do
not enter the persistent public contract.

## Portable slate instance

Current schema: `1`. Export includes display/profile settings, exact runtime target, memory,
lifecycle preferences, and optional provider modpack reference. Credentials, account selection,
absolute local paths, worlds unless explicitly packaged, runtime binaries, caches, and logs are not
portable authority. Import assigns new local IDs and validates settings through normal commands.

## Support report manifest

Current schema: `1`. The archive records selected sections, bounded file counts/bytes, anonymous
runtime compatibility, and bounded crash-finding codes. It excludes credentials, player identity,
absolute paths, raw Minecraft logs/chat, worlds, saves, screenshots, and arbitrary files. Upload is
explicit and the server returns a generated report ID.

## Update manifest

Static channel manifests contain SemVer version, optional notes/date, and a platform map of HTTPS
artifact URL plus signature. The release API emits the Tauri updater shape only after target,
channel, version, URL, signature, and rollout validation. Stable rejects prerelease versions and all
channels reject non-advancing versions.

## IPC schemas and events

IPC schema major is `1`; database schema is `20`. Renderer DTO schemas are generated from
`slate-contracts` into `schemas/ipc` and checked by `xtask`. Event envelopes carry schema version,
source/entity sequencing, revision metadata, kind, and typed payload. A breaking field or enum
change requires a schema/version decision and migration, not a silent overwrite.
