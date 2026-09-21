CREATE TABLE telemetry_identity (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    installation_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);

CREATE TABLE telemetry_queue (
    id TEXT PRIMARY KEY,
    payload_json TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    created_at TEXT NOT NULL
);

CREATE INDEX idx_telemetry_queue_created
    ON telemetry_queue(created_at, id);
