CREATE TABLE instance_snapshot_mods (
    snapshot_id TEXT NOT NULL REFERENCES instance_snapshots(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('curseforge', 'modrinth')),
    project_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    hashes_json TEXT NOT NULL CHECK (json_valid(hashes_json)),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    pinned INTEGER NOT NULL CHECK (pinned IN (0, 1)),
    installed_at TEXT NOT NULL,
    PRIMARY KEY (snapshot_id, provider, project_id)
) STRICT;
