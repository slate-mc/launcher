CREATE TABLE accounts (
    id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL,
    external_profile_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    credential_ref TEXT NOT NULL,
    status TEXT NOT NULL,
    last_validated_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (provider, external_profile_id)
) STRICT;

CREATE TABLE account_preferences (
    account_id TEXT PRIMARY KEY NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    default_flag INTEGER NOT NULL DEFAULT 0 CHECK (default_flag IN (0, 1)),
    skin_ref TEXT,
    local_display_settings TEXT NOT NULL DEFAULT '{}'
) STRICT;

CREATE UNIQUE INDEX one_default_account
    ON account_preferences(default_flag)
    WHERE default_flag = 1;

CREATE TABLE storage_roots (
    id TEXT PRIMARY KEY NOT NULL,
    canonical_path TEXT NOT NULL UNIQUE,
    device_fingerprint TEXT,
    capacity_snapshot TEXT,
    state TEXT NOT NULL CHECK (state IN ('available', 'unavailable', 'read_only')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE instance_groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    archived_at TEXT,
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE instances (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    mode TEXT NOT NULL CHECK (mode IN ('vanilla', 'modded', 'pvp')),
    management_mode TEXT NOT NULL CHECK (management_mode IN ('local', 'community')),
    group_id TEXT REFERENCES instance_groups(id) ON DELETE SET NULL,
    root_id TEXT NOT NULL REFERENCES storage_roots(id) ON DELETE RESTRICT,
    relative_path TEXT NOT NULL,
    active_revision_id TEXT REFERENCES instance_revisions(id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    preferred_account_id TEXT REFERENCES accounts(id) ON DELETE SET NULL,
    favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1)),
    archived_at TEXT,
    trashed_at TEXT,
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (root_id, relative_path)
) STRICT;

CREATE TABLE instance_revisions (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES instance_revisions(id) ON DELETE RESTRICT,
    manifest_digest TEXT NOT NULL,
    game_version TEXT NOT NULL,
    loader_kind TEXT,
    loader_version TEXT,
    client_version TEXT,
    source_release_id TEXT,
    installed_at TEXT,
    status TEXT NOT NULL CHECK (status IN ('proposed', 'staged', 'installed', 'failed', 'superseded')),
    created_at TEXT NOT NULL
) STRICT;

CREATE INDEX instances_library
    ON instances(archived_at, trashed_at, favorite DESC, name COLLATE NOCASE);

CREATE INDEX instance_revisions_timeline
    ON instance_revisions(instance_id, created_at DESC);

CREATE TABLE jobs (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    entity_id TEXT,
    request_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'paused', 'succeeded', 'failed', 'cancelled')),
    phase TEXT NOT NULL,
    progress_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (kind, request_id)
) STRICT;

CREATE INDEX jobs_active
    ON jobs(state, updated_at DESC);

CREATE TABLE job_steps (
    job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    step_id TEXT NOT NULL,
    dependency_ids TEXT NOT NULL DEFAULT '[]',
    state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
    journal_ref TEXT,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    error_code TEXT,
    PRIMARY KEY (job_id, step_id)
) STRICT;

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE RESTRICT,
    revision_id TEXT NOT NULL REFERENCES instance_revisions(id) ON DELETE RESTRICT,
    account_id TEXT REFERENCES accounts(id) ON DELETE SET NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    pid INTEGER,
    process_start_time TEXT,
    state TEXT NOT NULL CHECK (state IN ('queued', 'preparing', 'downloading', 'verifying', 'ready', 'starting', 'running', 'exited', 'failed', 'crashed', 'cancelled')),
    readiness TEXT NOT NULL DEFAULT 'unknown',
    exit_code INTEGER,
    helper_ref TEXT
) STRICT;

CREATE INDEX sessions_by_instance
    ON sessions(instance_id, started_at DESC);

CREATE TABLE session_events (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    kind TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    payload_ref TEXT,
    UNIQUE (session_id, sequence)
) STRICT;

CREATE TABLE local_audit (
    id TEXT PRIMARY KEY NOT NULL,
    time TEXT NOT NULL,
    action TEXT NOT NULL,
    entity_ref TEXT,
    before_revision INTEGER,
    after_revision INTEGER,
    source TEXT NOT NULL,
    result TEXT NOT NULL
) STRICT;

