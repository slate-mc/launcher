CREATE TABLE runtime_installations (
    id TEXT PRIMARY KEY NOT NULL,
    vendor TEXT NOT NULL,
    release_name TEXT NOT NULL,
    java_version TEXT NOT NULL,
    major INTEGER NOT NULL CHECK (major > 0),
    os TEXT NOT NULL,
    arch TEXT NOT NULL,
    executable_ref TEXT NOT NULL,
    source_digest TEXT NOT NULL,
    managed INTEGER NOT NULL CHECK (managed IN (0, 1)),
    verified_at TEXT NOT NULL,
    UNIQUE (vendor, release_name, os, arch)
) STRICT;

ALTER TABLE instance_revisions
    ADD COLUMN runtime_id TEXT REFERENCES runtime_installations(id) ON DELETE RESTRICT;

CREATE INDEX runtime_installations_by_major
    ON runtime_installations(major, os, arch, verified_at DESC);
