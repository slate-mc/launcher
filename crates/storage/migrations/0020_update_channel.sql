ALTER TABLE app_preferences
ADD COLUMN update_channel TEXT NOT NULL DEFAULT 'stable'
CHECK (update_channel IN ('stable', 'beta'));
