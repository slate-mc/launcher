CREATE UNIQUE INDEX instance_groups_name_unique
    ON instance_groups(name COLLATE NOCASE)
    WHERE archived_at IS NULL;

CREATE TABLE instance_settings (
    instance_id TEXT PRIMARY KEY NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 500),
    notes TEXT NOT NULL DEFAULT '' CHECK (length(notes) <= 4000),
    tags_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tags_json)),
    icon_mime TEXT CHECK (icon_mime IN ('image/png', 'image/jpeg', 'image/webp')),
    banner_mime TEXT CHECK (banner_mime IN ('image/png', 'image/jpeg', 'image/webp')),
    banner_position_x INTEGER NOT NULL DEFAULT 50 CHECK (banner_position_x BETWEEN 0 AND 100),
    banner_position_y INTEGER NOT NULL DEFAULT 50 CHECK (banner_position_y BETWEEN 0 AND 100),
    window_mode TEXT NOT NULL DEFAULT 'windowed'
        CHECK (window_mode IN ('windowed', 'maximized', 'fullscreen')),
    resolution_width INTEGER CHECK (resolution_width BETWEEN 320 AND 16384),
    resolution_height INTEGER CHECK (resolution_height BETWEEN 240 AND 16384),
    launcher_behavior TEXT NOT NULL DEFAULT 'keep_open'
        CHECK (launcher_behavior IN ('keep_open', 'minimize', 'hide')),
    game_language TEXT NOT NULL DEFAULT 'en_us'
        CHECK (length(trim(game_language)) BETWEEN 2 AND 32),
    quick_play_server TEXT CHECK (length(quick_play_server) <= 255),
    process_priority TEXT NOT NULL DEFAULT 'normal'
        CHECK (process_priority IN ('low', 'below_normal', 'normal', 'above_normal', 'high')),
    memory_mode TEXT NOT NULL DEFAULT 'auto' CHECK (memory_mode IN ('auto', 'custom')),
    initial_memory_mb INTEGER NOT NULL DEFAULT 512 CHECK (initial_memory_mb BETWEEN 256 AND 32768),
    java_mode TEXT NOT NULL DEFAULT 'managed' CHECK (java_mode IN ('managed', 'detected', 'custom')),
    custom_java_path TEXT,
    custom_java_label TEXT CHECK (length(custom_java_label) <= 160),
    performance_preset TEXT NOT NULL DEFAULT 'balanced'
        CHECK (performance_preset IN ('balanced', 'throughput', 'low_latency', 'custom')),
    jvm_arguments_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(jvm_arguments_json)),
    environment_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(environment_json)),
    backup_before_changes INTEGER NOT NULL DEFAULT 1 CHECK (backup_before_changes IN (0, 1)),
    backup_retention INTEGER NOT NULL DEFAULT 5 CHECK (backup_retention BETWEEN 1 AND 50),
    log_retention_days INTEGER NOT NULL DEFAULT 30 CHECK (log_retention_days BETWEEN 1 AND 365),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

INSERT INTO instance_settings (instance_id, created_at, updated_at)
SELECT id, created_at, updated_at FROM instances;

CREATE TABLE instance_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    source_revision_id TEXT REFERENCES instance_revisions(id) ON DELETE SET NULL,
    kind TEXT NOT NULL CHECK (kind IN ('profile', 'worlds', 'full')),
    consistency TEXT NOT NULL CHECK (consistency IN ('stopped', 'best_effort')),
    manifest_ref TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    verified_at TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE INDEX instance_snapshots_by_instance
    ON instance_snapshots(instance_id, created_at DESC);
