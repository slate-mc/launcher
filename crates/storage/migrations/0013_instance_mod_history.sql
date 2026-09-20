CREATE TABLE instance_mod_history (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN ('curseforge', 'modrinth')),
    project_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    changed_at TEXT NOT NULL,
    FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE INDEX idx_instance_mod_history_identity
    ON instance_mod_history (instance_id, provider, project_id, changed_at DESC);
