CREATE TABLE instance_provider_content_history (
    id TEXT PRIMARY KEY,
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    provider TEXT NOT NULL,
    project_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    changed_at TEXT NOT NULL
) STRICT;

CREATE INDEX instance_provider_content_history_identity
    ON instance_provider_content_history (
        instance_id,
        kind,
        provider,
        project_id,
        changed_at DESC
    );
