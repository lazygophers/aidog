use super::*;
use rusqlite::{Connection, Result as SqlResult};

/// Migrations 001–020（基础 schema / 索引 / 列补全 / 中间件基座 + seed / 通知 / MCP 表）。
/// 自 init_tables 拆出（纯结构搬移，执行顺序不变）。
pub(crate) fn run_migrations_early(conn: &Connection) -> SqlResult<()> {
                // Migration 001: 基础 schema（platform / group / group_platform / setting）。
                // proxy_log 建表已移至 run_migrations_proxy_log_early（落 log.db）。
                conn.execute_batch(
                    r#"-- AiDog Schema (v2 — singular table names, uint64 PKs, ms timestamps, soft delete, no NULL)
-- config-db-split: platform / "group" / group_platform CREATE 迁出 → run_migrations_platform_early（落 platform.db）。
-- 主库仅留 setting（+ model_price 003 / middleware_rule 013 / mcp_server 020，见后续 migration）。

PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

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
"#,
                )?;
                // Migration 002: proxy_log 索引已移至 run_migrations_proxy_log_early（落 log.db）。
                // Migration 003: 模型价格表 model_price。
                conn.execute_batch(
                    r#"-- Model price table: stores per-model pricing data synced from LiteLLM or entered manually

CREATE TABLE IF NOT EXISTS model_price (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    model_name       TEXT NOT NULL DEFAULT '',
    source           TEXT NOT NULL DEFAULT 'manual',  -- 'litellm' | 'manual'
    price_data       TEXT NOT NULL DEFAULT '{}',       -- JSON: {input_cost_per_token, output_cost_per_token, cache_read_input_token_cost, pricing: {platform_type: {...}}, default_platform, ...}
    created_at       INTEGER NOT NULL DEFAULT 0,
    updated_at       INTEGER NOT NULL DEFAULT 0,
    deleted_at       INTEGER NOT NULL DEFAULT 0,
    UNIQUE(model_name, source)
);

-- idx_model_price_name 已删：UNIQUE(model_name, source) 自带的隐式索引前导列即 model_name，
-- 已覆盖按 model_name 的等值/前缀查找，单列偏索引纯重复。旧库由 migration 035 DROP。
"#,
                )?;
                // Migration 004–012: platform / "group" 的 ALTER 与 012 kimi endpoint 修正
                // → run_migrations_platform_early / run_migrations_platform_late（落 platform.db）。
                // Migration 013: 中间件规则引擎基座（C1）。单表 middleware_rule，
                // 8 类规则 + 三级作用域就近覆盖；schema 严格按 design.md。
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS middleware_rule (
                       id           INTEGER PRIMARY KEY AUTOINCREMENT,
                       name         TEXT NOT NULL,
                       description  TEXT NOT NULL DEFAULT '',
                       rule_type    TEXT NOT NULL,
                       scope        TEXT NOT NULL DEFAULT 'global',
                       scope_ref    TEXT NOT NULL DEFAULT '',
                       match_type   TEXT NOT NULL DEFAULT 'contains',
                       pattern      TEXT NOT NULL DEFAULT '',
                       action       TEXT NOT NULL DEFAULT 'warn',
                       config       TEXT NOT NULL DEFAULT '{}',
                       priority     INTEGER NOT NULL DEFAULT 0,
                       enabled      INTEGER NOT NULL DEFAULT 1,
                       is_builtin   INTEGER NOT NULL DEFAULT 0,
                       created_at   INTEGER NOT NULL,
                       updated_at   INTEGER NOT NULL
                     );
                     CREATE INDEX IF NOT EXISTS idx_mw_rule_lookup ON middleware_rule(enabled, rule_type, scope);",
                )?;
                // Migration 014: proxy_log blocked_by/blocked_reason → run_migrations_proxy_log_early
                // Migration 015: 内置预设中间件规则 seed（C4）。
                // is_builtin=1 默认 enabled；幂等——按 (name, is_builtin=1) 唯一判定，已存在跳过（尊重用户禁用状态，不重新启用）。
                seed_builtin_middleware_rules(conn)?;
                // Migration 016: Platform 级熔断配置列 → run_migrations_platform_late（platform 表已迁 platform.db）
                // Migration 017/018: notification 表 → run_migrations_proxy_log_early（落 log.db）
                // Migration 019: idx_proxy_log_stats → run_migrations_proxy_log_early
                // Migration 020: MCP 管理模块。集中存 MCP server 配置 + per-agent 启用态。
                // enabled_agents = 逗号分隔 agent slug（claude-code/codex）。
                // env_json/headers_json 含敏感值（token/key/secret），前端展示经 mcp.rs::mask_env 脱敏。
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS mcp_server (
                       id             INTEGER PRIMARY KEY AUTOINCREMENT,
                       name           TEXT NOT NULL UNIQUE,
                       transport      TEXT NOT NULL DEFAULT 'stdio',
                       command        TEXT NOT NULL DEFAULT '',
                       args_json      TEXT NOT NULL DEFAULT '[]',
                       env_json       TEXT NOT NULL DEFAULT '{}',
                       url            TEXT NOT NULL DEFAULT '',
                       headers_json   TEXT NOT NULL DEFAULT '{}',
                       enabled_agents TEXT NOT NULL DEFAULT '',
                       created_at     INTEGER NOT NULL,
                       updated_at     INTEGER NOT NULL
                     );",
                )?;
    Ok(())
}

/// proxy_log 表的 early migrations（001–020 范围内的 proxy_log 部分）。
///
/// 拆库后这些 DDL 跑在 log.db 写连接（`call_proxy_log_traced`），主库不再建 proxy_log 表。
/// migration 内容与原 run_migrations_early 中的 proxy_log 语句一一对应，幂等 idiom 不变
/// （CREATE IF NOT EXISTS / `let _ =` 吞 dup）。
///
/// 注：stats_agg_hourly 原 Mig 032（log.db late）已迁回主库 Mig 051（stats-agg-to-main-db），
/// log.db 现仅承载 proxy_log + notification。
pub(crate) fn run_migrations_proxy_log_early(conn: &Connection) -> SqlResult<()> {
                // Migration 001 (proxy_log): 建表。
                conn.execute_batch(
                    r#"-- proxy_log PK 用无连字符 uuid（请求 ID），R7 uint64 主键规则的明示例外（R8）
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
"#,
                )?;
                // Migration 002: proxy_log model 索引。
                conn.execute_batch(
                    r#"CREATE INDEX IF NOT EXISTS idx_proxy_log_model
    ON proxy_log(model) WHERE deleted_at = 0;

CREATE INDEX IF NOT EXISTS idx_proxy_log_actual_model
    ON proxy_log(actual_model) WHERE deleted_at = 0;
"#,
                )?;
                // Migration 008: proxy_log 预估花费列。
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN est_cost REAL NOT NULL DEFAULT 0", []);
                // Migration 010: proxy_log 流式标记列。
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN is_stream INTEGER NOT NULL DEFAULT 0", []);
                // Migration 011 (proxy_log): 每次尝试快照 + 重试次数。
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN attempts TEXT NOT NULL DEFAULT '[]'", []);
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0", []);
                // Migration 014: proxy_log 中间件拦截审计列。
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN blocked_by TEXT NOT NULL DEFAULT ''", []);
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN blocked_reason TEXT NOT NULL DEFAULT ''", []);
                // Migration 019: usage stats 覆盖索引。
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_stats \
                     ON proxy_log(created_at, est_cost, input_tokens, output_tokens, cache_tokens, status_code) \
                     WHERE deleted_at = 0",
                    [],
                );
                // Migration 017: notification 表（从主库迁入 log.db）。
                // notify(type) → InboxOnly/PopupOnly/Full 落库一行；前端通知中心 list/clear 消费。
                // 设置（NotificationSettings）走 settings KV scope=notification（主库），不在此表。
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS notification (
                       id          INTEGER PRIMARY KEY AUTOINCREMENT,
                       notif_type  TEXT NOT NULL,
                       title       TEXT NOT NULL DEFAULT '',
                       body        TEXT NOT NULL DEFAULT '',
                       created_at  INTEGER NOT NULL
                     );",
                )?;
                // Migration 018: 去 read 列 + idx_notif_read 索引（通知完成即结束，无已读未读）。
                // 旧装库（017 建表含 read）走 DROP；新装无 read 列，DROP COLUMN 报错被吞。
                let _ = conn.execute("DROP INDEX IF EXISTS idx_notif_read", []);
                let _ = conn.execute("ALTER TABLE notification DROP COLUMN read", []);
    Ok(())
}

/// platform.db 的 early migrations：建 platform / "group" / group_platform 三表。
///
/// config-db-split：从原 `run_migrations_early` 抽出（CREATE 语句原样搬移，幂等 idiom 不变）。
/// 由 `Db::init_tables` Phase 3 在 `call_platform_traced` 闭包内调用。内存库 fallback 下
/// platform handle = 主内存连接 clone，三表落在同一物理库，行为与拆库前一致。
///
/// 三表 schema 与原 `run_migrations_early` Migration 001 完全一致（含 enabled / sort_order /
/// status / max_retries / manual_budgets 等列 —— 对齐旧库经 004–016 ALTER 后的终态，省去
/// platform_late 的 30+ 条 `let _ = ALTER` 幂等重试）。
pub(crate) fn run_migrations_platform_early(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        r#"PRAGMA journal_mode=WAL;
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
    deleted_at       INTEGER NOT NULL DEFAULT 0,
    sort_order       INTEGER NOT NULL DEFAULT 0,
    manual_budgets   TEXT NOT NULL DEFAULT '[]',
    status           TEXT NOT NULL DEFAULT 'enabled',
    auto_disabled_until     INTEGER NOT NULL DEFAULT 0,
    auto_disable_strikes    INTEGER NOT NULL DEFAULT 0,
    breaker_failure_threshold INTEGER NOT NULL DEFAULT 0,
    breaker_open_secs         INTEGER NOT NULL DEFAULT 0,
    breaker_half_open_max     INTEGER NOT NULL DEFAULT 0,
    auto_group       INTEGER NOT NULL DEFAULT 1,
    expires_at       INTEGER NOT NULL DEFAULT 0,
    last_error       TEXT NOT NULL DEFAULT '',
    last_error_at    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS "group" (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    name                 TEXT NOT NULL DEFAULT '',
    group_key            TEXT NOT NULL DEFAULT '',
    routing_mode         TEXT NOT NULL DEFAULT '',
    auto_from_platform   TEXT NOT NULL DEFAULT '',
    source_protocol      TEXT NOT NULL DEFAULT 'anthropic',
    model_mappings       TEXT NOT NULL DEFAULT '[]',
    request_timeout_secs INTEGER NOT NULL DEFAULT 0,
    connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
    created_at           INTEGER NOT NULL DEFAULT 0,
    updated_at           INTEGER NOT NULL DEFAULT 0,
    deleted_at           INTEGER NOT NULL DEFAULT 0,
    sort_order           INTEGER NOT NULL DEFAULT 0,
    max_retries          INTEGER NOT NULL DEFAULT 2,
    is_default           INTEGER NOT NULL DEFAULT 0,
    env_vars             TEXT NOT NULL DEFAULT '[]',
    extra                TEXT NOT NULL DEFAULT '',
    UNIQUE(name),
    UNIQUE(group_key)
);

CREATE TABLE IF NOT EXISTS group_platform (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id       INTEGER NOT NULL DEFAULT 0,
    platform_id    INTEGER NOT NULL DEFAULT 0,
    priority       INTEGER NOT NULL DEFAULT 0,
    weight         INTEGER NOT NULL DEFAULT 1,
    level_priority INTEGER NOT NULL DEFAULT 5,
    created_at     INTEGER NOT NULL DEFAULT 0,
    updated_at     INTEGER NOT NULL DEFAULT 0,
    deleted_at     INTEGER NOT NULL DEFAULT 0,
    UNIQUE(group_id, platform_id)
);
"#,
    )?;
    Ok(())
}
