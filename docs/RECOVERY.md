# Recovery model

Recovery protects user worlds and personal additions while making launcher-owned state repairable.
SQLite records intent and durable state; journals, immutable manifests, staging directories,
backups, snapshots, trash, and content ownership make filesystem operations recoverable.

## Installation interruption

Install jobs are durable and single-worker ordered. On startup, jobs left queued or running by a
previous launcher process become interrupted failures; jobs for trashed instances become
cancelled. Retry creates a new operation attempt from the recorded request. Partial download files
are removed. Cached artifacts are reused only after hash verification.

Base installation publishes a revision only after all metadata, game artifacts, Java, loader work,
natives, and hashes pass. A failed preparation leaves the prior ready revision selected.

## Content transactions

Modpack, mod, resource-pack, shader-pack, and data-pack changes are staged outside live paths.
Before replacement/removal, affected files move to a bounded backup area and a pending content
transaction is recorded. Commit updates ownership and revision metadata only after every download,
hash, extraction, and filesystem move succeeds. Cancellation or commit failure restores prior
files. Exact installed dependencies are reused; conflicts stop before mutation.

Pack update creates a snapshot first, removes obsolete pack-owned files, preserves user-added
content, and atomically updates provider/runtime metadata. Resource-pack ordering changes only
Minecraft's pack-list keys and preserves unknown game options.

## Snapshots and backups

Users may create, pin, restore, and delete instance snapshots. Automatic snapshots run before
configured pack/runtime changes. Retention never removes pinned snapshots. Restore is an explicit
destructive boundary and should be followed by repair/verification before launch when runtime files
changed outside slate.

SQLite destructive migrations require `VACUUM INTO` backup and must preserve original bytes when a
migration fails. Filesystem work must not occur while holding a database write transaction.

## Trash and permanent deletion

Moving an instance to trash is recoverable and removes it from launch/library queries without
immediately deleting its directory. Storage shows retained entries, approximate size, retention,
restore, and explicit permanent deletion. Permanent deletion requires confirmation and validates
the resolved target under a configured storage root. Active instances cannot move to trash.

Cache cleanup is category-scoped. Managed game/loader/runtime data requires stronger confirmation
than disposable metadata/cache. Cleanup never accepts an arbitrary renderer-supplied path.

## Process and session recovery

On startup, stale session rows are refreshed so the UI does not claim Minecraft is still running.
The current supervisor does not reattach to an orphaned Java process after slate itself exits; see
[GAME_PROTOCOL.md](GAME_PROTOCOL.md). Retained session logs and crash findings remain available for
diagnosis according to per-instance retention.

## User-facing recovery order

1. Retry the recorded operation when the failure is transient.
2. Repair and verify the instance to restore launcher-owned artifacts.
3. Restore the latest appropriate snapshot for a failed content/runtime change.
4. Export a reviewed support report when the evidence is insufficient.
5. Permanently delete only after worlds/personal files are backed up or intentionally discarded.

Every failure path must preserve a stable error code and request/correlation ID internally while
showing the player a short explanation and safe next action.
