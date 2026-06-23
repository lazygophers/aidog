use super::*;
use rusqlite::{params, Connection, Result as SqlResult};

/// Migrations 021–033。自 init_tables 拆出（执行顺序不变）。
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
                // Migration 028: platform / group 维度用量聚合偏索引（平台列表页 usage 批量化 N+1 消除）。
                // platform_usage_stats_all 的 eff_pid 子查询按 platform_id 分组 + group_key 回溯，
                // get_all_group_usage_stats 按 group_key 分组；现有 idx_proxy_log_stats 前导列是
                // created_at（不覆盖 platform_id/group_key 的等值/分组扫描）。带 WHERE deleted_at=0
                // 偏索引缩范围、减写放大与磁盘占用。幂等（IF NOT EXISTS），新装/老库均建。
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

                // Migration 031: 前瞻覆盖索引 + notification 时间索引。
                //
                // ① idx_proxy_log_group_key_stats：覆盖 get_all_group_usage_stats（GROUP BY group_key
                //    + SUM input/output/cache_tokens + SUM est_cost + status_code 成功率）。现有
                //    idx_proxy_log_group_key 只含 group_key，SUM/status_code 须回表；本覆盖索引把所有
                //    被聚合列纳入 → index-only scan 免回表。列序：分组键前导 + 各 SUM/谓词列。
                //    带 WHERE deleted_at=0 偏索引，与查询谓词对齐、缩范围减写放大。
                //    当前 13680 行收益有限，proxy_log 增长到数十万行后显著（前瞻）。幂等 IF NOT EXISTS。
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_group_key_stats \
                     ON proxy_log(group_key, est_cost, input_tokens, output_tokens, cache_tokens, status_code) \
                     WHERE deleted_at = 0",
                    [],
                );
                // ② idx_notification_created：notification 表原无二级索引，收件箱 ORDER BY created_at DESC
                //    LIMIT + retention DELETE WHERE created_at< 现走全表扫。低成本、前瞻。幂等。
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_notification_created ON notification(created_at)",
                    [],
                );
                // Migration 032: 小时级聚合统计表 stats_agg_hourly（建表 + 索引 + 存量一次性回填）。
                // 统计读取改查预聚合表，写入解耦于日志开关（关日志也写聚合）。回填幂等
                // （NOT EXISTS 空表守卫），SQL 内联于本模块顶 STATS_AGG_HOURLY_SQL。
                conn.execute_batch(STATS_AGG_HOURLY_SQL)?;
                // Migration 033: 删除无意义的 proxy_log.is_final 列（旧版本曾 ALTER 加过）。
                // bundled sqlite 支持 DROP COLUMN；列不存在则报错忽略（新库本就无此列）。
                let _ = conn.execute("ALTER TABLE proxy_log DROP COLUMN is_final", []);
                // Migration 034: proxy_log 索引精简 + 复合化 + ANALYZE 统计。
                //
                // ① 删 2 个完全冗余索引：
                //    idx_proxy_log_group(group_name 旧列，已 RENAME 为 group_key) 与
                //    idx_proxy_log_platform(platform_id) 分别与 idx_proxy_log_group_key /
                //    idx_proxy_log_platform_id 同列同 WHERE 条件，纯重复占写放大与磁盘。
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_group", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_platform", []);
                // ② 建 3 个 (等值列, created_at) 复合偏索引。Logs 页所有 filter 均带
                //    ORDER BY created_at DESC：纯单列等值索引命中后仍需 TEMP B-TREE 排序
                //    （EXPLAIN 实测）。复合索引把 created_at 纳入第二列 → 索引天然有序，
                //    消除 TEMP B-TREE；第一列等值仍覆盖原单列 COUNT/最近N次子查询用途
                //    （usage_stats.rs 最近5次 / 最近测试 ORDER BY created_at DESC LIMIT）。
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
                // ③ 删被复合索引取代的纯单列索引。EXPLAIN 实测：删后等值/COUNT 查询走
                //    复合索引第一列（COUNT 仍 COVERING INDEX，不退化全表扫），filter+ORDER BY
                //    无 TEMP B-TREE。保留 *_stats / idx_proxy_log_created / model 类索引（用途不同）。
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_status", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_platform_id", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_group_key", []);
                // ④ ANALYZE 建/重建 sqlite_stat1（真实库从未 ANALYZE，规划器靠默认估算）。
                //    给规划器真实选择度，避免错选索引。compact/VACUUM 后统计失效，由
                //    maintenance.rs 维护钩子重建（见 cleanup_proxy_logs / compact_database 后追加）。
                let _ = conn.execute("ANALYZE proxy_log", []);
    Ok(())
}
