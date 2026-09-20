CREATE TABLE onboarding_state (
    singleton_id INTEGER PRIMARY KEY NOT NULL CHECK (singleton_id = 1),
    completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
    default_storage_root_id TEXT REFERENCES storage_roots(id) ON DELETE SET NULL,
    updated_at TEXT NOT NULL
) STRICT;

INSERT INTO onboarding_state (singleton_id, completed, default_storage_root_id, updated_at)
SELECT 1,
       CASE
           WHEN EXISTS(SELECT 1 FROM accounts) OR EXISTS(SELECT 1 FROM instances) THEN 1
           ELSE 0
       END,
       NULL,
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
