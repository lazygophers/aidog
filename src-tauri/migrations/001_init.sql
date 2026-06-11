-- AiDog Schema (v2 — singular table names, uint64 PKs, ms timestamps, soft delete, no NULL)

PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS platform (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT NOT NULL DEFAULT '',
    platform_type    TEXT NOT NULL DEFAULT '',
    base_url         TEXT NOT NULL DEFAULT '',
    api_key          TEXT NOT NULL DEFAULT '',
    extra            TEXT NOT NULL DEFAULT '',
    models           TEXT NOT NULL DEFAULT '{}',
    available_models TEXT NOT NULL DEFAULT '[]',
    endpoints        TEXT NOT NULL DEFAULT '[]',
    enabled          INTEGER NOT NULL DEFAULT 1,
    est_balance_remaining REAL NOT NULL DEFAULT 0,
    est_coding_plan       TEXT NOT NULL DEFAULT '',
    last_real_query_at    INTEGER NOT NULL DEFAULT 0,
    estimate_count        INTEGER NOT NULL DEFAULT 0,
    show_in_tray          INTEGER NOT NULL DEFAULT 0,
    tray_display          TEXT NOT NULL DEFAULT 'balance',
    created_at       INTEGER NOT NULL DEFAULT 0,
    updated_at       INTEGER NOT NULL DEFAULT 0,
    deleted_at       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS "group" (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    name                 TEXT NOT NULL DEFAULT '',
    path                 TEXT NOT NULL DEFAULT '',
    routing_mode         TEXT NOT NULL DEFAULT '',
    auto_from_platform   TEXT NOT NULL DEFAULT '',
    source_protocol      TEXT NOT NULL DEFAULT 'anthropic',
    model_mappings       TEXT NOT NULL DEFAULT '[]',
    request_timeout_secs INTEGER NOT NULL DEFAULT 0,
    connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
    created_at           INTEGER NOT NULL DEFAULT 0,
    updated_at           INTEGER NOT NULL DEFAULT 0,
    deleted_at           INTEGER NOT NULL DEFAULT 0,
    UNIQUE(path)
);

CREATE TABLE IF NOT EXISTS group_platform (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id    INTEGER NOT NULL DEFAULT 0,
    platform_id INTEGER NOT NULL DEFAULT 0,
    priority    INTEGER NOT NULL DEFAULT 0,
    weight      INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL DEFAULT 0,
    deleted_at  INTEGER NOT NULL DEFAULT 0,
    UNIQUE(group_id, platform_id)
);

CREATE TABLE IF NOT EXISTS setting (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    scope      TEXT NOT NULL DEFAULT '',
    key        TEXT NOT NULL DEFAULT '',
    value      TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER NOT NULL DEFAULT 0,
    UNIQUE(scope, key)
);

-- proxy_log PK 用无连字符 uuid（请求 ID），R7 uint64 主键规则的明示例外（R8）
CREATE TABLE IF NOT EXISTS proxy_log (
    id                        TEXT PRIMARY KEY,
    group_name                TEXT NOT NULL DEFAULT '',
    model                     TEXT NOT NULL DEFAULT '',
    actual_model              TEXT NOT NULL DEFAULT '',
    source_protocol           TEXT NOT NULL DEFAULT '',
    target_protocol           TEXT NOT NULL DEFAULT '',
    platform_id               INTEGER NOT NULL DEFAULT 0,
    request_headers           TEXT NOT NULL DEFAULT '{}',
    request_body              TEXT NOT NULL DEFAULT '',
    upstream_request_headers  TEXT NOT NULL DEFAULT '',
    upstream_request_body     TEXT NOT NULL DEFAULT '',
    response_body             TEXT NOT NULL DEFAULT '',
    request_url               TEXT NOT NULL DEFAULT '',
    upstream_request_url      TEXT NOT NULL DEFAULT '',
    upstream_response_headers TEXT NOT NULL DEFAULT '',
    upstream_status_code      INTEGER NOT NULL DEFAULT 0,
    user_response_headers     TEXT NOT NULL DEFAULT '',
    user_response_body        TEXT NOT NULL DEFAULT '',
    status_code               INTEGER NOT NULL DEFAULT 0,
    duration_ms               INTEGER NOT NULL DEFAULT 0,
    input_tokens              INTEGER NOT NULL DEFAULT 0,
    output_tokens             INTEGER NOT NULL DEFAULT 0,
    cache_tokens              INTEGER NOT NULL DEFAULT 0,
    created_at                INTEGER NOT NULL DEFAULT 0,
    updated_at                INTEGER NOT NULL DEFAULT 0,
    deleted_at                INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_proxy_log_group ON proxy_log(group_name) WHERE deleted_at = 0;
CREATE INDEX IF NOT EXISTS idx_proxy_log_created ON proxy_log(created_at) WHERE deleted_at = 0;
