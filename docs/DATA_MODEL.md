# Local data model

SQLite is the local source of truth for identities, current revision pointers, durable jobs,
content metadata, settings, sessions, and indexes. Large artifacts, game directories, worlds,
screenshots, logs, immutable install manifests, journals, snapshots, and artwork stay file-backed
under managed roots.

Twenty forward-only migrations currently define these areas:

- accounts, account preferences, and credential references;
- storage roots, groups, instances, immutable revisions, and active configuration;
- durable jobs and job steps, installed runtimes, sessions, and session events;
- modpack sources, installed mods, dependency edges, per-mod history, provider content, pinning, and
  provider-content history;
- application and instance settings, artwork metadata, saved servers, and onboarding state;
- snapshots with captured mod and pack state;
- local audit records, telemetry identity/queue, launcher feature configuration, crash-reporting
  consent, and update channel preferences.

The database stores credential references only. Microsoft refresh tokens live in the operating
system credential vault; short-lived access tokens remain in native memory. Provider install plans
and installed revision manifests retain stable provider/file identities and hashes without storing
provider credentials.

Every mutable instance has a monotonically increasing revision. Commands that depend on current
state compare an expected revision before writing. Installation creates an immutable revision and
switches the current pointer only after verified files and the content transaction are ready.
Changing memory or presentation settings does not create a new install job; changing Minecraft or
loader targets marks the instance for installation.

SQLite runs with foreign keys, WAL, normal synchronous mode, and a five-second busy timeout.
Filesystem work is kept outside write transactions. Startup reconciliation handles interrupted
jobs and sessions. Destructive migration support uses a consistent `VACUUM INTO` backup before the
migration is attempted, preserving the original database on failure.

Instance removal is recoverable: database state and files move into managed trash, with configurable
retention, restore, typed-name permanent deletion, and empty-trash operations. Content removal uses
a separate recoverable content trash so pack defaults and user additions can be reasoned about
independently.

