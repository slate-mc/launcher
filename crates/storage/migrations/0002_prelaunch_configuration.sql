CREATE TABLE instance_configuration (
    instance_id TEXT PRIMARY KEY NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    minecraft_version TEXT NOT NULL CHECK (length(trim(minecraft_version)) BETWEEN 1 AND 64),
    loader_kind TEXT NOT NULL CHECK (loader_kind IN ('vanilla', 'fabric', 'neoforge')),
    loader_version TEXT,
    memory_mb INTEGER NOT NULL DEFAULT 4096 CHECK (memory_mb BETWEEN 1024 AND 32768),
    setup_state TEXT NOT NULL DEFAULT 'configured'
        CHECK (setup_state IN ('configured', 'preparing', 'ready', 'blocked')),
    CHECK (
        (loader_kind = 'vanilla' AND loader_version IS NULL) OR
        (loader_kind IN ('fabric', 'neoforge') AND length(trim(loader_version)) BETWEEN 1 AND 64)
    )
) STRICT;

INSERT INTO instance_configuration (
    instance_id,
    minecraft_version,
    loader_kind,
    loader_version,
    memory_mb,
    setup_state
)
SELECT
    id,
    'unconfigured',
    CASE WHEN mode = 'vanilla' THEN 'vanilla' ELSE 'fabric' END,
    CASE WHEN mode = 'vanilla' THEN NULL ELSE 'unconfigured' END,
    4096,
    'blocked'
FROM instances;

CREATE TABLE app_preferences (
    singleton_id INTEGER PRIMARY KEY NOT NULL DEFAULT 1 CHECK (singleton_id = 1),
    theme TEXT NOT NULL DEFAULT 'dark' CHECK (theme IN ('dark', 'light', 'system')),
    download_concurrency INTEGER NOT NULL DEFAULT 4 CHECK (download_concurrency BETWEEN 1 AND 8),
    telemetry_enabled INTEGER NOT NULL DEFAULT 0 CHECK (telemetry_enabled IN (0, 1)),
    reduce_motion TEXT NOT NULL DEFAULT 'system' CHECK (reduce_motion IN ('system', 'on', 'off')),
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE saved_servers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    address TEXT NOT NULL CHECK (length(trim(address)) BETWEEN 1 AND 255),
    preferred_instance_id TEXT REFERENCES instances(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (address COLLATE NOCASE)
) STRICT;

CREATE INDEX saved_servers_by_name ON saved_servers(name COLLATE NOCASE, id);
