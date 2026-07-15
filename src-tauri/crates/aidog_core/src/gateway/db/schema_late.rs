use super::*;
use rusqlite::{params, Connection, Result as SqlResult};

/// Migrations 021–036。自 init_tables 拆出（执行顺序不变）。
pub(crate) fn run_migrations_late(conn: &Connection) -> SqlResult<()> {
                // Migration 021: model_price 加模型信息列（max_tokens / context_window）。
                // 列为索引快速读取（出站裁剪、列表展示）；price_data JSON 仍存完整原始数据。
                // NULL = 未知/无限制。源自旧 008_model_info_columns（已内联为下方 ALTER）。
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
                    conn.execute_batch(
                        r#"-- Migration 009: 移除 group.path，分组路由纯按 apikey(group.name)。
-- 原 path 用作 URL 前缀路由 fallback + UNIQUE 标识；现统一按
-- Authorization Bearer(apikey = group.name) 精确匹配，不再支持路径前缀路由。
-- SQLite 不能直接 DROP COLUMN + 加表级约束，重建表。
-- group 无独立 index，重建不丢索引；列名显式匹配保证幂等（path 已删的库 SELECT 同样命中）。
-- name 加 UNIQUE（apikey 语义唯一，防重名 group 创建）。
CREATE TABLE IF NOT EXISTS "group_new" (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    name                 TEXT NOT NULL DEFAULT '',
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
    UNIQUE(name)
);
INSERT INTO "group_new"
    (id, name, routing_mode, auto_from_platform, source_protocol, model_mappings,
     request_timeout_secs, connect_timeout_secs, created_at, updated_at, deleted_at,
     sort_order, max_retries)
SELECT
    id, name, routing_mode, auto_from_platform, source_protocol, model_mappings,
    request_timeout_secs, connect_timeout_secs, created_at, updated_at, deleted_at,
    sort_order, max_retries
FROM "group";
DROP TABLE "group";
ALTER TABLE "group_new" RENAME TO "group";
"#,
                    )?;
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
                    conn.execute_batch(
                        r#"-- Migration 010: group 拆 group_key（密钥/路由/日志归属键）+ name（显示名）。
-- group_key UNIQUE: Bearer token + 路由匹配键 + proxy_log 归属键（前端按 group_key 反查 name 显示）。
-- name UNIQUE: 防重名显示。
-- 老 group.group_key 初值 = 旧 name（statusline 脚本 / 已分发 token 不破，用户后续可改）。
-- SQLite 不能给现存表加 UNIQUE 列约束，重建表（仿 009_drop_group_path.sql 幂等范式）。
CREATE TABLE IF NOT EXISTS "group_new" (
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
    UNIQUE(name),
    UNIQUE(group_key)
);
-- 仅当源表存在 group_key 列时取已存值，否则用 name 兜底（首次迁移 + 兼容已迁移库）。
-- SQLite 无 IF_COL_EXISTS；靠列名显式匹配：旧库无 group_key → 用 name 作 group_key。
INSERT INTO "group_new"
    (id, name, group_key, routing_mode, auto_from_platform, source_protocol, model_mappings,
     request_timeout_secs, connect_timeout_secs, created_at, updated_at, deleted_at,
     sort_order, max_retries)
SELECT
    id, name, name, routing_mode, auto_from_platform, source_protocol, model_mappings,
    request_timeout_secs, connect_timeout_secs, created_at, updated_at, deleted_at,
    sort_order, max_retries
FROM "group";
DROP TABLE "group";
ALTER TABLE "group_new" RENAME TO "group";
"#,
                    )?;
                }
                // proxy_log.group_key RENAME → run_migrations_proxy_log_late（proxy_log.db）
                // Migration 025: GLM Coding Plan anthropic 端点补标 coding_plan=true。
                // 根因：Platforms.tsx glm 预设曾把 anthropic 端点漏标 coding_plan，coding plan 平台
                // (含 openai coding 端点)的 anthropic(Claude Code)入站经 select_endpoint_for_protocol
                // 同协议匹配落空 → 回退 openai coding 端点 → 被转换成 openai。GLM 与 Kimi 不同：
                // openai/anthropic 端点同处 open.bigmodel.cn 同一把 key 通用，anthropic 端点合法。
                // 仅修「已是 coding plan(有 coding openai 端点)且 anthropic 端点未标 coding_plan」的 GLM 平台。
                // 幂等：已标 coding_plan 的不动；非 coding plan GLM(无 coding 端点)不动。
                if let Ok(mut stmt) =
                    conn.prepare("SELECT id, endpoints FROM platform WHERE platform_type = 'glm'")
                {
                    let rows: Vec<(i64, String)> = stmt
                        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                        .ok()
                        .map(|iter| iter.filter_map(Result::ok).collect())
                        .unwrap_or_default();
                    for (id, endpoints_json) in rows {
                        let mut eps = parse_endpoints(&endpoints_json);
                        // 仅当该平台确为 coding plan（存在 coding_plan 的 openai 端点）才补标 anthropic。
                        let is_coding_plan = eps
                            .iter()
                            .any(|ep| ep.coding_plan && ep.protocol == Protocol::OpenAI);
                        if !is_coding_plan {
                            continue;
                        }
                        let mut changed = false;
                        for ep in &mut eps {
                            if ep.protocol == Protocol::Anthropic && !ep.coding_plan {
                                ep.coding_plan = true;
                                changed = true;
                            }
                        }
                        if changed {
                            let new_json = serialize_endpoints(&eps);
                            let _ = conn.execute(
                                "UPDATE platform SET endpoints = ?1 WHERE id = ?2",
                                params![new_json, id],
                            );
                            tracing::info!(platform_id = id, "migration 025: glm coding-plan anthropic endpoint coding_plan→true");
                        }
                    }
                }
                // Migration 026: platform 表精简 —— 删 auto_group(022) + 3 breaker 列(016)。
                // auto_group 改为创建时一次性判断（CreatePlatform transient 输入），不持久化。
                // breaker 阈值覆盖移入 extra JSON 的 breaker 对象（extra.breaker.{failure_threshold,
                // open_secs, half_open_max}），语义不变（0/缺省=继承全局默认）。
                // 步骤：① 把每行非 0 breaker 列值无损 backfill 进 extra.breaker；② DROP 4 列。
                // 幂等：PRAGMA 探测 breaker_failure_threshold 列存在性 —— 存在才迁移，已迁库跳过。
                let has_breaker_col = conn
                    .prepare("PRAGMA table_info(platform)")?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .any(|c| c == "breaker_failure_threshold");
                if has_breaker_col {
                    // ① backfill：逐行读旧 breaker 列 + extra，合并写回 extra。
                    let rows: Vec<(i64, String, i64, i64, i64)> = {
                        let mut stmt = conn.prepare(
                            "SELECT id, extra, breaker_failure_threshold, breaker_open_secs, breaker_half_open_max FROM platform",
                        )?;
                        let mapped = stmt.query_map([], |r| {
                            Ok((
                                r.get::<_, i64>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, i64>(2)?,
                                r.get::<_, i64>(3)?,
                                r.get::<_, i64>(4)?,
                            ))
                        })?;
                        mapped.filter_map(Result::ok).collect()
                    };
                    for (id, extra, ft, os, hom) in rows {
                        if ft == 0 && os == 0 && hom == 0 {
                            continue; // 无覆盖 → 不动 extra
                        }
                        let breaker = crate::gateway::models::PlatformBreaker {
                            failure_threshold: ft.max(0) as u32,
                            open_secs: os.max(0) as u64,
                            half_open_max: hom.max(0) as u32,
                        };
                        let new_extra = crate::gateway::models::merge_breaker_into_extra(&extra, &breaker);
                        conn.execute(
                            "UPDATE platform SET extra = ?1 WHERE id = ?2",
                            params![new_extra, id],
                        )?;
                    }
                    // ② DROP 4 列（SQLite 3.35+ 支持 ALTER DROP COLUMN，逐列执行）。
                    let _ = conn.execute("ALTER TABLE platform DROP COLUMN breaker_failure_threshold", []);
                    let _ = conn.execute("ALTER TABLE platform DROP COLUMN breaker_open_secs", []);
                    let _ = conn.execute("ALTER TABLE platform DROP COLUMN breaker_half_open_max", []);
                    let _ = conn.execute("ALTER TABLE platform DROP COLUMN auto_group", []);
                    tracing::info!("migration 026: backfilled breaker into extra + dropped auto_group/breaker_* columns");
                }
                // Migration 027: 默认分组标记（单选）。is_default=1 的组 config merge 写入
                // ~/.claude/settings.json + ~/.codex/config.toml，使用户直接 claude/codex
                // 不带 -c/--profile 即走该组。幂等：旧库 ALTER 无 IF NOT EXISTS，忽略 duplicate column。
                let _ = conn.execute("ALTER TABLE \"group\" ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0", []);
                // Migration 028: proxy_log 偏索引 → run_migrations_proxy_log_late
                // Migration 029: group_platform per-group 平台优先级 level_priority（1~10，默认 5，10=最高优先）。
                // 独立于 priority（拖拽排序连续序号）与 weight（负载均衡权重）。
                // Failover：level_priority 降序 tiebreak；weighted（LoadBalance/HealthAware/Sticky）：有效权重=weight×level_priority；
                // LeastLatency：延迟 EMA 主导，level_priority 作次级 tiebreaker。
                // 幂等：旧库 ALTER 无 IF NOT EXISTS，忽略 duplicate column；老行默认 5。
                let _ = conn.execute("ALTER TABLE group_platform ADD COLUMN level_priority INTEGER NOT NULL DEFAULT 5", []);

                // Migration 030: 「Claude Code / Codex 联动」重命名为通用「AI 编程工具」。
                // 把旧 settings key cc_codex_settings 迁到 coding_tools_settings，保留老用户两开关状态
                // （apply_to_claude_plugin / skip_claude_onboarding），避免重命名后开关回到默认关。
                // 幂等：仅当存在旧 key 时 UPDATE 改名；新库无旧 key 时空操作。
                let _ = conn.execute(
                    "UPDATE settings SET key='coding_tools_settings' WHERE scope='global' AND key='cc_codex_settings'",
                    [],
                );

                // Migration 031 ①: idx_proxy_log_group_key_stats → run_migrations_proxy_log_late
                // Migration 031 ②: notification 时间索引（主库）。
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_notification_created ON notification(created_at)",
                    [],
                );
                // Migration 032: stats_agg_hourly 建表 + 回填 → run_migrations_proxy_log_late
                // Migration 033: proxy_log.is_final DROP → run_migrations_proxy_log_late
                // Migration 034: proxy_log 索引精简 → run_migrations_proxy_log_late
                // Migration 035: 删冗余索引（proxy_log/stats_agg 相关 → proxy_log_late；idx_model_price_name 留主库）。
                let _ = conn.execute("DROP INDEX IF EXISTS idx_model_price_name", []);
                tracing::info!("migration 035: dropped redundant indexes (proxy_log/stats_agg部分在proxy_log_late)");
                // Migration 036: platform 过期时间（毫秒 unix 时间戳，0 = 永不过期）。
                // >0 且 now>=expires_at 时路由 candidate_state 排除（等效自动禁用，独立于 status 枚举）；
                // purge_auto_disabled_platforms 也一并清过期平台。幂等：旧库 ALTER 无 IF NOT EXISTS，忽略 duplicate column。
                let _ = conn.execute(
                    "ALTER TABLE platform ADD COLUMN expires_at INTEGER NOT NULL DEFAULT 0",
                    [],
                );

                // Migration 037: 平台最近一次错误信息（卡片展示用，非请求记录实时取）。
                // 上游非 2xx / 连接失败 / 空 2xx 重试时写 last_error+last_error_at；成功时清空。
                // 幂等：旧库 ALTER 无 IF NOT EXISTS，忽略 duplicate column。
                let _ = conn.execute(
                    "ALTER TABLE platform ADD COLUMN last_error TEXT NOT NULL DEFAULT ''",
                    [],
                );
                let _ = conn.execute(
                    "ALTER TABLE platform ADD COLUMN last_error_at INTEGER NOT NULL DEFAULT 0",
                    [],
                );

                // Migration 038: group 自定义环境变量（内联 JSON 数组，仿 model_mappings）。
                // sync 时注入 settings.{group}.json 的 env block（ANTHROPIC_BASE_URL /
                // ANTHROPIC_AUTH_TOKEN 由 aidog 强写，用户同名 key 在 sync_settings 过滤丢弃）。
                // 幂等：旧库 ALTER 无 IF NOT EXISTS，忽略 duplicate column；老行回填 '[]'。
                let _ = conn.execute(
                    "ALTER TABLE \"group\" ADD COLUMN env_vars TEXT NOT NULL DEFAULT '[]'",
                    [],
                );

                // Migration 039: 重写历史 last_error 残留完整 JSON body 为提取后 message。
                // 037 加列时 last_error 直接存 `HTTP {code}: {truncate_attempt_error(body)}`（含完整 JSON），
                // 后续 b9f82ed 才在写入前接入 extract_error_message。3 小时窗口内落库的旧行需一次性重提：
                // 拆 `HTTP {code}: ` 前缀后的 body → extract_error_message → 命中则重写。
                // 幂等：重提后行再跑命中相同 message（已是字符串非 JSON），不变；非 JSON / 无字段不动。
                // 仅处理 message 能提取且与原文不同的，其余（含连接错 / 纯文本限流）保留。
                reextract_legacy_last_error(conn);

                // Migration 040–042 已移除：旧 mitm_ca / mitm_whitelist 两表数据迁 setting（scope=mitm）
                // + DROP 两表。新库不再建两表，MITM 配置复用 setting 的 get_setting/set_setting + 缓存机制。
                // 详见 migration 043（migrate_mitm_legacy_tables_to_setting）。
                migrate_mitm_legacy_tables_to_setting(conn);

                // Migration 044: group.extra JSON 列（_ui_* UI 态 + 未来业务扩展，仿 platform.extra）。
                // 空串 = "{}" 的轻量表示（update_extra_key 读时统一视作 {}）。幂等：旧库 ALTER 无 IF NOT EXISTS，
                // duplicate column 错误被忽略；新库本就有此列。
                let _ = conn.execute(
                    "ALTER TABLE \"group\" ADD COLUMN extra TEXT NOT NULL DEFAULT ''",
                    [],
                );

                // Migration 045: cli_proxy_provider 表 —— cpa-standalone-module s1。
                // 独立的 CLI 代理上游 provider（与 platform 表解耦，路由/转换 s2/s4 接入）。
                // wire_protocol = 入站协议标识（anthropic/openai/glm_coding 等，对应 Protocol serde 形式）；
                // models = JSON 数组（Vec<String>）；extra = 原始 JSON 串（仿 platform.extra，空串视作 "{}"）；
                // status = active/disabled（默认 active）；group_id = 可空，归属分组（s2 路由层消费）。
                // 幂等：CREATE TABLE IF NOT EXISTS（对齐项目 migration idiom —— 无版本号机制，每次 init 跑全部）。
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS cli_proxy_provider (
                       id            INTEGER PRIMARY KEY AUTOINCREMENT,
                       name          TEXT NOT NULL,
                       wire_protocol TEXT NOT NULL,
                       base_url      TEXT NOT NULL,
                       api_key       TEXT NOT NULL DEFAULT '',
                       models        TEXT NOT NULL DEFAULT '[]',
                       extra         TEXT NOT NULL DEFAULT '{}',
                       status        TEXT NOT NULL DEFAULT 'active',
                       group_id      INTEGER,
                       created_at    INTEGER NOT NULL,
                       updated_at    INTEGER NOT NULL
                     );
                     CREATE INDEX IF NOT EXISTS idx_cli_proxy_group ON cli_proxy_provider(group_id) WHERE group_id IS NOT NULL;",
                )?;

                // Migration 046: 清理旧 CPA(CLIProxyAPI) 平台数据 —— cpa-standalone-module s4。
                // proxy_log / stats_agg_hourly 删除 → run_migrations_proxy_log_late（proxy_log.db，
                // 主库无这两表）。主库仅删 group_platform + platform。
                // CPA platform IDs 由 init_tables 预查主库后传入 proxy_log_late（跨库不能 JOIN）。
                // 幂等：无 cpa 行时 DELETE 0 行不报错；每次启动重跑无副作用。
                let _ = conn.execute(
                    "DELETE FROM group_platform WHERE platform_id IN \
                     (SELECT id FROM platform WHERE platform_type LIKE '\"cpa-%')",
                    [],
                );
                let _ = conn.execute(
                    "DELETE FROM platform WHERE platform_type LIKE '\"cpa-%'",
                    [],
                );

                // Migration 047: proxy_log 加 cli_proxy_provider_id → run_migrations_proxy_log_late
    Ok(())
}

/// proxy_log / stats_agg_hourly 表的 late migrations（021–047 范围内的 proxy_log / stats_agg 部分）。
///
/// 拆库后这些 DDL 跑在 proxy_log.db 写连接。`auto_map` 由 init_tables 从主库 `"group"` 表
/// 预加载传入（proxy_log.db 无 group 表，无法在闭包内 `load_auto_from_map`）。`cpa_pids` 为
/// migration 046 需清理的 CPA 平台 ID 列表（主库预查，跨库不能子查询 JOIN platform）。
pub(crate) fn run_migrations_proxy_log_late(
    conn: &Connection,
    auto_map: &HashMap<String, i64>,
    cpa_pids: &[i64],
) -> SqlResult<()> {
                // Migration 024 (proxy_log): group_name → group_key（幂等：探测列存在性）。
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
                // Migration 028: proxy_log 偏索引。
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_platform_id \
                     ON proxy_log(platform_id) WHERE deleted_at = 0",
                    [],
                );
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_group_key \
                     ON proxy_log(group_key) WHERE deleted_at = 0",
                    [],
                );
                // Migration 031 ①: idx_proxy_log_group_key_stats 覆盖索引。
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_group_key_stats \
                     ON proxy_log(group_key, est_cost, input_tokens, output_tokens, cache_tokens, status_code) \
                     WHERE deleted_at = 0",
                    [],
                );
                // Migration 032: stats_agg_hourly 建表 + 存量回填。
                conn.execute_batch(STATS_AGG_HOURLY_SQL)?;
                backfill_stats_agg_if_empty(conn, auto_map)?;
                // Migration 033: 删 proxy_log.is_final 列。
                let _ = conn.execute("ALTER TABLE proxy_log DROP COLUMN is_final", []);
                // Migration 034: proxy_log 索引精简 + 复合化 + ANALYZE。
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_group", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_platform", []);
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_status_created \
                     ON proxy_log(status_code, created_at) WHERE deleted_at = 0",
                    [],
                );
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_platform_created \
                     ON proxy_log(platform_id, created_at) WHERE deleted_at = 0",
                    [],
                );
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_group_created \
                     ON proxy_log(group_key, created_at) WHERE deleted_at = 0",
                    [],
                );
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_status", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_platform_id", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_group_key", []);
                let _ = conn.execute("ANALYZE proxy_log", []);
                // Migration 035 (proxy_log/stats_agg 部分): 删冗余索引。
                let _ = conn.execute("DROP INDEX IF EXISTS idx_stats_agg_model", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_stats_agg_group", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_created", []);
                // Migration 046 (proxy_log 部分): CPA 数据清理。cpa_pids 由主库预查传入。
                for pid in cpa_pids {
                    let _ = conn.execute(
                        "DELETE FROM proxy_log WHERE platform_id = ?1",
                        params![pid],
                    );
                    let _ = conn.execute(
                        "DELETE FROM stats_agg_hourly WHERE platform_id = ?1",
                        params![pid],
                    );
                }
                // Migration 047: proxy_log 加 cli_proxy_provider_id。
                let _ = conn.execute(
                    "ALTER TABLE proxy_log ADD COLUMN cli_proxy_provider_id INTEGER",
                    [],
                );
    Ok(())
}

/// Migration 039: 把 037 引入但未走 extract_error_message 的历史 last_error 行重提为 message。
fn reextract_legacy_last_error(conn: &Connection) {    // ponytail: SELECT 后逐行 UPDATE，避免 SQLite 无 JSON 函数；行数有限（仅失败过的平台）。
    let Ok(mut stmt) = conn.prepare("SELECT id, last_error FROM platform WHERE last_error != ''") else {
        return;
    };
    let entries: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .ok()
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default();
    for (id, stored) in entries {
        // stored = `HTTP {code}: {body}`；只切首个 `: `，保留 message 内可能出现的 `: `。
        let Some((prefix, body)) = stored.split_once(": ") else {
            continue; // 无 `: ` 分隔 → 非标准格式（如纯 "HTTP 429"），不动
        };
        let Some(msg) = crate::gateway::proxy::extract_error_message(body) else {
            continue; // body 非 JSON / 无 error.message → 保留原值（纯文本限流/连接错）
        };
        let new_val = format!("{prefix}: {msg}");
        if new_val != stored {
            let _ = conn.execute(
                "UPDATE platform SET last_error = ?1 WHERE id = ?2",
                params![new_val, id],
            );
        }
    }
}

/// Migration 043: MITM 配置从专属表（mitm_ca / mitm_whitelist）迁到通用 setting 表
///（scope=mitm，2 key：ca 对象 + whitelist 数组），并 DROP 两旧表。
///
/// 三种库状态全覆盖：
///  1. 旧库（有 mitm_ca / mitm_whitelist 表 + 数据）：读旧表 → 构造 JSON → INSERT OR IGNORE setting
///     → DROP 两表。数据不丢。
///  2. 旧库已迁（无两表，setting 已有 mitm 行）：INSERT OR IGNORE 幂等跳过，DROP IF EXISTS 空操作。
///  3. 新库（从未建两表）：跳过数据迁移，仅 seed 默认白名单到 setting（若 setting 无 mitm:whitelist）。
///
/// seed 并入 migration（单源，避免 seed 函数与新表脱节）：新库或旧库无白名单数据时，
/// 填 37 条 DEFAULT_RULES + 已配平台 base_url host 到 setting (mitm, whitelist)。
/// 幂等：INSERT OR IGNORE setting + DROP TABLE IF EXISTS。
fn migrate_mitm_legacy_tables_to_setting(conn: &Connection) {
    let now = now();
    let scope = "mitm";

    // ── 1. mitm_ca → setting (mitm, ca) ──
    let has_mitm_ca = table_exists(conn, "mitm_ca");
    if has_mitm_ca {
        if let Ok(ca_json) = conn.query_row(
            "SELECT private_key_pem, cert_pem, fingerprint, created_at, enabled, ca_installed \
             FROM mitm_ca WHERE id = 1",
            [],
            |r| {
                let private_key_pem: String = r.get(0)?;
                let cert_pem: String = r.get(1)?;
                let fingerprint: String = r.get(2)?;
                let created_at: i64 = r.get(3)?;
                let enabled: bool = r.get::<_, i64>(4)? != 0;
                let ca_installed: bool = r.get::<_, i64>(5)? != 0;
                Ok(serde_json::json!({
                    "private_key_pem": private_key_pem,
                    "cert_pem": cert_pem,
                    "fingerprint": fingerprint,
                    "created_at": created_at,
                    "enabled": enabled,
                    "ca_installed": ca_installed,
                }))
            },
        ) {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO setting (scope, key, value, created_at, updated_at, deleted_at) \
                 VALUES (?1, 'ca', ?2, ?3, ?3, 0)",
                params![scope, ca_json.to_string(), now],
            );
        }
        let _ = conn.execute("DROP TABLE IF EXISTS mitm_ca", []);
    }

    // ── 2. mitm_whitelist → setting (mitm, whitelist) ──
    let has_mitm_whitelist = table_exists(conn, "mitm_whitelist");
    if has_mitm_whitelist {
        // 读全表 ORDER BY created_at ASC（数组顺序 = created_at 升序，保旧行为）。
        let Ok(mut stmt) = conn.prepare(
            "SELECT host_pattern, rule_type, enabled, source FROM mitm_whitelist \
             ORDER BY created_at ASC",
        ) else {
            let _ = conn.execute("DROP TABLE IF EXISTS mitm_whitelist", []);
            return;
        };
        let entries: Vec<serde_json::Value> = stmt
            .query_map([], |r| {
                let host_pattern: String = r.get(0)?;
                let rule_type: String = r.get(1)?;
                let enabled: bool = r.get::<_, i64>(2)? != 0;
                let source: String = r.get(3)?;
                Ok(serde_json::json!({
                    "host_pattern": host_pattern,
                    "rule_type": rule_type,
                    "enabled": enabled,
                    "source": source,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default();
        let whitelist_json = serde_json::Value::Array(entries);
        let _ = conn.execute(
            "INSERT OR IGNORE INTO setting (scope, key, value, created_at, updated_at, deleted_at) \
             VALUES (?1, 'whitelist', ?2, ?3, ?3, 0)",
            params![scope, whitelist_json.to_string(), now],
        );
        let _ = conn.execute("DROP TABLE IF EXISTS mitm_whitelist", []);
    }

    // ── 3. seed 默认白名单（新库 / setting 无 mitm:whitelist）──
    // 仅 setting (mitm, whitelist) 不存在或空数组时填默认。已迁过的库（whitelist 有数据）跳过。
    let need_seed: bool = conn
        .query_row(
            "SELECT value FROM setting WHERE scope = ?1 AND key = 'whitelist' AND deleted_at = 0",
            params![scope],
            |r| {
                let v: String = r.get(0)?;
                // 空数组 `[]` 或无行 → 需 seed；非空数组 → 跳过。
                let parsed: serde_json::Value =
                    serde_json::from_str(&v).unwrap_or(serde_json::Value::Null);
                Ok(parsed.as_array().map(|a| a.is_empty()).unwrap_or(true))
            },
        )
        .unwrap_or(true); // 无行 → 需 seed
    if !need_seed {
        return;
    }

    let mut entries: Vec<serde_json::Value> = Vec::new();
    // Clash 规则集 37 条（Claude 3 + OpenAI 34）— 单源常量在 whitelist.rs。
    for (rule_type, pattern) in super::super::mitm::whitelist::DEFAULT_RULES {
        entries.push(serde_json::json!({
            "host_pattern": pattern,
            "rule_type": rule_type,
            "enabled": true,
            "source": "default",
        }));
    }
    // 已配平台 base_url host（domain 精确 host）。仅未删除平台。
    if let Ok(mut stmt) = conn.prepare(
        "SELECT base_url FROM platform WHERE deleted_at = 0 AND base_url != ''",
    ) {
        let hosts: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .ok()
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default();
        for base_url in hosts {
            if let Some(host) = crate::gateway::proxy::endpoint_host(&base_url) {
                // 去重：不与 DEFAULT_RULES / 已加平台 host 重复。
                let dup = entries.iter().any(|e| {
                    e.get("host_pattern").and_then(|v| v.as_str()) == Some(host.as_str())
                });
                if !dup {
                    entries.push(serde_json::json!({
                        "host_pattern": host,
                        "rule_type": "domain",
                        "enabled": true,
                        "source": "default",
                    }));
                }
            }
        }
    }
    let whitelist_json = serde_json::Value::Array(entries);
    // upsert（INSERT OR IGNORE 已迁过的会跳过，但本路径已判 need_seed，这里用 INSERT OR REPLACE
    // 确保空数组被覆盖为 seed）。幂等：再跑 need_seed=false 跳过。
    let _ = conn.execute(
        "INSERT INTO setting (scope, key, value, created_at, updated_at, deleted_at) \
         VALUES (?1, 'whitelist', ?2, ?3, ?3, 0) \
         ON CONFLICT(scope, key) DO UPDATE SET value = ?2, updated_at = ?3, deleted_at = 0",
        params![scope, whitelist_json.to_string(), now],
    );
}

/// 检查表是否存在（PRAGMA table_info 返 0 行 = 表不存在）。
fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.prepare(&format!("PRAGMA table_info({table})"))
        .and_then(|mut stmt| stmt.query_map([], |_| Ok(())).map(|i| i.count()))
        .map(|n| n > 0)
        .unwrap_or(false)
}


#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Helper: creates a minimal in-memory schema matching what run_migrations_late expects.
    /// Includes the tables referenced in the migration but with old/legacy schema
    /// (e.g., group without group_key, group with path).
    fn make_legacy_conn_with_group_path() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // Legacy group table WITH path column and WITHOUT group_key column.
        // Note: stats_agg_hourly is intentionally omitted — migration creates it via CREATE IF NOT EXISTS.
        conn.execute_batch(r#"
            CREATE TABLE "group" (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL DEFAULT '',
                path TEXT NOT NULL DEFAULT '',
                routing_mode TEXT NOT NULL DEFAULT '',
                auto_from_platform TEXT NOT NULL DEFAULT '',
                source_protocol TEXT NOT NULL DEFAULT 'anthropic',
                model_mappings TEXT NOT NULL DEFAULT '[]',
                request_timeout_secs INTEGER NOT NULL DEFAULT 0,
                connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                deleted_at INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 2
            );
            INSERT INTO "group" (name, path, created_at, updated_at) VALUES ('test-group', '/test', 0, 0);
            CREATE TABLE model_price (id INTEGER PRIMARY KEY, model TEXT, input_price REAL, output_price REAL);
            CREATE TABLE platform (id INTEGER PRIMARY KEY, name TEXT, platform_type TEXT NOT NULL DEFAULT '', endpoints TEXT NOT NULL DEFAULT '[]', extra TEXT NOT NULL DEFAULT '{}', auto_group INTEGER NOT NULL DEFAULT 1);
            CREATE TABLE proxy_log (id TEXT PRIMARY KEY, group_name TEXT, platform_id INTEGER, model TEXT, actual_model TEXT, source_protocol TEXT, target_protocol TEXT, status_code INTEGER, duration_ms INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_tokens INTEGER, est_cost REAL, is_stream INTEGER, retry_count INTEGER, blocked_by TEXT, blocked_reason TEXT, request_url TEXT, request_headers TEXT, request_body TEXT, upstream_request_url TEXT, upstream_request_headers TEXT, upstream_request_body TEXT, upstream_status_code INTEGER, upstream_response_headers TEXT, user_response_headers TEXT, user_response_body TEXT, response_body TEXT, created_at INTEGER, updated_at INTEGER, deleted_at INTEGER NOT NULL DEFAULT 0, attempts TEXT);
            CREATE TABLE IF NOT EXISTS settings (scope TEXT, key TEXT, value TEXT, PRIMARY KEY (scope, key));
            CREATE TABLE IF NOT EXISTS group_platform (id INTEGER PRIMARY KEY, group_id INTEGER, platform_id INTEGER, priority INTEGER, weight INTEGER);
            CREATE TABLE IF NOT EXISTS notification (id TEXT PRIMARY KEY, created_at INTEGER);
        "#).unwrap();
        conn
    }

    /// run_migrations_late on a legacy DB that has group.path → exercises has_group_path=true branch.
    #[test]
    fn migrations_late_group_path_migration_executed() {
        let conn = make_legacy_conn_with_group_path();
        // The legacy DB has group.path but no group.group_key.
        // run_migrations_late should:
        //   1. Detect has_group_path=true → rebuild group table (removes path, adds UNIQUE(name))
        //   2. Detect !has_group_key=true → rebuild group table again (adds group_key)
        let result = run_migrations_late(&conn);
        assert!(result.is_ok(), "run_migrations_late failed: {:?}", result);
        // After migration, group_key column should exist.
        let has_gk = conn
            .prepare("PRAGMA table_info(\"group\")")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|c| c == "group_key");
        assert!(has_gk, "group_key column should exist after migration");
        // path column should be gone.
        let has_path = conn
            .prepare("PRAGMA table_info(\"group\")")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|c| c == "path");
        assert!(!has_path, "path column should be removed after migration");
    }

    /// Helper: minimal "fully modern" schema — all conditional migrations skip (idempotent path).
    /// Uses modern table definitions with group_key, group_key in proxy_log, no breaker columns,
    /// and includes notification table.
    fn make_modern_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"
            CREATE TABLE "group" (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL DEFAULT '',
                group_key TEXT NOT NULL DEFAULT '',
                routing_mode TEXT NOT NULL DEFAULT '',
                auto_from_platform TEXT NOT NULL DEFAULT '',
                source_protocol TEXT NOT NULL DEFAULT 'anthropic',
                model_mappings TEXT NOT NULL DEFAULT '[]',
                request_timeout_secs INTEGER NOT NULL DEFAULT 0,
                connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                deleted_at INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 2,
                UNIQUE(name),
                UNIQUE(group_key)
            );
            CREATE TABLE model_price (id INTEGER PRIMARY KEY, model TEXT, input_price REAL, output_price REAL);
            CREATE TABLE platform (id INTEGER PRIMARY KEY, name TEXT, platform_type TEXT NOT NULL DEFAULT '', endpoints TEXT NOT NULL DEFAULT '[]', extra TEXT NOT NULL DEFAULT '{}');
            CREATE TABLE proxy_log (id TEXT PRIMARY KEY, group_key TEXT, platform_id INTEGER, model TEXT, actual_model TEXT, source_protocol TEXT, target_protocol TEXT, status_code INTEGER, duration_ms INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_tokens INTEGER, est_cost REAL, is_stream INTEGER, retry_count INTEGER, blocked_by TEXT, blocked_reason TEXT, request_url TEXT, request_headers TEXT, request_body TEXT, upstream_request_url TEXT, upstream_request_headers TEXT, upstream_request_body TEXT, upstream_status_code INTEGER, upstream_response_headers TEXT, user_response_headers TEXT, user_response_body TEXT, response_body TEXT, created_at INTEGER, updated_at INTEGER, deleted_at INTEGER NOT NULL DEFAULT 0, attempts TEXT);
            CREATE TABLE IF NOT EXISTS settings (scope TEXT, key TEXT, value TEXT, PRIMARY KEY (scope, key));
            CREATE TABLE IF NOT EXISTS group_platform (id INTEGER PRIMARY KEY, group_id INTEGER, platform_id INTEGER, priority INTEGER, weight INTEGER);
            CREATE TABLE IF NOT EXISTS notification (id TEXT PRIMARY KEY, created_at INTEGER);
        "#).unwrap();
        conn
    }

    /// run_migrations_late on a fully modern schema (all conditional branches skip) → idempotent.
    #[test]
    fn migrations_late_modern_schema_idempotent() {
        let conn = make_modern_conn();
        let result = run_migrations_late(&conn);
        assert!(result.is_ok(), "modern schema migration should succeed: {:?}", result);
    }

    /// Migration 044: group.extra 列。两条路径：
    /// ① 无 extra 列 → ALTER ADD 成功；② 已有 extra 列 → duplicate column 错误被 `let _` 忽略，幂等。
    #[test]
    fn migrations_late_group_extra_column_044() {
        let conn = make_modern_conn(); // 现代库但 group 无 extra 列
        // 预插一行 group 验证迁移不丢数据
        conn.execute(
            "INSERT INTO \"group\" (name, group_key, created_at, updated_at) VALUES ('g044', 'gk044', 0, 0)",
            [],
        )
        .unwrap();

        // ① 首次跑：ALTER ADD extra 列
        let r1 = run_migrations_late(&conn);
        assert!(r1.is_ok(), "first migration 044 should succeed: {:?}", r1);
        let has_extra = conn
            .prepare("PRAGMA table_info(\"group\")")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|c| c == "extra");
        assert!(has_extra, "extra column must exist after migration 044");
        // 行数据保留 + extra 默认 ''（空串 = "{}" 轻量表示）
        let extra: String = conn
            .query_row("SELECT extra FROM \"group\" WHERE name='g044'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(extra, "", "extra default should be empty string");

        // ② 再跑：duplicate column 错误被忽略，幂等（不返 Err，extra 列仍存在）
        let r2 = run_migrations_late(&conn);
        assert!(r2.is_ok(), "re-running migration 044 must be idempotent: {:?}", r2);
    }

    /// Migration 026: platform with breaker columns → backfill into extra + drop columns.
    /// Uses a platform row with non-zero breaker values to exercise the backfill branch.
    #[test]
    fn migrations_late_breaker_backfill_exercises_026() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"
            CREATE TABLE "group" (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL DEFAULT '',
                group_key TEXT NOT NULL DEFAULT '',
                routing_mode TEXT NOT NULL DEFAULT '',
                auto_from_platform TEXT NOT NULL DEFAULT '',
                source_protocol TEXT NOT NULL DEFAULT 'anthropic',
                model_mappings TEXT NOT NULL DEFAULT '[]',
                request_timeout_secs INTEGER NOT NULL DEFAULT 0,
                connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                deleted_at INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 2,
                UNIQUE(name), UNIQUE(group_key)
            );
            CREATE TABLE model_price (id INTEGER PRIMARY KEY, model TEXT, input_price REAL, output_price REAL);
            CREATE TABLE platform (
                id INTEGER PRIMARY KEY,
                name TEXT,
                platform_type TEXT NOT NULL DEFAULT '',
                endpoints TEXT NOT NULL DEFAULT '[]',
                extra TEXT NOT NULL DEFAULT '{}',
                breaker_failure_threshold INTEGER NOT NULL DEFAULT 0,
                breaker_open_secs INTEGER NOT NULL DEFAULT 0,
                breaker_half_open_max INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE proxy_log (id TEXT PRIMARY KEY, group_key TEXT, platform_id INTEGER, model TEXT, actual_model TEXT, source_protocol TEXT, target_protocol TEXT, status_code INTEGER, duration_ms INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_tokens INTEGER, est_cost REAL, is_stream INTEGER, retry_count INTEGER, blocked_by TEXT, blocked_reason TEXT, request_url TEXT, request_headers TEXT, request_body TEXT, upstream_request_url TEXT, upstream_request_headers TEXT, upstream_request_body TEXT, upstream_status_code INTEGER, upstream_response_headers TEXT, user_response_headers TEXT, user_response_body TEXT, response_body TEXT, created_at INTEGER, updated_at INTEGER, deleted_at INTEGER NOT NULL DEFAULT 0, attempts TEXT);
            CREATE TABLE IF NOT EXISTS settings (scope TEXT, key TEXT, value TEXT, PRIMARY KEY (scope, key));
            CREATE TABLE IF NOT EXISTS group_platform (id INTEGER PRIMARY KEY, group_id INTEGER, platform_id INTEGER, priority INTEGER, weight INTEGER);
            CREATE TABLE IF NOT EXISTS notification (id TEXT PRIMARY KEY, created_at INTEGER);
        "#).unwrap();
        // Insert a platform with non-zero breaker values to exercise the backfill path.
        conn.execute(
            "INSERT INTO platform (name, platform_type, endpoints, extra, breaker_failure_threshold, breaker_open_secs, breaker_half_open_max) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params!["test-plat", "openai", "[]", "{}", 5_i64, 60_i64, 2_i64],
        ).unwrap();
        // Also insert a platform with all-zero breaker values (exercises the skip branch).
        conn.execute(
            "INSERT INTO platform (name, platform_type, endpoints, extra, breaker_failure_threshold, breaker_open_secs, breaker_half_open_max) VALUES (?1, ?2, ?3, ?4, 0, 0, 0)",
            rusqlite::params!["zero-plat", "openai", "[]", "{}"],
        ).unwrap();
        let result = run_migrations_late(&conn);
        assert!(result.is_ok(), "breaker backfill migration should succeed: {:?}", result);
        // After migration, breaker_failure_threshold column should be gone.
        let has_breaker = conn
            .prepare("PRAGMA table_info(platform)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|c| c == "breaker_failure_threshold");
        assert!(!has_breaker, "breaker_failure_threshold should be dropped after migration 026");
        // The non-zero platform's extra should now contain breaker data.
        let extra: String = conn
            .query_row("SELECT extra FROM platform WHERE name = 'test-plat'", [], |r| r.get(0))
            .unwrap();
        assert!(extra.contains("breaker") || extra.contains("failure_threshold"),
            "extra should contain breaker data after backfill, got: {}", extra);
    }

    /// Migration 025: GLM platform with coding openai endpoint + anthropic endpoint not tagged coding_plan
    /// → should set anthropic endpoint's coding_plan=true.
    #[test]
    fn migrations_late_glm_coding_plan_backfill_025() {
        let conn = Connection::open_in_memory().unwrap();
        // GLM platform endpoints: openai with coding_plan=true + anthropic with coding_plan=false.
        let endpoints_json = serde_json::json!([
            {
                "protocol": "openai",
                "base_url": "",
                "coding_plan": true
            },
            {
                "protocol": "anthropic",
                "base_url": "",
                "coding_plan": false
            }
        ]).to_string();
        conn.execute_batch(r#"
            CREATE TABLE "group" (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL DEFAULT '',
                group_key TEXT NOT NULL DEFAULT '',
                routing_mode TEXT NOT NULL DEFAULT '',
                auto_from_platform TEXT NOT NULL DEFAULT '',
                source_protocol TEXT NOT NULL DEFAULT 'anthropic',
                model_mappings TEXT NOT NULL DEFAULT '[]',
                request_timeout_secs INTEGER NOT NULL DEFAULT 0,
                connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                deleted_at INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 2,
                UNIQUE(name), UNIQUE(group_key)
            );
            CREATE TABLE model_price (id INTEGER PRIMARY KEY, model TEXT, input_price REAL, output_price REAL);
            CREATE TABLE platform (id INTEGER PRIMARY KEY, name TEXT, platform_type TEXT NOT NULL DEFAULT '', endpoints TEXT NOT NULL DEFAULT '[]', extra TEXT NOT NULL DEFAULT '{}');
            CREATE TABLE proxy_log (id TEXT PRIMARY KEY, group_key TEXT, platform_id INTEGER, model TEXT, actual_model TEXT, source_protocol TEXT, target_protocol TEXT, status_code INTEGER, duration_ms INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_tokens INTEGER, est_cost REAL, is_stream INTEGER, retry_count INTEGER, blocked_by TEXT, blocked_reason TEXT, request_url TEXT, request_headers TEXT, request_body TEXT, upstream_request_url TEXT, upstream_request_headers TEXT, upstream_request_body TEXT, upstream_status_code INTEGER, upstream_response_headers TEXT, user_response_headers TEXT, user_response_body TEXT, response_body TEXT, created_at INTEGER, updated_at INTEGER, deleted_at INTEGER NOT NULL DEFAULT 0, attempts TEXT);
            CREATE TABLE IF NOT EXISTS settings (scope TEXT, key TEXT, value TEXT, PRIMARY KEY (scope, key));
            CREATE TABLE IF NOT EXISTS group_platform (id INTEGER PRIMARY KEY, group_id INTEGER, platform_id INTEGER, priority INTEGER, weight INTEGER);
            CREATE TABLE IF NOT EXISTS notification (id TEXT PRIMARY KEY, created_at INTEGER);
        "#).unwrap();
        conn.execute(
            "INSERT INTO platform (name, platform_type, endpoints, extra) VALUES (?1, 'glm', ?2, '{}')",
            rusqlite::params!["GLM Test", endpoints_json],
        ).unwrap();
        let result = run_migrations_late(&conn);
        assert!(result.is_ok(), "GLM coding_plan migration should succeed: {:?}", result);
        // After migration, anthropic endpoint should have coding_plan=true.
        let ep_json: String = conn
            .query_row("SELECT endpoints FROM platform WHERE name = 'GLM Test'", [], |r| r.get(0))
            .unwrap();
        let eps: Vec<serde_json::Value> = serde_json::from_str(&ep_json).unwrap();
        let anthropic_ep = eps.iter().find(|ep| ep.get("protocol").and_then(|v| v.as_str()) == Some("anthropic")).unwrap();
        assert_eq!(
            anthropic_ep.get("coding_plan").and_then(|v| v.as_bool()),
            Some(true),
            "anthropic endpoint should have coding_plan=true after migration 025"
        );
    }

    /// run_migrations_late on a DB without group.path but also without group_key → exercises !has_group_key branch.
    #[test]
    fn migrations_late_missing_group_key_migration_executed() {
        let conn = Connection::open_in_memory().unwrap();
        // Group table without path AND without group_key.
        conn.execute_batch(r#"
            CREATE TABLE "group" (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL DEFAULT '',
                routing_mode TEXT NOT NULL DEFAULT '',
                auto_from_platform TEXT NOT NULL DEFAULT '',
                source_protocol TEXT NOT NULL DEFAULT 'anthropic',
                model_mappings TEXT NOT NULL DEFAULT '[]',
                request_timeout_secs INTEGER NOT NULL DEFAULT 0,
                connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                deleted_at INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 2,
                UNIQUE(name)
            );
            INSERT INTO "group" (name, created_at, updated_at) VALUES ('my-group', 0, 0);
            CREATE TABLE model_price (id INTEGER PRIMARY KEY, model TEXT, input_price REAL, output_price REAL);
            CREATE TABLE platform (id INTEGER PRIMARY KEY, name TEXT, platform_type TEXT NOT NULL DEFAULT '', endpoints TEXT NOT NULL DEFAULT '[]', extra TEXT NOT NULL DEFAULT '{}', auto_group INTEGER NOT NULL DEFAULT 1);
            CREATE TABLE proxy_log (id TEXT PRIMARY KEY, group_name TEXT, platform_id INTEGER, model TEXT, actual_model TEXT, source_protocol TEXT, target_protocol TEXT, status_code INTEGER, duration_ms INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_tokens INTEGER, est_cost REAL, is_stream INTEGER, retry_count INTEGER, blocked_by TEXT, blocked_reason TEXT, request_url TEXT, request_headers TEXT, request_body TEXT, upstream_request_url TEXT, upstream_request_headers TEXT, upstream_request_body TEXT, upstream_status_code INTEGER, upstream_response_headers TEXT, user_response_headers TEXT, user_response_body TEXT, response_body TEXT, created_at INTEGER, updated_at INTEGER, deleted_at INTEGER NOT NULL DEFAULT 0, attempts TEXT);
            CREATE TABLE IF NOT EXISTS settings (scope TEXT, key TEXT, value TEXT, PRIMARY KEY (scope, key));
            CREATE TABLE IF NOT EXISTS group_platform (id INTEGER PRIMARY KEY, group_id INTEGER, platform_id INTEGER, priority INTEGER, weight INTEGER);
            CREATE TABLE IF NOT EXISTS notification (id TEXT PRIMARY KEY, created_at INTEGER);
        "#).unwrap();
        let result = run_migrations_late(&conn);
        assert!(result.is_ok(), "run_migrations_late failed: {:?}", result);
        let has_gk = conn
            .prepare("PRAGMA table_info(\"group\")")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|c| c == "group_key");
        assert!(has_gk, "group_key should exist after migration");
    }

    /// Migration 039: 历史 last_error 残留完整 JSON body → 重提为 message。幂等。
    #[test]
    fn migrations_late_reextract_last_error_039() {
        let conn = Connection::open_in_memory().unwrap();
        // 建带 last_error 列的 platform（已过 037），插 3 类典型行：
        //  - stale JSON body（应被重提为 message）
        //  - 纯文本限流（非 JSON，保留）
        //  - 已提取 message（已是字符串非 JSON，保留，验证幂等）
        conn.execute_batch(r#"
            CREATE TABLE "group" (
                id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL DEFAULT '',
                group_key TEXT NOT NULL DEFAULT '', routing_mode TEXT NOT NULL DEFAULT '',
                auto_from_platform TEXT NOT NULL DEFAULT '', source_protocol TEXT NOT NULL DEFAULT 'anthropic',
                model_mappings TEXT NOT NULL DEFAULT '[]', request_timeout_secs INTEGER NOT NULL DEFAULT 0,
                connect_timeout_secs INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0, deleted_at INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0, max_retries INTEGER NOT NULL DEFAULT 2,
                UNIQUE(name), UNIQUE(group_key)
            );
            CREATE TABLE model_price (id INTEGER PRIMARY KEY, model TEXT, input_price REAL, output_price REAL);
            CREATE TABLE platform (
                id INTEGER PRIMARY KEY, name TEXT, platform_type TEXT NOT NULL DEFAULT '',
                endpoints TEXT NOT NULL DEFAULT '[]', extra TEXT NOT NULL DEFAULT '{}',
                last_error TEXT NOT NULL DEFAULT '', last_error_at INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE proxy_log (id TEXT PRIMARY KEY, group_key TEXT, platform_id INTEGER, model TEXT, actual_model TEXT, source_protocol TEXT, target_protocol TEXT, status_code INTEGER, duration_ms INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_tokens INTEGER, est_cost REAL, is_stream INTEGER, retry_count INTEGER, blocked_by TEXT, blocked_reason TEXT, request_url TEXT, request_headers TEXT, request_body TEXT, upstream_request_url TEXT, upstream_request_headers TEXT, upstream_request_body TEXT, upstream_status_code INTEGER, upstream_response_headers TEXT, user_response_headers TEXT, user_response_body TEXT, response_body TEXT, created_at INTEGER, updated_at INTEGER, deleted_at INTEGER NOT NULL DEFAULT 0, attempts TEXT);
            CREATE TABLE IF NOT EXISTS settings (scope TEXT, key TEXT, value TEXT, PRIMARY KEY (scope, key));
            CREATE TABLE IF NOT EXISTS group_platform (id INTEGER PRIMARY KEY, group_id INTEGER, platform_id INTEGER, priority INTEGER, weight INTEGER);
            CREATE TABLE IF NOT EXISTS notification (id TEXT PRIMARY KEY, created_at INTEGER);
        "#).unwrap();
        // stale: 完整 JSON body（afcd6fb 旧路径写入）
        let stale = r#"HTTP 429: {"error":{"message":"余额不足或无可用资源包,请充值。","type":"upstream_error","param":"","code":"1113"}}"#;
        // plain: 纯文本限流（非 JSON，保留）
        let plain = "HTTP 429: Too many requests";
        // already: 已提取的 message 字符串（再跑幂等，不变）
        let already = "HTTP 429: quota exhausted";
        // stale_toplevel: 顶层 message（非嵌套 error.message）—— 另一种命中分支
        let stale_toplevel = r#"HTTP 401: {"message":"身份验证失败。","type":"1000"}"#;
        conn.execute(
            "INSERT INTO platform (name, last_error) VALUES ('stale', ?1), ('plain', ?2), ('already', ?3), ('toplevel', ?4)",
            rusqlite::params![stale, plain, already, stale_toplevel],
        ).unwrap();

        let result = run_migrations_late(&conn);
        assert!(result.is_ok(), "run_migrations_late failed: {:?}", result);

        let get_last_error = |name: &str| -> String {
            conn.query_row("SELECT last_error FROM platform WHERE name = ?1", [name], |r| r.get(0)).unwrap()
        };
        assert_eq!(get_last_error("stale"), "HTTP 429: 余额不足或无可用资源包,请充值。");
        assert_eq!(get_last_error("plain"), "HTTP 429: Too many requests");
        assert_eq!(get_last_error("already"), "HTTP 429: quota exhausted");
        assert_eq!(get_last_error("toplevel"), "HTTP 401: 身份验证失败。");

        // 幂等：再跑一次所有行不变。
        let _ = run_migrations_late(&conn);
        assert_eq!(get_last_error("stale"), "HTTP 429: 余额不足或无可用资源包,请充值。");
        assert_eq!(get_last_error("plain"), "HTTP 429: Too many requests");
    }

    /// Migration 040–043: MITM 两表迁 setting + 默认白名单 seed。
    /// 验证（新库路径）：① 两表不建；② setting (mitm, whitelist) 含 37 条默认 + 平台 host；
    /// ③ setting (mitm, ca) 无行（首次启用时 ensure_root_ca 写入）；④ 幂等。
    #[test]
    fn migrations_late_mitm_seed_to_setting_043() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"
            CREATE TABLE "group" (
                id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL DEFAULT '',
                group_key TEXT NOT NULL DEFAULT '', routing_mode TEXT NOT NULL DEFAULT '',
                auto_from_platform TEXT NOT NULL DEFAULT '', source_protocol TEXT NOT NULL DEFAULT 'anthropic',
                model_mappings TEXT NOT NULL DEFAULT '[]', request_timeout_secs INTEGER NOT NULL DEFAULT 0,
                connect_timeout_secs INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0, deleted_at INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0, max_retries INTEGER NOT NULL DEFAULT 2,
                UNIQUE(name), UNIQUE(group_key)
            );
            CREATE TABLE model_price (id INTEGER PRIMARY KEY, model TEXT, input_price REAL, output_price REAL);
            CREATE TABLE platform (
                id INTEGER PRIMARY KEY, name TEXT, platform_type TEXT NOT NULL DEFAULT '',
                base_url TEXT NOT NULL DEFAULT '', endpoints TEXT NOT NULL DEFAULT '[]',
                extra TEXT NOT NULL DEFAULT '{}', last_error TEXT NOT NULL DEFAULT '',
                last_error_at INTEGER NOT NULL DEFAULT 0, env_vars TEXT NOT NULL DEFAULT '[]',
                expires_at INTEGER NOT NULL DEFAULT 0, deleted_at INTEGER NOT NULL DEFAULT 0
            );
            -- 插一个已配平台，验证 base_url host 被提取进默认白名单
            INSERT INTO platform (name, platform_type, base_url) VALUES ('test-anthropic', 'anthropic', 'https://api.anthropic.com/v1');
            CREATE TABLE proxy_log (id TEXT PRIMARY KEY, group_key TEXT, platform_id INTEGER, model TEXT, actual_model TEXT, source_protocol TEXT, target_protocol TEXT, status_code INTEGER, duration_ms INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_tokens INTEGER, est_cost REAL, is_stream INTEGER, retry_count INTEGER, blocked_by TEXT, blocked_reason TEXT, request_url TEXT, request_headers TEXT, request_body TEXT, upstream_request_url TEXT, upstream_request_headers TEXT, upstream_request_body TEXT, upstream_status_code INTEGER, upstream_response_headers TEXT, user_response_headers TEXT, user_response_body TEXT, response_body TEXT, created_at INTEGER, updated_at INTEGER, deleted_at INTEGER NOT NULL DEFAULT 0, attempts TEXT);
            CREATE TABLE IF NOT EXISTS setting (id INTEGER PRIMARY KEY AUTOINCREMENT, scope TEXT NOT NULL DEFAULT '', key TEXT NOT NULL DEFAULT '', value TEXT NOT NULL DEFAULT '{}', created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0, deleted_at INTEGER NOT NULL DEFAULT 0, UNIQUE(scope, key));
            CREATE TABLE IF NOT EXISTS group_platform (id INTEGER PRIMARY KEY, group_id INTEGER, platform_id INTEGER, priority INTEGER, weight INTEGER, level_priority INTEGER NOT NULL DEFAULT 5);
            CREATE TABLE IF NOT EXISTS notification (id TEXT PRIMARY KEY, created_at INTEGER);
        "#).unwrap();

        let result = run_migrations_late(&conn);
        assert!(result.is_ok(), "run_migrations_late failed: {:?}", result);

        // ① 两表不再建（DROP / 新库从不建）
        let has_mitm_ca = table_exists(&conn, "mitm_ca");
        assert!(!has_mitm_ca, "mitm_ca table must NOT exist (migrated to setting)");
        let has_mitm_whitelist = table_exists(&conn, "mitm_whitelist");
        assert!(!has_mitm_whitelist, "mitm_whitelist table must NOT exist (migrated to setting)");

        // ② setting (mitm, whitelist) 含 37 条默认 + 已配平台 host（api.anthropic.com）
        let whitelist_json: String = conn
            .query_row(
                "SELECT value FROM setting WHERE scope='mitm' AND key='whitelist' AND deleted_at=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let entries: serde_json::Value = serde_json::from_str(&whitelist_json).unwrap();
        let arr = entries.as_array().expect("whitelist value must be array");
        assert!(arr.len() >= 37, "default whitelist should contain 37 Clash ruleset entries, got {}", arr.len());
        // 已配平台 host（api.anthropic.com，domain）
        let has_platform_host = arr.iter().any(|e| {
            e.get("host_pattern").and_then(|v| v.as_str()) == Some("api.anthropic.com")
                && e.get("rule_type").and_then(|v| v.as_str()) == Some("domain")
        });
        assert!(has_platform_host, "platform base_url host 'api.anthropic.com' (domain) should be seeded");

        // ③ setting (mitm, ca) 无行（首次启用时 ensure_root_ca 写入）
        let ca_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM setting WHERE scope='mitm' AND key='ca' AND deleted_at=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ca_count, 0, "mitm:ca should not exist until ensure_root_ca");

        // ④ 幂等：再跑一次，whitelist 行数不变
        let _ = run_migrations_late(&conn);
        let whitelist_json2: String = conn
            .query_row(
                "SELECT value FROM setting WHERE scope='mitm' AND key='whitelist' AND deleted_at=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let entries2: serde_json::Value = serde_json::from_str(&whitelist_json2).unwrap();
        assert_eq!(entries2.as_array().unwrap().len(), arr.len(), "re-running migration should not duplicate whitelist entries");

        // ⑤ 4 类型各有代表（domain/suffix/keyword/ipcidr）
        for (rule_type, expected) in [
            ("domain", "cdn.usefathom.com"),
            ("suffix", "openai.com"),
            ("keyword", "openai"),
            ("ipcidr", "24.199.123.28/32"),
        ] {
            let has = arr.iter().any(|e| {
                e.get("rule_type").and_then(|v| v.as_str()) == Some(rule_type)
                    && e.get("host_pattern").and_then(|v| v.as_str()) == Some(expected)
            });
            assert!(has, "default whitelist should contain rule_type={rule_type} host_pattern={expected}");
        }
    }

    /// Migration 043 验收（旧库迁移路径）：旧 mitm_ca + mitm_whitelist 行 →
    /// setting JSON + 两表 DROP。数据不丢，旧 schema 退出。
    #[test]
    fn migrations_late_043_legacy_tables_to_setting() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"
            CREATE TABLE "group" (
                id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL DEFAULT '',
                group_key TEXT NOT NULL DEFAULT '', routing_mode TEXT NOT NULL DEFAULT '',
                auto_from_platform TEXT NOT NULL DEFAULT '', source_protocol TEXT NOT NULL DEFAULT 'anthropic',
                model_mappings TEXT NOT NULL DEFAULT '[]', request_timeout_secs INTEGER NOT NULL DEFAULT 0,
                connect_timeout_secs INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0, deleted_at INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0, max_retries INTEGER NOT NULL DEFAULT 2,
                UNIQUE(name), UNIQUE(group_key)
            );
            CREATE TABLE model_price (id INTEGER PRIMARY KEY, model TEXT, input_price REAL, output_price REAL);
            CREATE TABLE platform (
                id INTEGER PRIMARY KEY, name TEXT, platform_type TEXT NOT NULL DEFAULT '',
                base_url TEXT NOT NULL DEFAULT '', endpoints TEXT NOT NULL DEFAULT '[]',
                extra TEXT NOT NULL DEFAULT '{}', last_error TEXT NOT NULL DEFAULT '',
                last_error_at INTEGER NOT NULL DEFAULT 0, env_vars TEXT NOT NULL DEFAULT '[]',
                expires_at INTEGER NOT NULL DEFAULT 0, deleted_at INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE proxy_log (id TEXT PRIMARY KEY, group_key TEXT, platform_id INTEGER, model TEXT, actual_model TEXT, source_protocol TEXT, target_protocol TEXT, status_code INTEGER, duration_ms INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_tokens INTEGER, est_cost REAL, is_stream INTEGER, retry_count INTEGER, blocked_by TEXT, blocked_reason TEXT, request_url TEXT, request_headers TEXT, request_body TEXT, upstream_request_url TEXT, upstream_request_headers TEXT, upstream_request_body TEXT, upstream_status_code INTEGER, upstream_response_headers TEXT, user_response_headers TEXT, user_response_body TEXT, response_body TEXT, created_at INTEGER, updated_at INTEGER, deleted_at INTEGER NOT NULL DEFAULT 0, attempts TEXT);
            CREATE TABLE IF NOT EXISTS setting (id INTEGER PRIMARY KEY AUTOINCREMENT, scope TEXT NOT NULL DEFAULT '', key TEXT NOT NULL DEFAULT '', value TEXT NOT NULL DEFAULT '{}', created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0, deleted_at INTEGER NOT NULL DEFAULT 0, UNIQUE(scope, key));
            CREATE TABLE IF NOT EXISTS group_platform (id INTEGER PRIMARY KEY, group_id INTEGER, platform_id INTEGER, priority INTEGER, weight INTEGER, level_priority INTEGER NOT NULL DEFAULT 5);
            CREATE TABLE IF NOT EXISTS notification (id TEXT PRIMARY KEY, created_at INTEGER);
            -- 旧 mitm_ca 表（含已装 CA 行）
            CREATE TABLE mitm_ca (
                id INTEGER PRIMARY KEY,
                private_key_pem TEXT NOT NULL,
                cert_pem TEXT NOT NULL,
                fingerprint TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER NOT NULL DEFAULT 0,
                ca_installed INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO mitm_ca (id, private_key_pem, cert_pem, fingerprint, created_at, enabled, ca_installed)
                VALUES (1, 'TEST_PRIV_KEY', 'TEST_CERT_PEM', 'AB:CD', 12345, 1, 1);
            -- 旧 mitm_whitelist 表（含 rule_type 列 + 3 条数据）
            CREATE TABLE mitm_whitelist (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                host_pattern TEXT NOT NULL,
                rule_type TEXT NOT NULL DEFAULT 'suffix',
                enabled INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL DEFAULT 'user',
                created_at INTEGER NOT NULL DEFAULT 0,
                UNIQUE(host_pattern)
            );
            INSERT INTO mitm_whitelist (host_pattern, rule_type, enabled, source, created_at) VALUES
                ('anthropic.com', 'suffix', 1, 'default', 100),
                ('api.openai.com', 'domain', 1, 'default', 101),
                ('my-custom.example.com', 'suffix', 0, 'user', 102);
        "#).unwrap();

        let result = run_migrations_late(&conn);
        assert!(result.is_ok(), "run_migrations_late failed: {:?}", result);

        // ① 两表已 DROP
        assert!(!table_exists(&conn, "mitm_ca"), "mitm_ca must be DROPped after migration");
        assert!(!table_exists(&conn, "mitm_whitelist"), "mitm_whitelist must be DROPped after migration");

        // ② setting (mitm, ca) 含旧 CA 数据
        let ca_json: String = conn
            .query_row(
                "SELECT value FROM setting WHERE scope='mitm' AND key='ca' AND deleted_at=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let ca: serde_json::Value = serde_json::from_str(&ca_json).unwrap();
        assert_eq!(ca.get("private_key_pem").and_then(|v| v.as_str()), Some("TEST_PRIV_KEY"));
        assert_eq!(ca.get("cert_pem").and_then(|v| v.as_str()), Some("TEST_CERT_PEM"));
        assert_eq!(ca.get("fingerprint").and_then(|v| v.as_str()), Some("AB:CD"));
        assert_eq!(ca.get("created_at").and_then(|v| v.as_i64()), Some(12345));
        assert_eq!(ca.get("enabled").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(ca.get("ca_installed").and_then(|v| v.as_bool()), Some(true));

        // ③ setting (mitm, whitelist) 含旧白名单数组（非空，seed 跳过）
        let wl_json: String = conn
            .query_row(
                "SELECT value FROM setting WHERE scope='mitm' AND key='whitelist' AND deleted_at=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let wl: serde_json::Value = serde_json::from_str(&wl_json).unwrap();
        let arr = wl.as_array().unwrap();
        // 旧表 3 条（非空 → seed 不触发，不补 37 条默认）
        assert_eq!(arr.len(), 3, "legacy whitelist (non-empty) should migrate as-is, seed skipped");
        // 验 created_at 升序：第一条 anthropic.com（created_at=100），第二条 api.openai.com（101）
        assert_eq!(arr[0].get("host_pattern").and_then(|v| v.as_str()), Some("anthropic.com"));
        assert_eq!(arr[1].get("host_pattern").and_then(|v| v.as_str()), Some("api.openai.com"));
        // 验 rule_type / enabled / source 字段迁移正确
        assert_eq!(arr[0].get("rule_type").and_then(|v| v.as_str()), Some("suffix"));
        assert_eq!(arr[2].get("enabled").and_then(|v| v.as_bool()), Some(false)); // disabled 用户条目
        assert_eq!(arr[2].get("source").and_then(|v| v.as_str()), Some("user"));

        // ④ 幂等：再跑一次，两表仍不存在，setting 数据不变
        let _ = run_migrations_late(&conn);
        let ca_count2: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM setting WHERE scope='mitm' AND key='ca' AND deleted_at=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ca_count2, 1, "re-running migration must not duplicate mitm:ca");
        let wl_json2: String = conn
            .query_row(
                "SELECT value FROM setting WHERE scope='mitm' AND key='whitelist' AND deleted_at=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let wl2: serde_json::Value = serde_json::from_str(&wl_json2).unwrap();
        assert_eq!(wl2.as_array().unwrap().len(), 3, "re-running migration must not change whitelist");
    }
}