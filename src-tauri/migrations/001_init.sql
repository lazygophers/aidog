-- AiDog Schema

PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS platforms (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    protocol          TEXT NOT NULL,
    base_url          TEXT NOT NULL,
    api_key           TEXT NOT NULL,
    extra             TEXT,
    models            TEXT NOT NULL DEFAULT '{}',
    available_models  TEXT NOT NULL DEFAULT '[]',
    enabled           INTEGER NOT NULL DEFAULT 1,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS groups (
    id                  TEXT PRIMARY KEY,
    name                TEXT NOT NULL,
    path                TEXT NOT NULL UNIQUE,
    routing_mode        TEXT NOT NULL,
    auto_from_platform  TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS group_platforms (
    group_id    TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    platform_id TEXT NOT NULL REFERENCES platforms(id) ON DELETE CASCADE,
    priority    INTEGER NOT NULL DEFAULT 0,
    weight      INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (group_id, platform_id)
);

CREATE TABLE IF NOT EXISTS model_mappings (
    id                 TEXT PRIMARY KEY,
    group_id           TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    source_model       TEXT NOT NULL,
    target_platform_id TEXT NOT NULL REFERENCES platforms(id),
    target_model       TEXT NOT NULL,
    UNIQUE(group_id, source_model)
);

CREATE TABLE IF NOT EXISTS settings (
    scope       TEXT NOT NULL,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL DEFAULT '{}',
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (scope, key)
);

CREATE TABLE IF NOT EXISTS proxy_logs (
    id              TEXT PRIMARY KEY,
    group_name      TEXT NOT NULL,
    model           TEXT NOT NULL DEFAULT '',
    actual_model    TEXT NOT NULL DEFAULT '',
    source_protocol TEXT NOT NULL DEFAULT '',
    target_protocol TEXT NOT NULL DEFAULT '',
    request_headers TEXT NOT NULL DEFAULT '{}',
    request_body    TEXT NOT NULL DEFAULT '',
    response_body   TEXT NOT NULL DEFAULT '',
    status_code     INTEGER NOT NULL DEFAULT 0,
    duration_ms     INTEGER NOT NULL DEFAULT 0,
    input_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens   INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_proxy_logs_group ON proxy_logs(group_name);
CREATE INDEX IF NOT EXISTS idx_proxy_logs_created ON proxy_logs(created_at);
