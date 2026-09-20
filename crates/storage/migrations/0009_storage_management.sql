ALTER TABLE app_preferences
ADD COLUMN trash_retention_days INTEGER NOT NULL DEFAULT 30
    CHECK (trash_retention_days = 0 OR trash_retention_days BETWEEN 7 AND 365);
