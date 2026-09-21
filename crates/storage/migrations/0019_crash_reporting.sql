ALTER TABLE app_preferences
ADD COLUMN crash_reporting_enabled INTEGER NOT NULL DEFAULT 0
CHECK (crash_reporting_enabled IN (0, 1));
