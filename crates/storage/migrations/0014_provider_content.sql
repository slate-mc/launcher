CREATE TABLE instance_provider_content (
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('resource_pack', 'shader_pack', 'data_pack')),
    provider TEXT NOT NULL CHECK (provider IN ('curseforge', 'modrinth')),
    project_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    icon_url TEXT,
    file_path TEXT NOT NULL,
    hashes_json TEXT NOT NULL CHECK (json_valid(hashes_json)),
    installed_at TEXT NOT NULL,
    PRIMARY KEY (instance_id, kind, provider, project_id),
    UNIQUE (instance_id, file_path)
) STRICT;

CREATE INDEX instance_provider_content_by_instance
    ON instance_provider_content(instance_id, kind, display_name COLLATE NOCASE);
