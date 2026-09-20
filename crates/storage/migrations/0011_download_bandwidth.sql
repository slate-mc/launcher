ALTER TABLE app_preferences
ADD COLUMN download_bandwidth_limit_mib INTEGER NOT NULL DEFAULT 0
    CHECK (download_bandwidth_limit_mib BETWEEN 0 AND 1024);
