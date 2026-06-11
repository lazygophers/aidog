-- Indexes for proxy log filtering

CREATE INDEX IF NOT EXISTS idx_proxy_log_platform
    ON proxy_log(platform_id) WHERE deleted_at = 0;

CREATE INDEX IF NOT EXISTS idx_proxy_log_status
    ON proxy_log(status_code) WHERE deleted_at = 0;

CREATE INDEX IF NOT EXISTS idx_proxy_log_model
    ON proxy_log(model) WHERE deleted_at = 0;

CREATE INDEX IF NOT EXISTS idx_proxy_log_actual_model
    ON proxy_log(actual_model) WHERE deleted_at = 0;
