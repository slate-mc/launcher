CREATE TABLE instance_snapshot_modpacks (
    snapshot_id TEXT PRIMARY KEY NOT NULL REFERENCES instance_snapshots(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('curseforge', 'modrinth', 'ftb')),
    project_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    selected_optional_json TEXT NOT NULL CHECK (json_valid(selected_optional_json)),
    display_name TEXT NOT NULL,
    icon_url TEXT,
    banner_url TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

ALTER TABLE instance_settings
ADD COLUMN cpu_affinity_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(cpu_affinity_json));
