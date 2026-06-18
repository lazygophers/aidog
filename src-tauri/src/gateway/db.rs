use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio_rusqlite::Connection as AsyncConnection;

use super::models::*;

/// 进程内热路径缓存（随 Db 实例生命周期，clone 共享同一份）。
///
/// 为什么挂在 `Db` 内而非全局 static：cargo test 单进程多线程跑，每个 test 各开一个
/// `:memory:` Db；全局缓存会跨 test 串味（test A 写 proxy/logging，test B 读到脏值）。
/// 内嵌 `Arc<RwLock<..>>` 保证「每个 Db 实例独立缓存 + clone 共享」两个性质同时成立。
#[derive(Default)]
struct DbCache {
    /// setting 表 (scope,key)→JSON 值缓存。`None` 槽位表示「已查过且不存在」，
    /// 用 `Option<Option<Value>>`：外层 = 是否缓存，内层 = 行是否存在。
    settings: RwLock<HashMap<(String, String), Option<serde_json::Value>>>,
    /// list_groups() 结果缓存（resolve_group 热路径用），写 group 表时整体失效。
    groups: RwLock<Option<Vec<Group>>>,
}

/// 异步 SQLite 连接封装。
///
/// tokio-rusqlite 内部以单后台线程顺序执行所有 `call` 闭包，天然串行化，
/// 故无需 `Mutex`。`AsyncConnection` 自身 `Clone + Send + Sync`（内部仅一个 channel sender），
/// 可直接 `app.manage(Db)` / `State<Db>`，克隆廉价（共享同一后台线程连接）。
#[derive(Clone)]
pub struct Db(pub AsyncConnection, Arc<DbCache>);

/// 从 JSON 字符串反序列化 models
fn parse_models(json: &str) -> PlatformModels {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse platform models failed, using default (stored JSON corrupt?)");
        PlatformModels::default()
    })
}

/// 将 models 序列化为 JSON 字符串
fn serialize_models(models: &PlatformModels) -> String {
    serde_json::to_string(models).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize platform models failed, persisting empty object");
        "{}".to_string()
    })
}

/// 从 JSON 字符串反序列化可用模型列表
fn parse_available_models(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse available_models failed, using empty list (stored JSON corrupt?)");
        Vec::new()
    })
}

/// 将可用模型列表序列化为 JSON 字符串
fn serialize_available_models(models: &[String]) -> String {
    serde_json::to_string(models).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize available_models failed, persisting empty array");
        "[]".to_string()
    })
}

/// 从 JSON 字符串反序列化协议端点列表
fn parse_endpoints(json: &str) -> Vec<PlatformEndpoint> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse platform endpoints failed, using empty list (stored JSON corrupt?)");
        Vec::new()
    })
}

/// 将协议端点列表序列化为 JSON 字符串
fn serialize_endpoints(endpoints: &[PlatformEndpoint]) -> String {
    serde_json::to_string(endpoints).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize platform endpoints failed, persisting empty array");
        "[]".to_string()
    })
}

impl Db {
    pub async fn new(path: &str) -> Result<Self, String> {
        let conn = AsyncConnection::open(path).await.map_err(|e| e.to_string())?;
        // pragma 是 connection 级状态，绑定后台线程那条物理连接，设一次永久生效。
        // WAL 下 synchronous=NORMAL 安全；单连接模型下 busy_timeout 实际罕触发，设置无害。
        conn.call(|c| {
            c.execute_batch(
                "PRAGMA journal_mode=WAL; \
                 PRAGMA foreign_keys=ON; \
                 PRAGMA busy_timeout=5000; \
                 PRAGMA synchronous=NORMAL;",
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(Self(conn, Arc::new(DbCache::default())))
    }

    /// 失效全部 setting 缓存槽（写入端粗粒度失效，settings 写入低频，无需按 key 精修）。
    fn invalidate_settings_cache(&self) {
        if let Ok(mut g) = self.1.settings.write() {
            g.clear();
        }
    }

    /// 失效 list_groups 缓存（任意 group 表写入后调用）。
    fn invalidate_groups_cache(&self) {
        if let Ok(mut g) = self.1.groups.write() {
            *g = None;
        }
    }

    /// 同时失效 setting + group 两类热路径缓存。
    /// 供绕过 set_setting/group 函数直接写表的路径（如 import_export 事务批量写入）调用，
    /// 防止 setting/group 表被旁路改写后缓存仍返回旧值。
    pub fn invalidate_hot_caches(&self) {
        self.invalidate_settings_cache();
        self.invalidate_groups_cache();
    }

    pub async fn init_tables(&self) -> Result<(), String> {
        self.0
            .call(|conn| {
                conn.execute_batch(include_str!("../../migrations/001_init.sql"))?;
                conn.execute_batch(include_str!("../../migrations/002_log_filter_indexes.sql"))?;
                conn.execute_batch(include_str!("../../migrations/003_model_price.sql"))?;
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
                // Migration 008: proxy_log 预估花费列
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN est_cost REAL NOT NULL DEFAULT 0", []);
                // Migration 009: platform 手动预算列（无上游 quota 平台手动限额 + 耗尽阻断）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN manual_budgets TEXT NOT NULL DEFAULT '[]'", []);
                // Migration 010: proxy_log 流式标记列（流式 SSE 请求显式标记，替代 response_body=="[stream]" 哨兵）
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN is_stream INTEGER NOT NULL DEFAULT 0", []);
                // Migration 011: 多平台重试 + 401/403 自动禁用 + 尝试记录（见 migrations/007_retry_failover.sql）
                // platform 三态 status + 退避字段；enabled 列保留向后兼容（写入端从 status 同步）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN status TEXT NOT NULL DEFAULT 'enabled'", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN auto_disabled_until INTEGER NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN auto_disable_strikes INTEGER NOT NULL DEFAULT 0", []);
                // 数据迁移：旧 enabled=0 → status='disabled'（幂等：仅作用于仍为默认 'enabled' 的行，
                // 绝不覆盖 auto_disabled，避免重启误判用户禁用 vs 自动禁用）
                let _ = conn.execute("UPDATE platform SET status = 'disabled' WHERE enabled = 0 AND status = 'enabled'", []);
                // group 分组级最大重试次数
                let _ = conn.execute("ALTER TABLE \"group\" ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 2", []);
                // proxy_log 每次尝试快照（JSON 数组）+ 重试次数
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN attempts TEXT NOT NULL DEFAULT '[]'", []);
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0", []);
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
                                && ep.client_type == ClientType::CodexTui
                            {
                                ep.client_type = ClientType::ClaudeCode;
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
                // Migration 014: proxy_log 中间件拦截审计列（C2 入站 block）。
                // blocked_by = 命中规则标识（rule_type#id name）；blocked_reason = 人读拦截原因。
                // 空值表示未被拦截。拦截类请求不计费（est_cost 仍为 0），但写完整审计行。
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN blocked_by TEXT NOT NULL DEFAULT ''", []);
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN blocked_reason TEXT NOT NULL DEFAULT ''", []);
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
                // Migration 017: 系统通知收件箱表（N1 — 系统通知模块）。
                // notify(type) → InboxOnly/PopupOnly/Full 落库一行；前端通知中心 list/clear 消费。
                // 设置（NotificationSettings）走 settings KV scope=notification，不在此表。
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
                // Migration 019: usage stats 覆盖索引（问题3 数据层优化）。
                // today/group/platform stats 聚合走 created_at 范围扫 + SUM(est_cost/tokens)；
                // 此覆盖索引让计费聚合无需回表（index-only scan），命中 deleted_at=0 部分索引。
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_stats \
                     ON proxy_log(created_at, est_cost, input_tokens, output_tokens, cache_tokens, status_code) \
                     WHERE deleted_at = 0",
                    [],
                );
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
                // Migration 021: model_price 加模型信息列（max_tokens / context_window）。
                // 列为索引快速读取（出站裁剪、列表展示）；price_data JSON 仍存完整原始数据。
                // NULL = 未知/无限制。源见 migrations/008_model_info_columns.sql。
                let _ = conn.execute("ALTER TABLE model_price ADD COLUMN max_input_tokens INTEGER", []);
                let _ = conn.execute("ALTER TABLE model_price ADD COLUMN max_output_tokens INTEGER", []);
                let _ = conn.execute("ALTER TABLE model_price ADD COLUMN context_window INTEGER", []);
                // Migration 022: platform auto_group 开关（false = 不建/不维护默认分组，
                // ensure_platform_groups 永久跳过）。DEFAULT 1 = 老平台保持旧行为。
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN auto_group INTEGER NOT NULL DEFAULT 1", []);
                // Migration 023: 移除 group.path（路由纯按 apikey=group_key）+ name 加 UNIQUE。
                // 门控：仅老库（仍有 path 列）重建。009 重建出的新表无 group_key 列，会触发 010
                // 用 name 兜底重建 group_key —— 若 009 无门控每次启动重跑，group_key 会被反复
                // 覆盖回 name（含中文 name 时污染路由键）。已迁移库无 path 列 → 跳过 → group_key 稳定。
                let has_group_path = conn
                    .prepare("PRAGMA table_info(\"group\")")?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .any(|c| c == "path");
                if has_group_path {
                    conn.execute_batch(include_str!("../../migrations/009_drop_group_path.sql"))?;
                }
                // Migration 024: group 拆 group_key（密钥/路由/日志归属键）+ name（显示名）。
                // group_key UNIQUE: Bearer token + 路由匹配键 + proxy_log 归属键（前端按 group_key 反查 name 显示）。
                // name UNIQUE: 防重名。老 group.group_key 初值 = 旧 name（statusline 脚本/已分发 token 不破）。
                // 幂等：PRAGMA 探测 group_key 列存在性，已迁移则跳过重建。
                let has_group_key = conn
                    .prepare("PRAGMA table_info(\"group\")")?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .any(|c| c == "group_key");
                if !has_group_key {
                    conn.execute_batch(include_str!("../../migrations/010_group_key.sql"))?;
                }
                // proxy_log.group_key → group_key（幂等：探测列存在性）。
                let has_log_group_key = conn
                    .prepare("PRAGMA table_info(proxy_log)")?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .any(|c| c == "group_key");
                if !has_log_group_key {
                    let _ = conn.execute(
                        "ALTER TABLE proxy_log RENAME COLUMN group_name TO group_key",
                        [],
                    );
                }
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())
    }
}

/// 内置预设手机号正则（中国大陆 11 位 + 通用国际 E.164 形式）。
/// C2 无内置手机检测器，故此规则走显式 regex（content_filter match_type=regex），
/// 与 C2 的密钥/邮箱内置检测器（content_filter 空 pattern）互补不冲突。
pub(crate) const BUILTIN_PHONE_PATTERN: &str =
    r"(?:\+?\d{1,3}[\s\-]?)?1[3-9]\d{9}|\+\d{6,15}";

/// 单条内置规则种子定义。INSERT 时按 (name, is_builtin=1) 幂等。
struct BuiltinRuleSpec {
    name: &'static str,
    description: &'static str,
    rule_type: &'static str,
    match_type: &'static str,
    /// 空 pattern → content_filter 类复用 C2 内置密钥/邮箱检测器（BUILTIN_SECRET/EMAIL_PATTERN）。
    pattern: &'static str,
    action: &'static str,
    config: &'static str,
    priority: i64,
}

/// 内置预设规则清单（密钥/邮箱/手机脱敏 + 默认 error_rules 分类）。
/// 密钥/邮箱用 content_filter 空 pattern 复用 C2 内置检测器；手机用显式 regex。
/// error_rules 覆盖 research category 集，pattern 用 (?i) 不区分大小写匹配上游错误消息。
fn builtin_rule_specs() -> &'static [BuiltinRuleSpec] {
    &[
        // ── 脱敏/内容过滤（content_filter，action=mask，global，就近覆盖语义下作为最底层默认）──
        BuiltinRuleSpec {
            name: "内置·密钥脱敏",
            description: "脱敏常见 API key（sk-/ghp_/AKIA/AIza/xox 等）。复用引擎内置密钥检测器。",
            rule_type: "content_filter",
            match_type: "regex",
            pattern: "", // 空 → C2 BUILTIN_SECRET_PATTERN 检测器
            action: "mask",
            config: r#"{"replacement":"****","fields":["messages","system"]}"#,
            priority: 10,
        },
        BuiltinRuleSpec {
            name: "内置·邮箱脱敏",
            description: "脱敏邮箱地址。复用引擎内置邮箱检测器。",
            rule_type: "content_filter",
            match_type: "regex",
            pattern: "", // 空 → C2 BUILTIN_EMAIL_PATTERN 检测器
            action: "mask",
            config: r#"{"replacement":"****","fields":["messages","system"]}"#,
            priority: 11,
        },
        BuiltinRuleSpec {
            name: "内置·手机号脱敏",
            description: "脱敏手机号（中国大陆 11 位 + E.164 国际形式）。",
            rule_type: "content_filter",
            match_type: "regex",
            pattern: BUILTIN_PHONE_PATTERN,
            action: "mask",
            config: r#"{"replacement":"****","fields":["messages","system"]}"#,
            priority: 12,
        },
        // ── 默认 error_rules（error_rule，action=classify，global）──
        BuiltinRuleSpec {
            name: "内置·上下文超限",
            description: "上游报上下文/prompt 过长 → prompt_limit（不可重试，换候选无益）。",
            rule_type: "error_rule",
            match_type: "regex",
            pattern: r"(?i)(context length|context window|maximum context|prompt is too long|too many tokens|reduce the length|maximum.*tokens)",
            action: "classify",
            config: r#"{"category":"prompt_limit","retryable":false}"#,
            priority: 20,
        },
        BuiltinRuleSpec {
            name: "内置·内容审查拦截",
            description: "上游内容安全过滤拦截 → content_filter（不可重试）。",
            rule_type: "error_rule",
            match_type: "regex",
            pattern: r"(?i)(content filter|content_filter|content policy|safety|flagged|moderation|responsible_ai_policy)",
            action: "classify",
            config: r#"{"category":"content_filter","retryable":false}"#,
            priority: 21,
        },
        BuiltinRuleSpec {
            name: "内置·PDF/文件超限",
            description: "上游报 PDF/文件页数或大小超限 → pdf_limit（不可重试）。",
            rule_type: "error_rule",
            match_type: "regex",
            pattern: r"(?i)(pdf.*(too many pages|exceed|too large|limit)|too many pages|file.*too large|maximum.*pages)",
            action: "classify",
            config: r#"{"category":"pdf_limit","retryable":false}"#,
            priority: 22,
        },
        BuiltinRuleSpec {
            name: "内置·思考链错误",
            description: "上游报 thinking/reasoning 字段错误 → thinking_error（不可重试）。",
            rule_type: "error_rule",
            match_type: "regex",
            pattern: r"(?i)(thinking|reasoning).*(not (supported|allowed|enabled)|invalid|must be|required|error)",
            action: "classify",
            config: r#"{"category":"thinking_error","retryable":false}"#,
            priority: 23,
        },
        BuiltinRuleSpec {
            name: "内置·参数错误",
            description: "上游报参数非法 → parameter_error（不可重试，换候选同样会失败）。",
            rule_type: "error_rule",
            match_type: "regex",
            pattern: r"(?i)(invalid.*parameter|unsupported parameter|unknown parameter|parameter.*(invalid|not supported)|unexpected.*field)",
            action: "classify",
            config: r#"{"category":"parameter_error","retryable":false}"#,
            priority: 24,
        },
        BuiltinRuleSpec {
            name: "内置·非法请求",
            description: "上游报 invalid_request → invalid_request（不可重试）。",
            rule_type: "error_rule",
            match_type: "regex",
            pattern: r"(?i)(invalid_request_error|invalid request|bad request|malformed)",
            action: "classify",
            config: r#"{"category":"invalid_request","retryable":false}"#,
            priority: 25,
        },
        BuiltinRuleSpec {
            name: "内置·缓存超限",
            description: "上游报 prompt cache 写入/数量超限 → cache_limit（不可重试）。",
            rule_type: "error_rule",
            match_type: "regex",
            pattern: r"(?i)(cache.*(limit|exceed|too many)|prompt cache|cache_control.*(limit|exceed|maximum))",
            action: "classify",
            config: r#"{"category":"cache_limit","retryable":false}"#,
            priority: 26,
        },
    ]
}

/// 首启 seed 内置预设中间件规则（C4）。幂等：按 (name, is_builtin=1) 判定，
/// 已存在跳过——不重新插入也不重新启用（尊重用户对内置规则的禁用状态，内置规则可禁不可硬删）。
/// 在 [`Db::init_tables`] migration 末尾、同一 connection 闭包内同步调用。
fn seed_builtin_middleware_rules(conn: &rusqlite::Connection) -> SqlResult<()> {
    let ts = now();
    let mut inserted = 0u32;
    for spec in builtin_rule_specs() {
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM middleware_rule WHERE name = ?1 AND is_builtin = 1 LIMIT 1",
                params![spec.name],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if exists {
            continue;
        }
        conn.execute(
            "INSERT INTO middleware_rule
               (name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'global', '', ?4, ?5, ?6, ?7, ?8, 1, 1, ?9, ?9)",
            params![
                spec.name,
                spec.description,
                spec.rule_type,
                spec.match_type,
                spec.pattern,
                spec.action,
                spec.config,
                spec.priority,
                ts,
            ],
        )?;
        inserted += 1;
    }
    if inserted > 0 {
        tracing::info!(inserted, "migration 015: seeded builtin middleware rules");
    }
    Ok(())
}

/// 当前毫秒级 Unix 时间戳
pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// 计算保留期截止时间戳（毫秒）。`days == 0` 表示跳过清理，返回 None。
fn retention_cutoff(days: u32) -> Option<i64> {
    if days == 0 {
        return None;
    }
    Some((chrono::Utc::now() - chrono::Duration::days(days as i64)).timestamp_millis())
}

// ─── Platform CRUD ─────────────────────────────────────────

/// SELECT 列序
const PLATFORM_COLUMNS: &str =
    "id, name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at, est_balance_remaining, est_coding_plan, last_real_query_at, estimate_count, show_in_tray, tray_display, sort_order, manual_budgets, status, auto_disabled_until, auto_disable_strikes, breaker_failure_threshold, breaker_open_secs, breaker_half_open_max, auto_group";

/// 同 PLATFORM_COLUMNS，但每列加 `p.` 限定，用于与其他表 JOIN 时消除同名列歧义（如 created_at/updated_at）
const PLATFORM_COLUMNS_PREFIXED: &str =
    "p.id, p.name, p.platform_type, p.base_url, p.api_key, p.extra, p.models, p.available_models, p.endpoints, p.enabled, p.created_at, p.updated_at, p.est_balance_remaining, p.est_coding_plan, p.last_real_query_at, p.estimate_count, p.show_in_tray, p.tray_display, p.sort_order, p.manual_budgets, p.status, p.auto_disabled_until, p.auto_disable_strikes, p.breaker_failure_threshold, p.breaker_open_secs, p.breaker_half_open_max, p.auto_group";

/// 从查询行构造 Platform
fn row_to_platform(row: &rusqlite::Row) -> SqlResult<Platform> {
    let platform_type_str: String = row.get(2)?;
    let models_str: String = row.get(6)?;
    let available_str: String = row.get(7)?;
    let endpoints_str: String = row.get(8)?;
    Ok(Platform {
        id: row.get::<_, i64>(0)? as u64,
        name: row.get(1)?,
        platform_type: serde_json::from_str(&platform_type_str).unwrap(),
        base_url: row.get(3)?,
        api_key: row.get(4)?,
        extra: row.get(5)?,
        models: parse_models(&models_str),
        available_models: parse_available_models(&available_str),
        endpoints: parse_endpoints(&endpoints_str),
        enabled: row.get::<_, i64>(9)? == 1,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        deleted_at: 0,
        est_balance_remaining: row.get(12)?,
        est_coding_plan: row.get(13)?,
        last_real_query_at: row.get(14)?,
        estimate_count: row.get(15)?,
        show_in_tray: row.get::<_, i64>(16)? == 1,
        tray_display: row.get(17)?,
        sort_order: row.get::<_, i64>(18)?,
        manual_budgets: super::models::parse_manual_budgets(&row.get::<_, String>(19)?),
        status: super::models::PlatformStatus::from_db_str(&row.get::<_, String>(20)?),
        auto_disabled_until: row.get::<_, i64>(21)?,
        auto_disable_strikes: row.get::<_, i64>(22)?,
        breaker_failure_threshold: row.get::<_, i64>(23)? as u32,
        breaker_open_secs: row.get::<_, i64>(24)? as u64,
        breaker_half_open_max: row.get::<_, i64>(25)? as u32,
        auto_group: row.get::<_, i64>(26)? == 1,
        balance_level: String::new(),
    })
}

pub async fn create_platform(db: &Db, mut input: CreatePlatform) -> Result<Platform, String> {
    let ts = now();
    let platform_type_str = serde_json::to_string(&input.platform_type).unwrap();
    // If name is empty, auto-generate: {platform_type}-{random8}
    if input.name.trim().is_empty() {
        let proto_label = format!("{:?}", input.platform_type).to_lowercase();
        let rand_suffix = &uuid::Uuid::new_v4().simple().to_string()[..8];
        input.name = format!("{}-{}", proto_label, rand_suffix);
    }
    let models = input.models.unwrap_or_default();
    let models_str = serialize_models(&models);
    let available_models = input.available_models.unwrap_or_default();
    let available_str = serialize_available_models(&available_models);
    let endpoints = input.endpoints.unwrap_or_default();
    let endpoints_str = serialize_endpoints(&endpoints);
    let manual_budgets = input.manual_budgets.unwrap_or_default();
    let manual_budgets_str = super::models::serialize_manual_budgets(&manual_budgets);

    let id = db
        .0
        .call({
            let name = input.name.clone();
            let base_url = input.base_url.clone();
            let api_key = input.api_key.clone();
            let extra = input.extra.clone();
            let auto_group = input.auto_group.unwrap_or(true) as i64;
            move |conn| {
                conn.execute(
                    "INSERT INTO platform (name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at, manual_budgets, auto_group) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![name, platform_type_str, base_url, api_key, extra, models_str, available_str, endpoints_str, true as i64, ts, ts, manual_budgets_str, auto_group],
                )?;
                Ok(conn.last_insert_rowid() as u64)
            }
        })
        .await
        .map_err(|e| format!("create platform: {e}"))?;

    Ok(Platform {
        id,
        name: input.name,
        platform_type: input.platform_type,
        base_url: input.base_url,
        api_key: input.api_key,
        extra: input.extra,
        models,
        available_models,
        endpoints,
        enabled: true,
        created_at: ts,
        updated_at: ts,
        deleted_at: 0,
        est_balance_remaining: 0.0,
        est_coding_plan: String::new(),
        last_real_query_at: 0,
        estimate_count: 0,
        show_in_tray: false,
        tray_display: "balance".to_string(),
        sort_order: 0,
        auto_group: input.auto_group.unwrap_or(true),
        manual_budgets,
        status: super::models::PlatformStatus::Enabled,
        auto_disabled_until: 0,
        auto_disable_strikes: 0,
        breaker_failure_threshold: 0,
        breaker_open_secs: 0,
        breaker_half_open_max: 0,
        balance_level: String::new(),
    })
}

pub async fn list_platforms(db: &Db) -> Result<Vec<Platform>, String> {
    db.0
        .call(|conn| {
            let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE deleted_at = 0 ORDER BY sort_order, created_at");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], row_to_platform)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_platform(db: &Db, id: u64) -> Result<Option<Platform>, String> {
    db.0
        .call(move |conn| {
            let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE id = ?1 AND deleted_at = 0");
            let mut stmt = conn.prepare(&sql)?;
            Ok(stmt.query_row(params![id as i64], row_to_platform).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn update_platform(db: &Db, input: UpdatePlatform) -> Result<Platform, String> {
    let existing = get_platform(db, input.id).await?.ok_or("platform not found")?;

    // 手动预算：编辑表单只提供配置（kind/unit/amount/window_hours/enabled），
    // consumed/window_start_at 由系统维护——按 id 对齐既有项，保留运行时累计值，
    // 避免每次保存清零已用额度。新增项（id 无匹配）保留传入 consumed（默认 0）。
    let manual_budgets = match input.manual_budgets {
        Some(incoming) => incoming
            .into_iter()
            .map(|mut b| {
                if let Some(prev) = existing.manual_budgets.iter().find(|p| p.id == b.id) {
                    b.consumed = prev.consumed;
                    b.window_start_at = prev.window_start_at;
                }
                b
            })
            .collect(),
        None => existing.manual_budgets.clone(),
    };

    // ── 三态 status 解析（优先级：显式 status > 旧 enabled 兼容入参 > 既有值）──
    // 前端三态切换走 status；旧前端 / 旧调用仍可只传 enabled（true→Enabled, false→Disabled）。
    // 禁止从前端入参置 AutoDisabled（仅系统 401/403 联动 set_platform_auto_disabled 设置）。
    use super::models::PlatformStatus;
    let mut new_status = match input.status {
        Some(PlatformStatus::AutoDisabled) => existing.status, // 拒绝外部置自动禁用，保持原状
        Some(s) => s,
        None => match input.enabled {
            Some(true) => PlatformStatus::Enabled,
            Some(false) => PlatformStatus::Disabled,
            None => existing.status,
        },
    };
    let mut auto_disabled_until = existing.auto_disabled_until;
    let mut auto_disable_strikes = existing.auto_disable_strikes;

    // 手动重新启用 auto_disabled / disabled 平台 → 清退避状态
    if new_status == PlatformStatus::Enabled {
        auto_disabled_until = 0;
        auto_disable_strikes = 0;
    }

    // ── 改 api_key 自恢复：当前 auto_disabled 且 api_key 变化 → 立即恢复 enabled 清退避 ──
    let new_api_key = input.api_key.clone().unwrap_or_else(|| existing.api_key.clone());
    if existing.status == PlatformStatus::AutoDisabled
        && new_api_key != existing.api_key
        && new_status == PlatformStatus::AutoDisabled
    {
        new_status = PlatformStatus::Enabled;
        auto_disabled_until = 0;
        auto_disable_strikes = 0;
    }

    let updated = Platform {
        name: input.name.unwrap_or(existing.name),
        platform_type: input.platform_type.unwrap_or(existing.platform_type),
        base_url: input.base_url.unwrap_or(existing.base_url),
        api_key: input.api_key.unwrap_or(existing.api_key),
        extra: input.extra.unwrap_or(existing.extra),
        models: input.models.unwrap_or(existing.models),
        available_models: input.available_models.unwrap_or(existing.available_models),
        endpoints: input.endpoints.unwrap_or(existing.endpoints),
        // enabled 列从 status 同步（向后兼容）：仅 Enabled → true
        enabled: new_status == PlatformStatus::Enabled,
        status: new_status,
        auto_disabled_until,
        auto_disable_strikes,
        breaker_failure_threshold: input.breaker_failure_threshold.unwrap_or(existing.breaker_failure_threshold),
        breaker_open_secs: input.breaker_open_secs.unwrap_or(existing.breaker_open_secs),
        breaker_half_open_max: input.breaker_half_open_max.unwrap_or(existing.breaker_half_open_max),
        manual_budgets,
        auto_group: input.auto_group.unwrap_or(existing.auto_group),
        updated_at: now(),
        ..existing
    };

    let platform_type_str = serde_json::to_string(&updated.platform_type).unwrap();
    let models_str = serialize_models(&updated.models);
    let available_str = serialize_available_models(&updated.available_models);
    let endpoints_str = serialize_endpoints(&updated.endpoints);
    let manual_budgets_str = super::models::serialize_manual_budgets(&updated.manual_budgets);
    db.0
        .call({
            let name = updated.name.clone();
            let base_url = updated.base_url.clone();
            let api_key = updated.api_key.clone();
            let extra = updated.extra.clone();
            let enabled = updated.enabled as i64;
            let status_str = updated.status.as_db_str().to_string();
            let auto_disabled_until = updated.auto_disabled_until;
            let auto_disable_strikes = updated.auto_disable_strikes;
            let breaker_failure_threshold = updated.breaker_failure_threshold as i64;
            let breaker_open_secs = updated.breaker_open_secs as i64;
            let breaker_half_open_max = updated.breaker_half_open_max as i64;
            let auto_group = updated.auto_group as i64;
            let updated_at = updated.updated_at;
            let id = updated.id as i64;
            move |conn| {
                conn.execute(
                    "UPDATE platform SET name=?1, platform_type=?2, base_url=?3, api_key=?4, extra=?5, models=?6, available_models=?7, endpoints=?8, enabled=?9, updated_at=?10, manual_budgets=?11, status=?12, auto_disabled_until=?13, auto_disable_strikes=?14, breaker_failure_threshold=?15, breaker_open_secs=?16, breaker_half_open_max=?17, auto_group=?18 WHERE id=?19",
                    params![
                        name,
                        platform_type_str,
                        base_url,
                        api_key,
                        extra,
                        models_str,
                        available_str,
                        endpoints_str,
                        enabled,
                        updated_at,
                        manual_budgets_str,
                        status_str,
                        auto_disabled_until,
                        auto_disable_strikes,
                        breaker_failure_threshold,
                        breaker_open_secs,
                        breaker_half_open_max,
                        auto_group,
                        id,
                    ],
                )?;
                Ok(())
            }
        })
        .await
        .map_err(|e| format!("update platform: {e}"))?;

    Ok(updated)
}

/// 自动禁用退避基础时长（1 小时，毫秒）；第 n 次禁用退避 = BASE * 2^(strikes-1)。
const AUTO_DISABLE_BASE_MS: i64 = 60 * 60 * 1000;
/// 退避指数上限（防溢出 / 过长）：strikes 超过此值后退避封顶。
const AUTO_DISABLE_MAX_STRIKES: i64 = 12; // 2^11 h ≈ 85 天封顶

/// 401/403 触发：将平台标记 auto_disabled，strikes++，按指数退避计算下次试探时间。
/// 仅在当前非用户手动 disabled 时生效（不覆盖用户主动关闭的平台）。
/// 返回更新后的退避截止时间戳（毫秒），供日志记录。
pub async fn set_platform_auto_disabled(db: &Db, id: u64) -> Result<i64, String> {
    let ts = now();
    db.0
        .call(move |conn| {
            // 读当前状态 + strikes（仅对 enabled / auto_disabled 生效，跳过用户 disabled）
            let row: Option<(String, i64)> = conn
                .query_row(
                    "SELECT status, auto_disable_strikes FROM platform WHERE id = ?1 AND deleted_at = 0",
                    params![id as i64],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
                )
                .optional()?;
            let (status, strikes) = match row {
                Some(v) => v,
                None => return Ok(0i64),
            };
            // 用户手动禁用 → 不动（避免 401/403 把用户禁用平台改成自动禁用语义）
            if status == "disabled" {
                return Ok(0i64);
            }
            let new_strikes = (strikes + 1).min(AUTO_DISABLE_MAX_STRIKES);
            let backoff = AUTO_DISABLE_BASE_MS.saturating_mul(1i64 << (new_strikes - 1).max(0));
            let until = ts + backoff;
            conn.execute(
                "UPDATE platform SET status='auto_disabled', enabled=0, auto_disable_strikes=?1, auto_disabled_until=?2, updated_at=?3 WHERE id=?4",
                params![new_strikes, until, ts, id as i64],
            )?;
            Ok(until)
        })
        .await
        .map_err(|e| format!("set platform auto-disabled: {e}"))
}

/// 连续多少次 404/405（死端点信号）后才临时禁用平台。
/// 语义区别 401/403（鉴权失败，单次即禁）：404/405 = 端点不存在 / 方法不允许，
/// 可能为上游瞬时配置抖动，须连续累计到阈值再禁，避免偶发 405 误伤健康平台。
pub const DEAD_ENDPOINT_STRIKE_THRESHOLD: i64 = 3;

/// 404/405 触发（死端点信号）：累计连续失败次数 auto_disable_strikes++。
/// 仅当累计达到 `threshold` 时才把平台标记 auto_disabled（指数退避），换下个候选；
/// 未达阈值仅递增计数、保持 enabled（继续参与调度），返回 until=0。
/// 语义：404=端点不存在 / 405=方法不允许 → 该上游路径是死端点；连续 N 次确认非瞬时后隔离。
/// 退避复用 401/403 同一指数机制（基于达阈后的额外 strikes 计算），不另起一套退避。
/// 仅在当前非用户手动 disabled 时生效；返回 (新 strikes, 退避截止时间戳 / 未禁则 0)。
pub async fn record_dead_endpoint_strike(
    db: &Db,
    id: u64,
    threshold: i64,
) -> Result<(i64, i64), String> {
    let ts = now();
    db.0
        .call(move |conn| {
            let row: Option<(String, i64)> = conn
                .query_row(
                    "SELECT status, auto_disable_strikes FROM platform WHERE id = ?1 AND deleted_at = 0",
                    params![id as i64],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
                )
                .optional()?;
            let (status, strikes) = match row {
                Some(v) => v,
                None => return Ok((0i64, 0i64)),
            };
            // 用户手动禁用 → 不动（保持手动语义，不被死端点信号改成自动禁用）
            if status == "disabled" {
                return Ok((0i64, 0i64));
            }
            let new_strikes = (strikes + 1).min(AUTO_DISABLE_MAX_STRIKES);
            // 未达阈值：仅累计计数，保持 enabled，继续参与调度（容忍瞬时 404/405）
            if new_strikes < threshold.max(1) {
                conn.execute(
                    "UPDATE platform SET auto_disable_strikes=?1, updated_at=?2 WHERE id=?3",
                    params![new_strikes, ts, id as i64],
                )?;
                return Ok((new_strikes, 0i64));
            }
            // 达阈值：临时禁用 + 指数退避（退避指数按达阈后的额外 strikes 算，与 401/403 一致量级）
            let over = (new_strikes - threshold.max(1)).max(0); // 0,1,2,... → 1h,2h,4h,...
            let backoff = AUTO_DISABLE_BASE_MS.saturating_mul(1i64 << over.min(AUTO_DISABLE_MAX_STRIKES - 1));
            let until = ts + backoff;
            conn.execute(
                "UPDATE platform SET status='auto_disabled', enabled=0, auto_disable_strikes=?1, auto_disabled_until=?2, updated_at=?3 WHERE id=?4",
                params![new_strikes, until, ts, id as i64],
            )?;
            Ok((new_strikes, until))
        })
        .await
        .map_err(|e| format!("record dead-endpoint strike: {e}"))
}

/// 2xx 成功且平台仍 enabled 但有累计 strikes（死端点累计未达阈值）：清零计数。
/// 一次成功即证明上游端点并非死端点，连续累计须从头重数（避免跨越长时间的偶发失败误累计）。
/// 仅作用于 enabled 平台；auto_disabled 平台的恢复走 recover_platform_auto_disabled。
pub async fn reset_dead_endpoint_strikes(db: &Db, id: u64) -> Result<(), String> {
    let ts = now();
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET auto_disable_strikes=0, updated_at=?1 WHERE id=?2 AND status='enabled' AND auto_disable_strikes>0",
                params![ts, id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("reset dead-endpoint strikes: {e}"))
}

/// 2xx 成功：若平台当前为 auto_disabled（试探成功），恢复 enabled 并清退避状态。
/// 用户手动 disabled / 已 enabled 平台不动。
pub async fn recover_platform_auto_disabled(db: &Db, id: u64) -> Result<(), String> {
    let ts = now();
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET status='enabled', enabled=1, auto_disable_strikes=0, auto_disabled_until=0, updated_at=?1 WHERE id=?2 AND status='auto_disabled'",
                params![ts, id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("recover platform auto-disabled: {e}"))
}

/// 将 quota 查询结果写回 platform 表（余额 + coding plan JSON）。
/// 直写 est_balance/est_coding_plan（不校准、不重置基线）。
/// 已被 estimate::calibrate_from_quota 取代（真查须严格对齐 est=真实 + 重置拟合基线），保留备用。
#[allow(dead_code)]
pub async fn update_platform_quota(db: &Db, id: u64, balance: f64, coding_plan_json: &str) -> Result<(), String> {
    let coding_plan_json = coding_plan_json.to_string();
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET est_balance_remaining = ?1, est_coding_plan = ?2 WHERE id = ?3",
                params![balance, coding_plan_json, id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("update platform quota: {e}"))
}

pub async fn delete_platform(db: &Db, id: u64) -> Result<(), String> {
    // 删除关联的自动分组
    let auto_group_ids: Vec<i64> = db
        .0
        .call(move |conn| {
            Ok(conn.prepare("SELECT id FROM \"group\" WHERE auto_from_platform = ?1 AND deleted_at = 0")?
                .query_map(params![id.to_string()], |row| row.get(0))?
                .collect::<SqlResult<Vec<i64>>>()?)
        })
        .await
        .map_err(|e| e.to_string())?;

    for gid in &auto_group_ids {
        force_delete_group(db, *gid as u64).await?;
    }

    db.0
        .call(move |conn| {
            conn.execute("UPDATE platform SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete platform: {e}"))
}

// ─── Tray 展示（互斥单平台）─────────────────────────────────

/// 互斥设置 tray 展示平台：单事务先清所有 show_in_tray，再置选中平台为 1。
/// `tray_display`: "balance" | "coding"。
pub async fn set_tray_platform(db: &Db, platform_id: u64, tray_display: &str) -> Result<(), String> {
    let display = if tray_display == "coding" { "coding" } else { "balance" }.to_string();
    let ts = now();
    db.0
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute("UPDATE platform SET show_in_tray = 0, updated_at = ?1 WHERE show_in_tray = 1", params![ts])?;
            tx.execute(
                "UPDATE platform SET show_in_tray = 1, tray_display = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at = 0",
                params![display, ts, platform_id as i64],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| format!("set tray: {e}"))
}

/// 清空所有 tray 展示（关闭）。
pub async fn clear_tray(db: &Db) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute("UPDATE platform SET show_in_tray = 0, updated_at = ?1 WHERE show_in_tray = 1", params![now()])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("clear tray: {e}"))
}

/// 取当前 tray 展示平台（show_in_tray = 1），无则 None。
pub async fn get_tray_platform(db: &Db) -> Result<Option<Platform>, String> {
    db.0
        .call(|conn| {
            let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE show_in_tray = 1 AND deleted_at = 0 LIMIT 1");
            let mut stmt = conn.prepare(&sql)?;
            Ok(stmt.query_row([], row_to_platform).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
}

// ─── Tray Config (settings: scope="tray", key="config") ────

/// 读取 TrayConfig。无配置时（首次/升级）从旧 `show_in_tray=1` 平台迁移生成默认配置并持久化。
/// 返回 None 仅当迁移后仍无任何 enabled 平台（即旧配置也为空）。
pub async fn get_tray_config(db: &Db) -> Result<Option<TrayConfig>, String> {
    if let Some(v) = get_setting(db, "tray", "config").await? {
        if !v.is_null() {
            // 旧全局 layout(single_line/two_line) → 各 item line_mode 迁移：
            // 解析前先抓顶层 layout，若旧配置含该字段则映射到所有 item（two_line→"two" / 其他→"single"）。
            let legacy_line_mode = v
                .get("layout")
                .and_then(|l| l.as_str())
                .map(|l| if l == "two_line" { "two" } else { "single" }.to_string());
            // 容错解析：损坏配置回退默认空（避免整条链 panic）。layout 为未知字段，serde 默认忽略。
            let mut cfg: TrayConfig = serde_json::from_value(v).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "tray config JSON is corrupt, falling back to empty default");
                TrayConfig::default()
            });
            if let Some(lm) = legacy_line_mode {
                for item in &mut cfg.items {
                    item.line_mode = lm.clone();
                }
            }
            return Ok(Some(cfg));
        }
    }
    // 迁移：无 tray config → 从旧 show_in_tray=1 平台生成默认。
    let migrated = migrate_tray_config(db).await?;
    Ok(migrated)
}

/// 从旧 `show_in_tray=1` 平台生成默认 TrayConfig 并存入 settings。
/// 无旧平台 → 存空配置（避免每次启动重复迁移），返回空配置。
async fn migrate_tray_config(db: &Db) -> Result<Option<TrayConfig>, String> {
    let legacy = get_tray_platform(db).await?;
    let mut cfg = TrayConfig::default();
    if let Some(p) = legacy {
        let display = if p.tray_display == "coding" { "coding" } else { "balance" };
        cfg.items.push(TrayItem {
            item_type: "platform".to_string(),
            platform_id: Some(p.id),
            display: display.to_string(),
            metric: None,
            label: None,
decimals: None,
            color: TrayColor::default(),
            font_size: 9.0,
            line_mode: "single".to_string(),
            align: "left".to_string(),
            align_row2: None,
            enabled: true,
            order: 0,
        });
    }
    set_tray_config(db, &cfg).await?;
    Ok(Some(cfg))
}

/// 写入 TrayConfig 到 settings。
pub async fn set_tray_config(db: &Db, cfg: &TrayConfig) -> Result<(), String> {
    let value = serde_json::to_value(cfg).map_err(|e| format!("serialize tray config: {e}"))?;
    set_setting(db, SetSettingInput {
        scope: "tray".to_string(),
        key: "config".to_string(),
        value,
    })
    .await
}

/// 今日（本地时区 00:00 起）累计 token 总量（input + output），未删除日志。
#[cfg(test)]
pub async fn today_token_total(db: &Db) -> Result<i64, String> {
    use chrono::{Local, TimeZone};
    let today = Local::now().date_naive();
    let start_dt = today.and_hms_opt(0, 0, 0).ok_or("invalid local midnight")?;
    let start_local = Local
        .from_local_datetime(&start_dt)
        .single()
        .ok_or("ambiguous local midnight")?;
    let start_ms = start_local.timestamp_millis();

    db.0
        .call(move |conn| {
            Ok(conn.query_row(
                "SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM proxy_log WHERE created_at >= ?1 AND deleted_at = 0",
                params![start_ms],
                |row| row.get(0),
            )?)
        })
        .await
        .map_err(|e| format!("today token total: {e}"))
}

/// 今日统计摘要（供托盘预览使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodayStats {
    /// 今日总 token 数（input + output）
    pub tokens: i64,
    /// 今日 cache 命中率（cache_tokens / input_tokens * 100）
    pub cache_rate: f64,
    /// 今日预估花费（$），基于 model_price 定价
    pub cost: f64,
    /// 今日总请求数
    pub total_requests: i64,
}

/// 获取今日统计（本地时区 00:00 起，未删除日志）
pub async fn today_stats(db: &Db) -> Result<TodayStats, String> {
    use chrono::{Local, TimeZone};
    let today = Local::now().date_naive();
    let start_dt = today.and_hms_opt(0, 0, 0).ok_or("invalid local midnight")?;
    let start_local = Local
        .from_local_datetime(&start_dt)
        .single()
        .ok_or("ambiguous local midnight")?;
    let start_ms = start_local.timestamp_millis();

    db.0
        .call(move |conn| {
            // 基础统计
            let (tokens, cache_tokens, input_tokens, total_requests): (i64, i64, i64, i64) = conn
                .query_row(
                    "SELECT COALESCE(SUM(input_tokens + output_tokens), 0), \
                     COALESCE(SUM(cache_tokens), 0), \
                     COALESCE(SUM(input_tokens), 0), \
                     COUNT(*) \
                     FROM proxy_log WHERE created_at >= ?1 AND deleted_at = 0",
                    params![start_ms],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?;

            let cache_rate = if input_tokens + cache_tokens > 0 {
                cache_tokens as f64 / (input_tokens + cache_tokens) as f64 * 100.0
            } else {
                0.0
            };

            // 计算花费：直接使用持久化的 est_cost
            let cost: f64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(est_cost), 0.0) FROM proxy_log WHERE created_at >= ?1 AND deleted_at = 0",
                    params![start_ms],
                    |row| row.get(0),
                )
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "today cost sum query failed, reporting cost=0.0");
                    0.0
                });

            Ok(TodayStats {
                tokens,
                cache_rate,
                cost,
                total_requests,
            })
        })
        .await
        .map_err(|e| format!("today stats: {e}"))
}

/// 单平台当日使用统计（供 popover「各平台当日」展示）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodayPlatformStat {
    /// 归属平台 id（platform_id=0 自动分组日志已回溯到源平台）。
    pub platform_id: u64,
    /// 平台名（回溯失败 / 平台已删则为空，前端归「未知平台」）。
    pub platform_name: String,
    /// 当日 token 总量（input + output）。
    pub tokens: i64,
    /// 当日预估花费（$）。
    pub cost: f64,
    /// 当日请求数。
    pub requests: i64,
}

/// 各平台当日使用（本地时区 00:00 起，未删除日志），只返回有用量（已用）的平台。
///
/// platform_id=0 的自动分组日志经 `group.auto_from_platform` 回溯到源平台后归并，
/// 回溯不到（auto 分组已删 / 非 auto 分组的 platform_id=0）则归 platform_id=0（前端显「未知平台」）。
/// 平台名 JOIN platform 表（含已软删平台，名仍可显示；查不到则空字符串）。
pub async fn today_platform_stats(db: &Db) -> Result<Vec<TodayPlatformStat>, String> {
    use chrono::{Local, TimeZone};
    let today = Local::now().date_naive();
    let start_dt = today.and_hms_opt(0, 0, 0).ok_or("invalid local midnight")?;
    let start_local = Local
        .from_local_datetime(&start_dt)
        .single()
        .ok_or("ambiguous local midnight")?;
    let start_ms = start_local.timestamp_millis();

    db.0
        .call(move |conn| {
            // 有效平台 id = COALESCE(自动分组回溯, 原 platform_id)。
            // 自动分组日志 platform_id=0，通过 group_key → "group".auto_from_platform（十进制字符串）回溯。
            // GROUP BY 该有效 id，天然只含当日有日志（已用）的平台。
            let sql = "
                SELECT eff_pid,
                       COALESCE(SUM(input_tokens + output_tokens), 0) AS tokens,
                       COALESCE(SUM(est_cost), 0.0) AS cost,
                       COUNT(*) AS reqs
                FROM (
                    SELECT
                        CASE WHEN platform_id = 0 THEN COALESCE(
                            (SELECT CAST(g.auto_from_platform AS INTEGER)
                             FROM \"group\" g
                             WHERE g.name = proxy_log.group_key
                               AND g.auto_from_platform != ''
                               AND g.deleted_at = 0
                             LIMIT 1), 0)
                        ELSE platform_id END AS eff_pid,
                        input_tokens, output_tokens, est_cost
                    FROM proxy_log
                    WHERE created_at >= ?1 AND deleted_at = 0
                )
                GROUP BY eff_pid
                ORDER BY cost DESC, tokens DESC";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt
                .query_map(params![start_ms], |row| {
                    let pid: i64 = row.get(0)?;
                    Ok((pid, row.get::<_, i64>(1)?, row.get::<_, f64>(2)?, row.get::<_, i64>(3)?))
                })?
                .collect::<SqlResult<Vec<_>>>()?;

            // 平台名映射（含软删平台，名仍可显示）。
            let mut name_stmt = conn.prepare("SELECT id, name FROM platform")?;
            let names: std::collections::HashMap<i64, String> = name_stmt
                .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?
                .collect::<SqlResult<Vec<_>>>()?
                .into_iter()
                .collect();

            Ok(rows
                .into_iter()
                .map(|(pid, tokens, cost, reqs)| TodayPlatformStat {
                    platform_id: pid.max(0) as u64,
                    platform_name: names.get(&pid).cloned().unwrap_or_default(),
                    tokens,
                    cost,
                    requests: reqs,
                })
                .collect())
        })
        .await
        .map_err(|e| format!("today platform stats: {e}"))
}

// ─── Popover Config (settings: scope="popover", key="config") ─

/// 读取 PopoverConfig。无配置 / 损坏 → 默认配置（不持久化，按需懒生成）。
pub async fn get_popover_config(db: &Db) -> Result<super::models::PopoverConfig, String> {
    if let Some(v) = get_setting(db, "popover", "config").await? {
        if !v.is_null() {
            let cfg: super::models::PopoverConfig =
                serde_json::from_value(v).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "popover config JSON is corrupt, falling back to default");
                    super::models::PopoverConfig::default()
                });
            return Ok(cfg);
        }
    }
    Ok(super::models::PopoverConfig::default())
}

/// 写入 PopoverConfig 到 settings。
pub async fn set_popover_config(db: &Db, cfg: &super::models::PopoverConfig) -> Result<(), String> {
    let value = serde_json::to_value(cfg).map_err(|e| format!("serialize popover config: {e}"))?;
    set_setting(db, SetSettingInput {
        scope: "popover".to_string(),
        key: "config".to_string(),
        value,
    })
    .await
}

/// 根据 model_price 定价计算单次请求预估花费（$）
///
/// 复用 `resolve_price` 的回退链（pricing[platform_type] > top_level >
/// default_platform > fallback 默认价），与 preview 命令 `model_price_resolve` 行为一致：
/// 无模型价 / 价为 0 时回退到 `PriceSyncSettings` 的 fallback 默认价（默认 3.0 $/M），不再返回 0。
///
/// 锁安全：本函数不持有 `db.0.lock()`；`get_sync_settings` / `resolve_price`
/// （内部 `get_model_price`）各自获取并释放 db 锁，不会重入死锁。
///
/// `platform_type` 传入平台主类型的 serde 裸名（如 `"deepseek"`）以启用 pricing override；
/// 传 `""` 时 override 不命中，但回退链仍保证非 0。
pub async fn calc_est_cost(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
) -> f64 {
    let settings = super::price_sync::get_sync_settings(db).await;
    let rp = resolve_price(
        db,
        model_name,
        platform_type,
        settings.fallback_input_price,
        settings.fallback_output_price,
        input_tokens as i64,
    )
    .await
    .unwrap_or_else(|_| crate::gateway::models::ResolvedPrice {
        // 安全默认：直接用 fallback 默认价（$/M → $/token），保证非 0、不 panic
        input_cost_per_token: settings.fallback_input_price / 1_000_000.0,
        output_cost_per_token: settings.fallback_output_price / 1_000_000.0,
        cache_read_input_token_cost: 0.0,
        source: "fallback".to_string(),
    });

    input_tokens as f64 * rp.input_cost_per_token
        + output_tokens as f64 * rp.output_cost_per_token
        + cache_tokens as f64 * rp.cache_read_input_token_cost
}

// ─── Group CRUD ────────────────────────────────────────────

/// 序列化 / 反序列化内联 model_mappings
fn serialize_mappings(mappings: &[ModelMapping]) -> String {
    serde_json::to_string(mappings).unwrap_or_else(|_| "[]".to_string())
}

fn parse_mappings(json: &str) -> Vec<ModelMapping> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Group SELECT 列序
const GROUP_COLUMNS: &str =
    "id, name, routing_mode, auto_from_platform, created_at, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, model_mappings, sort_order, max_retries, group_key";

fn row_to_group(row: &rusqlite::Row) -> SqlResult<Group> {
    let routing_str: String = row.get(2)?;
    let mappings_str: String = row.get(9)?;
    Ok(Group {
        id: row.get::<_, i64>(0)? as u64,
        name: row.get(1)?,
        routing_mode: serde_json::from_str(&routing_str).unwrap(),
        auto_from_platform: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        request_timeout_secs: row.get::<_, i64>(6)? as u64,
        connect_timeout_secs: row.get::<_, i64>(7)? as u64,
        source_protocol: row.get::<_, String>(8)?,
        model_mappings: parse_mappings(&mappings_str),
        deleted_at: 0,
        sort_order: row.get::<_, i64>(10)?,
        max_retries: row.get::<_, i64>(11)? as u32,
        group_key: row.get(12)?,
    })
}

pub async fn create_group(db: &Db, input: CreateGroup) -> Result<Group, String> {
    let ts = now();
    let routing_str = serde_json::to_string(&input.routing_mode).unwrap();
    let source_protocol = input.source_protocol.unwrap_or_else(|| "anthropic".to_string());
    let mappings_str = serialize_mappings(&input.model_mappings);
    // group_key：用户提供则用，否则自动生成 gk_<32hex>（创建后锁定不可改）。
    let group_key = input
        .group_key
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| format!("gk_{}", uuid::Uuid::new_v4().simple()));

    let id = db
        .0
        .call({
            let name = input.name.clone();
            let group_key = group_key.clone();
            let auto_from_platform = input.auto_from_platform.clone();
            let request_timeout_secs = input.request_timeout_secs as i64;
            let connect_timeout_secs = input.connect_timeout_secs as i64;
            let source_protocol = source_protocol.clone();
            let max_retries = input.max_retries as i64;
            move |conn| {
                conn.execute(
                    "INSERT INTO \"group\" (name, group_key, routing_mode, auto_from_platform, created_at, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, model_mappings, max_retries) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![name, group_key, routing_str, auto_from_platform, ts, ts, request_timeout_secs, connect_timeout_secs, source_protocol, mappings_str, max_retries],
                )?;
                Ok(conn.last_insert_rowid() as u64)
            }
        })
        .await
        .map_err(|e| format!("create group: {e}"))?;
    db.invalidate_groups_cache();

    Ok(Group {
        id,
        name: input.name,
        group_key,
        routing_mode: input.routing_mode,
        auto_from_platform: input.auto_from_platform,
        created_at: ts,
        updated_at: ts,
        request_timeout_secs: input.request_timeout_secs,
        connect_timeout_secs: input.connect_timeout_secs,
        source_protocol,
        model_mappings: input.model_mappings,
        deleted_at: 0,
        sort_order: 0,
        max_retries: input.max_retries,
    })
}

/// 批量更新 group 的 sort_order：接收有序 id 列表，按序赋值 1, 2, 3, …
pub async fn reorder_groups(db: &Db, ordered_ids: &[u64]) -> Result<(), String> {
    let ordered_ids = ordered_ids.to_vec();
    db.0
        .call(move |conn| {
            for (i, &id) in ordered_ids.iter().enumerate() {
                conn.execute(
                    "UPDATE \"group\" SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                    params![(i + 1) as i64, now(), id as i64],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("reorder group: {e}"))?;
    db.invalidate_groups_cache();
    Ok(())
}

/// 批量更新 platform 的 sort_order
pub async fn reorder_platforms(db: &Db, ordered_ids: &[u64]) -> Result<(), String> {
    let ordered_ids = ordered_ids.to_vec();
    db.0
        .call(move |conn| {
            for (i, &id) in ordered_ids.iter().enumerate() {
                conn.execute(
                    "UPDATE platform SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                    params![(i + 1) as i64, now(), id as i64],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("reorder platform: {e}"))
}

/// 批量更新某分组内平台的 priority（拖拽排序）。ordered_platform_ids 按序赋 1,2,3…
pub async fn reorder_group_platforms(
    db: &Db,
    group_id: u64,
    ordered_platform_ids: &[u64],
) -> Result<(), String> {
    let group_id = group_id as i64;
    let ordered = ordered_platform_ids.to_vec();
    let ts = now();
    db.0
        .call(move |conn| {
            for (i, &pid) in ordered.iter().enumerate() {
                conn.execute(
                    "UPDATE group_platform SET priority = ?1, updated_at = ?2 \
                     WHERE group_id = ?3 AND platform_id = ?4 AND deleted_at = 0",
                    params![(i + 1) as i64, ts, group_id, pid as i64],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("reorder group platforms: {e}"))
}

/// 跨分组移动平台：从 from 组移除、加入 to 组（priority = to 组现有最大 + 1）。
pub async fn move_group_platform(
    db: &Db,
    platform_id: u64,
    from_group_id: u64,
    to_group_id: u64,
) -> Result<(), String> {
    let pid = platform_id as i64;
    let from = from_group_id as i64;
    let to = to_group_id as i64;
    let ts = now();
    db.0
        .call(move |conn| {
            conn.execute(
                "DELETE FROM group_platform WHERE group_id = ?1 AND platform_id = ?2 AND deleted_at = 0",
                params![from, pid],
            )?;
            // 物理清除目标组内该平台的所有历史行(含软删残留),避免 UNIQUE(group_id,platform_id) 冲突
            // 场景: 平台曾加入该组又移除(软删行 deleted_at≠0 残留), 重新加入时 INSERT 撞 UNIQUE
            conn.execute(
                "DELETE FROM group_platform WHERE group_id = ?1 AND platform_id = ?2",
                params![to, pid],
            )?;
            let max_pri: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(priority), 0) FROM group_platform \
                     WHERE group_id = ?1 AND deleted_at = 0",
                    params![to],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            conn.execute(
                "INSERT INTO group_platform (group_id, platform_id, priority, weight, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, 1, ?4, ?4)",
                params![to, pid, max_pri + 1, ts],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("move group platform: {e}"))
}

pub async fn list_groups(db: &Db) -> Result<Vec<Group>, String> {
    if let Ok(g) = db.1.groups.read() {
        if let Some(cached) = g.as_ref() {
            return Ok(cached.clone());
        }
    }
    let groups = db
        .0
        .call(|conn| {
            let mut stmt = conn.prepare(&format!("SELECT {GROUP_COLUMNS} FROM \"group\" WHERE deleted_at = 0 ORDER BY sort_order, created_at"))?;
            let rows = stmt.query_map([], row_to_group)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())?;
    if let Ok(mut g) = db.1.groups.write() {
        *g = Some(groups.clone());
    }
    Ok(groups)
}

pub async fn get_group(db: &Db, id: u64) -> Result<Option<Group>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(&format!("SELECT {GROUP_COLUMNS} FROM \"group\" WHERE id = ?1 AND deleted_at = 0"))?;
            Ok(stmt.query_row(params![id as i64], row_to_group).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn update_group(db: &Db, input: UpdateGroup) -> Result<Group, String> {
    let existing = get_group(db, input.id).await?.ok_or("group not found")?;

    let updated = Group {
        name: input.name.unwrap_or(existing.name),
        routing_mode: input.routing_mode.unwrap_or(existing.routing_mode),
        request_timeout_secs: if input.request_timeout_secs > 0 { input.request_timeout_secs } else { existing.request_timeout_secs },
        connect_timeout_secs: if input.connect_timeout_secs > 0 { input.connect_timeout_secs } else { existing.connect_timeout_secs },
        source_protocol: input.source_protocol.unwrap_or(existing.source_protocol),
        max_retries: input.max_retries.unwrap_or(existing.max_retries),
        model_mappings: input.model_mappings,
        updated_at: now(),
        ..existing
    };

    let routing_str = serde_json::to_string(&updated.routing_mode).unwrap();
    let mappings_str = serialize_mappings(&updated.model_mappings);
    db.0
        .call({
            let name = updated.name.clone();
            let updated_at = updated.updated_at;
            let request_timeout_secs = updated.request_timeout_secs as i64;
            let connect_timeout_secs = updated.connect_timeout_secs as i64;
            let source_protocol = updated.source_protocol.clone();
            let max_retries = updated.max_retries as i64;
            let id = updated.id as i64;
            move |conn| {
                conn.execute(
                    "UPDATE \"group\" SET name=?1, routing_mode=?2, updated_at=?3, request_timeout_secs=?4, connect_timeout_secs=?5, source_protocol=?6, model_mappings=?7, max_retries=?8 WHERE id=?9",
                    params![name, routing_str, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, mappings_str, max_retries, id],
                )?;
                Ok(())
            }
        })
        .await
        .map_err(|e| format!("update group: {e}"))?;
    db.invalidate_groups_cache();

    Ok(updated)
}

pub async fn delete_group(db: &Db, id: u64) -> Result<(), String> {
    // 检查是否为自动分组
    let group = get_group(db, id).await?.ok_or("group not found")?;
    if !group.auto_from_platform.is_empty() {
        // auto 分组：仅当关联平台已空（源平台已删的孤儿分组）时允许手动删除
        let plats = get_group_platforms(db, id).await?;
        if !plats.is_empty() {
            return Err("auto-created group with linked platforms cannot be deleted manually".to_string());
        }
    }
    force_delete_group(db, id).await
}

/// 强制删除分组（含自动分组），仅供平台删除时内部调用
pub async fn force_delete_group(db: &Db, id: u64) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute("UPDATE \"group\" SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete group: {e}"))?;
    db.invalidate_groups_cache();
    Ok(())
}

// ─── GroupPlatform 关联 ────────────────────────────────────

pub async fn set_group_platforms(
    db: &Db,
    group_id: u64,
    platforms: &[GroupPlatformInput],
) -> Result<(), String> {
    let ts = now();
    let platforms = platforms.to_vec();
    db.0
        .call(move |conn| {
            // 物理清除旧关联后重建（关联表无需软删保留）
            conn.execute(
                "DELETE FROM group_platform WHERE group_id = ?1",
                params![group_id as i64],
            )?;

            for p in &platforms {
                conn.execute(
                    "INSERT INTO group_platform (group_id, platform_id, priority, weight, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![group_id as i64, p.platform_id as i64, p.priority.unwrap_or(0), p.weight.unwrap_or(1), ts, ts],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("set group platforms: {e}"))
}

/// 全量同步某平台的「手动」组成员关系（platform_update 用）：
/// 把 platform 加入 `target_group_ids` 内的每个组、移出不在列表内的手动组。
/// **auto 分组（`group.auto_from_platform` 非空）永不动**——仅操作手动组。
/// group_platform 表本身无 auto 标记，靠 join `group.auto_from_platform` 区分。
pub async fn sync_platform_manual_groups(
    db: &Db,
    platform_id: u64,
    target_group_ids: &[u64],
) -> Result<(), String> {
    // 该平台当前所在的所有 (group_id, auto_from_platform)。
    let current: Vec<(i64, String)> = db
        .0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT g.id, g.auto_from_platform FROM group_platform gp \
                 JOIN \"group\" g ON gp.group_id = g.id \
                 WHERE gp.platform_id = ?1 AND gp.deleted_at = 0 AND g.deleted_at = 0",
            )?;
            let rows = stmt
                .query_map(params![platform_id as i64], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            Ok(rows)
        })
        .await
        .map_err(|e| format!("sync_platform_manual_groups: list current: {e}"))?;

    let target: std::collections::HashSet<i64> =
        target_group_ids.iter().map(|&g| g as i64).collect();

    // 移出：当前在、target 不含、且非 auto 组。
    for (gid, auto_from) in &current {
        if auto_from.is_empty() && !target.contains(gid) {
            let gid = *gid;
            db.0
                .call(move |conn| {
                    conn.execute(
                        "DELETE FROM group_platform WHERE group_id = ?1 AND platform_id = ?2",
                        params![gid, platform_id as i64],
                    )?;
                    Ok(())
                })
                .await
                .map_err(|e| format!("sync_platform_manual_groups: remove from group {gid}: {e}"))?;
        }
    }

    // 加入：target 含、当前不在的组。复用 set_group_platforms 追加本平台（保留组内其他平台）。
    for &gid in &target {
        let already = current.iter().any(|(g, _)| *g == gid);
        if !already {
            let existing = get_group_platforms(db, gid as u64).await.unwrap_or_default();
            let mut inputs: Vec<GroupPlatformInput> = existing
                .into_iter()
                .map(|d| GroupPlatformInput {
                    platform_id: d.platform.id,
                    priority: Some(d.priority),
                    weight: Some(d.weight),
                })
                .collect();
            if !inputs.iter().any(|i| i.platform_id == platform_id) {
                inputs.push(GroupPlatformInput {
                    platform_id,
                    priority: Some(0),
                    weight: Some(1),
                });
            }
            set_group_platforms(db, gid as u64, &inputs).await?;
        }
    }

    Ok(())
}

pub async fn get_group_platforms(db: &Db, group_id: u64) -> Result<Vec<GroupPlatformDetail>, String> {
    db.0
        .call(move |conn| {
    let mut stmt = conn
        .prepare(
            &format!(
                "SELECT gp.priority, gp.weight, {PLATFORM_COLUMNS_PREFIXED} \
                 FROM group_platform gp JOIN platform p ON gp.platform_id = p.id \
                 WHERE gp.group_id = ?1 AND gp.deleted_at = 0 AND p.deleted_at = 0 ORDER BY gp.priority"
            ),
        )?;

    let rows = stmt
        .query_map(params![group_id as i64], |row| {
            // row layout: priority(0), weight(1), then platform columns starting at 2
            let platform_type_str: String = row.get(4)?;
            let models_str: String = row.get(8)?;
            let available_str: String = row.get(9)?;
            let endpoints_str: String = row.get(10)?;
            Ok(GroupPlatformDetail {
                platform: Platform {
                    id: row.get::<_, i64>(2)? as u64,
                    name: row.get(3)?,
                    platform_type: serde_json::from_str(&platform_type_str).unwrap(),
                    base_url: row.get(5)?,
                    api_key: row.get(6)?,
                    extra: row.get(7)?,
                    models: parse_models(&models_str),
                    available_models: parse_available_models(&available_str),
                    endpoints: parse_endpoints(&endpoints_str),
                    enabled: row.get::<_, i64>(11)? == 1,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                    deleted_at: 0,
                    est_balance_remaining: row.get(14)?,
                    est_coding_plan: row.get(15)?,
                    last_real_query_at: row.get(16)?,
                    estimate_count: row.get(17)?,
                    show_in_tray: row.get::<_, i64>(18)? == 1,
                    tray_display: row.get(19)?,
                    sort_order: row.get::<_, i64>(20)?,
                    manual_budgets: super::models::parse_manual_budgets(&row.get::<_, String>(21)?),
                    status: super::models::PlatformStatus::from_db_str(&row.get::<_, String>(22)?),
                    auto_disabled_until: row.get::<_, i64>(23)?,
                    auto_disable_strikes: row.get::<_, i64>(24)?,
                    breaker_failure_threshold: row.get::<_, i64>(25)? as u32,
                    breaker_open_secs: row.get::<_, i64>(26)? as u64,
                    breaker_half_open_max: row.get::<_, i64>(27)? as u32,
                    auto_group: row.get::<_, i64>(28)? == 1,
                    balance_level: String::new(),
                },
                priority: row.get(0)?,
                weight: row.get(1)?,
            })
        })?;

    Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

// ─── 聚合查询 ──────────────────────────────────────────────

pub async fn get_group_detail(db: &Db, id: u64) -> Result<Option<GroupDetail>, String> {
    let group = match get_group(db, id).await? {
        Some(g) => g,
        None => return Ok(None),
    };
    let platforms = get_group_platforms(db, id).await?;
    // GroupDetail 同时携带 group（含其 model_mappings）与独立的 model_mappings 副本，
    // 二者均被消费方读取（见测试 r4_group_detail_mappings_from_group_field），故须 clone 而非 move。
    let model_mappings = group.model_mappings.clone();

    Ok(Some(GroupDetail {
        group,
        platforms,
        model_mappings,
    }))
}

pub async fn list_group_details(db: &Db) -> Result<Vec<GroupDetail>, String> {
    let groups = list_groups(db).await?;
    let mut details = Vec::with_capacity(groups.len());
    for g in groups {
        let platforms = get_group_platforms(db, g.id).await?;
        let model_mappings = g.model_mappings.clone();
        details.push(GroupDetail {
            group: g,
            platforms,
            model_mappings,
        });
    }
    Ok(details)
}

// ─── Settings CRUD ─────────────────────────────────────────

pub async fn get_setting(
    db: &Db,
    scope: &str,
    key: &str,
) -> Result<Option<serde_json::Value>, String> {
    // 缓存命中：热路径（log_settings/lang/sync_settings 每请求多次读）走内存，绕过后台线程往返。
    {
        let cache_key = (scope.to_string(), key.to_string());
        if let Ok(g) = db.1.settings.read() {
            if let Some(hit) = g.get(&cache_key) {
                return Ok(hit.clone());
            }
        }
    }
    let scope = scope.to_string();
    let key = key.to_string();
    let result = db
        .0
        .call({
            let scope = scope.clone();
            let key = key.clone();
            move |conn| {
                let mut stmt = conn.prepare("SELECT value FROM setting WHERE scope = ?1 AND key = ?2 AND deleted_at = 0")?;
                stmt.query_row(params![scope, key], |row| {
                    let v: String = row.get(0)?;
                    Ok(serde_json::from_str(&v).unwrap_or_else(|e| {
                        tracing::warn!(scope = %scope, key = %key, error = %e, "stored setting value is not valid JSON, returning Null");
                        serde_json::Value::Null
                    }))
                })
                .optional()
                .map_err(tokio_rusqlite::Error::from)
            }
        })
        .await
        .map_err(|e| e.to_string())?;
    if let Ok(mut g) = db.1.settings.write() {
        g.insert((scope, key), result.clone());
    }
    Ok(result)
}

pub async fn set_setting(db: &Db, input: SetSettingInput) -> Result<(), String> {
    let ts = now();
    let value_str =
        serde_json::to_string(&input.value).map_err(|e| format!("serialize setting: {e}"))?;
    db.0
        .call(move |conn| {
            conn.execute(
                "INSERT INTO setting (scope, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(scope, key) DO UPDATE SET value = ?3, updated_at = ?4, deleted_at = 0",
                params![input.scope, input.key, value_str, ts],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert setting: {e}"))?;
    db.invalidate_settings_cache();
    Ok(())
}

pub async fn delete_setting(db: &Db, scope: &str, key: &str) -> Result<(), String> {
    let scope = scope.to_string();
    let key = key.to_string();
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE setting SET deleted_at = ?1 WHERE scope = ?2 AND key = ?3",
                params![now(), scope, key],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete setting: {e}"))?;
    db.invalidate_settings_cache();
    Ok(())
}

pub async fn list_setting_keys(db: &Db, scope: &str) -> Result<Vec<String>, String> {
    let scope = scope.to_string();
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare("SELECT key FROM setting WHERE scope = ?1 AND deleted_at = 0 ORDER BY key")?;
            let rows = stmt.query_map(params![scope], |row| row.get(0))?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 导入导出用：列出全部未删除 setting 原始行（scope, key, value_json）。
pub async fn list_all_settings_raw(db: &Db) -> Result<Vec<(String, String, String)>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT scope, key, value FROM setting WHERE deleted_at = 0 ORDER BY scope, key",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 导入导出用：列出 group→platform 全部关联（按名称解析，跨机迁移友好）。
pub async fn list_all_group_platform_pairs(
    db: &Db,
) -> Result<Vec<(String, String)>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT g.name, p.name FROM group_platform gp
                 JOIN \"group\" g ON g.id = gp.group_id
                 JOIN platform p ON p.id = gp.platform_id
                 WHERE gp.deleted_at = 0 ORDER BY g.name, p.name",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

// ─── Middleware Rule CRUD (C1 基座) ────────────────────────

use super::models::{
    CreateMiddlewareRule, MatchType, MiddlewareRule, RuleAction, RuleScope, RuleType,
    UpdateMiddlewareRule,
};

/// middleware_rule 全列序（INSERT 列子集 + SELECT 共用，与表定义列序一致）。
const MIDDLEWARE_RULE_COLUMNS: &str =
    "id, name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at";

/// 从查询行构造 MiddlewareRule。未知 rule_type 不会出现在结果（行被 list 过滤前已按 from_db_str 处理）。
/// 此处 rule_type 用 from_db_str → 未知值兜底为 RequestFilter 会误导，故 list 时遇未知直接跳过（见 list_middleware_rules）。
fn row_to_middleware_rule(row: &rusqlite::Row) -> SqlResult<MiddlewareRule> {
    let rule_type_str: String = row.get(3)?;
    let scope_str: String = row.get(4)?;
    let match_type_str: String = row.get(6)?;
    let action_str: String = row.get(8)?;
    Ok(MiddlewareRule {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        // 未知 rule_type 极少（仅手改 DB）；兜底为 RequestFilter 不影响引擎（引擎按 from_db_str 分桶时同样会跳过未知）。
        rule_type: RuleType::from_db_str(&rule_type_str).unwrap_or(RuleType::RequestFilter),
        scope: RuleScope::from_db_str(&scope_str),
        scope_ref: row.get(5)?,
        match_type: MatchType::from_db_str(&match_type_str),
        pattern: row.get(7)?,
        action: RuleAction::from_db_str(&action_str),
        config: row.get(9)?,
        priority: row.get(10)?,
        enabled: row.get::<_, i64>(11)? == 1,
        is_builtin: row.get::<_, i64>(12)? == 1,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

/// 列出全部中间件规则（按 priority 升序，再 id 升序）。引擎 reload 与前端列表共用。
pub async fn list_middleware_rules(db: &Db) -> Result<Vec<MiddlewareRule>, String> {
    let sql = format!(
        "SELECT {MIDDLEWARE_RULE_COLUMNS} FROM middleware_rule ORDER BY priority ASC, id ASC"
    );
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], row_to_middleware_rule)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn create_middleware_rule(
    db: &Db,
    input: CreateMiddlewareRule,
) -> Result<MiddlewareRule, String> {
    let ts = now();
    let rule_type = input.rule_type.as_str().to_string();
    let scope = input.scope.as_str().to_string();
    let match_type = input.match_type.as_str().to_string();
    let action = input.action.as_str().to_string();
    db.0
        .call(move |conn| {
            conn.execute(
                "INSERT INTO middleware_rule
                   (name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
                params![
                    input.name,
                    input.description,
                    rule_type,
                    scope,
                    input.scope_ref,
                    match_type,
                    input.pattern,
                    action,
                    input.config,
                    input.priority,
                    if input.enabled { 1 } else { 0 },
                    if input.is_builtin { 1 } else { 0 },
                    ts,
                ],
            )?;
            let id = conn.last_insert_rowid();
            let mut stmt = conn.prepare(
                "SELECT id, name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at FROM middleware_rule WHERE id = ?1",
            )?;
            stmt.query_row(params![id], row_to_middleware_rule)
                .map_err(tokio_rusqlite::Error::from)
        })
        .await
        .map_err(|e| format!("create middleware rule: {e}"))
}

pub async fn update_middleware_rule(
    db: &Db,
    input: UpdateMiddlewareRule,
) -> Result<MiddlewareRule, String> {
    let ts = now();
    let rule_type = input.rule_type.as_str().to_string();
    let scope = input.scope.as_str().to_string();
    let match_type = input.match_type.as_str().to_string();
    let action = input.action.as_str().to_string();
    db.0
        .call(move |conn| {
            let affected = conn.execute(
                "UPDATE middleware_rule SET
                   name = ?2, description = ?3, rule_type = ?4, scope = ?5, scope_ref = ?6,
                   match_type = ?7, pattern = ?8, action = ?9, config = ?10, priority = ?11,
                   enabled = ?12, is_builtin = ?13, updated_at = ?14
                 WHERE id = ?1",
                params![
                    input.id,
                    input.name,
                    input.description,
                    rule_type,
                    scope,
                    input.scope_ref,
                    match_type,
                    input.pattern,
                    action,
                    input.config,
                    input.priority,
                    if input.enabled { 1 } else { 0 },
                    if input.is_builtin { 1 } else { 0 },
                    ts,
                ],
            )?;
            if affected == 0 {
                return Err(tokio_rusqlite::Error::Other(
                    format!("middleware rule {} not found", input.id).into(),
                ));
            }
            let mut stmt = conn.prepare(
                "SELECT id, name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at FROM middleware_rule WHERE id = ?1",
            )?;
            stmt.query_row(params![input.id], row_to_middleware_rule)
                .map_err(tokio_rusqlite::Error::from)
        })
        .await
        .map_err(|e| format!("update middleware rule: {e}"))
}

pub async fn delete_middleware_rule(db: &Db, id: i64) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute("DELETE FROM middleware_rule WHERE id = ?1", params![id])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete middleware rule: {e}"))
}

/// 读取中间件总设置（settings scope="middleware" key="settings"）。
/// 无记录或解析失败 → Default（总开关 ON，各类型默认启用）。C2/C3 执行层调用。
pub async fn get_middleware_settings(db: &Db) -> super::models::MiddlewareSettings {
    match get_setting(db, "middleware", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => super::models::MiddlewareSettings::default(),
    }
}

/// 全局调度 + 熔断默认设置（settings scope=`scheduling`, key=`settings`）。
/// 缺省 / 解析失败 → 默认值（5/1800/2，enabled=true，load_balance）。
pub async fn get_scheduling_settings(db: &Db) -> super::models::SchedulingBreakerSettings {
    match get_setting(db, "scheduling", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => super::models::SchedulingBreakerSettings::default(),
    }
}

// ─── Notification（N1 — 系统通知模块）──────────────────────

/// 通知设置（settings scope=`notification`, key=`settings`）。缺省 / 解析失败 → 默认（全开 CrossPlatform）。
pub async fn get_notification_settings(db: &Db) -> super::models::NotificationSettings {
    match get_setting(db, "notification", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => super::models::NotificationSettings::default(),
    }
}

/// 插入收件箱通知，返回新行 id。
pub async fn insert_notification(
    db: &Db,
    notif_type: &str,
    title: &str,
    body: &str,
) -> Result<i64, String> {
    let notif_type = notif_type.to_string();
    let title = title.to_string();
    let body = body.to_string();
    let ts = now();
    db.0
        .call(move |conn| {
            conn.execute(
                "INSERT INTO notification (notif_type, title, body, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![notif_type, title, body, ts],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .map_err(|e| format!("insert notification: {e}"))
}

/// 列收件箱（按 created_at 倒序），limit 上限。
pub async fn list_notifications(
    db: &Db,
    limit: i64,
) -> Result<Vec<super::models::Notification>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, notif_type, title, body, created_at FROM notification ORDER BY created_at DESC, id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(super::models::Notification {
                    id: row.get(0)?,
                    notif_type: row.get(1)?,
                    title: row.get(2)?,
                    body: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 清空收件箱（删全部行）。
pub async fn clear_notifications(db: &Db) -> Result<(), String> {
    db.0
        .call(|conn| {
            conn.execute("DELETE FROM notification", [])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("clear notifications: {e}"))
}

// ─── ProxyLog CRUD ─────────────────────────────────────────

/// proxy_log 全列序（INSERT / 单行 SELECT 共用，与表定义列序一致）
const PROXY_LOG_COLUMNS: &str =
    "id, group_key, model, actual_model, source_protocol, target_protocol, platform_id, request_headers, request_body, upstream_request_headers, upstream_request_body, response_body, request_url, upstream_request_url, upstream_response_headers, upstream_status_code, user_response_headers, user_response_body, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, est_cost, is_stream, attempts, retry_count, blocked_by, blocked_reason, created_at, updated_at, deleted_at";

/// 从查询行构造 ProxyLog（列序须与 PROXY_LOG_COLUMNS 一致）
fn row_to_proxy_log(row: &rusqlite::Row) -> SqlResult<super::models::ProxyLog> {
    Ok(super::models::ProxyLog {
        id: row.get(0)?,
        group_key: row.get(1)?,
        model: row.get(2)?,
        actual_model: row.get(3)?,
        source_protocol: row.get(4)?,
        target_protocol: row.get(5)?,
        platform_id: row.get::<_, i64>(6)? as u64,
        request_headers: row.get(7)?,
        request_body: row.get(8)?,
        upstream_request_headers: row.get(9)?,
        upstream_request_body: row.get(10)?,
        response_body: row.get(11)?,
        request_url: row.get(12)?,
        upstream_request_url: row.get(13)?,
        upstream_response_headers: row.get(14)?,
        upstream_status_code: row.get(15)?,
        user_response_headers: row.get(16)?,
        user_response_body: row.get(17)?,
        status_code: row.get(18)?,
        duration_ms: row.get(19)?,
        input_tokens: row.get(20)?,
        output_tokens: row.get(21)?,
        cache_tokens: row.get(22)?,
        est_cost: row.get(23)?,
        is_stream: row.get::<_, i64>(24)? == 1,
        attempts: super::models::parse_attempts(&row.get::<_, String>(25)?),
        retry_count: row.get(26)?,
        blocked_by: row.get(27)?,
        blocked_reason: row.get(28)?,
        created_at: row.get(29)?,
        updated_at: row.get(30)?,
        deleted_at: row.get(31)?,
    })
}

/// Upsert (INSERT OR REPLACE) a proxy log entry — used for incremental logging.
/// 取 owned `ProxyLog`：调用方（upsert_log）已为脱敏 clone 一份，此处接管所有权
/// 直接 move 进后台线程闭包，消除原先「调用方 clone + 本函数再 clone」的双重全量复制。
pub async fn upsert_proxy_log(db: &Db, log: super::models::ProxyLog) -> Result<(), String> {
    db.0
        .call(move |conn| {
            let attempts_str = super::models::serialize_attempts(&log.attempts);
            conn.execute(
                &format!("INSERT OR REPLACE INTO proxy_log ({PROXY_LOG_COLUMNS})
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32)"),
                params![log.id, log.group_key, log.model, log.actual_model, log.source_protocol, log.target_protocol, log.platform_id as i64, log.request_headers, log.request_body, log.upstream_request_headers, log.upstream_request_body, log.response_body, log.request_url, log.upstream_request_url, log.upstream_response_headers, log.upstream_status_code, log.user_response_headers, log.user_response_body, log.status_code, log.duration_ms, log.input_tokens, log.output_tokens, log.cache_tokens, log.est_cost, log.is_stream as i64, attempts_str, log.retry_count, log.blocked_by, log.blocked_reason, log.created_at, log.updated_at, log.deleted_at],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert proxy log: {e}"))
}

/// 渐进式日志的「DB 就绪列快照」：32 列已转成入库类型（脱敏已在构造时就地应用）。
///
/// 用途：替代每节点全列 INSERT OR REPLACE 重写。构造一次 → 首节点 INSERT 建行，
/// 后续节点与上一快照逐列 diff，仅 UPDATE 变化列。配合 upsert_log 的按需脱敏，
/// 彻底消除 proxy.rs 每次写都 `log.clone()` 整结构的开销。
///
/// 字段顺序与值语义须与 `PROXY_LOG_COLUMNS` / `upsert_proxy_log` 完全一致（字段完整性红线）。
#[derive(Clone, PartialEq)]
pub struct ProxyLogColumns {
    pub id: String,
    pub group_key: String,
    pub model: String,
    pub actual_model: String,
    pub source_protocol: String,
    pub target_protocol: String,
    pub platform_id: i64,
    pub request_headers: String,
    pub request_body: String,
    pub upstream_request_headers: String,
    pub upstream_request_body: String,
    pub response_body: String,
    pub request_url: String,
    pub upstream_request_url: String,
    pub upstream_response_headers: String,
    pub upstream_status_code: i32,
    pub user_response_headers: String,
    pub user_response_body: String,
    pub status_code: i32,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    pub est_cost: f64,
    pub is_stream: i64,
    pub attempts: String,
    pub retry_count: i32,
    pub blocked_by: String,
    pub blocked_reason: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: i64,
}

impl ProxyLogColumns {
    /// 由 `ProxyLog` 构造入库列快照。
    /// `*_headers` 字段（元数据，Authorization 已在上游脱敏为 `[REDACTED]`）始终入库，
    /// 不受 `log_user_request` / `log_upstream_request` 开关控制；仅 `*_body`（prompt / 响应正文，
    /// 含敏感内容）受 `strip_user` / `strip_upstream` 控制就地清空。
    /// attempts 在此序列化一次。仅克隆 String 字段（入库本就需 owned 值），不克隆整 ProxyLog 结构。
    pub fn from_log(log: &super::models::ProxyLog, strip_user: bool, strip_upstream: bool) -> Self {
        let empty = String::new;
        ProxyLogColumns {
            id: log.id.clone(),
            group_key: log.group_key.clone(),
            model: log.model.clone(),
            actual_model: log.actual_model.clone(),
            source_protocol: log.source_protocol.clone(),
            target_protocol: log.target_protocol.clone(),
            platform_id: log.platform_id as i64,
            request_headers: log.request_headers.clone(),
            request_body: if strip_user { empty() } else { log.request_body.clone() },
            upstream_request_headers: log.upstream_request_headers.clone(),
            upstream_request_body: if strip_upstream { empty() } else { log.upstream_request_body.clone() },
            response_body: log.response_body.clone(),
            request_url: log.request_url.clone(),
            upstream_request_url: log.upstream_request_url.clone(),
            upstream_response_headers: log.upstream_response_headers.clone(),
            upstream_status_code: log.upstream_status_code,
            user_response_headers: log.user_response_headers.clone(),
            user_response_body: if strip_user { empty() } else { log.user_response_body.clone() },
            status_code: log.status_code,
            duration_ms: log.duration_ms,
            input_tokens: log.input_tokens,
            output_tokens: log.output_tokens,
            cache_tokens: log.cache_tokens,
            est_cost: log.est_cost,
            is_stream: log.is_stream as i64,
            attempts: super::models::serialize_attempts(&log.attempts),
            retry_count: log.retry_count,
            blocked_by: log.blocked_by.clone(),
            blocked_reason: log.blocked_reason.clone(),
            created_at: log.created_at,
            updated_at: log.updated_at,
            deleted_at: log.deleted_at,
        }
    }

    /// 与上一快照 `old` 逐列对比，返回 (列名, 绑定值) 的变化集。id 主键不在内（用于 WHERE）。
    fn changed_since(&self, old: &ProxyLogColumns) -> Vec<(&'static str, Box<dyn rusqlite::types::ToSql + Send>)> {
        let mut out: Vec<(&'static str, Box<dyn rusqlite::types::ToSql + Send>)> = Vec::new();
        macro_rules! diff {
            ($col:literal, $field:ident) => {
                if self.$field != old.$field {
                    out.push(($col, Box::new(self.$field.clone())));
                }
            };
        }
        diff!("group_key", group_key);
        diff!("model", model);
        diff!("actual_model", actual_model);
        diff!("source_protocol", source_protocol);
        diff!("target_protocol", target_protocol);
        diff!("platform_id", platform_id);
        diff!("request_headers", request_headers);
        diff!("request_body", request_body);
        diff!("upstream_request_headers", upstream_request_headers);
        diff!("upstream_request_body", upstream_request_body);
        diff!("response_body", response_body);
        diff!("request_url", request_url);
        diff!("upstream_request_url", upstream_request_url);
        diff!("upstream_response_headers", upstream_response_headers);
        diff!("upstream_status_code", upstream_status_code);
        diff!("user_response_headers", user_response_headers);
        diff!("user_response_body", user_response_body);
        diff!("status_code", status_code);
        diff!("duration_ms", duration_ms);
        diff!("input_tokens", input_tokens);
        diff!("output_tokens", output_tokens);
        diff!("cache_tokens", cache_tokens);
        diff!("est_cost", est_cost);
        diff!("is_stream", is_stream);
        diff!("attempts", attempts);
        diff!("retry_count", retry_count);
        diff!("blocked_by", blocked_by);
        diff!("blocked_reason", blocked_reason);
        diff!("created_at", created_at);
        diff!("updated_at", updated_at);
        diff!("deleted_at", deleted_at);
        out
    }
}

/// 渐进式日志首节点：INSERT 建行（非 REPLACE，行不应已存在）。失败上抛。
pub async fn insert_proxy_log_columns(db: &Db, cols: ProxyLogColumns) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute(
                &format!("INSERT INTO proxy_log ({PROXY_LOG_COLUMNS})
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32)"),
                params![cols.id, cols.group_key, cols.model, cols.actual_model, cols.source_protocol, cols.target_protocol, cols.platform_id, cols.request_headers, cols.request_body, cols.upstream_request_headers, cols.upstream_request_body, cols.response_body, cols.request_url, cols.upstream_request_url, cols.upstream_response_headers, cols.upstream_status_code, cols.user_response_headers, cols.user_response_body, cols.status_code, cols.duration_ms, cols.input_tokens, cols.output_tokens, cols.cache_tokens, cols.est_cost, cols.is_stream, cols.attempts, cols.retry_count, cols.blocked_by, cols.blocked_reason, cols.created_at, cols.updated_at, cols.deleted_at],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("insert proxy log: {e}"))
}

/// 渐进式日志后续节点：仅 UPDATE 相对 `prev` 变化的列。无变化则 no-op（不发 SQL）。
/// 若目标行不存在（理论不应，节点1 必先 INSERT），UPDATE 影响 0 行，静默（与旧 REPLACE
/// 的「不存在则建行」语义偏离已由 upsert_log 的快照存在性保证：有快照 ⇒ 已 INSERT 过）。
pub async fn update_proxy_log_columns(db: &Db, new: ProxyLogColumns, prev: &ProxyLogColumns) -> Result<(), String> {
    let changed = new.changed_since(prev);
    if changed.is_empty() {
        return Ok(());
    }
    let id = new.id.clone();
    db.0
        .call(move |conn| {
            let set_sql: String = changed
                .iter()
                .enumerate()
                .map(|(i, (col, _))| format!("{col} = ?{}", i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            let id_idx = changed.len() + 1;
            let sql = format!("UPDATE proxy_log SET {set_sql} WHERE id = ?{id_idx}");
            let mut binds: Vec<&dyn rusqlite::types::ToSql> = changed.iter().map(|(_, v)| v.as_ref() as &dyn rusqlite::types::ToSql).collect();
            binds.push(&id);
            conn.execute(&sql, binds.as_slice())?;
            Ok(())
        })
        .await
        .map_err(|e| format!("update proxy log: {e}"))
}

pub async fn list_proxy_logs(db: &Db, limit: u32, offset: u32) -> Result<Vec<super::models::ProxyLogSummary>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, group_key, model, actual_model, source_protocol, target_protocol, platform_id, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at
                 FROM proxy_log WHERE deleted_at = 0 ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
            )?;
            let rows = stmt.query_map(params![limit, offset], row_to_proxy_log_summary)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Summary row mapper (column order must match SELECT)
fn row_to_proxy_log_summary(row: &rusqlite::Row) -> SqlResult<super::models::ProxyLogSummary> {
    Ok(super::models::ProxyLogSummary {
        id: row.get(0)?,
        group_key: row.get(1)?,
        model: row.get(2)?,
        actual_model: row.get(3)?,
        source_protocol: row.get(4)?,
        target_protocol: row.get(5)?,
        platform_id: row.get::<_, i64>(6)? as u64,
        status_code: row.get(7)?,
        duration_ms: row.get(8)?,
        input_tokens: row.get(9)?,
        output_tokens: row.get(10)?,
        cache_tokens: row.get(11)?,
        is_stream: row.get::<_, i64>(12)? == 1,
        retry_count: row.get(13)?,
        created_at: row.get(14)?,
    })
}

pub async fn filtered_list_proxy_logs(
    db: &Db,
    filter: &super::models::ProxyLogFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<super::models::ProxyLogSummary>, String> {
    let filter = filter.clone();
    db.0
        .call(move |conn| {
            let (where_sql, mut p) = build_filter_where(&filter);
            p.push(Box::new(limit));
            p.push(Box::new(offset));
            let sql = format!(
                "SELECT id, group_key, model, actual_model, source_protocol, target_protocol, platform_id, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at \
                 FROM proxy_log WHERE deleted_at = 0{where_sql} ORDER BY created_at DESC LIMIT ? OFFSET ?"
            );
            let mut stmt = conn.prepare(&sql)?;
            let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
            let rows = stmt.query_map(refs.as_slice(), row_to_proxy_log_summary)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn filtered_count_proxy_logs(
    db: &Db,
    filter: &super::models::ProxyLogFilter,
) -> Result<u32, String> {
    let filter = filter.clone();
    db.0
        .call(move |conn| {
            let (where_sql, p) = build_filter_where(&filter);
            let sql = format!("SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0{where_sql}");
            let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
            Ok(conn.query_row(&sql, refs.as_slice(), |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Build WHERE clause extensions + params from filter.
/// Returns (" AND ...", params). Empty filter → ("", []).
fn build_filter_where(filter: &super::models::ProxyLogFilter) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut parts: Vec<String> = Vec::new();
    let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1u32;

    if let Some(ref v) = filter.platform_id {
        parts.push(format!("AND platform_id = ?{idx}"));
        p.push(Box::new(*v as i64));
        idx += 1;
    }
    if let Some(ref v) = filter.group_key {
        parts.push(format!("AND group_key = ?{idx}"));
        p.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(s) = filter.status {
        if s == 200 {
            parts.push("AND status_code >= 200 AND status_code < 300".to_string());
        } else if s == -1 {
            parts.push("AND (status_code < 200 OR status_code >= 300)".to_string());
        } else {
            parts.push(format!("AND status_code = ?{idx}"));
            p.push(Box::new(s));
            idx += 1;
        }
    }
    if let Some(ts) = filter.time_start {
        parts.push(format!("AND created_at >= ?{idx}"));
        p.push(Box::new(ts));
        idx += 1;
    }
    if let Some(ts) = filter.time_end {
        parts.push(format!("AND created_at <= ?{idx}"));
        p.push(Box::new(ts));
        idx += 1;
    }
    if let Some(ref v) = filter.model {
        let col = match filter.model_type.as_deref() {
            Some("actual") => "actual_model",
            _ => "model",
        };
        parts.push(format!("AND {col} = ?{idx}"));
        p.push(Box::new(v.clone()));
    }

    let where_sql = if parts.is_empty() { String::new() } else { format!(" {}", parts.join(" ")) };
    (where_sql, p)
}

pub async fn get_proxy_log(db: &Db, id: &str) -> Result<Option<super::models::ProxyLog>, String> {
    let id = id.to_string();
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {PROXY_LOG_COLUMNS} FROM proxy_log WHERE id = ?1 AND deleted_at = 0"
            ))?;
            Ok(stmt.query_row(params![id], row_to_proxy_log).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn clear_proxy_logs(db: &Db) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute("UPDATE proxy_log SET deleted_at = ?1 WHERE deleted_at = 0", params![now()])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("clear proxy logs: {e}"))
}

/// Delete logs older than N days. Pass 0 to skip.
pub async fn cleanup_proxy_logs(db: &Db, retention_days: u32) -> Result<(), String> {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db.0
        .call(move |conn| {
            conn.execute("UPDATE proxy_log SET deleted_at = ?1 WHERE created_at < ?2 AND deleted_at = 0", params![now(), cutoff])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup proxy logs: {e}"))
}

/// Clear user request body fields for logs older than retention_days.
/// `*_headers`（元数据，已脱敏）始终保留至行级 retention 删除；仅清 `*_body`（prompt / 响应正文）。
/// Does NOT delete the log row — keeps token stats and metadata.
pub async fn cleanup_user_request_fields(db: &Db, retention_days: u32) -> Result<(), String> {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE proxy_log SET request_body = '', user_response_body = '' WHERE created_at < ?1 AND (request_body != '' OR user_response_body != '')",
                params![cutoff],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup user request fields: {e}"))
}

/// Clear upstream request body fields for logs older than retention_days.
/// `*_headers`（元数据，已脱敏）始终保留至行级 retention 删除；仅清 `*_body`（上游请求 / 响应正文）。
/// Does NOT delete the log row — keeps token stats and metadata.
pub async fn cleanup_upstream_request_fields(db: &Db, retention_days: u32) -> Result<(), String> {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE proxy_log SET upstream_request_body = '' WHERE created_at < ?1 AND upstream_request_body != ''",
                params![cutoff],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup upstream request fields: {e}"))
}

pub async fn count_proxy_logs(db: &Db) -> Result<u32, String> {
    db.0
        .call(move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0", [], |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 共用使用量聚合：按给定 WHERE 子句统计总量 + 最近 5 次健康度。
/// `where_clause` 不含 `WHERE` 关键字；`params` 须与 `where_clause` 占位符一一对应。
fn usage_stats(
    conn: &Connection,
    where_clause: &str,
    params: &[&dyn rusqlite::types::ToSql],
) -> SqlResult<super::models::PlatformUsageStats> {
    let stats: super::models::PlatformUsageStats = conn.query_row(
        &format!("SELECT COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), \
         COALESCE(SUM(est_cost), 0.0) \
         FROM proxy_log WHERE {where_clause}"),
        params,
        |row| {
            let total: i64 = row.get(0).unwrap_or(0);
            let success: i64 = row.get(1).unwrap_or(0);
            let inp: i64 = row.get(2).unwrap_or(0);
            let out: i64 = row.get(3).unwrap_or(0);
            let cache: i64 = row.get(4).unwrap_or(0);
            let cost: f64 = row.get(5).unwrap_or(0.0);
            Ok(super::models::PlatformUsageStats {
                total_requests: total,
                success_count: success,
                total_input_tokens: inp,
                total_output_tokens: out,
                total_cache_tokens: cache,
                cache_rate: if inp + cache > 0 { cache as f64 / (inp + cache) as f64 * 100.0 } else { 0.0 },
                recent_failures: 0,
                recent_total: 0,
                total_cost: cost,
            })
        },
    )?;

    // Recent 5 requests health
    let (recent_failures, recent_total): (i64, i64) = conn.query_row(
        &format!("SELECT COUNT(*), SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END) \
         FROM (SELECT status_code FROM proxy_log WHERE {where_clause} ORDER BY created_at DESC LIMIT 5)"),
        params,
        |row| Ok((row.get(1).unwrap_or(0), row.get(0).unwrap_or(0))),
    ).unwrap_or((0, 0));

    Ok(super::models::PlatformUsageStats {
        recent_failures,
        recent_total,
        ..stats
    })
}

pub async fn get_platform_usage_stats(db: &Db, platform_id: u64) -> Result<super::models::PlatformUsageStats, String> {
    db.0
        .call(move |conn| {
            // platform_id 现为整数；自动分组日志可能未带 platform_id（=0），通过 group.auto_from_platform（存十进制字符串）回溯
            let where_clause = "deleted_at = 0 AND (platform_id = ?1 OR (platform_id = 0 AND group_key IN (SELECT name FROM \"group\" WHERE auto_from_platform = ?2 AND deleted_at = 0)))";
            let pid = platform_id as i64;
            let pid_str = platform_id.to_string();
            Ok(usage_stats(conn, where_clause, &[&pid, &pid_str])?)
        })
        .await
        .map_err(|e| format!("platform usage stats: {e}"))
}

pub async fn get_group_usage_stats(db: &Db, group_key: &str) -> Result<super::models::PlatformUsageStats, String> {
    let group_key = group_key.to_string();
    db.0
        .call(move |conn| {
            Ok(usage_stats(conn, "group_key = ?1 AND deleted_at = 0", &[&group_key])?)
        })
        .await
        .map_err(|e| format!("group usage stats: {e}"))
}

/// 批量：单查 `GROUP BY group_key` 返回所有 group → 聚合 map（问题6 N+1 消除）。
/// 替代前端逐 group 调 `get_group_usage_stats`（N 次往返 → 1 次）。
/// `GROUP BY group_key` 天然满足 CLAUDE.md「共享平台不重复计入」：日志按 group_key 归属，
/// 同一平台被多 group 共享时各 group 只统计经本 group 进来的请求，无重复。
/// recent_failures/recent_total/cache_rate 不在批量结果内（Groups 页不渲染，避免每组 5 行子查询）。
pub async fn get_all_group_usage_stats(
    db: &Db,
) -> Result<std::collections::HashMap<String, super::models::PlatformUsageStats>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT group_key, COUNT(*), \
                 SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
                 SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), \
                 COALESCE(SUM(est_cost), 0.0) \
                 FROM proxy_log WHERE deleted_at = 0 AND group_key <> '' \
                 GROUP BY group_key",
            )?;
            let rows = stmt.query_map([], |row| {
                let group_key: String = row.get(0)?;
                let total: i64 = row.get(1).unwrap_or(0);
                let success: i64 = row.get(2).unwrap_or(0);
                let inp: i64 = row.get(3).unwrap_or(0);
                let out: i64 = row.get(4).unwrap_or(0);
                let cache: i64 = row.get(5).unwrap_or(0);
                let cost: f64 = row.get(6).unwrap_or(0.0);
                Ok((
                    group_key,
                    super::models::PlatformUsageStats {
                        total_requests: total,
                        success_count: success,
                        total_input_tokens: inp,
                        total_output_tokens: out,
                        total_cache_tokens: cache,
                        cache_rate: if inp + cache > 0 { cache as f64 / (inp + cache) as f64 * 100.0 } else { 0.0 },
                        recent_failures: 0,
                        recent_total: 0,
                        total_cost: cost,
                    },
                ))
            })?;
            let mut map = std::collections::HashMap::new();
            for r in rows {
                let (name, stats) = r?;
                map.insert(name, stats);
            }
            Ok(map)
        })
        .await
        .map_err(|e| format!("all group usage stats: {e}"))
}

/// 动态窗口日速率公共常量。
const RATE_MIN_SPAN_MS: i64 = 5 * 60 * 1000; // 5min
const RATE_MAX_SPAN_MS: i64 = 7 * 24 * 60 * 60 * 1000; // 7d

/// 动态窗口日用量速率核心（同步，锁内调用）。
///
/// 算法（prd B）：`?1` = window_start（now-7d），`scope_sql` 为附加维度过滤（group / platform），
/// `scope_params` 从 `?3` 起绑定。span = clamp(now - 最早有效 est_cost 数据时间, 5min, 7d)，
/// `rate_per_hour = SUM(est_cost in span) / span_hours`。无任何用量 → None。
fn hourly_rate_inner(
    conn: &Connection,
    now_ms: i64,
    window_start: i64,
    scope_sql: &str,
    scope_params: &[&dyn rusqlite::types::ToSql],
) -> SqlResult<Option<f64>> {
    let mut binds: Vec<&dyn rusqlite::types::ToSql> = vec![&window_start];
    binds.extend_from_slice(scope_params);
    // 7d 窗口内最早一条有 est_cost(>0) 数据的时间。
    let earliest_sql = format!(
        "SELECT MIN(created_at) FROM proxy_log \
         WHERE created_at >= ?1 AND deleted_at = 0 AND est_cost > 0 AND ({scope_sql})"
    );
    let earliest: Option<i64> = conn
        .query_row(&earliest_sql, binds.as_slice(), |row| row.get(0))
        .optional()?
        .flatten();
    let earliest = match earliest {
        Some(e) => e,
        None => return Ok(None), // 无任何用量 → None
    };
    let total_sql = format!(
        "SELECT COALESCE(SUM(est_cost), 0.0) FROM proxy_log \
         WHERE created_at >= ?1 AND deleted_at = 0 AND ({scope_sql})"
    );
    let total: f64 = conn.query_row(&total_sql, binds.as_slice(), |row| row.get(0))?;
    if total <= 0.0 {
        return Ok(None);
    }
    // span = clamp(now - earliest, 5min, 7d)
    let span_ms = (now_ms - earliest).clamp(RATE_MIN_SPAN_MS, RATE_MAX_SPAN_MS);
    let span_hours = span_ms as f64 / 3_600_000.0;
    Ok(Some(total / span_hours))
}

/// 分组动态窗口日用量速率（$ / 小时），供 statusline 余额「剩余可用天数」配色。
/// 无任何用量 → None（配色侧视作中性 / 不报警）。短持锁，不跨 await。
pub async fn get_group_hourly_rate(db: &Db, group_key: &str) -> Result<Option<f64>, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let window_start = now_ms - RATE_MAX_SPAN_MS;
    let group_key = group_key.to_string();
    db.0
        .call(move |conn| {
            Ok(hourly_rate_inner(conn, now_ms, window_start, "group_key = ?2", &[&group_key])?)
        })
        .await
        .map_err(|e| format!("group hourly rate: {e}"))
}

/// 单平台动态窗口日用量速率（$ / 小时），供 Platforms 列表页余额按速率配色。
///
/// platform 维度过滤同 `get_platform_usage_stats`：自动分组日志可能 platform_id=0，
/// 经 group.auto_from_platform 回溯。无任何用量 → None（前端退中性）。短持锁，不跨 await。
pub async fn get_platform_hourly_rate(db: &Db, platform_id: u64) -> Result<Option<f64>, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let window_start = now_ms - RATE_MAX_SPAN_MS;
    db.0
        .call(move |conn| {
            let pid = platform_id as i64;
            let pid_str = platform_id.to_string();
            let scope = "platform_id = ?2 OR (platform_id = 0 AND group_key IN (SELECT name FROM \"group\" WHERE auto_from_platform = ?3 AND deleted_at = 0))";
            Ok(hourly_rate_inner(conn, now_ms, window_start, scope, &[&pid, &pid_str])?)
        })
        .await
        .map_err(|e| format!("platform hourly rate: {e}"))
}

struct QueryParams {
    start: i64,
    end: i64,
    filter_group: Option<String>,
    filter_model: Option<String>,
    filter_platform: Option<String>,
}

impl QueryParams {
    fn to_sql_params(&self) -> Vec<Box<dyn rusqlite::types::ToSql>> {
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(self.start),
            Box::new(self.end),
        ];
        if let Some(ref v) = self.filter_group { p.push(Box::new(v.clone())); }
        if let Some(ref v) = self.filter_model { p.push(Box::new(v.clone())); }
        if let Some(ref v) = self.filter_platform { p.push(Box::new(v.clone())); }
        p
    }
}

/// 时间分桶 SQL 表达式（select 列）。粒度决定分桶宽度：
/// - `minute` → 每分钟一桶 `%Y-%m-%d %H:%M`（不带秒：前端 x 轴标注取末 5 字符须为 HH:MM）
/// - `5min`   → 每 5 分钟一桶；strftime 无原生 floor，先把 epoch 秒整除 300 再 *300 向下取整到 5min 边界
/// - `hourly` → 每小时一桶 `%Y-%m-%d %H:00`
/// - 其余（含 `daily`/None）→ 每天一桶 `%Y-%m-%d`
fn bucket_time_expr(granularity: Option<&str>) -> String {
    match granularity {
        Some("minute") => "strftime('%Y-%m-%d %H:%M', created_at/1000, 'unixepoch')".to_string(),
        // epoch 秒 floor 到 300s 边界后再格式化为分钟（桶 key 形如 "2026-06-16 10:05"）
        Some("5min") => {
            "strftime('%Y-%m-%d %H:%M', (created_at/1000/300)*300, 'unixepoch')".to_string()
        }
        Some("hourly") => "strftime('%Y-%m-%d %H:00', created_at/1000, 'unixepoch')".to_string(),
        _ => "strftime('%Y-%m-%d', created_at/1000, 'unixepoch')".to_string(),
    }
}

pub async fn query_stats(db: &Db, query: &StatsQuery) -> Result<StatsResult, String> {
    let query = query.clone();
    db.0
        .call(move |conn| {
            query_stats_inner(conn, &query)
                .map_err(|e| tokio_rusqlite::Error::Other(e.into()))
        })
        .await
        .map_err(|e| e.to_string())
}

fn query_stats_inner(conn: &Connection, query: &StatsQuery) -> Result<StatsResult, String> {
    let end = query.end.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let start = query.start.unwrap_or_else(|| {
        (chrono::Utc::now() - chrono::Duration::days(7)).timestamp_millis()
    });

    let qp = QueryParams {
        start,
        end,
        filter_group: query.filter_group.clone(),
        filter_model: query.filter_model.clone(),
        filter_platform: query.filter_platform.clone(),
    };

    // 有效 platform_id 表达式：原 platform_id，auto 分组（platform_id=0）经
    // group.auto_from_platform 回溯到源平台（与 get_platform_usage_stats 同语义）。
    const EFF_PID: &str = "\
CASE WHEN proxy_log.platform_id = 0 THEN COALESCE(\
(SELECT CAST(g.auto_from_platform AS INTEGER) FROM \"group\" g \
 WHERE g.name = proxy_log.group_key AND g.auto_from_platform != '' AND g.deleted_at = 0 LIMIT 1), 0)\
ELSE proxy_log.platform_id END";

    // Build WHERE clause（列名一律 proxy_log. 前缀：dimension platform 分支 LEFT JOIN platform 后，
    // deleted_at / created_at 等列两表皆有，裸列名会触发 ambiguous column 错误）
    let mut where_parts = vec!["proxy_log.created_at >= ?1".to_string(), "proxy_log.created_at <= ?2".to_string()];
    if qp.filter_group.is_some() {
        where_parts.push("proxy_log.group_key = ?3".to_string());
    }
    if qp.filter_model.is_some() {
        let idx = 3 + qp.filter_group.is_some() as usize;
        where_parts.push(format!("(proxy_log.model = ?{idx} OR proxy_log.actual_model = ?{idx})"));
    }
    if qp.filter_platform.is_some() {
        let idx = 3 + qp.filter_group.is_some() as usize + qp.filter_model.is_some() as usize;
        // value = platform_id 十进制字符串；按有效平台 id（含 auto 分组回溯）匹配
        where_parts.push(format!("({EFF_PID}) = CAST(?{idx} AS INTEGER)"));
    }
    let where_sql = where_parts.join(" AND ");

    let bucket_expr = bucket_time_expr(query.granularity.as_deref());

    // ── Overview ──
    let overview_sql = format!(
        "SELECT COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
         COALESCE(SUM(est_cost), 0.0) \
         FROM proxy_log WHERE deleted_at = 0 AND {where_sql}"
    );
    let p = qp.to_sql_params();
    let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
    let overview = conn.prepare(&overview_sql)
        .map_err(|e| e.to_string())?
        .query_row(refs.as_slice(), |row| {
            let total: i32 = row.get(0).unwrap_or(0);
            let success: i32 = row.get(1).unwrap_or(0);
            Ok(StatsOverview {
                total_requests: total,
                success_rate: if total > 0 { success as f64 / total as f64 * 100.0 } else { 0.0 },
                total_input_tokens: row.get(2).unwrap_or(0),
                total_output_tokens: row.get(3).unwrap_or(0),
                total_cache_tokens: row.get(4).unwrap_or(0),
                cache_rate: {
                    let inp: i64 = row.get(2).unwrap_or(0);
                    let cache: i64 = row.get(4).unwrap_or(0);
                    if inp + cache > 0 { cache as f64 / (inp + cache) as f64 * 100.0 } else { 0.0 }
                },
                avg_duration_ms: row.get(5).unwrap_or(0.0),
                total_cost: row.get(6).unwrap_or(0.0),
            })
        }).map_err(|e| format!("overview: {e}"))?;

    // ── Time buckets ──
    let bucket_sql = format!(
        "SELECT {bucket_expr}, COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
         COALESCE(SUM(est_cost), 0.0) \
         FROM proxy_log WHERE deleted_at = 0 AND {where_sql} GROUP BY 1 ORDER BY 1"
    );
    let p = qp.to_sql_params();
    let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
    let buckets: Vec<StatsBucket> = conn.prepare(&bucket_sql)
        .map_err(|e| e.to_string())?
        .query_map(refs.as_slice(), |row| {
            Ok(StatsBucket {
                time_bucket: row.get(0).unwrap_or_default(),
                total_requests: row.get(1).unwrap_or(0),
                success_count: row.get(2).unwrap_or(0),
                error_count: row.get(3).unwrap_or(0),
                input_tokens: row.get(4).unwrap_or(0),
                output_tokens: row.get(5).unwrap_or(0),
                cache_tokens: row.get(6).unwrap_or(0),
                avg_duration_ms: row.get(7).unwrap_or(0.0),
                total_cost: row.get(8).unwrap_or(0.0),
            })
        }).map_err(|e| format!("buckets: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // ── Dimension breakdown ──
    let dimension_data = if let Some(ref gb) = query.group_by {
        // platform 维度按有效 platform_id（含 auto 分组回溯）聚合，JOIN platform 取真名
        let dim_sql = if gb == "platform" {
            format!(
                "SELECT COALESCE(p.name, '未知'), COUNT(*), \
                 SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
                 SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
                 COALESCE(SUM(est_cost), 0.0) \
                 FROM proxy_log LEFT JOIN platform p ON p.id = ({EFF_PID}) \
                 WHERE proxy_log.deleted_at = 0 AND {where_sql} GROUP BY ({EFF_PID}) ORDER BY 2 DESC LIMIT 50"
            )
        } else {
            let dim_col = match gb.as_str() {
                "model" => "actual_model",
                _ => "group_key",
            };
            format!(
                "SELECT {dim_col}, COUNT(*), \
                 SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
                 SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
                 COALESCE(SUM(est_cost), 0.0) \
                 FROM proxy_log WHERE deleted_at = 0 AND {where_sql} GROUP BY 1 ORDER BY 2 DESC LIMIT 50"
            )
        };
        let p = qp.to_sql_params();
        let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
        conn.prepare(&dim_sql)
            .map_err(|e| e.to_string())?
            .query_map(refs.as_slice(), |row| {
                Ok(DimensionEntry {
                    name: row.get(0).unwrap_or_default(),
                    total_requests: row.get(1).unwrap_or(0),
                    success_count: row.get(2).unwrap_or(0),
                    input_tokens: row.get(3).unwrap_or(0),
                    output_tokens: row.get(4).unwrap_or(0),
                    cache_tokens: row.get(5).unwrap_or(0),
                    avg_duration_ms: row.get(6).unwrap_or(0.0),
                    total_cost: row.get(7).unwrap_or(0.0),
                })
            }).map_err(|e| format!("dimension: {e}"))?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        vec![]
    };

    // available_models：当前筛选范围（date + group + platform，不含 filter_model）内
    // 实际有记录的模型名。列表达式与 filter_model 行为一致（actual_model 优先，回退 model），
    // 使下拉项与筛选语义对齐——选中某项必能命中。
    let am_where = {
        let mut parts = vec![
            "proxy_log.created_at >= ?1".to_string(),
            "proxy_log.created_at <= ?2".to_string(),
        ];
        if qp.filter_group.is_some() {
            parts.push("proxy_log.group_key = ?3".to_string());
        }
        if qp.filter_platform.is_some() {
            let idx = 3 + qp.filter_group.is_some() as usize;
            parts.push(format!("({EFF_PID}) = CAST(?{idx} AS INTEGER)"));
        }
        parts.join(" AND ")
    };
    let am_refs: Vec<&dyn rusqlite::ToSql> = {
        let mut v: Vec<&dyn rusqlite::ToSql> = vec![&start, &end];
        if let Some(ref g) = qp.filter_group { v.push(g); }
        if let Some(ref p) = qp.filter_platform { v.push(p); }
        v
    };
    let available_models: Vec<String> = conn
        .prepare(&format!(
            "SELECT DISTINCT CASE WHEN proxy_log.actual_model != '' THEN proxy_log.actual_model ELSE proxy_log.model END AS m \
             FROM proxy_log WHERE {am_where} ORDER BY m"
        ))
        .map_err(|e| e.to_string())?
        .query_map(am_refs.as_slice(), |row| row.get::<_, String>(0))
        .map_err(|e| format!("available_models: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(StatsResult { overview, buckets, dimension_data, available_models })
}

// ─── Model Price CRUD ──────────────────────────────────────

const MODEL_PRICE_COLUMNS: &str =
    "id, model_name, source, price_data, max_input_tokens, max_output_tokens, context_window, created_at, updated_at, deleted_at";

fn row_to_model_price(row: &rusqlite::Row) -> SqlResult<super::models::ModelPrice> {
    Ok(super::models::ModelPrice {
        id: row.get::<_, i64>(0)? as u64,
        model_name: row.get(1)?,
        source: row.get(2)?,
        price_data: row.get(3)?,
        max_input_tokens: row.get::<_, Option<i64>>(4)?,
        max_output_tokens: row.get::<_, Option<i64>>(5)?,
        context_window: row.get::<_, Option<i64>>(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        deleted_at: row.get(9)?,
    })
}

/// 提取关键字段构建摘要
fn price_data_to_summary(mp: &super::models::ModelPrice) -> super::models::ModelPriceSummary {
    let pd: serde_json::Value = serde_json::from_str(&mp.price_data).unwrap_or_default();
    let input = pd.get("input_cost_per_token").and_then(|v| v.as_f64());
    let output = pd.get("output_cost_per_token").and_then(|v| v.as_f64());
    let cache_read = pd.get("cache_read_input_token_cost").and_then(|v| v.as_f64());
    let default_platform = pd.get("default_platform").and_then(|v| v.as_str()).map(String::from);

    super::models::ModelPriceSummary {
        id: mp.id,
        model_name: mp.model_name.clone(),
        source: mp.source.clone(),
        default_platform,
        // Convert $/token → $/M tokens for display
        input_price: input.map(|v| v * 1_000_000.0),
        output_price: output.map(|v| v * 1_000_000.0),
        cache_read_price: cache_read.map(|v| v * 1_000_000.0),
        max_input_tokens: mp.max_input_tokens,
        max_output_tokens: mp.max_output_tokens,
        context_window: mp.context_window,
        updated_at: mp.updated_at,
    }
}

pub async fn list_model_prices(db: &Db, limit: u32, offset: u32) -> Result<Vec<super::models::ModelPriceSummary>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE deleted_at = 0 ORDER BY model_name LIMIT ?1 OFFSET ?2")
            )?;
            let rows = stmt.query_map(params![limit, offset], row_to_model_price)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(price_data_to_summary(&r?));
            }
            Ok(result)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn count_model_prices(db: &Db) -> Result<u32, String> {
    db.0
        .call(move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM model_price WHERE deleted_at = 0", [], |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 获取指定模型的最新价格记录（优先 manual > github）
pub async fn get_model_price(db: &Db, model_name: &str) -> Result<Option<super::models::ModelPrice>, String> {
    let model_name = model_name.to_string();
    db.0
        .call(move |conn| {
            // 优先取 manual 记录
            let mut stmt = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE model_name = ?1 AND source = 'manual' AND deleted_at = 0")
            )?;
            if let Some(mp) = stmt.query_row(params![model_name], row_to_model_price).optional()? {
                return Ok(Some(mp));
            }
            // 回退到 github（同步源）
            let mut stmt2 = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE model_name = ?1 AND source = 'github' AND deleted_at = 0")
            )?;
            Ok(stmt2.query_row(params![model_name], row_to_model_price).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Upsert a model price record (INSERT OR REPLACE by model_name + source)
pub async fn upsert_model_price(
    db: &Db,
    model_name: &str,
    source: &str,
    price_data: &str,
    max_input_tokens: Option<i64>,
    max_output_tokens: Option<i64>,
    context_window: Option<i64>,
) -> Result<(), String> {
    let ts = now();
    let model_name = model_name.to_string();
    let source = source.to_string();
    let price_data = price_data.to_string();
    db.0
        .call(move |conn| {
            conn.execute(
                "INSERT INTO model_price (model_name, source, price_data, max_input_tokens, max_output_tokens, context_window, created_at, updated_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0)
                 ON CONFLICT(model_name, source) DO UPDATE SET
                   price_data = ?3,
                   max_input_tokens = ?4,
                   max_output_tokens = ?5,
                   context_window = ?6,
                   updated_at = ?7,
                   deleted_at = 0",
                params![model_name, source, price_data, max_input_tokens, max_output_tokens, context_window, ts],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert model price: {e}"))
}

/// 取模型最大输出 token（出站裁剪用）。列优先，NULL 时回退 price_data JSON。
/// 返回 None = 未知/无限制（不裁剪）。
pub async fn get_model_max_output_tokens(db: &Db, model_name: &str) -> Result<Option<i64>, String> {
    let mp = get_model_price(db, model_name).await?;
    if let Some(m) = mp {
        if let Some(v) = m.max_output_tokens {
            return Ok(Some(v));
        }
        // 回退 price_data JSON（旧库 / 手动录入仅写 JSON 的兼容路径）
        let pd: serde_json::Value = serde_json::from_str(&m.price_data).unwrap_or_default();
        return Ok(pd.get("max_output_tokens").and_then(|v| v.as_i64()));
    }
    Ok(None)
}

/// 解析价格：model_name + platform_type → ResolvedPrice
/// 优先级: pricing[platform_type] > top_level > default_platform pricing > fallback settings
pub async fn resolve_price(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    fallback_input: f64,
    fallback_output: f64,
    input_tokens: i64,
) -> Result<crate::gateway::models::ResolvedPrice, String> {
    let mp = get_model_price(db, model_name).await?;
    let pd: serde_json::Value = match &mp {
        Some(m) => serde_json::from_str(&m.price_data).unwrap_or_default(),
        None => serde_json::Value::Null,
    };

    // 1. Try pricing[platform_type]
    if let Some(pricing_node) = pd.get("pricing").and_then(|p| p.get(platform_type)) {
        let input = pricing_node.get("input_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let output = pricing_node.get("output_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let cache = pricing_node.get("cache_read_input_token_cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if input > 0.0 || output > 0.0 {
            return Ok(apply_context_tier(
                crate::gateway::models::ResolvedPrice {
                    input_cost_per_token: input,
                    output_cost_per_token: output,
                    cache_read_input_token_cost: cache,
                    source: "platform_override".to_string(),
                },
                &pd,
                input_tokens,
            ));
        }
    }

    // 2. Try top-level price
    let top_input = pd.get("input_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let top_output = pd.get("output_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let top_cache = pd.get("cache_read_input_token_cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if top_input > 0.0 || top_output > 0.0 {
        return Ok(apply_context_tier(
            crate::gateway::models::ResolvedPrice {
                input_cost_per_token: top_input,
                output_cost_per_token: top_output,
                cache_read_input_token_cost: top_cache,
                source: "top_level".to_string(),
            },
            &pd,
            input_tokens,
        ));
    }

    // 3. Try default_platform pricing
    if let Some(dp) = pd.get("default_platform").and_then(|v| v.as_str()) {
        if let Some(pricing_node) = pd.get("pricing").and_then(|p| p.get(dp)) {
            let input = pricing_node.get("input_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let output = pricing_node.get("output_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let cache = pricing_node.get("cache_read_input_token_cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if input > 0.0 || output > 0.0 {
                return Ok(apply_context_tier(
                    crate::gateway::models::ResolvedPrice {
                        input_cost_per_token: input,
                        output_cost_per_token: output,
                        cache_read_input_token_cost: cache,
                        source: "default_platform".to_string(),
                    },
                    &pd,
                    input_tokens,
                ));
            }
        }
    }

    // 4. Fallback
    Ok(crate::gateway::models::ResolvedPrice {
        input_cost_per_token: fallback_input / 1_000_000.0,
        output_cost_per_token: fallback_output / 1_000_000.0,
        cache_read_input_token_cost: 0.0,
        source: "fallback".to_string(),
    })
}

/// 上下文阶梯选档：取 `context_tiers` 中 `min_tokens <= input_tokens` 的最大档，
/// 非 null 字段覆盖 base 价（null 字段继承 base，如某些模型长档无 cache 价）。
/// `context_tiers` 缺失/非数组/无命中档 → 返回 base 不变（向后兼容旧 price_data）。
fn apply_context_tier(
    mut base: crate::gateway::models::ResolvedPrice,
    pd: &serde_json::Value,
    input_tokens: i64,
) -> crate::gateway::models::ResolvedPrice {
    let Some(tiers) = pd.get("context_tiers").and_then(|v| v.as_array()) else {
        return base;
    };
    // 选 min_tokens <= input_tokens 中阈值最大的档（最高适用档）
    let best = tiers
        .iter()
        .filter_map(|t| {
            let min_tokens = t.get("min_tokens").and_then(|v| v.as_i64())?;
            (min_tokens <= input_tokens).then_some((min_tokens, t))
        })
        .max_by_key(|(min_tokens, _)| *min_tokens);
    let Some((_, tier)) = best else {
        return base;
    };
    if let Some(v) = tier.get("input_cost_per_token").and_then(|v| v.as_f64()) {
        base.input_cost_per_token = v;
    }
    if let Some(v) = tier.get("output_cost_per_token").and_then(|v| v.as_f64()) {
        base.output_cost_per_token = v;
    }
    if let Some(v) = tier.get("cache_read_input_token_cost").and_then(|v| v.as_f64()) {
        base.cache_read_input_token_cost = v;
    }
    base.source.push_str("+tier");
    base
}

/// 搜索模型价格
pub async fn search_model_prices(db: &Db, query: &str, limit: u32) -> Result<Vec<super::models::ModelPriceSummary>, String> {
    let pattern = format!("%{query}%");
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE deleted_at = 0 AND model_name LIKE ?1 ORDER BY model_name LIMIT ?2")
            )?;
            let rows = stmt.query_map(params![pattern, limit], row_to_model_price)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(price_data_to_summary(&r?));
            }
            Ok(result)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Filtered list: optional query (LIKE model_name), optional source, limit, offset.
pub async fn filtered_list_model_prices(
    db: &Db,
    query: Option<&str>,
    source: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<super::models::ModelPriceSummary>, String> {
    let query = query.map(|s| s.to_string());
    let source = source.map(|s| s.to_string());
    db.0
        .call(move |conn| {
            let query = query.as_deref();
            let source = source.as_deref();
    let mut where_parts = vec!["deleted_at = 0".to_string()];
    let mut param_idx = 1;
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(q) = query {
        if !q.is_empty() {
            where_parts.push(format!("model_name LIKE ?{param_idx}"));
            params.push(Box::new(format!("%{q}%")));
            param_idx += 1;
        }
    }
    if let Some(s) = source {
        if !s.is_empty() {
            where_parts.push(format!("source = ?{param_idx}"));
            params.push(Box::new(s.to_string()));
            param_idx += 1;
        }
    }

    let where_sql = where_parts.join(" AND ");
    let sql = format!(
        "SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE {where_sql} ORDER BY model_name LIMIT ?{param_idx} OFFSET ?{}",
        param_idx + 1
    );
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), row_to_model_price)?;
    let mut result = Vec::new();
    for r in rows {
        result.push(price_data_to_summary(&r?));
    }
    Ok(result)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Count matching model prices with optional filters.
pub async fn filtered_count_model_prices(
    db: &Db,
    query: Option<&str>,
    source: Option<&str>,
) -> Result<u32, String> {
    let query = query.map(|s| s.to_string());
    let source = source.map(|s| s.to_string());
    db.0
        .call(move |conn| {
            let query = query.as_deref();
            let source = source.as_deref();
    let mut where_parts = vec!["deleted_at = 0".to_string()];
    let mut param_idx = 1;
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(q) = query {
        if !q.is_empty() {
            where_parts.push(format!("model_name LIKE ?{param_idx}"));
            params.push(Box::new(format!("%{q}%")));
            param_idx += 1;
        }
    }
    if let Some(s) = source {
        if !s.is_empty() {
            where_parts.push(format!("source = ?{param_idx}"));
            params.push(Box::new(s.to_string()));
        }
    }

    let where_sql = where_parts.join(" AND ");
    let sql = format!("SELECT COUNT(*) FROM model_price WHERE {where_sql}");
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    Ok(conn.query_row(&sql, param_refs.as_slice(), |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
}

// ─── MCP server CRUD ───────────────────────────────────────
// 集中存 MCP server 配置（migration 020）。行结构见 super::mcp::McpServerRow。
// env_json/headers_json 含原始敏感值，调用方负责脱敏后再返前端。

pub async fn list_mcp_servers(db: &Db) -> Result<Vec<super::mcp::McpServerRow>, String> {
    db.0.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, transport, command, args_json, env_json, url, headers_json, \
             enabled_agents, created_at, updated_at FROM mcp_server ORDER BY name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(super::mcp::McpServerRow {
                id: r.get(0)?,
                name: r.get(1)?,
                transport: r.get(2)?,
                command: r.get(3)?,
                args_json: r.get(4)?,
                env_json: r.get(5)?,
                url: r.get(6)?,
                headers_json: r.get(7)?,
                enabled_agents: r.get(8)?,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
            })
        })?;
        let mut out = vec![];
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    })
    .await
    .map_err(|e| format!("list mcp servers: {e}"))
}

pub async fn get_mcp_server(
    db: &Db,
    name: &str,
) -> Result<Option<super::mcp::McpServerRow>, String> {
    let name = name.to_string();
    db.0.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, transport, command, args_json, env_json, url, headers_json, \
             enabled_agents, created_at, updated_at FROM mcp_server WHERE name = ?1",
        )?;
        let mut rows = stmt.query_map(params![name], |r| {
            Ok(super::mcp::McpServerRow {
                id: r.get(0)?,
                name: r.get(1)?,
                transport: r.get(2)?,
                command: r.get(3)?,
                args_json: r.get(4)?,
                env_json: r.get(5)?,
                url: r.get(6)?,
                headers_json: r.get(7)?,
                enabled_agents: r.get(8)?,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    })
    .await
    .map_err(|e| format!("get mcp server: {e}"))
}

/// INSERT 或 UPDATE（按 name 唯一冲突）。created_at 仅首次写入生效（UPDATE 不覆盖）。
pub async fn upsert_mcp_server(db: &Db, row: &super::mcp::McpServerRow) -> Result<(), String> {
    let row = row.clone();
    db.0.call(move |conn| {
        conn.execute(
            "INSERT INTO mcp_server \
             (name, transport, command, args_json, env_json, url, headers_json, enabled_agents, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(name) DO UPDATE SET \
               transport=excluded.transport, command=excluded.command, args_json=excluded.args_json, \
               env_json=excluded.env_json, url=excluded.url, headers_json=excluded.headers_json, \
               enabled_agents=excluded.enabled_agents, updated_at=excluded.updated_at",
            params![
                row.name,
                row.transport,
                row.command,
                row.args_json,
                row.env_json,
                row.url,
                row.headers_json,
                row.enabled_agents,
                row.created_at,
                row.updated_at
            ],
        )?;
        Ok(())
    })
    .await
    .map_err(|e| format!("upsert mcp server: {e}"))
}

pub async fn delete_mcp_server(db: &Db, name: &str) -> Result<(), String> {
    let name = name.to_string();
    db.0.call(move |conn| {
        conn.execute("DELETE FROM mcp_server WHERE name = ?1", params![name])?;
        Ok(())
    })
    .await
    .map_err(|e| format!("delete mcp server: {e}"))
}

pub async fn set_mcp_server_enabled_agents(
    db: &Db,
    name: &str,
    agents_csv: &str,
) -> Result<(), String> {
    let name = name.to_string();
    let csv = agents_csv.to_string();
    db.0.call(move |conn| {
        conn.execute(
            "UPDATE mcp_server SET enabled_agents = ?1, updated_at = ?2 WHERE name = ?3",
            params![csv, now(), name],
        )?;
        Ok(())
    })
    .await
    .map_err(|e| format!("set mcp enabled agents: {e}"))
}

pub async fn list_mcp_server_names(db: &Db) -> Result<Vec<String>, String> {
    db.0.call(move |conn| {
        let mut stmt = conn.prepare("SELECT name FROM mcp_server")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = vec![];
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    })
    .await
    .map_err(|e| format!("list mcp server names: {e}"))
}

// ─── Tests: DB Schema v2 规范固化 ──────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个初始化好的内存库
    async fn test_db() -> Db {
        let db = Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");
        db
    }

    #[test]
    fn apply_context_tier_selects_long_tier() {
        // OpenAI gpt-5.5: short in=5e-6/out=3e-5/cache=5e-7, long@272000 in=1e-5/out=4.5e-5/cache=1e-6
        let pd = serde_json::json!({
            "input_cost_per_token": 5e-6,
            "output_cost_per_token": 3e-5,
            "cache_read_input_token_cost": 5e-7,
            "context_tiers": [{
                "min_tokens": 272000,
                "input_cost_per_token": 1e-5,
                "output_cost_per_token": 4.5e-5,
                "cache_read_input_token_cost": 1e-6
            }]
        });
        let base = crate::gateway::models::ResolvedPrice {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 3e-5,
            cache_read_input_token_cost: 5e-7,
            source: "top_level".to_string(),
        };
        // 短档: input < 272000 → base 不变 (无 +tier 后缀)
        let short = apply_context_tier(base.clone(), &pd, 100_000);
        assert_eq!(short.input_cost_per_token, 5e-6);
        assert_eq!(short.output_cost_per_token, 3e-5);
        assert_eq!(short.source, "top_level");
        // 长档: input >= 272000 → tier 覆盖
        let long = apply_context_tier(base.clone(), &pd, 300_000);
        assert_eq!(long.input_cost_per_token, 1e-5);
        assert_eq!(long.output_cost_per_token, 4.5e-5);
        assert_eq!(long.cache_read_input_token_cost, 1e-6);
        assert_eq!(long.source, "top_level+tier");
        // 边界: 恰好等于阈值 → long
        let edge = apply_context_tier(base.clone(), &pd, 272_000);
        assert_eq!(edge.input_cost_per_token, 1e-5);
    }

    #[test]
    fn apply_context_tier_no_tier_passthrough() {
        // 无 context_tiers 字段 → base 不变 (向后兼容旧 price_data)
        let pd = serde_json::json!({"input_cost_per_token": 2.5e-6});
        let base = crate::gateway::models::ResolvedPrice {
            input_cost_per_token: 2.5e-6,
            output_cost_per_token: 1.5e-5,
            cache_read_input_token_cost: 2.5e-7,
            source: "top_level".to_string(),
        };
        let r = apply_context_tier(base.clone(), &pd, 999_999_999);
        assert_eq!(r.input_cost_per_token, 2.5e-6);
        assert_eq!(r.source, "top_level");
        // tiers 为空数组 → 同样不变
        let pd2 = serde_json::json!({"context_tiers": []});
        let r2 = apply_context_tier(base, &pd2, 999_999_999);
        assert_eq!(r2.source, "top_level");
    }

    #[test]
    fn apply_context_tier_partial_override() {
        // 长档仅覆盖部分字段 (如某些模型长档无 cache 价 → 继承 base cache)
        let pd = serde_json::json!({
            "input_cost_per_token": 3e-5,
            "output_cost_per_token": 1.8e-4,
            "cache_read_input_token_cost": 0.0,
            "context_tiers": [{
                "min_tokens": 272000,
                "input_cost_per_token": 6e-5,
                "output_cost_per_token": 2.7e-4
                // cache_read_input_token_cost 缺失 → 继承 base
            }]
        });
        let base = crate::gateway::models::ResolvedPrice {
            input_cost_per_token: 3e-5,
            output_cost_per_token: 1.8e-4,
            cache_read_input_token_cost: 0.0,
            source: "top_level".to_string(),
        };
        let r = apply_context_tier(base, &pd, 300_000);
        assert_eq!(r.input_cost_per_token, 6e-5);
        assert_eq!(r.output_cost_per_token, 2.7e-4);
        assert_eq!(r.cache_read_input_token_cost, 0.0); // 继承 base
    }

    #[tokio::test]
    async fn query_stats_platform_dim_and_filter() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("P1")).await.unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let mut lg = sample_log("l1", "g1", now);
        lg.platform_id = p.id;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg, false, false)).await.unwrap();
        let q = StatsQuery { start: None, end: None, granularity: Some("daily".into()), group_by: Some("platform".into()), filter_group: None, filter_model: None, filter_platform: None };
        let r = query_stats(&db, &q).await;
        println!("NO-FILTER platform dim: {:?}", r.as_ref().err());
        assert!(r.is_ok(), "no-filter platform dim failed: {:?}", r.err());
        let q2 = StatsQuery { start: None, end: None, granularity: Some("daily".into()), group_by: Some("platform".into()), filter_group: None, filter_model: None, filter_platform: Some(p.id.to_string()) };
        let r2 = query_stats(&db, &q2).await;
        println!("PLATFORM-FILTER: {:?}", r2.as_ref().err());
        assert!(r2.is_ok(), "platform filter failed: {:?}", r2.err());
        let res = r2.unwrap();
        println!("overview total_requests = {}", res.overview.total_requests);
        println!("dim entries = {}", res.dimension_data.len());
    }

    /// cache_rate 必须 ≤100%：cache_tokens=9900（命中缓存）+ input_tokens=100（新输入），
    /// 旧公式 cache/input=9900% 错误；新公式 cache/(input+cache)≈99%。
    #[tokio::test]
    async fn cache_rate_never_exceeds_100() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        let mut lg = sample_log("c1", "g1", now);
        lg.input_tokens = 100;
        lg.cache_tokens = 9900;
        lg.output_tokens = 50;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg, false, false)).await.unwrap();

        let ts = today_stats(&db).await.expect("today_stats");
        println!("today cache_rate = {}", ts.cache_rate);
        assert!(ts.cache_rate <= 100.0, "today cache_rate > 100: {}", ts.cache_rate);
        assert!(ts.cache_rate > 98.0 && ts.cache_rate < 100.0, "today cache_rate expected ~99: {}", ts.cache_rate);

        let q = StatsQuery { start: None, end: None, granularity: None, group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let s = query_stats(&db, &q).await.expect("query_stats");
        println!("overview cache_rate = {}", s.overview.cache_rate);
        assert!(s.overview.cache_rate <= 100.0, "overview cache_rate > 100: {}", s.overview.cache_rate);
        // buckets 非空（防 query_stats_inner 回归致趋势图无数据）
        assert!(!s.buckets.is_empty(), "buckets empty — trend chart would not render");
    }

    /// available_models 只含实际有记录的模型（actual_model 优先），不含未请求的。
    /// 防回归：前端模型筛选项曾派生自配置列表（platform.available_models ∪ group mappings），
    /// 导致下拉列出从未请求过的模型。
    #[tokio::test]
    async fn stats_available_models_only_recorded() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        let mut lg1 = sample_log("m1", "g1", now);
        lg1.model = "claude-sonnet-4".into();
        lg1.actual_model = "glm-4-plus".into();
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg1, false, false)).await.unwrap();
        let mut lg2 = sample_log("m2", "g1", now);
        lg2.model = "gpt-4o".into();
        lg2.actual_model = String::new(); // 回退到 model
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg2, false, false)).await.unwrap();

        let q = StatsQuery { start: None, end: None, granularity: None, group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let s = query_stats(&db, &q).await.expect("query_stats");
        // actual_model 优先 → glm-4-plus；actual_model 空 → 回退 gpt-4o
        assert!(s.available_models.contains(&"glm-4-plus".to_string()), "missing glm-4-plus: {:?}", s.available_models);
        assert!(s.available_models.contains(&"gpt-4o".to_string()), "missing gpt-4o: {:?}", s.available_models);
        // 未请求过的模型不应出现
        assert!(!s.available_models.iter().any(|m| m == "claude-sonnet-4"), "requested model leaked: {:?}", s.available_models);
        assert!(!s.available_models.iter().any(|m| m == "never-used-model"), "unrecorded model leaked: {:?}", s.available_models);

        // filter_model 不应收缩 available_models（否则选中后下拉自缩）
        let q2 = StatsQuery { start: None, end: None, granularity: None, group_by: None, filter_group: None, filter_model: Some("glm-4-plus".into()), filter_platform: None };
        let s2 = query_stats(&db, &q2).await.expect("query_stats filtered");
        assert!(s2.available_models.contains(&"gpt-4o".to_string()), "filter_model shrank available_models: {:?}", s2.available_models);
    }

    /// 分钟 / 5 分钟分桶：合成同一小时内不同分钟的日志，断言分桶宽度正确。
    /// minute → 每分钟一桶；5min → floor 到 5 分钟边界一桶；hourly → 全部归一桶。
    #[tokio::test]
    async fn stats_minute_and_5min_buckets() {
        let db = test_db().await;
        // 固定基准：2026-06-16 10:00:00 UTC（毫秒）
        let base = chrono::DateTime::parse_from_rfc3339("2026-06-16T10:00:00Z")
            .unwrap()
            .timestamp_millis();
        // 6 条日志，分布在 10:00 / 10:01 / 10:03 / 10:06 / 10:12 / 10:14
        let offsets_min = [0i64, 1, 3, 6, 12, 14];
        for (i, m) in offsets_min.iter().enumerate() {
            let ts = base + m * 60_000;
            let lg = sample_log(&format!("b{i}"), "g1", ts);
            insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg, false, false))
                .await
                .unwrap();
        }
        let start = base - 60_000;
        let end = base + 20 * 60_000;

        // minute：6 个不同分钟 → 6 桶
        let q_min = StatsQuery { start: Some(start), end: Some(end), granularity: Some("minute".into()), group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let r_min = query_stats(&db, &q_min).await.expect("minute stats");
        assert_eq!(r_min.buckets.len(), 6, "minute 应 6 桶: {:?}", r_min.buckets.iter().map(|b| &b.time_bucket).collect::<Vec<_>>());

        // 5min：分钟落入 [00-04]→2(00,01,03), [05-09]→1(06), [10-14]→2(12,14) → 3 桶
        let q_5 = StatsQuery { start: Some(start), end: Some(end), granularity: Some("5min".into()), group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let r_5 = query_stats(&db, &q_5).await.expect("5min stats");
        assert_eq!(r_5.buckets.len(), 3, "5min 应 3 桶: {:?}", r_5.buckets.iter().map(|b| &b.time_bucket).collect::<Vec<_>>());
        // 第一桶（10:00）应聚合 3 条请求
        let first = &r_5.buckets[0];
        assert_eq!(first.total_requests, 3, "5min 首桶应聚 3 条: {first:?}");

        // hourly：全在 10 点 → 1 桶
        let q_h = StatsQuery { start: Some(start), end: Some(end), granularity: Some("hourly".into()), group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let r_h = query_stats(&db, &q_h).await.expect("hourly stats");
        assert_eq!(r_h.buckets.len(), 1, "hourly 应 1 桶: {:?}", r_h.buckets.iter().map(|b| &b.time_bucket).collect::<Vec<_>>());
        assert_eq!(r_h.buckets[0].total_requests, 6, "hourly 桶应聚 6 条");
    }

    fn sample_platform(name: &str) -> CreatePlatform {
        CreatePlatform {
            name: name.to_string(),
            platform_type: Protocol::Anthropic,
            base_url: "https://example.com".to_string(),
            api_key: "sk-test".to_string(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
        }
    }

    fn sample_group(name: &str, mappings: Vec<ModelMapping>) -> CreateGroup {
        CreateGroup {
            name: name.to_string(),
            group_key: Some(name.to_string()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: 2,
            model_mappings: mappings,
        }
    }

    fn sample_log(id: &str, group_key: &str, created_at: i64) -> ProxyLog {
        ProxyLog {
            id: id.to_string(),
            group_key: group_key.to_string(),
            model: "claude-sonnet-4".to_string(),
            actual_model: "glm-4-plus".to_string(),
            source_protocol: "anthropic".to_string(),
            target_protocol: "openai".to_string(),
            platform_id: 1,
            request_headers: String::new(),
            request_body: String::new(),
            upstream_request_headers: String::new(),
            upstream_request_body: String::new(),
            response_body: String::new(),
            request_url: String::new(),
            upstream_request_url: String::new(),
            upstream_response_headers: String::new(),
            upstream_status_code: 200,
            user_response_headers: String::new(),
            user_response_body: String::new(),
            status_code: 200,
            duration_ms: 100,
            input_tokens: 10,
            output_tokens: 20,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: false,
            attempts: Vec::new(),
            retry_count: 0,
            blocked_by: String::new(),
            blocked_reason: String::new(),
            created_at,
            updated_at: created_at,
            deleted_at: 0,
        }
    }

    /// endpoints 反序列化容错：DB 中含未知 client_type（如旧数据 "anthropic"）的
    /// endpoint 数组应仍能完整解析，而非因单个未知枚举值整体失败 → 空 Vec → 前端丢失。
    #[tokio::test]
    async fn endpoints_with_unknown_client_type_still_parse() {
        let json = r#"[{"protocol":"openai","base_url":"https://x/v1","client_type":"codex_tui","coding_plan":false},{"protocol":"anthropic","base_url":"https://x/anthropic","client_type":"anthropic","coding_plan":false}]"#;
        let parsed = parse_endpoints(json);
        assert_eq!(parsed.len(), 2, "未知 client_type 不应导致整个数组解析失败");
        assert_eq!(parsed[1].client_type, ClientType::Default, "未知值回退为 Default");
        assert_eq!(parsed[1].protocol, Protocol::Anthropic);

        // 端到端：写入 DB 后 list_platforms 应带回 endpoints
        let db = test_db().await;
        let mut input = sample_platform("p1");
        input.endpoints = Some(vec![PlatformEndpoint {
            protocol: Protocol::OpenAI,
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            client_type: ClientType::CodexTui,
            coding_plan: true,
        }]);
        create_platform(&db, input).await.unwrap();
        let listed = list_platforms(&db).await.unwrap();
        assert_eq!(listed[0].endpoints.len(), 1, "list_platforms 应返回 endpoints");
    }

    // ── 单平台动态窗口日速率：按 platform_id 过滤 est_cost，span clamp 5min..7d ──
    #[tokio::test]
    async fn platform_hourly_rate_filters_by_platform() {
        let db = test_db().await;
        let now_ms = now();
        // platform 1：2h 前一条 est_cost=4.0；platform 2：另一条 est_cost=99（不应计入 p1）。
        let mut l1 = sample_log("r1", "g", now_ms - 2 * 3_600_000);
        l1.platform_id = 1;
        l1.est_cost = 4.0;
        let mut l2 = sample_log("r2", "g", now_ms - 1_000);
        l2.platform_id = 2;
        l2.est_cost = 99.0;
        upsert_proxy_log(&db, l1).await.unwrap();
        upsert_proxy_log(&db, l2).await.unwrap();

        // p1：span = clamp(now_internal - earliest, 5min, 7d) ≈ 2h → rate ≈ 4.0 / 2 = 2.0 $/h。
        // 容差放宽：查询内部 now 与测试 now 间有毫秒级时钟差 → span 略大于 2h（rate 略小于 2.0）。
        let rate = get_platform_hourly_rate(&db, 1).await.unwrap();
        assert!(rate.is_some());
        assert!((rate.unwrap() - 2.0).abs() < 0.01, "p1 rate = {rate:?}");

        // 无任何用量的平台 → None。
        let none = get_platform_hourly_rate(&db, 999).await.unwrap();
        assert!(none.is_none(), "无用量平台应 None，got {none:?}");
    }

    // ── R2 单数表名 + "group" 转义：init_tables 成功间接验证 DDL ──
    #[tokio::test]
    async fn r2_singular_table_names_and_group_escaped() {
        // init_tables() 已在 test_db 中执行；进一步断言单数表名存在、复数不存在
        let db = test_db().await;
        let names: Vec<String> = db.0.call(|conn| {
            Ok(conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?
                .query_map([], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect())
        }).await.unwrap();
        assert!(names.contains(&"platform".to_string()));
        assert!(names.contains(&"group".to_string()));
        assert!(names.contains(&"group_platform".to_string()));
        assert!(names.contains(&"setting".to_string()));
        assert!(names.contains(&"proxy_log".to_string()));
        // 复数旧表名禁止存在
        assert!(!names.contains(&"platforms".to_string()));
        assert!(!names.contains(&"groups".to_string()));
        assert!(!names.contains(&"model_mappings".to_string()));
    }

    // ── R7 / D1 主键自增 uint64 ──
    #[tokio::test]
    async fn r7_platform_pk_autoincrement_u64() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("p1")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("p2")).await.unwrap();
        assert!(p1.id >= 1, "first id should be >= 1, got {}", p1.id);
        assert_eq!(p2.id, p1.id + 1, "id should auto-increment");
        // 类型为 u64（编译期保证），运行期断言 >0
        let _: u64 = p2.id;
        assert!(p2.id > 0);
    }

    // ── R1 / R9 毫秒级时间戳 ──
    #[tokio::test]
    async fn r1_timestamps_are_millis() {
        let db = test_db().await;
        let before = chrono::Utc::now().timestamp_millis();
        let p = create_platform(&db, sample_platform("ts")).await.unwrap();
        let after = chrono::Utc::now().timestamp_millis();
        // 毫秒级：> 1e12（2001 年之后），且落在 before..=after 区间
        assert!(p.created_at > 1_000_000_000_000, "created_at not ms-level: {}", p.created_at);
        assert!(p.updated_at > 1_000_000_000_000, "updated_at not ms-level: {}", p.updated_at);
        assert!(p.created_at >= before && p.created_at <= after);
        assert_eq!(p.created_at, p.updated_at);
    }

    // ── R9 软删除：delete 后 deleted_at>0；list 不含；get 返回 None ──
    #[tokio::test]
    async fn r9_soft_delete_platform() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("del")).await.unwrap();
        assert_eq!(list_platforms(&db).await.unwrap().len(), 1);

        delete_platform(&db, p.id).await.unwrap();

        // list 不返回已删行
        assert_eq!(list_platforms(&db).await.unwrap().len(), 0);
        // get 返回 None
        assert!(get_platform(&db, p.id).await.unwrap().is_none());

        // 行仍存在且 deleted_at > 0（物理保留）
        let pid = p.id as i64;
        let deleted_at: i64 = db.0.call(move |conn| {
            Ok(conn.query_row("SELECT deleted_at FROM platform WHERE id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert!(deleted_at > 0, "deleted_at should be set, got {deleted_at}");
    }

    // ── R10 禁 NULL：未提供 extra 时为空串而非 NULL ──
    #[tokio::test]
    async fn r10_no_null_defaults() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("nn")).await.unwrap();
        assert_eq!(p.extra, "");

        let g = create_group(&db, sample_group("g", vec![])).await.unwrap();
        assert_eq!(g.auto_from_platform, "");
        assert_eq!(g.model_mappings.len(), 0);

        // 直接断言列值非 NULL
        let (null_count, g_null): (i64, i64) = db.0.call(|conn| {
            let null_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM platform WHERE extra IS NULL OR base_url IS NULL OR api_key IS NULL",
                [],
                |r| r.get(0),
            )?;
            let g_null: i64 = conn.query_row(
                "SELECT COUNT(*) FROM \"group\" WHERE auto_from_platform IS NULL OR model_mappings IS NULL OR source_protocol IS NULL",
                [],
                |r| r.get(0),
            )?;
            Ok((null_count, g_null))
        }).await.unwrap();
        assert_eq!(null_count, 0, "no platform column should be NULL");
        assert_eq!(g_null, 0, "no group column should be NULL");
    }

    // ── R3 platform_type 列（protocol 改名）往返 ──
    #[tokio::test]
    async fn r3_platform_type_roundtrip() {
        let db = test_db().await;
        let mut input = sample_platform("pt");
        input.platform_type = Protocol::Glm;
        let p = create_platform(&db, input).await.unwrap();
        let fetched = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(fetched.platform_type, Protocol::Glm);
        // 列名为 platform_type（间接：能写入该列即证明列存在）
        let pid = p.id as i64;
        let stored: String = db.0.call(move |conn| {
            Ok(conn.query_row("SELECT platform_type FROM platform WHERE id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(stored, "\"glm\"");
    }

    // ── R4 / D4 model_mappings 内联 JSON 往返 ──
    #[tokio::test]
    async fn r4_group_model_mappings_inline_roundtrip() {
        let db = test_db().await;
        let mappings = vec![
            ModelMapping {
                source_model: "claude-sonnet-4".to_string(),
                target_platform_id: 42,
                target_model: "glm-4-plus".to_string(),
                request_timeout_secs: 30,
                connect_timeout_secs: 5,
            },
            ModelMapping {
                source_model: "claude-haiku".to_string(),
                target_platform_id: 7,
                target_model: "glm-4-air".to_string(),
                request_timeout_secs: 0,
                connect_timeout_secs: 0,
            },
        ];
        let g = create_group(&db, sample_group("mm", mappings)).await.unwrap();

        let fetched = get_group(&db, g.id).await.unwrap().unwrap();
        assert_eq!(fetched.model_mappings.len(), 2);
        assert_eq!(fetched.model_mappings[0].source_model, "claude-sonnet-4");
        // target_platform_id 为 u64
        let tpid: u64 = fetched.model_mappings[0].target_platform_id;
        assert_eq!(tpid, 42);
        assert_eq!(fetched.model_mappings[0].target_model, "glm-4-plus");
        assert_eq!(fetched.model_mappings[0].request_timeout_secs, 30);
        assert_eq!(fetched.model_mappings[1].target_platform_id, 7);
    }

    // ── R4 model_mappings 来自 group 字段（get_group_detail）──
    #[tokio::test]
    async fn r4_group_detail_mappings_from_group_field() {
        let db = test_db().await;
        let mappings = vec![ModelMapping {
            source_model: "src".to_string(),
            target_platform_id: 3,
            target_model: "tgt".to_string(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
        }];
        let g = create_group(&db, sample_group("d", mappings)).await.unwrap();
        // 该分组无关联平台 → get_group_platforms join 为空，规避遗留 BUG-1（见任务遗留）
        let detail = get_group_detail(&db, g.id).await.unwrap().unwrap();
        // detail.model_mappings 来自 group 内联字段（逐字段一致）
        assert_eq!(detail.model_mappings.len(), 1);
        assert_eq!(detail.model_mappings.len(), detail.group.model_mappings.len());
        assert_eq!(detail.model_mappings[0].source_model, detail.group.model_mappings[0].source_model);
        assert_eq!(detail.model_mappings[0].target_platform_id, detail.group.model_mappings[0].target_platform_id);
        assert_eq!(detail.model_mappings[0].target_model, detail.group.model_mappings[0].target_model);
    }

    // ── R8 proxy_log 主键 TEXT hex32（无连字符），软删 + retention ──
    #[tokio::test]
    async fn r8_proxy_log_uuid_no_hyphen_and_retention() {
        let db = test_db().await;
        // hex32 无连字符 id（模拟生产生成方式 uuid simple）
        let new_id = uuid::Uuid::new_v4().simple().to_string();
        assert_eq!(new_id.len(), 32, "simple uuid should be 32 hex chars");
        assert!(!new_id.contains('-'), "uuid must not contain hyphen");

        let now_ms = chrono::Utc::now().timestamp_millis();
        // 一条最近日志
        upsert_proxy_log(&db, sample_log(&new_id, "g", now_ms)).await.unwrap();
        // 一条很旧的日志（100 天前）
        let old_id = uuid::Uuid::new_v4().simple().to_string();
        let old_ms = now_ms - 100 * 86_400_000;
        upsert_proxy_log(&db, sample_log(&old_id, "g", old_ms)).await.unwrap();

        assert_eq!(count_proxy_logs(&db).await.unwrap(), 2);

        // retention 30 天：旧日志被软删
        cleanup_proxy_logs(&db, 30).await.unwrap();
        assert_eq!(count_proxy_logs(&db).await.unwrap(), 1);
        assert!(get_proxy_log(&db, &old_id).await.unwrap().is_none());
        assert!(get_proxy_log(&db, &new_id).await.unwrap().is_some());

        // proxy_log 主键存储原样 TEXT
        let fetched = get_proxy_log(&db, &new_id).await.unwrap().unwrap();
        assert_eq!(fetched.id, new_id);
        assert!(fetched.created_at > 1_000_000_000_000);
    }

    // ── D3 复合唯一约束：group_platform 加代理主键 + UNIQUE(group_id, platform_id) ──
    #[tokio::test]
    async fn d3_group_platform_proxy_pk_and_unique() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("a")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("b")).await.unwrap();
        let g = create_group(&db, sample_group("g", vec![])).await.unwrap();

        set_group_platforms(
            &db,
            g.id,
            &[
                GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1) },
                GroupPlatformInput { platform_id: p2.id, priority: Some(1), weight: Some(2) },
            ],
        ).await
        .unwrap();

        let details = get_group_platforms(&db, g.id).await.unwrap();
        assert_eq!(details.len(), 2);

        // 代理主键 id 存在且自增
        let ids: Vec<i64> = db.0.call(|conn| {
            Ok(conn
                .prepare("SELECT id FROM group_platform ORDER BY id")?
                .query_map([], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect())
        }).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids[0] >= 1 && ids[1] > ids[0]);
    }

    // ── setting 软删除 + upsert ──
    #[tokio::test]
    async fn setting_upsert_and_soft_delete() {
        let db = test_db().await;
        set_setting(&db, SetSettingInput {
            scope: "proxy".to_string(),
            key: "logging".to_string(),
            value: serde_json::json!({"enabled": true}),
        }).await.unwrap();
        assert_eq!(list_setting_keys(&db, "proxy").await.unwrap(), vec!["logging".to_string()]);
        let v = get_setting(&db, "proxy", "logging").await.unwrap().unwrap();
        assert_eq!(v["enabled"], serde_json::json!(true));

        delete_setting(&db, "proxy", "logging").await.unwrap();
        assert!(get_setting(&db, "proxy", "logging").await.unwrap().is_none());
        assert_eq!(list_setting_keys(&db, "proxy").await.unwrap().len(), 0);
    }

    // ─── Tray Config ───────────────────────────────────────

    /// TrayConfig serde 往返：写入后读回各字段一致（separator/items 颜色三态/字号/line_mode/排序）。
    #[tokio::test]
    async fn tray_config_serde_roundtrip() {
        let db = test_db().await;
        let cfg = TrayConfig {
            separator: " | ".to_string(),
            items: vec![
                TrayItem {
                    item_type: "platform".to_string(),
                    platform_id: Some(7),
                    display: "coding".to_string(),
                    metric: None,
                    label: None,
decimals: None,
                    color: TrayColor { mode: "preset".to_string(), value: "green".to_string() },
                    font_size: 11.0,
                    line_mode: "two".to_string(),
                    align: "left".to_string(),
                    align_row2: None,
                    enabled: true,
                    order: 0,
                },
                TrayItem {
                    item_type: "today_usage".to_string(),
                    platform_id: None,
                    display: "balance".to_string(),
                    metric: Some("tokens".to_string()),
                    label: None,
decimals: None,
                    color: TrayColor { mode: "custom".to_string(), value: "#ff8800".to_string() },
                    font_size: 9.0,
                    line_mode: "single".to_string(),
                    align: "left".to_string(),
                    align_row2: None,
                    enabled: false,
                    order: 1,
                },
            ],
        };
        set_tray_config(&db, &cfg).await.unwrap();
        let got = get_tray_config(&db).await.unwrap().expect("config present");
        assert_eq!(got.separator, " | ");
        assert_eq!(got.items.len(), 2);
        assert_eq!(got.items[0].item_type, "platform");
        assert_eq!(got.items[0].platform_id, Some(7));
        assert_eq!(got.items[0].display, "coding");
        assert_eq!(got.items[0].color.mode, "preset");
        assert_eq!(got.items[0].color.value, "green");
        assert!((got.items[0].font_size - 11.0).abs() < 1e-9);
        assert_eq!(got.items[0].line_mode, "two");
        assert!(got.items[0].enabled);
        assert_eq!(got.items[1].line_mode, "single");
        assert_eq!(got.items[1].item_type, "today_usage");
        assert_eq!(got.items[1].metric.as_deref(), Some("tokens"));
        assert_eq!(got.items[1].color.mode, "custom");
        assert_eq!(got.items[1].color.value, "#ff8800");
        assert!(!got.items[1].enabled);
        assert_eq!(got.items[1].order, 1);
    }

    /// 迁移：无 tray config 且无旧 show_in_tray 平台 → 生成空配置并持久化（避免重复迁移）。
    #[tokio::test]
    async fn tray_config_migrate_empty() {
        let db = test_db().await;
        // 首次读取触发迁移；无旧平台 → 空 items。
        let cfg = get_tray_config(&db).await.unwrap().expect("migrated config");
        assert_eq!(cfg.items.len(), 0);
        // 已持久化：settings 中应存在 tray/config。
        assert!(get_setting(&db, "tray", "config").await.unwrap().is_some());
    }

    /// 迁移：旧 show_in_tray=1 平台 → 生成默认 platform item（保留 tray_display）。
    #[tokio::test]
    async fn tray_config_migrate_from_legacy_platform() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("legacy")).await.unwrap();
        set_tray_platform(&db, p.id, "coding").await.unwrap();

        let cfg = get_tray_config(&db).await.unwrap().expect("migrated config");
        assert_eq!(cfg.items.len(), 1, "应从旧平台生成 1 个 platform item");
        let item = &cfg.items[0];
        assert_eq!(item.item_type, "platform");
        assert_eq!(item.platform_id, Some(p.id));
        assert_eq!(item.display, "coding");
        assert!(item.enabled);
    }

    /// 迁移：旧全局 layout=two_line → 各 item line_mode="two"；缺 line_mode 字段时按 serde default "single"。
    #[tokio::test]
    async fn tray_config_migrate_legacy_layout() {
        let db = test_db().await;
        // 模拟旧版本写入：含全局 layout 字段，item 无 line_mode 字段。
        let legacy = serde_json::json!({
            "layout": "two_line",
            "separator": "  ",
            "items": [
                { "item_type": "platform", "platform_id": 3, "display": "balance",
                  "color": { "mode": "follow", "value": "" }, "font_size": 9.0,
                  "enabled": true, "order": 0 }
            ]
        });
        set_setting(&db, SetSettingInput {
            scope: "tray".to_string(),
            key: "config".to_string(),
            value: legacy,
        }).await.unwrap();

        let cfg = get_tray_config(&db).await.unwrap().expect("config present");
        assert_eq!(cfg.items.len(), 1);
        // 旧全局 two_line → item line_mode="two"。
        assert_eq!(cfg.items[0].line_mode, "two");
    }

    /// serde default：缺 line_mode 字段 → "two"（default_line_mode）。
    #[tokio::test]
    async fn tray_item_line_mode_serde_default() {
        let raw = serde_json::json!({
            "item_type": "platform", "platform_id": 1, "display": "balance",
            "color": { "mode": "follow", "value": "" }, "font_size": 9.0,
            "enabled": true, "order": 0
        });
        let item: TrayItem = serde_json::from_value(raw).unwrap();
        assert_eq!(item.line_mode, "two");
    }

    /// today_token_total：仅统计今日（本地 0 点起）未删除日志的 input+output。
    #[tokio::test]
    async fn today_token_total_sums_today_only() {
        use chrono::{Local, Duration};
        let db = test_db().await;
        let now_ms = now();
        // 今日两条：(10+20) + (10+20) = 60
        upsert_proxy_log(&db, sample_log("a", "g", now_ms)).await.unwrap();
        upsert_proxy_log(&db, sample_log("b", "g", now_ms)).await.unwrap();
        // 昨日一条：不计入。
        let yesterday_ms = (Local::now() - Duration::days(1)).timestamp_millis();
        upsert_proxy_log(&db, sample_log("c", "g", yesterday_ms)).await.unwrap();

        assert_eq!(today_token_total(&db).await.unwrap(), 60);
    }

    /// today_platform_stats：按平台分组今日用量；platform_id=0 自动分组日志经
    /// group.auto_from_platform 回溯到源平台后归并；只返回有用量的平台；昨日日志不计。
    #[tokio::test]
    async fn today_platform_stats_groups_and_retraces() {
        use chrono::{Local, Duration};
        let db = test_db().await;
        let now_ms = now();

        // 平台 1（源平台），平台 2（无用量，不应出现）。
        let p1 = create_platform(&db, sample_platform("p-one")).await.unwrap();
        let _p2 = create_platform(&db, sample_platform("p-two")).await.unwrap();

        // 自动分组：auto_from_platform = p1.id 的十进制字符串。
        let mut g = sample_group("autog", vec![]);
        g.auto_from_platform = p1.id.to_string();
        let group = create_group(&db, g).await.unwrap();

        // 直连 p1 的日志（platform_id = p1.id），10+20 = 30 tok。
        let mut direct = sample_log("d1", "autog", now_ms);
        direct.platform_id = p1.id;
        upsert_proxy_log(&db, direct).await.unwrap();

        // 自动分组日志（platform_id=0），回溯到 p1。10+20 = 30 tok。
        let mut auto = sample_log("a1", &group.name, now_ms);
        auto.platform_id = 0;
        upsert_proxy_log(&db, auto).await.unwrap();

        // 昨日日志：不计入。
        let yesterday_ms = (Local::now() - Duration::days(1)).timestamp_millis();
        let mut old = sample_log("o1", "autog", yesterday_ms);
        old.platform_id = p1.id;
        upsert_proxy_log(&db, old).await.unwrap();

        let stats = today_platform_stats(&db).await.unwrap();
        // 只 p1 有今日用量（direct + auto 归并），p2 无用量不出现。
        assert_eq!(stats.len(), 1, "仅有用量的平台出现");
        let s = &stats[0];
        assert_eq!(s.platform_id, p1.id);
        assert_eq!(s.platform_name, "p-one");
        assert_eq!(s.tokens, 60, "direct(30) + auto retrace(30) 归并");
        assert_eq!(s.requests, 2);
    }

    /// 批量 group stats（问题6）：单查 GROUP BY group_key 返回所有 group 聚合，
    /// 与逐 group get_group_usage_stats 数值一致；不同 group 互不串味；空 group_key 不出现。
    #[tokio::test]
    async fn all_group_usage_stats_matches_per_group() {
        let db = test_db().await;
        let now_ms = now();
        // group "ga"：2 条成功（各 10+20 tok）；group "gb"：1 条成功。
        upsert_proxy_log(&db, sample_log("a1", "ga", now_ms)).await.unwrap();
        upsert_proxy_log(&db, sample_log("a2", "ga", now_ms)).await.unwrap();
        upsert_proxy_log(&db, sample_log("b1", "gb", now_ms)).await.unwrap();
        // 空 group_key 的日志（未匹配分组场景）：批量结果中不应出现。
        upsert_proxy_log(&db, sample_log("e1", "", now_ms)).await.unwrap();

        let all = get_all_group_usage_stats(&db).await.unwrap();
        assert_eq!(all.len(), 2, "仅 ga/gb 两个非空 group");
        assert!(!all.contains_key(""), "空 group_key 不计入");

        // 与逐 group 查询数值逐字段一致。
        for name in ["ga", "gb"] {
            let single = get_group_usage_stats(&db, name).await.unwrap();
            let batch = all.get(name).expect("group in batch");
            assert_eq!(batch.total_requests, single.total_requests, "{name} requests");
            assert_eq!(batch.success_count, single.success_count, "{name} success");
            assert_eq!(batch.total_input_tokens, single.total_input_tokens, "{name} input");
            assert_eq!(batch.total_output_tokens, single.total_output_tokens, "{name} output");
            assert_eq!(batch.total_cache_tokens, single.total_cache_tokens, "{name} cache");
        }
        assert_eq!(all["ga"].total_requests, 2);
        assert_eq!(all["gb"].total_requests, 1);
    }

    /// 缓存正确性（问题2）：setting 写后读返回新值（失效生效）；
    /// group 写后 list_groups 返回新集合（不返回陈旧缓存）。
    #[tokio::test]
    async fn hot_cache_invalidates_on_write() {
        let db = test_db().await;
        // ── setting 缓存 ──
        // 先读（不存在 → 缓存 None 槽），再写，再读必须见新值。
        assert!(get_setting(&db, "proxy", "logging").await.unwrap().is_none());
        set_setting(&db, SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::json!({"enabled": true}),
        }).await.unwrap();
        let v = get_setting(&db, "proxy", "logging").await.unwrap();
        assert_eq!(v, Some(serde_json::json!({"enabled": true})), "写后读见新值（缓存已失效）");
        // 改值再读。
        set_setting(&db, SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::json!({"enabled": false}),
        }).await.unwrap();
        assert_eq!(
            get_setting(&db, "proxy", "logging").await.unwrap(),
            Some(serde_json::json!({"enabled": false})),
        );
        // delete 后读为 None。
        delete_setting(&db, "proxy", "logging").await.unwrap();
        assert!(get_setting(&db, "proxy", "logging").await.unwrap().is_none());

        // ── group 缓存 ──
        assert_eq!(list_groups(&db).await.unwrap().len(), 0);
        let g = create_group(&db, sample_group("gc", vec![])).await.unwrap();
        // 缓存失效 → list_groups 见新建 group。
        let groups = list_groups(&db).await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "gc");
        // 删除后 list_groups 不再含该 group。
        force_delete_group(&db, g.id).await.unwrap();
        assert_eq!(list_groups(&db).await.unwrap().len(), 0);
    }

    // ── 平台 auto_group 开关：持久化 + 手动组成员全量同步（auto 组不动）──

    #[tokio::test]
    async fn create_platform_persists_auto_group() {
        let db = test_db().await;
        // None → 默认 true（旧行为不变）。
        let mut input = sample_platform("ag-default");
        input.auto_group = None;
        let p = create_platform(&db, input).await.unwrap();
        assert!(get_platform(&db, p.id).await.unwrap().unwrap().auto_group, "None→true");

        // 显式 false。
        let mut input = sample_platform("ag-off");
        input.auto_group = Some(false);
        let p = create_platform(&db, input).await.unwrap();
        assert!(!get_platform(&db, p.id).await.unwrap().unwrap().auto_group, "Some(false)");

        // 显式 true。
        let mut input = sample_platform("ag-on");
        input.auto_group = Some(true);
        let p = create_platform(&db, input).await.unwrap();
        assert!(get_platform(&db, p.id).await.unwrap().unwrap().auto_group, "Some(true)");
    }

    #[tokio::test]
    async fn sync_platform_manual_groups_adds_removes_preserves_auto() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("sync-p")).await.unwrap();
        // 一个 auto 组（auto_from_platform 非空）+ 两个手动组。
        let auto_g = create_group(&db, CreateGroup {
            name: "auto-g".into(),
            group_key: Some("auto-g".into()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: p.id.to_string(),
            request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: None, max_retries: 2, model_mappings: vec![],
        }).await.unwrap();
        set_group_platforms(&db, auto_g.id, &[GroupPlatformInput {
            platform_id: p.id, priority: Some(0), weight: Some(1),
        }]).await.unwrap();
        let m1 = create_group(&db, sample_group("m1", vec![])).await.unwrap();
        let m2 = create_group(&db, sample_group("m2", vec![])).await.unwrap();

        // 初始：加入 m1，不动 m2、auto 组。
        sync_platform_manual_groups(&db, p.id, &[m1.id]).await.unwrap();
        assert_eq!(get_group_platforms(&db, m1.id).await.unwrap().len(), 1);
        assert_eq!(get_group_platforms(&db, m2.id).await.unwrap().len(), 0);
        assert_eq!(get_group_platforms(&db, auto_g.id).await.unwrap().len(), 1, "auto 组应在");

        // 全量同步 → 移出 m1、加入 m2。
        sync_platform_manual_groups(&db, p.id, &[m2.id]).await.unwrap();
        assert_eq!(get_group_platforms(&db, m1.id).await.unwrap().len(), 0, "m1 应被移出");
        assert_eq!(get_group_platforms(&db, m2.id).await.unwrap().len(), 1, "m2 应被加入");
        assert_eq!(get_group_platforms(&db, auto_g.id).await.unwrap().len(), 1, "auto 组不受手动同步影响");

        // 清空手动组（空 target）→ auto 组仍在。
        sync_platform_manual_groups(&db, p.id, &[]).await.unwrap();
        assert_eq!(get_group_platforms(&db, m2.id).await.unwrap().len(), 0);
        assert_eq!(get_group_platforms(&db, auto_g.id).await.unwrap().len(), 1, "清空手动组不删 auto");
    }

    // ── S1 async DB：增删改查全路径（内存库，验证 tokio-rusqlite 闭包往返）──
    #[tokio::test]
    async fn s1_async_platform_crud_roundtrip() {
        let db = test_db().await;
        // create
        let mut input = sample_platform("crud");
        input.base_url = "https://crud.example/v1".to_string();
        let created = create_platform(&db, input).await.unwrap();
        assert!(created.id >= 1);

        // read (list + get)
        assert_eq!(list_platforms(&db).await.unwrap().len(), 1);
        let got = get_platform(&db, created.id).await.unwrap().unwrap();
        assert_eq!(got.base_url, "https://crud.example/v1");

        // update
        let updated = update_platform(&db, UpdatePlatform {
            id: created.id,
            name: None,
            platform_type: None,
            base_url: Some("https://crud.example/v2".to_string()),
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: None,
            manual_budgets: None,
            breaker_failure_threshold: None,
            breaker_open_secs: None,
            breaker_half_open_max: None,
            auto_group: None,
            join_group_ids: None,
        }).await.unwrap();
        assert_eq!(updated.base_url, "https://crud.example/v2");
        assert_eq!(get_platform(&db, created.id).await.unwrap().unwrap().base_url, "https://crud.example/v2");

        // delete（软删）→ list 不含、get None
        delete_platform(&db, created.id).await.unwrap();
        assert_eq!(list_platforms(&db).await.unwrap().len(), 0);
        assert!(get_platform(&db, created.id).await.unwrap().is_none());
    }

    /// 401/403 自动禁用：状态变 auto_disabled，strikes 递增，退避指数 1h/2h/4h。
    #[tokio::test]
    async fn auto_disable_exponential_backoff() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("ad")).await.unwrap();
        assert_eq!(p.status, PlatformStatus::Enabled);

        let base = 60 * 60 * 1000i64;
        // 第 1 次：strikes=1, 退避 1h
        let t0 = now();
        let until1 = set_platform_auto_disabled(&db, p.id).await.unwrap();
        let g1 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g1.status, PlatformStatus::AutoDisabled);
        assert!(!g1.enabled, "auto_disabled 平台 enabled 列同步为 false");
        assert_eq!(g1.auto_disable_strikes, 1);
        assert!(until1 >= t0 + base && until1 <= now() + base + 1000, "first backoff ~1h");

        // 第 2 次：strikes=2, 退避 2h
        set_platform_auto_disabled(&db, p.id).await.unwrap();
        let g2 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g2.auto_disable_strikes, 2);
        assert!(g2.auto_disabled_until - now() >= 2 * base - 2000, "second backoff ~2h");

        // 第 3 次：strikes=3, 退避 4h
        set_platform_auto_disabled(&db, p.id).await.unwrap();
        let g3 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g3.auto_disable_strikes, 3);
        assert!(g3.auto_disabled_until - now() >= 4 * base - 2000, "third backoff ~4h");

        // 2xx 恢复：清状态
        recover_platform_auto_disabled(&db, p.id).await.unwrap();
        let g4 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g4.status, PlatformStatus::Enabled);
        assert!(g4.enabled);
        assert_eq!(g4.auto_disable_strikes, 0);
        assert_eq!(g4.auto_disabled_until, 0);
    }

    /// 用户手动 disabled 平台不受 401/403 自动禁用影响（区分手动 vs 自动）。
    #[tokio::test]
    async fn auto_disable_skips_user_disabled() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("ud")).await.unwrap();
        // 用户手动禁用
        let upd = update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: Some(PlatformStatus::Disabled), manual_budgets: None,
            breaker_failure_threshold: None, breaker_open_secs: None, breaker_half_open_max: None,
            auto_group: None, join_group_ids: None,
        }).await.unwrap();
        assert_eq!(upd.status, PlatformStatus::Disabled);
        assert!(!upd.enabled);

        // 401/403 触发不应改成 auto_disabled
        let until = set_platform_auto_disabled(&db, p.id).await.unwrap();
        assert_eq!(until, 0, "user-disabled 平台不进入退避");
        let g = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g.status, PlatformStatus::Disabled, "保持用户手动禁用");
    }

    /// 404/405 死端点：连续累计达阈值才禁用；未达阈值保持 enabled。
    #[tokio::test]
    async fn dead_endpoint_strikes_accumulate_then_disable() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("de")).await.unwrap();
        let th = DEAD_ENDPOINT_STRIKE_THRESHOLD; // 3
        assert!(th >= 2, "阈值须 ≥2 才能体现累计语义");

        // 前 th-1 次：仅累计计数，保持 enabled、不退避
        for i in 1..th {
            let (strikes, until) = record_dead_endpoint_strike(&db, p.id, th).await.unwrap();
            assert_eq!(strikes, i, "第 {i} 次 strikes 递增");
            assert_eq!(until, 0, "未达阈值不禁用");
            let g = get_platform(&db, p.id).await.unwrap().unwrap();
            assert_eq!(g.status, PlatformStatus::Enabled, "未达阈值仍 enabled，继续参与调度");
            assert!(g.enabled);
        }

        // 第 th 次：达阈值 → auto_disabled + 退避
        let (strikes, until) = record_dead_endpoint_strike(&db, p.id, th).await.unwrap();
        assert_eq!(strikes, th);
        assert!(until > now(), "达阈值后进入退避");
        let g = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g.status, PlatformStatus::AutoDisabled, "达阈值后临时禁用");
        assert!(!g.enabled);
    }

    /// 偶发 404/405：未达阈值 + 一次 2xx 成功 → 计数清零，平台不被误禁。
    #[tokio::test]
    async fn dead_endpoint_transient_reset_on_success() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("dt")).await.unwrap();
        let th = DEAD_ENDPOINT_STRIKE_THRESHOLD;

        // 累计 th-1 次（差一次就禁用）
        for _ in 1..th {
            record_dead_endpoint_strike(&db, p.id, th).await.unwrap();
        }
        assert_eq!(get_platform(&db, p.id).await.unwrap().unwrap().auto_disable_strikes, th - 1);
        assert_eq!(get_platform(&db, p.id).await.unwrap().unwrap().status, PlatformStatus::Enabled);

        // 一次成功 → 清零计数
        reset_dead_endpoint_strikes(&db, p.id).await.unwrap();
        let g = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g.auto_disable_strikes, 0, "成功后计数清零");
        assert_eq!(g.status, PlatformStatus::Enabled);

        // 之后再来一次 404 → 重新从 1 数起，不会因历史累计被立即禁
        let (strikes, until) = record_dead_endpoint_strike(&db, p.id, th).await.unwrap();
        assert_eq!(strikes, 1, "清零后重新从 1 累计");
        assert_eq!(until, 0);
    }

    /// 死端点累计跳过用户手动禁用平台（区分手动 vs 自动）。
    #[tokio::test]
    async fn dead_endpoint_skips_user_disabled() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("du")).await.unwrap();
        update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: Some(PlatformStatus::Disabled), manual_budgets: None,
            breaker_failure_threshold: None, breaker_open_secs: None, breaker_half_open_max: None,
            auto_group: None, join_group_ids: None,
        }).await.unwrap();

        let (strikes, until) = record_dead_endpoint_strike(&db, p.id, DEAD_ENDPOINT_STRIKE_THRESHOLD).await.unwrap();
        assert_eq!((strikes, until), (0, 0), "user-disabled 平台死端点信号不动它");
        assert_eq!(get_platform(&db, p.id).await.unwrap().unwrap().status, PlatformStatus::Disabled);
    }

    /// 改 api_key 自恢复：auto_disabled 平台改 api_key → 立即恢复 enabled 清退避。
    #[tokio::test]
    async fn api_key_change_recovers_auto_disabled() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("rk")).await.unwrap();
        set_platform_auto_disabled(&db, p.id).await.unwrap();
        assert_eq!(get_platform(&db, p.id).await.unwrap().unwrap().status, PlatformStatus::AutoDisabled);

        // 改 api_key（不显式传 status）
        let upd = update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None,
            api_key: Some("sk-new-key".to_string()),
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: None, manual_budgets: None,
            breaker_failure_threshold: None, breaker_open_secs: None, breaker_half_open_max: None,
            auto_group: None, join_group_ids: None,
        }).await.unwrap();
        assert_eq!(upd.status, PlatformStatus::Enabled, "改 api_key 立即恢复");
        assert_eq!(upd.auto_disable_strikes, 0);
        assert_eq!(upd.auto_disabled_until, 0);
    }

    /// group max_retries 持久化往返
    #[tokio::test]
    async fn group_max_retries_roundtrip() {
        let db = test_db().await;
        let mut input = sample_group("mr", vec![]);
        input.max_retries = 5;
        let g = create_group(&db, input).await.unwrap();
        assert_eq!(g.max_retries, 5);
        let fetched = get_group(&db, g.id).await.unwrap().unwrap();
        assert_eq!(fetched.max_retries, 5);

        let upd = update_group(&db, UpdateGroup {
            id: g.id, name: None, routing_mode: None,
            request_timeout_secs: 0, connect_timeout_secs: 0, source_protocol: None,
            max_retries: Some(0), model_mappings: vec![],
        }).await.unwrap();
        assert_eq!(upd.max_retries, 0);
        assert_eq!(get_group(&db, g.id).await.unwrap().unwrap().max_retries, 0);
    }

    /// proxy_log attempts JSON 列往返
    #[tokio::test]
    async fn proxy_log_attempts_roundtrip() {
        let db = test_db().await;
        let mut log = sample_log("attlog", "g", now());
        log.attempts = vec![
            super::super::models::ProxyAttempt {
                platform_id: 1, platform_name: "p1".into(), status_code: 503,
                error: "boom".into(), duration_ms: 12, ts: now(),
            },
            super::super::models::ProxyAttempt {
                platform_id: 2, platform_name: "p2".into(), status_code: 200,
                error: String::new(), duration_ms: 34, ts: now(),
            },
        ];
        log.retry_count = 1;
        upsert_proxy_log(&db, log).await.unwrap();
        let fetched = get_proxy_log(&db, "attlog").await.unwrap().unwrap();
        assert_eq!(fetched.attempts.len(), 2);
        assert_eq!(fetched.attempts[0].status_code, 503);
        assert_eq!(fetched.attempts[1].platform_name, "p2");
        assert_eq!(fetched.retry_count, 1);
    }

    /// 字段完整性红线：渐进式「首节点 INSERT + 后续节点部分列 UPDATE」累积写入后，
    /// proxy_log 整行所有列必须与旧「全列 INSERT OR REPLACE 终态」等价。
    /// 含 strip(脱敏)、token、est_cost、attempts、is_stream、blocked_* 等全字段覆盖。
    #[tokio::test]
    async fn progressive_columns_equals_full_replace() {
        let db = test_db().await;
        let now_ms = now();

        // 构造一条完整请求的「终态」ProxyLog（含全字段非默认值，验证无字段丢失）。
        let mut final_log = sample_log("prog", "grp", now_ms);
        final_log.actual_model = "deepseek-chat".into();
        final_log.request_headers = "{\"x\":\"1\"}".into();
        final_log.request_body = "{\"q\":\"hi\"}".into();
        final_log.upstream_request_headers = "{\"auth\":\"r\"}".into();
        final_log.upstream_request_body = "{\"m\":\"x\"}".into();
        final_log.response_body = "{\"ok\":true}".into();
        final_log.request_url = "http://localhost/v1/messages".into();
        final_log.upstream_request_url = "https://up/chat/completions".into();
        final_log.upstream_response_headers = "{\"ct\":\"json\"}".into();
        final_log.upstream_status_code = 200;
        final_log.user_response_headers = "{\"ct\":\"json\"}".into();
        final_log.user_response_body = "{\"ok\":true}".into();
        final_log.status_code = 200;
        final_log.duration_ms = 321;
        final_log.input_tokens = 111;
        final_log.output_tokens = 222;
        final_log.cache_tokens = 33;
        final_log.est_cost = 0.0042;
        final_log.is_stream = true;
        final_log.attempts = vec![super::super::models::ProxyAttempt {
            platform_id: 1, platform_name: "p1".into(), status_code: 200,
            error: String::new(), duration_ms: 99, ts: now_ms,
        }];
        final_log.retry_count = 0;
        final_log.blocked_by = String::new();
        final_log.blocked_reason = String::new();

        // 旧路径：直接全列 REPLACE 终态。
        let mut old_log = final_log.clone();
        old_log.id = "old".into();
        upsert_proxy_log(&db, old_log).await.unwrap();
        let old_row = get_proxy_log(&db, "old").await.unwrap().unwrap();

        // 新路径：模拟节点序列（每节点带「本阶段新增字段」，其余沿用上次）。
        // 节点1：请求建立（id/group/model/protocols/url，无 token/响应）。
        let mut n1 = sample_log("prog", "grp", now_ms);
        n1.model = final_log.model.clone();
        n1.source_protocol = final_log.source_protocol.clone();
        n1.target_protocol = final_log.target_protocol.clone();
        n1.actual_model = final_log.actual_model.clone();
        n1.request_headers = final_log.request_headers.clone();
        n1.request_body = final_log.request_body.clone();
        n1.request_url = final_log.request_url.clone();
        n1.status_code = 0;
        n1.duration_ms = 0;
        n1.input_tokens = 0;
        n1.output_tokens = 0;
        n1.cache_tokens = 0;
        n1.upstream_status_code = 0;
        n1.response_body = String::new();
        n1.user_response_body = String::new();
        n1.user_response_headers = String::new();
        n1.is_stream = false;
        let c1 = ProxyLogColumns::from_log(&n1, false, false);
        insert_proxy_log_columns(&db, c1.clone()).await.unwrap();

        // 节点2：上游请求/响应头（upstream_* 字段）。
        let mut n2 = n1.clone();
        n2.upstream_request_headers = final_log.upstream_request_headers.clone();
        n2.upstream_request_body = final_log.upstream_request_body.clone();
        n2.upstream_request_url = final_log.upstream_request_url.clone();
        n2.upstream_response_headers = final_log.upstream_response_headers.clone();
        n2.upstream_status_code = final_log.upstream_status_code;
        n2.is_stream = final_log.is_stream;
        let c2 = ProxyLogColumns::from_log(&n2, false, false);
        update_proxy_log_columns(&db, c2.clone(), &c1).await.unwrap();

        // 节点3：终态（token/est_cost/状态/body/attempts）。
        let c3 = ProxyLogColumns::from_log(&final_log, false, false);
        update_proxy_log_columns(&db, c3, &c2).await.unwrap();

        let new_row = get_proxy_log(&db, "prog").await.unwrap().unwrap();

        // 全列等价比对：序列化后比 JSON（覆盖所有字段，id 除外）。
        let mut a = serde_json::to_value(&old_row).unwrap();
        let mut b = serde_json::to_value(&new_row).unwrap();
        a.as_object_mut().unwrap().remove("id");
        b.as_object_mut().unwrap().remove("id");
        assert_eq!(a, b, "渐进式累积写入整行字段须与全列 REPLACE 终态完全等价");
    }

    /// strip(脱敏) 等价性：log_user_request/log_upstream_request 关时，仅 `*_body`
    /// （prompt / 响应正文）被清空；`*_headers`（元数据，auth 已脱敏）始终保留。
    #[tokio::test]
    async fn progressive_columns_strip_equivalence() {
        let db = test_db().await;
        let now_ms = now();
        let mut log = sample_log("strip", "grp", now_ms);
        log.request_headers = "secret-h".into();
        log.request_body = "secret-b".into();
        log.user_response_headers = "ur-h".into();
        log.user_response_body = "ur-b".into();
        log.upstream_request_headers = "up-rh".into();
        log.upstream_request_body = "up-rb".into();
        log.upstream_response_headers = "up-resp-h".into();

        // strip_user=true, strip_upstream=true → 仅 3 个 body 列清空，4 个 headers 列保留。
        let cols = ProxyLogColumns::from_log(&log, true, true);
        insert_proxy_log_columns(&db, cols).await.unwrap();
        let row = get_proxy_log(&db, "strip").await.unwrap().unwrap();

        // headers 始终记录（元数据，auth 已脱敏）。
        assert_eq!(row.request_headers, "secret-h");
        assert_eq!(row.user_response_headers, "ur-h");
        assert_eq!(row.upstream_request_headers, "up-rh");
        assert_eq!(row.upstream_response_headers, "up-resp-h");
        // body 受开关控制 → 清空。
        assert!(row.request_body.is_empty());
        assert!(row.user_response_body.is_empty());
        assert!(row.upstream_request_body.is_empty());
        // 非脱敏字段保留。
        assert_eq!(row.group_key, "grp");
        assert_eq!(row.model, "claude-sonnet-4");
    }

    // ── S1 async DB：OptionalExtension 路径（query_row().optional() 在闭包内）──
    #[tokio::test]
    async fn s1_async_optional_extension_returns_none_for_missing() {
        let db = test_db().await;
        // 不存在的 id → get_platform 走 query_row().optional() 返回 None（非 Err）
        assert!(get_platform(&db, 99_999).await.unwrap().is_none());
        // 存在则返回 Some
        let p = create_platform(&db, sample_platform("opt")).await.unwrap();
        assert!(get_platform(&db, p.id).await.unwrap().is_some());
        // get_setting 同样走 optional()：缺键 None
        assert!(get_setting(&db, "nope", "nope").await.unwrap().is_none());
    }

    // ════════════════════════════════════════════════════════════════════
    // C4：内置预设规则集 seed + 正则命中
    // ════════════════════════════════════════════════════════════════════

    /// 全新 db 首启即 seed 全部内置规则，is_builtin=1 且默认 enabled。
    #[tokio::test]
    async fn c4_fresh_db_seeds_builtin_rules() {
        let db = test_db().await;
        let rules = list_middleware_rules(&db).await.unwrap();
        let builtin: Vec<_> = rules.iter().filter(|r| r.is_builtin).collect();
        assert_eq!(
            builtin.len(),
            builtin_rule_specs().len(),
            "首启应 seed 全部内置规则"
        );
        for r in &builtin {
            assert!(r.enabled, "内置规则 {} 默认应 enabled", r.name);
            assert_eq!(r.scope, RuleScope::Global);
        }
        // 三条脱敏 + 默认 error_rules
        assert!(builtin.iter().any(|r| r.name == "内置·密钥脱敏"));
        assert!(builtin.iter().any(|r| r.name == "内置·邮箱脱敏"));
        assert!(builtin.iter().any(|r| r.name == "内置·手机号脱敏"));
        assert!(builtin
            .iter()
            .any(|r| r.rule_type == RuleType::ErrorRule));
    }

    /// 密钥/邮箱脱敏走 content_filter 空 pattern（复用 C2 内置检测器），手机走显式 regex。
    #[tokio::test]
    async fn c4_secret_email_reuse_c2_detector_phone_explicit() {
        let db = test_db().await;
        let rules = list_middleware_rules(&db).await.unwrap();
        let secret = rules.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();
        let email = rules.iter().find(|r| r.name == "内置·邮箱脱敏").unwrap();
        let phone = rules.iter().find(|r| r.name == "内置·手机号脱敏").unwrap();
        // 密钥/邮箱 pattern 留空 → C2 BUILTIN_SECRET/EMAIL 检测器接管，避免重复定义正则
        assert!(secret.pattern.is_empty(), "密钥规则 pattern 应留空复用 C2 检测器");
        assert!(email.pattern.is_empty(), "邮箱规则 pattern 应留空复用 C2 检测器");
        assert_eq!(secret.rule_type, RuleType::ContentFilter);
        assert_eq!(secret.action, RuleAction::Mask);
        // 手机 C2 无内置检测器 → 显式 regex
        assert!(!phone.pattern.is_empty(), "手机规则用显式 regex");
        assert_eq!(phone.match_type, MatchType::Regex);
    }

    /// 内置手机号正则命中中国大陆 11 位 + E.164；不误伤普通数字。
    #[test]
    fn c4_builtin_phone_pattern_matches_samples() {
        let re = regex::Regex::new(BUILTIN_PHONE_PATTERN).unwrap();
        assert!(re.is_match("联系我 13812345678 谢谢"), "中国大陆手机号");
        assert!(re.is_match("call +14155552671 now"), "E.164 国际号");
        assert!(!re.is_match("订单号 12345"), "短数字不应命中");
    }

    /// 内置默认 error_rules 正则命中各 category 的样例上游错误消息。
    #[test]
    fn c4_builtin_error_rules_match_samples() {
        // (category, 样例错误消息)
        let samples: &[(&str, &str)] = &[
            ("prompt_limit", "This model's maximum context length is 128000 tokens"),
            ("content_filter", "The response was flagged by our content filter"),
            ("pdf_limit", "PDF has too many pages, maximum is 100"),
            ("thinking_error", "thinking is not supported for this model"),
            ("parameter_error", "Unsupported parameter: 'temperature' is not allowed"),
            ("invalid_request", "invalid_request_error: missing field"),
            ("cache_limit", "prompt cache: too many cache_control blocks"),
        ];
        for (category, msg) in samples {
            let spec = builtin_rule_specs()
                .iter()
                .find(|s| s.rule_type == "error_rule" && s.config.contains(&format!("\"category\":\"{category}\"")))
                .unwrap_or_else(|| panic!("缺 category={category} 的 error_rule"));
            let re = regex::Regex::new(spec.pattern).unwrap();
            assert!(
                re.is_match(msg),
                "category={category} 正则 {} 应命中样例: {msg}",
                spec.pattern
            );
        }
    }

    /// seed 幂等：重复调用（模拟重启）不重复插入。
    #[tokio::test]
    async fn c4_seed_is_idempotent_on_restart() {
        let db = test_db().await;
        let before = list_middleware_rules(&db).await.unwrap().len();
        // 再次跑一遍 init_tables（含 seed），模拟重启
        db.init_tables().await.unwrap();
        let after = list_middleware_rules(&db).await.unwrap().len();
        assert_eq!(before, after, "重启不应重复 seed");
    }

    /// 用户禁用内置规则后重启不被重新启用（尊重用户禁用状态，内置可禁不可硬删）。
    #[tokio::test]
    async fn c4_seed_respects_user_disabled_state() {
        let db = test_db().await;
        let rules = list_middleware_rules(&db).await.unwrap();
        let secret = rules.iter().find(|r| r.name == "内置·密钥脱敏").unwrap().clone();
        // 用户禁用该内置规则
        update_middleware_rule(
            &db,
            UpdateMiddlewareRule {
                id: secret.id,
                name: secret.name.clone(),
                description: secret.description.clone(),
                rule_type: secret.rule_type,
                scope: secret.scope,
                scope_ref: secret.scope_ref.clone(),
                match_type: secret.match_type,
                pattern: secret.pattern.clone(),
                action: secret.action,
                config: secret.config.clone(),
                priority: secret.priority,
                enabled: false,
                is_builtin: true,
            },
        )
        .await
        .unwrap();
        // 重启
        db.init_tables().await.unwrap();
        let after = list_middleware_rules(&db).await.unwrap();
        let secret_after = after.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();
        assert!(!secret_after.enabled, "用户禁用的内置规则重启后不应被重新启用");
        // 仍只有一条（未重复插入）
        let count = after.iter().filter(|r| r.name == "内置·密钥脱敏").count();
        assert_eq!(count, 1, "禁用的内置规则不应被重复 seed");
    }

    // ── Notification 收件箱 CRUD（N1）──
    #[tokio::test]
    async fn notification_inbox_crud() {
        let db = test_db().await;
        // 空库
        assert!(list_notifications(&db, 50).await.unwrap().is_empty());

        let id1 = insert_notification(&db, "task_complete", "任务完成", "项目 X 完成").await.unwrap();
        let id2 = insert_notification(&db, "error", "出错", "构建失败").await.unwrap();
        assert!(id2 > id1);

        let list = list_notifications(&db, 50).await.unwrap();
        assert_eq!(list.len(), 2);
        // 倒序：最新在前
        assert_eq!(list[0].id, id2);
        assert_eq!(list[0].notif_type, "error");
        assert_eq!(list[1].title, "任务完成");

        // limit 生效
        for i in 0..5 {
            insert_notification(&db, "task_complete", &format!("t{i}"), "b").await.unwrap();
        }
        assert_eq!(list_notifications(&db, 3).await.unwrap().len(), 3);

        // 清空
        clear_notifications(&db).await.unwrap();
        assert!(list_notifications(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn notification_settings_default_when_absent() {
        let db = test_db().await;
        let s = get_notification_settings(&db).await;
        assert!(s.enabled && s.tts_enabled);
        // 写入后读回
        set_setting(&db, SetSettingInput {
            scope: "notification".into(),
            key: "settings".into(),
            value: serde_json::json!({"enabled": false, "tts_enabled": false}),
        }).await.unwrap();
        let s2 = get_notification_settings(&db).await;
        assert!(!s2.enabled && !s2.tts_enabled);
    }
}

