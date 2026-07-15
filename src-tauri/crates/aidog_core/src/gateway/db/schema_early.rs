use super::*;
use rusqlite::{params, Connection, Result as SqlResult};

/// Migrations 001–020（基础 schema / 索引 / 列补全 / 中间件基座 + seed / 通知 / MCP 表）。
/// 自 init_tables 拆出（纯结构搬移，执行顺序不变）。
pub(crate) fn run_migrations_early(conn: &Connection) -> SqlResult<()> {
                // Migration 001: 基础 schema（platform / group / group_platform / setting）。
                // proxy_log 建表已移至 run_migrations_proxy_log_early（落 log.db）。
                conn.execute_batch(
                    r#"-- AiDog Schema (v2 — singular table names, uint64 PKs, ms timestamps, soft delete, no NULL)

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
                // Migration 004: 旧库补预估列（ALTER 无 IF NOT EXISTS → 忽略 duplicate column）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN est_balance_remaining REAL NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN est_coding_plan TEXT NOT NULL DEFAULT ''", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN last_real_query_at INTEGER NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN estimate_count INTEGER NOT NULL DEFAULT 0", []);
                // Migration 005: tray 展示列（互斥单平台 show_in_tray + balance/coding 二选一 tray_display）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN show_in_tray INTEGER NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN tray_display TEXT NOT NULL DEFAULT 'balance'", []);
                // Migration 006: group 排序权重
                let _ = conn.execute("ALTER TABLE \"group\" ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0", []);
                // Migration 007: platform 排序权重
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0", []);
                // Migration 008: proxy_log 预估花费列 → run_migrations_proxy_log_early
                // Migration 009: platform 手动预算列（无上游 quota 平台手动限额 + 耗尽阻断）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN manual_budgets TEXT NOT NULL DEFAULT '[]'", []);
                // Migration 010: proxy_log 流式标记列 → run_migrations_proxy_log_early
                // Migration 011: 多平台重试 + 401/403 自动禁用 + 尝试记录（旧 007_retry_failover，逻辑已内联）
                // platform 三态 status + 退避字段；enabled 列保留向后兼容（写入端从 status 同步）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN status TEXT NOT NULL DEFAULT 'enabled'", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN auto_disabled_until INTEGER NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN auto_disable_strikes INTEGER NOT NULL DEFAULT 0", []);
                // 数据迁移：旧 enabled=0 → status='disabled'（幂等：仅作用于仍为默认 'enabled' 的行，
                // 绝不覆盖 auto_disabled，避免重启误判用户禁用 vs 自动禁用）
                let _ = conn.execute("UPDATE platform SET status = 'disabled' WHERE enabled = 0 AND status = 'enabled'", []);
                // group 分组级最大重试次数
                let _ = conn.execute("ALTER TABLE \"group\" ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 2", []);
                // proxy_log attempts/retry_count → run_migrations_proxy_log_early
                // Migration 012: Kimi Code Plan endpoint client_type 修正（codex_tui→claude_code）
                // 根因：Platforms.tsx 预设曾把 kimi coding openai endpoint 配为 codex_tui，
                // 但 Kimi coding 上游拒绝 Codex（只接 Kimi CLI/Claude Code/Roo Code/Kilo Code）。
                // 扫描已有 kimi 平台 endpoints JSON，修正该 endpoint 身份。幂等：仅改 codex_tui，已 claude_code 不动。
                if let Ok(mut stmt) = conn.prepare("SELECT id, endpoints FROM platform WHERE platform_type = 'kimi'") {
                    let rows: Vec<(i64, String)> = stmt
                        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                        .ok()
                        .map(|iter| iter.filter_map(Result::ok).collect())
                        .unwrap_or_default();
                    for (id, endpoints_json) in rows {
                        let mut eps = parse_endpoints(&endpoints_json);
                        let mut changed = false;
                        for ep in &mut eps {
                            if ep.protocol == Protocol::OpenAI
                                && ep.coding_plan
                                && ep.client_type == "codex_tui"
                            {
                                ep.client_type = "claude_code".to_string();
                                changed = true;
                            }
                        }
                        if changed {
                            let new_json = serialize_endpoints(&eps);
                            let _ = conn.execute(
                                "UPDATE platform SET endpoints = ?1 WHERE id = ?2",
                                params![new_json, id],
                            );
                            tracing::info!(platform_id = id, "migration 012: kimi coding endpoint client_type codex_tui→claude_code");
                        }
                    }
                }
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
                // Migration 016: Platform 级熔断配置列（GA — group 智能调度与熔断器）。
                // 0 = 继承全局 SchedulingBreakerSettings 默认（settings scope=scheduling）。
                // 熔断与 auto_disabled 解耦：熔断临时(5xx/超时自动恢复)，状态在内存(scheduling.rs)不持久化；
                // 本 3 列仅持久化阈值配置，运行态 BreakerState 不落库。
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN breaker_failure_threshold INTEGER NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN breaker_open_secs INTEGER NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN breaker_half_open_max INTEGER NOT NULL DEFAULT 0", []);
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

/// proxy_log / stats_agg_hourly 表的 early migrations（001–020 范围内的 proxy_log 部分）。
///
/// 拆库后这些 DDL 跑在 log.db 写连接（`call_proxy_log_traced`），主库不再建
/// proxy_log / stats_agg_hourly 表。migration 内容与原 run_migrations_early 中的
/// proxy_log 语句一一对应，幂等 idiom 不变（CREATE IF NOT EXISTS / `let _ =` 吞 dup）。
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
