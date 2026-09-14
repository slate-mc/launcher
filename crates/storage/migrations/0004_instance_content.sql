CREATE TABLE instance_modpacks (
    instance_id TEXT PRIMARY KEY NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('curseforge', 'modrinth', 'ftb')),
    project_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    selected_optional_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(selected_optional_json)),
    display_name TEXT NOT NULL,
    icon_url TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE instance_mods (
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('curseforge', 'modrinth')),
    project_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    hashes_json TEXT NOT NULL CHECK (json_valid(hashes_json)),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    installed_at TEXT NOT NULL,
    PRIMARY KEY (instance_id, provider, project_id)
) STRICT;

CREATE INDEX instance_mods_by_instance
    ON instance_mods(instance_id, display_name COLLATE NOCASE);
