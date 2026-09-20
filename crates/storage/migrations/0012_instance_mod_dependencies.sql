CREATE TABLE instance_mod_dependencies (
    instance_id TEXT NOT NULL,
    root_provider TEXT NOT NULL CHECK (root_provider IN ('curseforge', 'modrinth')),
    root_project_id TEXT NOT NULL,
    dependency_provider TEXT NOT NULL CHECK (dependency_provider IN ('curseforge', 'modrinth')),
    dependency_project_id TEXT NOT NULL,
    PRIMARY KEY (
        instance_id,
        root_provider,
        root_project_id,
        dependency_provider,
        dependency_project_id
    ),
    FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE,
    CHECK (
        root_provider <> dependency_provider
        OR root_project_id <> dependency_project_id
    )
);

CREATE INDEX idx_instance_mod_dependencies_dependency
    ON instance_mod_dependencies (
        instance_id,
        dependency_provider,
        dependency_project_id
    );
