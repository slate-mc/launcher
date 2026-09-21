CREATE TABLE launcher_feature_config (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    config_json TEXT NOT NULL,
    expires_at_unix INTEGER NOT NULL,
    updated_at TEXT NOT NULL
);
