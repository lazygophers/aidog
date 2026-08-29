use std::collections::HashMap;
use crate::helpers::*;
use crate::{now, load_auto_from_map, STATS_AGG_HOURLY_SQL};
use crate::models::*;
use rusqlite::{params, Connection, Result as SqlResult};

/// Migrations 20260727-12..18（原 021–052, 日期戳脱锚）。
/// 自 init_tables 拆出。编号格式 `YYYYMMDD-NN`：日期戳批次 + 库内序号（main 库独立空间）。
pub fn run_migrations_late(
    conn: &Connection,
    backfill: crate::schema::BackfillFn,
) -> SqlResult<()> {
                // Migration 20260727-12 (原 021): model_price 加模型信息列（max_tokens / context_window）。
                // 列为索引快速读取（出站裁剪、列表展示）；price_data JSON 仍存完整原始数据。
                // NULL = 未知/无限制。源自旧 008_model_info_columns（已内联为下方 ALTER）。
                let _ = conn.execute("ALTER TABLE model_price ADD COLUMN max_input_tokens INTEGER", []);
                let _ = conn.execute("ALTER TABLE model_price ADD COLUMN max_output_tokens INTEGER", []);
                let _ = conn.execute("ALTER TABLE model_price ADD COLUMN context_window INTEGER", []);
                // Migration 原编号 022–048 (platform.db 内重编为 20260727-02..16): platform / "group" / group_platform / cli_proxy_provider
                // 的 ALTER / 数据回填 / 045 建表 / 046 CPA 清理 / 048 quota →
                // run_migrations_platform_late（落 platform.db）。主库零 platform/group DDL。
                // Migration 20260727-13 (原 030): 「Claude Code / Codex 联动」重命名为通用「AI 编程工具」。
                // 把旧 settings key cc_codex_settings 迁到 coding_tools_settings，保留老用户两开关状态
                // （apply_to_claude_plugin / skip_claude_onboarding），避免重命名后开关回到默认关。
                // 幂等：仅当存在旧 key 时 UPDATE 改名；新库无旧 key 时空操作。
                let _ = conn.execute(
                    "UPDATE settings SET key='coding_tools_settings' WHERE scope='global' AND key='cc_codex_settings'",
                    [],
                );

                // Migration 原编号 031 ① (log.db 内重编为 20260727-12): idx_proxy_log_group_key_stats → run_migrations_proxy_log_late
                // Migration 原编号 031 ② (log.db 内重编为 20260727-19): notification 时间索引 → run_migrations_proxy_log_late（log.db）
                // Migration 原编号 032: stats_agg_hourly 建表 + 回填 → 已迁回本函数 Migration 20260727-16（落主库）
                // Migration 原编号 033 (log.db 内重编为 20260727-14): proxy_log.is_final DROP → run_migrations_proxy_log_late
                // Migration 原编号 034 (log.db 内重编为 20260727-15): proxy_log 索引精简 → run_migrations_proxy_log_late
                // Migration 20260727-14 (原 035): 删冗余索引（proxy_log/stats_agg 相关 → proxy_log_late；idx_model_price_name 留主库）。
                let _ = conn.execute("DROP INDEX IF EXISTS idx_model_price_name", []);
                tracing::info!("migration 20260727-14 (原 035): dropped redundant indexes (proxy_log/stats_agg部分在proxy_log_late)");

                // Migration 原编号 040–042 已移除：旧 mitm_ca / mitm_whitelist 两表数据迁 setting（scope=mitm）
                // + DROP 两表。新库不再建两表，MITM 配置复用 setting 的 get_setting/set_setting + 缓存机制。
                // 详见 migration 20260727-15（原 043）（migrate_mitm_legacy_tables_to_setting）。
                // 注意：migrate_mitm_legacy_tables_to_setting 内部 SELECT platform.base_url 提取 host
                // 入默认白名单 —— config-db-split 后 platform 表在 platform.db（主库无此表），
                // SELECT 失败被 `if let Ok` 吞，entries 仅含 37 条 Clash 默认规则，无平台 host。
                // 仅影响首启新装（无 platform 数据可提取）；老库首迁 Phase 1 时主库仍有 platform 表，host 正常。
                migrate_mitm_legacy_tables_to_setting(conn);

                // Migration 20260727-16 (原 051): stats_agg_hourly DDL 迁回主库（原落 log.db，
                // 拆库后 retention/VACUUM 误伤 + backup 归属错位 + 语义归属主库）。
                // CREATE IF NOT EXISTS 幂等：pre-split 老装用户主库已有 stats_agg_hourly
                // （原 Mig 050 不再 DROP 此表）→ 沿用；fresh install 空表。
                //
                // 回填前置：主库需有 legacy proxy_log（pre-split 升级路径，原 Mig 050 DROP 之前的
                // 存量行）。fresh install 主库从未有 proxy_log（Phase 2 log.db 才建）→ 跳过回填
                // （无存量可回填，stats_agg 留空正确）。post-split 升级（旧 Mig 050 已 DROP 主库
                // 两表）：主库空 stats_agg + 无 proxy_log → CREATE 空表 + 跳过；log.db 残留
                // stats_agg 数据由 s2 跨库搬迁恢复。
                //
                // 排序约束：必须先于 DROP legacy proxy_log 的 20260727-18（原 050）执行（050 DROP legacy proxy_log 后回填无源）。
                // 迁移编号 20260727-16 仍为 20260727-18 的前一序号（日期戳批次内源序 vs 编号分离）。
                conn.execute_batch(STATS_AGG_HOURLY_SQL)?;
                let has_legacy_proxy_log = conn
                    .prepare("PRAGMA table_info(proxy_log)")?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .next()
                    .is_some();
                if has_legacy_proxy_log {
                    let auto_map = load_auto_from_map(conn)?;
                    backfill(conn, &auto_map)?;
                }

                // Migration 20260727-17 (原 052): log.db 残留 stats_agg_hourly 跨库搬迁到主库。
                // post-split 升级路径：旧版 stats_agg_hourly 落 log.db（原 032），s1 把 DDL 迁回
                // 主库 20260727-16 后，log.db 残留存量数据需搬回主库统一查询 / retention / backup。
                //
                // 幂等三守卫（全满足才执行搬迁）：
                //  ① 主库 stats_agg_hourly 已有数据 → 已搬过 / 20260727-16 已回填，跳过
                //  ② 内存库（PRAGMA database_list main file 为空 / :memory: / mode=memory）→
                //    log.db 无独立文件（同内存连接），跳过
                //  ③ log.db 无 stats_agg_hourly 表 → 新装 / 已迁 / 表已删，跳过
                //
                // 失败不阻断启动（log::warn! + 继续）：数据可后续 rebuild（stats 重算路径）。
                // 搬迁后不 DROP log.db 旧表（YAGNI，下次 log.db VACUUM 自然回收）。
                //
                // ponytail: log.db 路径从 PRAGMA database_list 推导（主库同目录 "log.db"），
                // 与 Db::new 的路径派生同源；不改 run_migrations_late 签名。
                let main_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM stats_agg_hourly", [], |r| r.get(0))
                    .unwrap_or(0);
                if main_count == 0 {
                    let main_file: String = {
                        let mut stmt = conn.prepare("PRAGMA database_list")?;
                        stmt.query_map([], |r| {
                            let name: String = r.get(1)?;
                            let file: String = r.get(2).unwrap_or_default();
                            Ok((name, file))
                        })?
                        .filter_map(Result::ok)
                        .find(|(n, _)| n == "main")
                        .map(|(_, f)| f)
                        .unwrap_or_default()
                    };
                    let is_memory = main_file.is_empty()
                        || main_file == ":memory:"
                        || main_file.contains("mode=memory");
                    if !is_memory {
                        let log_path = std::path::Path::new(&main_file)
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new("."))
                            .join("log.db");
                        if log_path.exists() {
                            let log_path_sql = log_path.to_string_lossy().replace("'", "''");
                            let migrate_result = (|| -> SqlResult<()> {
                                conn.execute_batch(&format!(
                                    "ATTACH DATABASE '{log_path_sql}' AS src_log"
                                ))?;
                                let has_table: bool = conn
                                    .query_row(
                                        "SELECT 1 FROM src_log.sqlite_master \
                                         WHERE type='table' AND name='stats_agg_hourly'",
                                        [],
                                        |r| r.get::<_, i64>(0),
                                    )
                                    .is_ok();
                                if has_table {
                                    conn.execute_batch(
                                        "BEGIN;\
                                         INSERT OR IGNORE INTO stats_agg_hourly \
                                         SELECT * FROM src_log.stats_agg_hourly;\
                                         COMMIT;",
                                    )?;
                                    tracing::info!("migration 20260727-17 (原 052): stats_agg_hourly 跨库搬迁 log.db → main 完成");
                                }
                                let _ = conn.execute("DETACH DATABASE 'src_log'", []);
                                Ok(())
                            })();
                            if let Err(e) = migrate_result {
                                tracing::warn!(
                                    "migration 20260727-17 (原 052): stats_agg 跨库搬迁失败（不阻断启动，可后续 rebuild）: {e}"
                                );
                                let _ = conn.execute("DETACH DATABASE 'src_log'", []);
                            }
                        }
                    }
                }

                let _ = conn.execute("DROP TABLE IF EXISTS proxy_log", []);

                // Migration 20260824-02 (票 01 统一中间件引擎): middleware_rule 表迁移到统一模型。
                // 新增 conditions/actions/applies_to/failed 列；旧 8 类列按规则翻译为
                // ConditionNode/ActionStep/AppliesTo JSON（翻译不了的 → failed=1，前端引导手删）；
                // 最后 DROP 旧列 + 重建索引，并按新规格强制覆盖内置规则内容（保留用户 enabled 态）。
                // 幂等守卫：conditions 列已存在（迁移过 / 新装）→ 整块跳过。
                let has_mw_table = conn
                    .query_row(
                        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'middleware_rule'",
                        [],
                        |_| Ok(()),
                    )
                    .is_ok();
                let has_mw_conditions = has_mw_table
                    && conn
                    .prepare("PRAGMA table_info(middleware_rule)")?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .any(|c| c == "conditions");
                if has_mw_table && !has_mw_conditions {
                    conn.execute_batch(
                        "ALTER TABLE middleware_rule ADD COLUMN conditions TEXT NOT NULL DEFAULT '';
                         ALTER TABLE middleware_rule ADD COLUMN actions TEXT NOT NULL DEFAULT '[]';
                         ALTER TABLE middleware_rule ADD COLUMN applies_to TEXT NOT NULL DEFAULT '{}';
                         ALTER TABLE middleware_rule ADD COLUMN failed INTEGER NOT NULL DEFAULT 0;",
                    )?;
                    let translated = translate_legacy_middleware_rules(conn)?;
                    conn.execute_batch(
                        "DROP INDEX IF EXISTS idx_mw_rule_lookup;
                         ALTER TABLE middleware_rule DROP COLUMN rule_type;
                         ALTER TABLE middleware_rule DROP COLUMN scope;
                         ALTER TABLE middleware_rule DROP COLUMN scope_ref;
                         ALTER TABLE middleware_rule DROP COLUMN match_type;
                         ALTER TABLE middleware_rule DROP COLUMN pattern;
                         ALTER TABLE middleware_rule DROP COLUMN action;
                         ALTER TABLE middleware_rule DROP COLUMN config;
                         CREATE INDEX idx_mw_rule_lookup ON middleware_rule(enabled, priority);",
                    )?;
                    tracing::info!(
                        translated,
                        "migration 20260824-02: middleware_rule 迁移到统一引擎模型完成"
                    );
                    // 内置规则按新规格强制覆盖内容（seed 幂等：按 name 覆盖，保留 enabled）。
                    crate::schema::seed_builtin_middleware_rules(conn)?;
                }

                // Migration 20260826-01 (model-info 票 T2): registry 落库两表。
                //
                // model_entry —— 平台视角模型条目，主键 (platform_code, model_id)。
                // 同一模型在不同平台是各自独立的行（价格/上下文限制/官方标记逐平台不同），
                // 跨平台聚合走 canonical_model（故建索引）。
                // 价格全量留在 price_data JSON（同旧 model_price idiom）；单独提列的只有
                // 参与查询/排序/搜索的字段。**票 10 会在此表 ALTER ADD COLUMN display_name**
                // （展示名要参与排序与搜索，不塞 JSON），本 migration 不预建该列。
                //
                // platform_preset —— 一份 platform.json 整体快照，code 主键。
                // 品牌字段（name/logo_url/color/homepage/keywords/source_urls）不拆列：
                // 前端拿整份即可渲染，同步整体覆盖，失败时整份保留（票 12）。
                //
                // 旧 model_price 表本轮保留（当时 resolve_price / 导入导出 / 出站 max_tokens
                // 裁剪仍在读它），切换到 model_entry 是票 T4，DROP 见下方 20260826-03。
                // 数据不迁移——registry 是真值源，T3 同步一轮即重建。
                conn.execute_batch(
                    r#"CREATE TABLE IF NOT EXISTS model_entry (
    platform_code          TEXT NOT NULL,
    model_id               TEXT NOT NULL,
    canonical_model        TEXT NOT NULL DEFAULT '',
    family                 TEXT NOT NULL DEFAULT '',
    version                TEXT NOT NULL DEFAULT '',
    predecessor            TEXT NOT NULL DEFAULT '',
    capabilities           TEXT NOT NULL DEFAULT '[]',
    builtin_tools_excluded TEXT NOT NULL DEFAULT '[]',
    max_input_tokens       INTEGER,
    max_output_tokens      INTEGER,
    context_window         INTEGER,
    official               INTEGER NOT NULL DEFAULT 0,
    price_data             TEXT NOT NULL DEFAULT '{}',
    created_at             INTEGER NOT NULL DEFAULT 0,
    updated_at             INTEGER NOT NULL DEFAULT 0,
    deleted_at             INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (platform_code, model_id)
);

-- 模型维度聚合（group by canonical_model）与「这个模型有哪些平台在卖」查询的驱动索引。
CREATE INDEX IF NOT EXISTS idx_model_entry_canonical ON model_entry(canonical_model);

CREATE TABLE IF NOT EXISTS platform_preset (
    code        TEXT NOT NULL PRIMARY KEY,
    preset_data TEXT NOT NULL DEFAULT '{}',
    created_at  INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL DEFAULT 0,
    deleted_at  INTEGER NOT NULL DEFAULT 0
);"#,
                )?;

                // Migration 20260826-02 (model-info 票 T10): model_entry 加模型展示名列。
                // 单字符串全语言共用（模型名是品牌标识，不译）。独立成列而非塞 price_data JSON——
                // 它要参与列表排序与搜索，塞 JSON 会逼查询层解 JSON。
                // 列存 registry 原值（可为空串，registry 不为省事把展示名填成 model_id）；
                // 缺省/空串回落 model_id 发生在读取层（model_entry.rs::row_to_model_entry）。
                // `let _ =` 是本文件既有加列 idiom：列已存在时 ALTER 报错被吞，等价幂等。
                let _ = conn.execute(
                    "ALTER TABLE model_entry ADD COLUMN display_name TEXT NOT NULL DEFAULT ''",
                    [],
                );

                // Migration 20260826-03 (model-info 票 T6): DROP 旧 model_price 表。
                //
                // T2 起不再同步、T4 起计费与出站 max_tokens 裁剪全改查 model_entry，
                // 至此表已无任何读写方（读取层 model_price.rs 与 5 个查询 command 同票删除）。
                // 数据不迁移：registry 是真值源，同步一轮即由 model_entry 重建；
                // 表内仅有的用户自有数据是 source='manual' 的手工定价，该功能随旧 PricingTab
                // 一并下线（票 T5），没有承接入口，故直接丢弃。
                // 前向单线、无 down：DROP 幂等靠 IF EXISTS。
                conn.execute_batch("DROP TABLE IF EXISTS model_price;")?;
    Ok(())
}

/// 20260824-02 核心：把旧 8 类模型行翻译为统一模型 JSON 列，逐行 UPDATE。
/// 返回成功翻译行数；翻译不了的行置 failed=1（Failed Rule，引擎跳过 + 前端引导手删）。
fn translate_legacy_middleware_rules(conn: &Connection) -> SqlResult<usize> {
    use serde_json::json;
    /// 旧行原样读出：(id, rule_type, scope, scope_ref, match_type, pattern, action, config)。
    type LegacyRow = (i64, String, String, String, String, String, String, String);
    let rows: Vec<LegacyRow> = {
        let mut stmt = conn.prepare(
            "SELECT id, rule_type, scope, scope_ref, match_type, pattern, action, config
             FROM middleware_rule",
        )?;
        let mapped = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
            ))
        })?;
        mapped.collect::<SqlResult<Vec<_>>>()?
    };
    let mut translated = 0;
    for (id, rule_type, scope, scope_ref, match_type, pattern, action, config) in rows {
        // 旧 rule_type → 新 target（error_rule 按旧语义匹配 status+body 文本 → response_body）。
        let target: Option<&str> = match rule_type.as_str() {
            "request_filter" | "sensitive_word" | "redaction" | "content_filter"
            | "dynamic_injection" => Some("request_body"),
            "response_override" | "rectifier" | "error_rule" => Some("response_body"),
            _ => None,
        };
        // 旧 scope → applies_to（global 空 / group:[ref] / platform:[ref i64]）。
        let applies = match scope.as_str() {
            "global" => json!({}),
            "group" if !scope_ref.is_empty() => json!({ "groups": [scope_ref] }),
            "platform" if !scope_ref.is_empty() => match scope_ref.parse::<i64>() {
                Ok(pid) => json!({ "platforms": [pid] }),
                Err(_) => json!({}),
            },
            _ => json!({}),
        };
        // 空 pattern 的 content_filter/redaction 旧行为 = 引擎内置密钥/邮箱检测器
        // → 翻译为显式 regex any 组（不保留隐藏兜底，ADR 0003）。
        let effective_pattern = if pattern.is_empty()
            && matches!(rule_type.as_str(), "content_filter" | "redaction")
        {
            None
        } else {
            Some(pattern.clone())
        };
        let leaf = |target: &str, pat: &str| {
            json!({ "kind": "leaf", "target": target, "field": "", "match_type": match_type, "pattern": pat })
        };
        let conditions: Option<serde_json::Value> = match (&target, &effective_pattern) {
            (Some(t), Some(p)) if p.is_empty() && rule_type == "error_rule" => {
                // 旧 error_rule 空 pattern = 任意非 2xx 命中 → 翻译为 status 数字叶子
                //（classify_error 仅在非 2xx 路径调用，语义等价）。
                Some(json!({ "kind": "leaf", "target": "status", "field": "", "match_type": "regex", "pattern": "[0-9]+" }))
            }
            (Some(t), Some(p)) => Some(json!({ "kind": "all", "children": [leaf(t, p)] })),
            (Some(_), None) => Some(json!({
                "kind": "any",
                "children": [
                    // 检测器 pattern 是 regex；强制 match_type=regex，不复用旧行值（防 contains 语义错位）。
                    json!({ "kind": "leaf", "target": "request_body", "field": "", "match_type": "regex", "pattern": crate::schema::BUILTIN_SECRET_PATTERN }),
                    json!({ "kind": "leaf", "target": "request_body", "field": "", "match_type": "regex", "pattern": crate::schema::BUILTIN_EMAIL_PATTERN }),
                ],
            })),
            (None, _) => None,
        };
        // 旧 config → ActionParams（按 action 取相关字段）。
        let cfg: serde_json::Value = serde_json::from_str(&config).unwrap_or(json!({}));
        let params = match action.as_str() {
            "mask" | "override" => json!({
                "replacement": cfg.get("replacement").cloned().unwrap_or(json!("****")),
                "fields": cfg.get("fields").cloned().unwrap_or(json!([])),
            }),
            "inject" => json!({
                "inject_mode": cfg.get("inject_mode").cloned().unwrap_or(json!("")),
                "target": cfg.get("target").cloned().unwrap_or(json!("")),
                "value": cfg.get("value").cloned().unwrap_or(json!("")),
            }),
            "classify" => json!({
                "category": cfg.get("category").cloned().unwrap_or(json!("")),
                "retryable": cfg.get("retryable").cloned().unwrap_or(json!(true)),
                "override_status": cfg.get("override_status").cloned().unwrap_or(serde_json::Value::Null),
                "override_body": cfg.get("override_body").cloned().unwrap_or(serde_json::Value::Null),
            }),
            // block/warn 无参数。
            _ => json!({}),
        };
        let step = json!({ "kind": action, "params": params });
        let Some(conditions) = conditions else {
            conn.execute(
                "UPDATE middleware_rule SET failed = 1 WHERE id = ?1",
                params![id],
            )?;
            continue;
        };
        conn.execute(
            "UPDATE middleware_rule SET conditions = ?2, actions = ?3, applies_to = ?4 WHERE id = ?1",
            params![
                id,
                conditions.to_string(),
                json!([step]).to_string(),
                applies.to_string(),
            ],
        )?;
        translated += 1;
    }
    Ok(translated)
}

/// proxy_log 表的 late migrations（log.db 库内序 20260727-10..20，原 021–047 范围内的 proxy_log 部分）。
///
/// 拆库后这些 DDL 跑在 log.db 写连接。`cpa_pids` 为原 046 需清理的 CPA 平台 ID
/// 列表（主库预查，跨库不能子查询 JOIN platform）。
///
/// `_auto_map` stats-agg-to-main-db 后已无使用方（原为 032 stats_agg 回填用，已迁主库
/// 20260727-16）。参数保留避免改签名波及 init_tables 调用点（s3/s4 层合并时一并清理）。
pub fn run_migrations_proxy_log_late(
    conn: &Connection,
    _auto_map: &HashMap<String, i64>,
    cpa_pids: &[i64],
    notif_rows: &[(String, String, String, i64)],
) -> SqlResult<()> {
                // Migration 20260727-10 (原 024, proxy_log): group_name → group_key（幂等：探测列存在性）。
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
                // Migration 20260727-11 (原 028): proxy_log 偏索引。
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
                // Migration 20260727-12 (原 031 ①): idx_proxy_log_group_key_stats 覆盖索引。
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_proxy_log_group_key_stats \
                     ON proxy_log(group_key, est_cost, input_tokens, output_tokens, cache_tokens, status_code) \
                     WHERE deleted_at = 0",
                    [],
                );
                // Migration 原编号 032: stats_agg_hourly 建表 + 存量回填 → 已迁回主库 run_migrations_late
                // （20260727-16，落 main DB）。proxy_log 仍留 log.db，跨库数据搬迁由 s2 负责。
                // Migration 20260727-13 (原 033): 删 proxy_log.is_final 列。
                let _ = conn.execute("ALTER TABLE proxy_log DROP COLUMN is_final", []);
                // Migration 20260727-14 (原 034): proxy_log 索引精简 + 复合化 + ANALYZE。
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
                // Migration 20260727-15 (原 035, proxy_log/stats_agg 部分): 删冗余索引。
                // 注：stats-agg-to-main-db s1 后 stats_agg 索引建在主库 20260727-16（idx_stats_agg_time /
                // idx_stats_agg_platform）；此处对 idx_stats_agg_model/group 走 log.db DROP IF EXISTS
                // 是 cosmetic no-op（log.db 此前若建过则删，无则空转，幂等）。
                let _ = conn.execute("DROP INDEX IF EXISTS idx_stats_agg_model", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_stats_agg_group", []);
                let _ = conn.execute("DROP INDEX IF EXISTS idx_proxy_log_created", []);
                // Migration 20260727-16 (原 046, proxy_log 部分): CPA 数据清理。cpa_pids 由主库预查传入。
                // 注：`DELETE FROM stats_agg_hourly` 在 stats-agg-to-main-db s1 后 log.db 不再有
                // 此表 → execute 报 no such table，被 `let _ =` 吞掉（cosmetic no-op）。
                // CPA stats_agg 行清理由 schema.rs Phase 1 `cleanup_cpa_stats_agg` 在主库补做（s5）。
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
                // Migration 20260727-17 (原 047): proxy_log 加 cli_proxy_provider_id。
                let _ = conn.execute(
                    "ALTER TABLE proxy_log ADD COLUMN cli_proxy_provider_id INTEGER",
                    [],
                );
                // Migration 20260824-01 (票 06 stream-full-log): proxy_log 加 done 终态列，取代
                // `response_body == "[stream]"` 哨兵的终态判定（log.rs 背压/聚合 gate/快照移除）。
                let _ = conn.execute(
                    "ALTER TABLE proxy_log ADD COLUMN done INTEGER NOT NULL DEFAULT 0",
                    [],
                );
                // 回填 ①：历史真实终态行（status!=0 且非哨兵）标 done=1。
                // 关日志正文的非流式行 response_body='' 也算终态（哨兵只在流式路径写入）。
                let _ = conn.execute(
                    "UPDATE proxy_log SET done = 1 WHERE status_code != 0 AND response_body != '[stream]'",
                    [],
                );
                // 回填 ②：卡死在哨兵的残留行（旧 bug：flush 丢写，body 永停 '[stream]'）——
                // 终态翻为 done=1 + 清占位，与 sweep_incomplete 处置 status=0 行对称。
                let _ = conn.execute(
                    "UPDATE proxy_log SET done = 1, response_body = '' WHERE response_body = '[stream]'",
                    [],
                );
                // Migration 20260827-01 (票 10 field-trace): proxy_log 加 field_trace 列，
                // 承载出站 body 构造中被丢弃 / 被改写的字段名留痕（只记名不记值）。
                // 存量行按 DEFAULT '' 补齐 = 「无留痕」，语义正确，无需回填。
                let _ = conn.execute(
                    "ALTER TABLE proxy_log ADD COLUMN field_trace TEXT NOT NULL DEFAULT ''",
                    [],
                );
                // Migration 20260727-19 (原 031 ②): notification 时间索引（从主库迁入 log.db）。
                let _ = conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_notification_created ON notification(created_at)",
                    [],
                );
                // Migration 20260727-20 (原 049): notification 表归属 log.db —— 接收主库迁出的历史行。
                // 行由 init_tables Phase 1 从主库残留 notification 表读出（同批 DROP 主库表）；
                // 本处回填 log.db.notification（DDL 由 run_migrations_proxy_log_early 017 建好）。
                // 幂等：主库表已 DROP 后续启动 notif_rows 空 → for 空转，不重复写入。
                for (t, title, body, ts) in notif_rows {
                    let _ = conn.execute(
                        "INSERT INTO notification (notif_type, title, body, created_at) \
                         VALUES (?1, ?2, ?3, ?4)",
                        params![t, title, body, ts],
                    );
                }
    Ok(())
}

/// platform.db 的 late migrations（platform.db 库内序 20260727-01..16 + 20260729-01，原 `run_migrations_late`
/// 内所有操作 platform / "group" / group_platform / cli_proxy_provider 的迁移: 原 022 auto_group /
/// 023–024 group 重建 / 025 GLM coding_plan 回填 / 026 breaker backfill + 列裁剪 / 027 is_default /
/// 029 level_priority / 036 expires_at / 037 last_error / 038 env_vars / 039 last_error 重提 /
/// 044 extra / 045 cli_proxy_provider 建表 / 046 CPA 清理 / 048 quota / 20260729-01 清 W2 peak_hours 副本 /
/// 20260829-01 extra 键 time_models→time_windows / 20260829-02 extra 键 peak_hours→peak）。
///
/// 由 `Db::init_tables` Phase 3 在 `call_platform_traced` 闭包内、紧随
/// `run_migrations_platform_early` 之后调用。fresh install：platform_early 已建现代 schema，
/// 本函数各 ALTER 因 duplicate column 被 `let _ =` 吞 / 各 PRAGMA 探测分支跳过 → 全幂等空转。
/// 存量库（经 Phase 3 INSERT OR IGNORE 回填行）：列补齐 + 历史 data 回填逐条重放，幂等。
pub fn run_migrations_platform_late(conn: &Connection) -> SqlResult<()> {
                // Migration 20260727-01 (原 012): Kimi Code Plan endpoint client_type 修正（codex_tui→claude_code）。
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
                            tracing::info!(platform_id = id, "migration 20260727-01 (原 012): kimi coding endpoint client_type codex_tui→claude_code");
                        }
                    }
                }
                // Migration 20260727-02 (原 022): platform auto_group（已在 platform_early 建表时含此列，ALTER 幂等）。
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN auto_group INTEGER NOT NULL DEFAULT 1", []);

                // Migration 20260727-03 (原 023): 移除 group.path（路由纯按 apikey=group_key）+ name 加 UNIQUE。
                // 门控：仅老库（仍有 path 列）重建。已迁移库无 path 列 → 跳过 → group_key 稳定。
                let has_group_path = conn
                    .prepare("PRAGMA table_info(\"group\")")?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .any(|c| c == "path");
                if has_group_path {
                    conn.execute_batch(
                        r#"CREATE TABLE IF NOT EXISTS "group_new" (
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
                // Migration 20260727-04 (原 024): group 拆 group_key（密钥/路由/日志归属键）+ name（显示名）。
                let has_group_key = conn
                    .prepare("PRAGMA table_info(\"group\")")?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .any(|c| c == "group_key");
                if !has_group_key {
                    conn.execute_batch(
                        r#"CREATE TABLE IF NOT EXISTS "group_new" (
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

                // Migration 20260727-05 (原 025): GLM Coding Plan anthropic 端点补标 coding_plan=true。
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
                            tracing::info!(platform_id = id, "migration 20260727-05 (原 025): glm coding-plan anthropic endpoint coding_plan→true");
                        }
                    }
                }

                // Migration 20260727-06 (原 026): platform 表精简 —— 删 auto_group(原 022) + 3 breaker 列(原 016)，
                // backfill 进 extra JSON 后 DROP 4 列。已迁移库跳过（PRAGMA 探测）。
                let has_breaker_col = conn
                    .prepare("PRAGMA table_info(platform)")?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .any(|c| c == "breaker_failure_threshold");
                if has_breaker_col {
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
                            continue;
                        }
                        let breaker = crate::models::PlatformBreaker {
                            failure_threshold: ft.max(0) as u32,
                            open_secs: os.max(0) as u64,
                            half_open_max: hom.max(0) as u32,
                        };
                        let new_extra = crate::models::merge_breaker_into_extra(&extra, &breaker);
                        conn.execute(
                            "UPDATE platform SET extra = ?1 WHERE id = ?2",
                            params![new_extra, id],
                        )?;
                    }
                    let _ = conn.execute("ALTER TABLE platform DROP COLUMN breaker_failure_threshold", []);
                    let _ = conn.execute("ALTER TABLE platform DROP COLUMN breaker_open_secs", []);
                    let _ = conn.execute("ALTER TABLE platform DROP COLUMN breaker_half_open_max", []);
                    let _ = conn.execute("ALTER TABLE platform DROP COLUMN auto_group", []);
                    tracing::info!("migration 20260727-06 (原 026): backfilled breaker into extra + dropped auto_group/breaker_* columns");
                }

                // Migration 20260727-07 (原 027): 默认分组标记（已在 platform_early 建表含此列，ALTER 幂等）。
                let _ = conn.execute("ALTER TABLE \"group\" ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0", []);

                // Migration 20260727-08 (原 029): group_platform level_priority（已在 platform_early 建表含此列，ALTER 幂等）。
                let _ = conn.execute("ALTER TABLE group_platform ADD COLUMN level_priority INTEGER NOT NULL DEFAULT 5", []);

                // Migration 20260727-09 (原 036): platform 过期时间（已在 platform_early 建表含此列，ALTER 幂等）。
                let _ = conn.execute(
                    "ALTER TABLE platform ADD COLUMN expires_at INTEGER NOT NULL DEFAULT 0",
                    [],
                );

                // Migration 20260727-10 (原 037): 平台最近一次错误信息（已在 platform_early 建表含此列，ALTER 幂等）。
                let _ = conn.execute(
                    "ALTER TABLE platform ADD COLUMN last_error TEXT NOT NULL DEFAULT ''",
                    [],
                );
                let _ = conn.execute(
                    "ALTER TABLE platform ADD COLUMN last_error_at INTEGER NOT NULL DEFAULT 0",
                    [],
                );

                // Migration 20260727-11 (原 038): group 自定义环境变量（已在 platform_early 建表含此列，ALTER 幂等）。
                let _ = conn.execute(
                    "ALTER TABLE \"group\" ADD COLUMN env_vars TEXT NOT NULL DEFAULT '[]'",
                    [],
                );

                // Migration 20260727-12 (原 039): 重写历史 last_error 残留完整 JSON body 为提取后 message。
                reextract_legacy_last_error(conn);

                // Migration 20260727-13 (原 044): group.extra JSON 列（已在 platform_early 建表含此列，ALTER 幂等）。
                let _ = conn.execute(
                    "ALTER TABLE \"group\" ADD COLUMN extra TEXT NOT NULL DEFAULT ''",
                    [],
                );

                // Migration 20260727-14 (原 045): cli_proxy_provider 表。
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

                // Migration 20260727-15 (原 046): 清理旧 CPA(CLIProxyAPI) 平台数据 —— platform.db 部分。
                // proxy_log 删除归 run_migrations_proxy_log_late（log.db，cpa_pids 预查传入）；
                // stats_agg_hourly 删除归 schema.rs Phase 1 cleanup_cpa_stats_agg（主库，s5）。
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

                // Migration 20260727-16 (原 048): cli_proxy_provider 加 quota JSON 列。
                let _ = conn.execute(
                    "ALTER TABLE cli_proxy_provider ADD COLUMN quota TEXT NOT NULL DEFAULT '{}'",
                    [],
                );

                // Migration 20260729-01: 清 platform.extra.peak_hours 里「导入默认配置」历史遗留的
                // W2 副本（preset 已改 bundled 值，用户点过导入按钮复制进 extra 的旧窗口删不掉，
                // model-price-time-tiers design.md §7）。幂等：命中窗口已删/无 peak_hours 键 → 空转。
                strip_w2_peak_hours_copies(conn);

                // Migration 20260829-01 (peak-rename 票 03): platform.extra 键 time_models → time_windows
                // （词汇统一，行为零变）。幂等：无旧键 → 空转；新旧键并存取新键并 warn（异常态）。
                rename_extra_time_models_to_windows(conn);

                // Migration 20260829-02 (peak-rename 票 04): platform.extra 键 peak_hours → peak
                // （词汇统一，行为零变）。幂等：无旧键 → 空转；新旧键并存取新键并 warn（异常态）。
                rename_extra_peak_hours_to_peak(conn);
    Ok(())
}

/// W2 峰值窗口指纹：`start_at==1790784000 && multiplier==2.0 && start_hour==0 && end_hour==24`
/// 三条件全中才判定为历史遗留副本（宁漏勿误删用户自建窗口）。
fn is_w2_peak_window(w: &serde_json::Value) -> bool {
    w.get("start_at").and_then(|v| v.as_i64()) == Some(1790784000)
        && w.get("multiplier").and_then(|v| v.as_f64()) == Some(2.0)
        && w.get("start_hour").and_then(|v| v.as_i64()) == Some(0)
        && w.get("end_hour").and_then(|v| v.as_i64()) == Some(24)
}

/// Migration 20260729-01 实体：遍历 `platform.extra` JSON，命中 [`is_w2_peak_window`] 的窗口
/// 从 `peak_hours` 数组移除；数组清空则整个删掉 `peak_hours` 键。其余字段原样保留。
fn strip_w2_peak_hours_copies(conn: &Connection) {
    let rows: Vec<(i64, String)> = match conn
        .prepare("SELECT id, extra FROM platform WHERE extra LIKE '%peak_hours%'")
    {
        Ok(mut stmt) => stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .ok()
            .map(|iter| iter.filter_map(Result::ok).collect())
            .unwrap_or_default(),
        Err(_) => return,
    };
    let mut cleaned = 0u64;
    for (id, extra) in rows {
        let Ok(mut root) = serde_json::from_str::<serde_json::Value>(&extra) else {
            continue;
        };
        let Some(obj) = root.as_object_mut() else {
            continue;
        };
        let Some(arr) = obj.get("peak_hours").and_then(|v| v.as_array()) else {
            continue;
        };
        if !arr.iter().any(is_w2_peak_window) {
            continue;
        }
        let kept: Vec<serde_json::Value> =
            arr.iter().filter(|w| !is_w2_peak_window(w)).cloned().collect();
        if kept.is_empty() {
            obj.remove("peak_hours");
        } else {
            obj.insert("peak_hours".to_string(), serde_json::Value::Array(kept));
        }
        let new_extra = serde_json::to_string(&root).unwrap_or(extra);
        let _ = conn.execute(
            "UPDATE platform SET extra = ?1 WHERE id = ?2",
            params![new_extra, id],
        );
        cleaned += 1;
    }
    if cleaned > 0 {
        tracing::info!(cleaned, "migration 20260729-01: 清理 platform.extra 里的 W2 peak_hours 历史副本");
    }
}

/// Migration 20260829-01 实体：遍历 `platform.extra` JSON，`time_models` 键改名 `time_windows`。
/// 新旧键并存（异常态，正常只跑一次）→ **新键优先**，旧键丢弃并 warn；无旧键 → 空转；
/// 其余键原样保留。幂等：跑过一次后旧键不存在 → 再跑全空转。
/// 行筛选按 JSON 键字面量（带引号 `"time_models"`）LIKE 匹配：值里出现裸词不误命中。
fn rename_extra_time_models_to_windows(conn: &Connection) {
    let rows: Vec<(i64, String)> = match conn
        .prepare("SELECT id, extra FROM platform WHERE extra LIKE '%\"time_models\"%'")
    {
        Ok(mut stmt) => stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .ok()
            .map(|iter| iter.filter_map(Result::ok).collect())
            .unwrap_or_default(),
        Err(_) => return,
    };
    // 单事务包行循环：中途 crash 整批回滚，杜绝半迁移（逐行状态不一致）。外层调用无事务
    //（schema.rs Phase 3 裸跑）→ BEGIN IMMEDIATE 不会嵌套；万一失败（异常态）则裸跑，
    // 退化为旧行为，幂等重跑仍兜底。
    let in_txn = conn.execute_batch("BEGIN IMMEDIATE").is_ok();
    let mut renamed = 0u64;
    for (id, extra) in rows {
        let Ok(mut root) = serde_json::from_str::<serde_json::Value>(&extra) else {
            continue;
        };
        let Some(obj) = root.as_object_mut() else {
            continue;
        };
        let Some(old) = obj.remove("time_models") else {
            continue;
        };
        if obj.contains_key("time_windows") {
            tracing::warn!(platform_id = id,
                "migration 20260829-01: extra 同时含 time_models 与 time_windows，保留新键 time_windows（旧键丢弃）");
        } else {
            obj.insert("time_windows".to_string(), old);
        }
        let new_extra = serde_json::to_string(&root).unwrap_or(extra);
        let _ = conn.execute(
            "UPDATE platform SET extra = ?1 WHERE id = ?2",
            params![new_extra, id],
        );
        renamed += 1;
    }
    // in_txn 门控：BEGIN 失败时连接已在（外层）事务中，裸 COMMIT 会提前提交外层事务。
    if in_txn {
        let _ = conn.execute_batch("COMMIT");
    }
    if renamed > 0 {
        tracing::info!(renamed, "migration 20260829-01: platform.extra.time_models → time_windows 改名完成");
    }
}

/// Migration 20260829-02 实体：遍历 `platform.extra` JSON，`peak_hours` 键改名 `peak`。
/// 新旧键并存（异常态，正常只跑一次）→ **新键优先**，旧键丢弃并 warn；无旧键 → 空转；
/// 其余键原样保留。幂等：跑过一次后旧键不存在 → 再跑全空转。
/// 行筛选按 JSON 键字面量（带引号 `"peak_hours"`）LIKE 匹配：值里出现裸词不误命中。
fn rename_extra_peak_hours_to_peak(conn: &Connection) {
    let rows: Vec<(i64, String)> = match conn
        .prepare("SELECT id, extra FROM platform WHERE extra LIKE '%\"peak_hours\"%'")
    {
        Ok(mut stmt) => stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .ok()
            .map(|iter| iter.filter_map(Result::ok).collect())
            .unwrap_or_default(),
        Err(_) => return,
    };
    // 单事务包行循环：中途 crash 整批回滚，杜绝半迁移（逐行状态不一致）。外层调用无事务
    //（schema.rs Phase 3 裸跑）→ BEGIN IMMEDIATE 不会嵌套；万一失败（异常态）则裸跑，
    // 退化为旧行为，幂等重跑仍兜底。
    let in_txn = conn.execute_batch("BEGIN IMMEDIATE").is_ok();
    let mut renamed = 0u64;
    for (id, extra) in rows {
        let Ok(mut root) = serde_json::from_str::<serde_json::Value>(&extra) else {
            continue;
        };
        let Some(obj) = root.as_object_mut() else {
            continue;
        };
        let Some(old) = obj.remove("peak_hours") else {
            continue;
        };
        if obj.contains_key("peak") {
            tracing::warn!(platform_id = id,
                "migration 20260829-02: extra 同时含 peak_hours 与 peak，保留新键 peak（旧键丢弃）");
        } else {
            obj.insert("peak".to_string(), old);
        }
        let new_extra = serde_json::to_string(&root).unwrap_or(extra);
        let _ = conn.execute(
            "UPDATE platform SET extra = ?1 WHERE id = ?2",
            params![new_extra, id],
        );
        renamed += 1;
    }
    // in_txn 门控：BEGIN 失败时连接已在（外层）事务中，裸 COMMIT 会提前提交外层事务。
    if in_txn {
        let _ = conn.execute_batch("COMMIT");
    }
    if renamed > 0 {
        tracing::info!(renamed, "migration 20260829-02: platform.extra.peak_hours → peak 改名完成");
    }
}

/// Migration 20260727-12 (原 039): 把原 037 引入但未走 extract_error_message 的历史 last_error 行重提为 message。
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
        let Some(msg) = crate::extract_error_message(body) else {
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

/// Migration 20260727-15 (原 043): MITM 配置从专属表（mitm_ca / mitm_whitelist）迁到通用 setting 表
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
    for (rule_type, pattern) in crate::DEFAULT_RULES {
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
            if let Some(host) = crate::endpoint_host(&base_url) {
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

    fn no_op_backfill() -> crate::schema::BackfillFn {
        std::sync::Arc::new(|_c: &Connection, _m: &std::collections::HashMap<String, i64>| Ok(()))
    }

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

    /// run_migrations_platform_late on a legacy DB that has group.path → exercises has_group_path=true branch.
    #[test]
    fn migrations_late_group_path_migration_executed() {
        let conn = make_legacy_conn_with_group_path();
        // The legacy DB has group.path but no group.group_key.
        // run_migrations_platform_late should:
        //   1. Detect has_group_path=true → rebuild group table (removes path, adds UNIQUE(name))
        //   2. Detect !has_group_key=true → rebuild group table again (adds group_key)
        let result = run_migrations_platform_late(&conn);
        assert!(result.is_ok(), "run_migrations_platform_late failed: {:?}", result);
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

    /// Migration 20260824-02（票 01 统一引擎）：旧 8 类 middleware_rule 行翻译为统一模型；
    /// 未知 rule_type → failed=1；旧列 DROP；内置规则按新规格 seed（覆盖内容保留 enabled）。
    #[test]
    fn migrations_middleware_rule_unified_model_20260824() {
        let conn = make_modern_conn();
        conn.execute_batch(r#"
            CREATE TABLE middleware_rule (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               name TEXT NOT NULL,
               description TEXT NOT NULL DEFAULT '',
               rule_type TEXT NOT NULL,
               scope TEXT NOT NULL DEFAULT 'global',
               scope_ref TEXT NOT NULL DEFAULT '',
               match_type TEXT NOT NULL DEFAULT 'contains',
               pattern TEXT NOT NULL DEFAULT '',
               action TEXT NOT NULL DEFAULT 'warn',
               config TEXT NOT NULL DEFAULT '{}',
               priority INTEGER NOT NULL DEFAULT 0,
               enabled INTEGER NOT NULL DEFAULT 1,
               is_builtin INTEGER NOT NULL DEFAULT 0,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
            );
            INSERT INTO middleware_rule (name, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at) VALUES
              ('user-block', 'request_filter', 'group', 'gk1', 'contains', 'badword', 'block', '{}', 5, 1, 0, 0, 0),
              ('user-mask', 'redaction', 'platform', '7', 'regex', 'sk-abc', 'mask', '{"replacement":"[x]","fields":["messages"]}', 6, 1, 0, 0, 0),
              ('user-detector', 'content_filter', 'global', '', 'contains', '', 'mask', '{}', 7, 0, 0, 0, 0),
              ('user-unknown', 'weird_type', 'global', '', 'contains', 'p', 'warn', '{}', 8, 1, 0, 0, 0),
              ('user-ghost-builtin', 'weird_type', 'global', '', 'contains', 'p', 'warn', '{}', 9, 1, 1, 0, 0),
              ('内置·密钥脱敏', 'content_filter', 'global', '', 'contains', '', 'mask', '{}', 10, 0, 1, 0, 0);
        "#).unwrap();
        let r = run_migrations_late(&conn, no_op_backfill());
        assert!(r.is_ok(), "migration 20260824-02 should succeed: {:?}", r);

        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(middleware_rule)").unwrap()
            .query_map([], |r| r.get::<_, String>(1)).unwrap()
            .filter_map(Result::ok).collect();
        assert!(cols.contains(&"conditions".to_string()));
        assert!(!cols.contains(&"rule_type".to_string()), "old columns dropped");

        // ① request_filter block 翻译：request_body contains 叶子 + block 步骤 + applies_to.groups
        let (cond, acts, appl): (String, String, String) = conn.query_row(
            "SELECT conditions, actions, applies_to FROM middleware_rule WHERE name='user-block'", [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert!(cond.contains("\"kind\":\"all\"") && cond.contains("\"target\":\"request_body\"") && cond.contains("\"match_type\":\"contains\""));
        assert!(acts.contains("\"kind\":\"block\""));
        assert!(appl.contains("\"groups\":[\"gk1\"]"));

        // ② redaction mask 平台作用域 + config 参数迁移
        let (cond, acts, appl): (String, String, String) = conn.query_row(
            "SELECT conditions, actions, applies_to FROM middleware_rule WHERE name='user-mask'", [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert!(cond.contains("\"match_type\":\"regex\""));
        assert!(acts.contains("\"replacement\":\"[x]\"") && acts.contains("\"fields\":[\"messages\"]"));
        assert!(appl.contains("\"platforms\":[7]"));

        // ③ 空 pattern 检测器行 → 显式 secret/email any 组；enabled=0 保留
        let (cond, enabled): (String, i64) = conn.query_row(
            "SELECT conditions, enabled FROM middleware_rule WHERE name='user-detector'", [],
            |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        // JSON 转义后反斜杠翻倍，用无反斜杠子串断言。
        assert!(cond.contains("sk-[a-zA-Z0-9]{16,}") && cond.contains("@[a-zA-Z0-9."));
        assert_eq!(enabled, 0);

        // ④ 未知 rule_type → failed=1
        let failed: i64 = conn.query_row(
            "SELECT failed FROM middleware_rule WHERE name='user-unknown'", [], |r| r.get(0)).unwrap();
        assert_eq!(failed, 1);

        // ④b failed 内置残留（未知类型翻译失败）→ seed 自动清除，不留给用户
        let ghost: i64 = conn.query_row(
            "SELECT COUNT(*) FROM middleware_rule WHERE name='user-ghost-builtin'", [], |r| r.get(0)).unwrap();
        assert_eq!(ghost, 0, "failed builtin row auto-removed by seed");

        // ⑤ 内置规则：按新规格覆盖内容 + failed=0 重置 + enabled=0 保留（name 覆盖路径）
        let (cond, enabled, failed): (String, i64, i64) = conn.query_row(
            "SELECT conditions, enabled, failed FROM middleware_rule WHERE name='内置·密钥脱敏'", [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(enabled, 0, "user disabled builtin stays disabled");
        assert_eq!(failed, 0);
        assert!(cond.contains("\"kind\":\"leaf\""), "builtin content overwritten to new spec");
        // 新内置规格补齐（升级新增的 DB/Redis 脱敏等）
        let n_builtin: i64 = conn.query_row(
            "SELECT COUNT(*) FROM middleware_rule WHERE is_builtin=1", [], |r| r.get(0)).unwrap();
        assert_eq!(n_builtin as usize, crate::schema::builtin_rule_specs().len());

        // 幂等：再跑一次不炸（conditions 已存在 → 整块跳过）
        let r2 = run_migrations_late(&conn, no_op_backfill());
        assert!(r2.is_ok());
    }

    /// run_migrations_late on a fully modern schema (all conditional branches skip) → idempotent.
    #[test]
    fn migrations_late_modern_schema_idempotent() {
        let conn = make_modern_conn();
        let result = run_migrations_late(&conn, no_op_backfill());
        assert!(result.is_ok(), "modern schema migration should succeed: {:?}", result);
    }

    /// Migration 20260727-13 (原 044): group.extra 列。两条路径：
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
        let r1 = run_migrations_platform_late(&conn);
        assert!(r1.is_ok(), "first migration 20260727-13 should succeed: {:?}", r1);
        let has_extra = conn
            .prepare("PRAGMA table_info(\"group\")")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|c| c == "extra");
        assert!(has_extra, "extra column must exist after migration 20260727-13");
        // 行数据保留 + extra 默认 ''（空串 = "{}" 轻量表示）
        let extra: String = conn
            .query_row("SELECT extra FROM \"group\" WHERE name='g044'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(extra, "", "extra default should be empty string");

        // ② 再跑：duplicate column 错误被忽略，幂等（不返 Err，extra 列仍存在）
        let r2 = run_migrations_platform_late(&conn);
        assert!(r2.is_ok(), "re-running migration 20260727-13 must be idempotent: {:?}", r2);
    }

    /// Migration 20260727-06 (原 026): platform with breaker columns → backfill into extra + drop columns.
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
        let result = run_migrations_platform_late(&conn);
        assert!(result.is_ok(), "breaker backfill migration should succeed: {:?}", result);
        // After migration, breaker_failure_threshold column should be gone.
        let has_breaker = conn
            .prepare("PRAGMA table_info(platform)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|c| c == "breaker_failure_threshold");
        assert!(!has_breaker, "breaker_failure_threshold should be dropped after migration 20260727-06");
        // The non-zero platform's extra should now contain breaker data.
        let extra: String = conn
            .query_row("SELECT extra FROM platform WHERE name = 'test-plat'", [], |r| r.get(0))
            .unwrap();
        assert!(extra.contains("breaker") || extra.contains("failure_threshold"),
            "extra should contain breaker data after backfill, got: {}", extra);
    }

    /// Migration 20260727-05 (原 025): GLM platform with coding openai endpoint + anthropic endpoint not tagged coding_plan
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
        let result = run_migrations_platform_late(&conn);
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
            "anthropic endpoint should have coding_plan=true after migration 20260727-05"
        );
    }

    /// Migration 20260824-01（票 06 stream-full-log）：proxy_log 加 done 终态列 + 回填。
    /// ① 历史真实终态行（status!=0 且非哨兵）→ done=1；② 卡死哨兵行 → done=1 + 清占位；
    /// ③ status=0 中间行 → 保持 done=0（由 sweep 兜底翻 499/done=1）。
    #[test]
    fn migrations_proxy_log_done_column_backfill_20260824() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE proxy_log (
                id TEXT PRIMARY KEY, group_key TEXT NOT NULL DEFAULT '', model TEXT NOT NULL DEFAULT '',
                actual_model TEXT NOT NULL DEFAULT '', source_protocol TEXT NOT NULL DEFAULT '',
                target_protocol TEXT NOT NULL DEFAULT '', platform_id INTEGER NOT NULL DEFAULT 0,
                request_headers TEXT NOT NULL DEFAULT '', request_body TEXT NOT NULL DEFAULT '',
                upstream_request_headers TEXT NOT NULL DEFAULT '', upstream_request_body TEXT NOT NULL DEFAULT '',
                response_body TEXT NOT NULL DEFAULT '', request_url TEXT NOT NULL DEFAULT '',
                upstream_request_url TEXT NOT NULL DEFAULT '', upstream_response_headers TEXT NOT NULL DEFAULT '',
                upstream_status_code INTEGER NOT NULL DEFAULT 0, user_response_headers TEXT NOT NULL DEFAULT '',
                user_response_body TEXT NOT NULL DEFAULT '', status_code INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0, input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0, cache_tokens INTEGER NOT NULL DEFAULT 0,
                est_cost REAL NOT NULL DEFAULT 0, is_stream INTEGER NOT NULL DEFAULT 0,
                attempts TEXT NOT NULL DEFAULT '', retry_count INTEGER NOT NULL DEFAULT 0,
                blocked_by TEXT NOT NULL DEFAULT '', blocked_reason TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
                deleted_at INTEGER NOT NULL DEFAULT 0, cli_proxy_provider_id INTEGER
            );
            INSERT INTO proxy_log (id, status_code, response_body) VALUES ('term_ok', 200, '{\"ok\":1}');
            INSERT INTO proxy_log (id, status_code, response_body) VALUES ('term_err', 502, '');
            INSERT INTO proxy_log (id, status_code, response_body) VALUES ('stuck', 200, '[stream]');
            INSERT INTO proxy_log (id, status_code, response_body) VALUES ('mid', 0, '');",
        ).unwrap();
        run_migrations_proxy_log_late(&conn, &std::collections::HashMap::new(), &[], &[])
            .expect("run_migrations_proxy_log_late should succeed");
        let done: i64 = conn.query_row("SELECT done FROM proxy_log WHERE id='term_ok'", [], |r| r.get(0)).unwrap();
        assert_eq!(done, 1, "历史 200 终态行应回填 done=1");
        let done: i64 = conn.query_row("SELECT done FROM proxy_log WHERE id='term_err'", [], |r| r.get(0)).unwrap();
        assert_eq!(done, 1, "关日志正文的错误终态行（body 空）也应回填 done=1");
        let (done, body): (i64, String) = conn
            .query_row("SELECT done, response_body FROM proxy_log WHERE id='stuck'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((done, body.as_str()), (1, ""), "卡死哨兵行应翻 done=1 且清占位");
        let done: i64 = conn.query_row("SELECT done FROM proxy_log WHERE id='mid'", [], |r| r.get(0)).unwrap();
        assert_eq!(done, 0, "status=0 中间行保持 done=0（交 sweep 兜底）");
    }

    /// Migration 20260827-01（票 10 field-trace）：老库（无 field_trace 列）升级路径。
    /// ① 列被补上；② 存量行按 DEFAULT '' 补齐（= 无留痕）；③ 重跑幂等（duplicate column 被吞）
    /// 且不覆盖已写入的留痕值。
    #[test]
    fn migrations_proxy_log_field_trace_column_20260827() {
        let conn = Connection::open_in_memory().unwrap();
        // 老库 schema：done 之前的列集（field_trace 不存在）。
        conn.execute_batch(
            "CREATE TABLE proxy_log (
                id TEXT PRIMARY KEY, group_key TEXT NOT NULL DEFAULT '', model TEXT NOT NULL DEFAULT '',
                actual_model TEXT NOT NULL DEFAULT '', source_protocol TEXT NOT NULL DEFAULT '',
                target_protocol TEXT NOT NULL DEFAULT '', platform_id INTEGER NOT NULL DEFAULT 0,
                request_headers TEXT NOT NULL DEFAULT '', request_body TEXT NOT NULL DEFAULT '',
                upstream_request_headers TEXT NOT NULL DEFAULT '', upstream_request_body TEXT NOT NULL DEFAULT '',
                response_body TEXT NOT NULL DEFAULT '', request_url TEXT NOT NULL DEFAULT '',
                upstream_request_url TEXT NOT NULL DEFAULT '', upstream_response_headers TEXT NOT NULL DEFAULT '',
                upstream_status_code INTEGER NOT NULL DEFAULT 0, user_response_headers TEXT NOT NULL DEFAULT '',
                user_response_body TEXT NOT NULL DEFAULT '', status_code INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0, input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0, cache_tokens INTEGER NOT NULL DEFAULT 0,
                est_cost REAL NOT NULL DEFAULT 0, is_stream INTEGER NOT NULL DEFAULT 0,
                attempts TEXT NOT NULL DEFAULT '', retry_count INTEGER NOT NULL DEFAULT 0,
                blocked_by TEXT NOT NULL DEFAULT '', blocked_reason TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
                deleted_at INTEGER NOT NULL DEFAULT 0, cli_proxy_provider_id INTEGER
            );
            INSERT INTO proxy_log (id, status_code) VALUES ('legacy', 200);",
        ).unwrap();
        run_migrations_proxy_log_late(&conn, &std::collections::HashMap::new(), &[], &[])
            .expect("run_migrations_proxy_log_late should succeed on legacy schema");
        let ft: String = conn
            .query_row("SELECT field_trace FROM proxy_log WHERE id='legacy'", [], |r| r.get(0))
            .expect("field_trace column should exist after migration");
        assert_eq!(ft, "", "存量行 field_trace 应为空串（无留痕）");

        // 写入留痕后重跑迁移：幂等，不重建列、不覆盖值。
        conn.execute("UPDATE proxy_log SET field_trace = 'drop:user' WHERE id='legacy'", []).unwrap();
        run_migrations_proxy_log_late(&conn, &std::collections::HashMap::new(), &[], &[])
            .expect("re-run should be idempotent");
        let ft: String = conn
            .query_row("SELECT field_trace FROM proxy_log WHERE id='legacy'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ft, "drop:user", "重跑迁移不应覆盖已写入的留痕");
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
        let result = run_migrations_platform_late(&conn);
        assert!(result.is_ok(), "run_migrations_platform_late failed: {:?}", result);
        let has_gk = conn
            .prepare("PRAGMA table_info(\"group\")")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|c| c == "group_key");
        assert!(has_gk, "group_key should exist after migration");
    }

    /// Migration 20260727-12 (原 039): 历史 last_error 残留完整 JSON body → 重提为 message。幂等。
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

        let result = run_migrations_platform_late(&conn);
        assert!(result.is_ok(), "run_migrations_platform_late failed: {:?}", result);

        let get_last_error = |name: &str| -> String {
            conn.query_row("SELECT last_error FROM platform WHERE name = ?1", [name], |r| r.get(0)).unwrap()
        };
        assert_eq!(get_last_error("stale"), "HTTP 429: 余额不足或无可用资源包,请充值。");
        assert_eq!(get_last_error("plain"), "HTTP 429: Too many requests");
        assert_eq!(get_last_error("already"), "HTTP 429: quota exhausted");
        assert_eq!(get_last_error("toplevel"), "HTTP 401: 身份验证失败。");

        // 幂等：再跑一次所有行不变。
        let _ = run_migrations_platform_late(&conn);
        assert_eq!(get_last_error("stale"), "HTTP 429: 余额不足或无可用资源包,请充值。");
        assert_eq!(get_last_error("plain"), "HTTP 429: Too many requests");
    }

    /// Migration 20260727-15 (原 040–043): MITM 两表迁 setting + 默认白名单 seed。
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

        let result = run_migrations_late(&conn, no_op_backfill());
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
        let _ = run_migrations_late(&conn, no_op_backfill());
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

    /// Migration 20260727-15 (原 043) 验收（旧库迁移路径）：旧 mitm_ca + mitm_whitelist 行 →
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

        let result = run_migrations_late(&conn, no_op_backfill());
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
        let _ = run_migrations_late(&conn, no_op_backfill());
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

    /// Migration 20260829-01（peak-rename 票 03）：platform.extra 键 `time_models` → `time_windows`。
    /// 覆盖：旧键→新键 + 无关键原样、新旧并存取新键、无旧键空转、跑两遍幂等。
    #[test]
    fn migrations_platform_extra_time_models_renamed_20260829() {
        let conn = make_modern_conn();
        conn.execute_batch(r#"
            INSERT INTO platform (id, name, extra) VALUES
            (1, 'old-key', '{"time_models":[{"windows":[{"start_hour":0,"end_hour":24}],"models":{"default":"m1"}}],"breaker":{"threshold":3}}'),
            (2, 'both-keys', '{"time_models":[{"models":{"default":"old"}}],"time_windows":[{"models":{"default":"new"}}]}'),
            (3, 'new-key-only', '{"time_windows":[{"models":{"default":"kept"}}],"mock":{"v":1}}'),
            (4, 'unrelated', '{"breaker":{"threshold":5},"disable_during_peak":true}'),
            (5, 'value-substring', '{"note":"legacy time_models mentioned in a value"}');
        "#).unwrap();
        let r1 = run_migrations_platform_late(&conn);
        assert!(r1.is_ok(), "run_migrations_platform_late failed: {:?}", r1);

        let extra_of = |id: i64| -> serde_json::Value {
            let raw: String = conn
                .query_row("SELECT extra FROM platform WHERE id = ?1", params![id], |r| r.get(0))
                .unwrap();
            serde_json::from_str(&raw).unwrap()
        };
        // ① 旧键 → 新键（值原样搬），无关键 breaker 保留，旧键消失
        let e1 = extra_of(1);
        assert_eq!(e1["time_windows"][0]["models"]["default"], "m1", "old-key row: value moved to time_windows");
        assert!(e1.get("time_models").is_none(), "old-key row: time_models must be removed");
        assert_eq!(e1["breaker"]["threshold"], 3, "unrelated key breaker preserved");
        // ② 新旧并存 → 新键优先，旧键丢弃
        let e2 = extra_of(2);
        assert_eq!(e2["time_windows"][0]["models"]["default"], "new", "both-keys row: new key wins");
        assert!(e2.get("time_models").is_none(), "both-keys row: old key dropped");
        // ③ 本就只有新键 → 原样（空转路径）
        let e3 = extra_of(3);
        assert_eq!(e3["time_windows"][0]["models"]["default"], "kept", "new-key-only row untouched");
        assert_eq!(e3["mock"]["v"], 1, "unrelated key mock preserved");
        // ④ 无任一键 → 完全不动
        let e4 = extra_of(4);
        assert!(e4.get("time_windows").is_none() && e4.get("time_models").is_none(), "unrelated row gains no key");
        assert_eq!(e4["disable_during_peak"], true, "unrelated row values intact");
        // ④b 值里含裸词 time_models（非带引号键）→ LIKE 不误命中，行不动
        let e5 = extra_of(5);
        assert!(e5.get("time_windows").is_none() && e5.get("time_models").is_none(),
            "value-substring row gains no key");
        assert!(e5["note"].as_str().unwrap().contains("time_models"), "value-substring row note preserved");

        // ⑤ 幂等：再跑一遍，5 行 extra 全部不变
        let before: Vec<serde_json::Value> = (1..=5).map(extra_of).collect();
        let r2 = run_migrations_platform_late(&conn);
        assert!(r2.is_ok(), "second run failed: {:?}", r2);
        for (id, b) in before.iter().enumerate() {
            let after = extra_of(id as i64 + 1);
            assert_eq!(&after, b, "row {} extra must be identical after re-run", id + 1);
        }
    }

    /// Migration 20260829-02（peak-rename 票 04）：platform.extra 键 `peak_hours` → `peak`。
    /// 覆盖：旧键→新键 + 无关键原样、新旧并存取新键、无旧键空转、跑两遍幂等。
    #[test]
    fn migrations_platform_extra_peak_hours_renamed_20260829() {
        let conn = make_modern_conn();
        conn.execute_batch(r#"
            INSERT INTO platform (id, name, extra) VALUES
            (1, 'old-key', '{"peak_hours":[{"start_hour":6,"end_hour":10,"multiplier":3.0}],"breaker":{"threshold":3}}'),
            (2, 'both-keys', '{"peak_hours":[{"multiplier":2.0}],"peak":[{"multiplier":1.5}]}'),
            (3, 'new-key-only', '{"peak":[{"multiplier":1.25}],"mock":{"v":1}}'),
            (4, 'unrelated', '{"breaker":{"threshold":5},"disable_during_peak":true}'),
            (5, 'value-substring', '{"note":"legacy peak_hours mentioned in a value"}');
        "#).unwrap();
        let r1 = run_migrations_platform_late(&conn);
        assert!(r1.is_ok(), "run_migrations_platform_late failed: {:?}", r1);

        let extra_of = |id: i64| -> serde_json::Value {
            let raw: String = conn
                .query_row("SELECT extra FROM platform WHERE id = ?1", params![id], |r| r.get(0))
                .unwrap();
            serde_json::from_str(&raw).unwrap()
        };
        // ① 旧键 → 新键（值原样搬），无关键 breaker 保留，旧键消失
        let e1 = extra_of(1);
        assert_eq!(e1["peak"][0]["multiplier"], 3.0, "old-key row: value moved to peak");
        assert!(e1.get("peak_hours").is_none(), "old-key row: peak_hours must be removed");
        assert_eq!(e1["breaker"]["threshold"], 3, "unrelated key breaker preserved");
        // ② 新旧并存 → 新键优先，旧键丢弃
        let e2 = extra_of(2);
        assert_eq!(e2["peak"][0]["multiplier"], 1.5, "both-keys row: new key wins");
        assert!(e2.get("peak_hours").is_none(), "both-keys row: old key dropped");
        // ③ 本就只有新键 → 原样（空转路径）
        let e3 = extra_of(3);
        assert_eq!(e3["peak"][0]["multiplier"], 1.25, "new-key-only row untouched");
        assert_eq!(e3["mock"]["v"], 1, "unrelated key mock preserved");
        // ④ 无任一键 → 完全不动
        let e4 = extra_of(4);
        assert!(e4.get("peak").is_none() && e4.get("peak_hours").is_none(), "unrelated row gains no key");
        assert_eq!(e4["disable_during_peak"], true, "unrelated row values intact");
        // ④b 值里含裸词 peak_hours（非带引号键）→ LIKE 不误命中，行不动
        let e5 = extra_of(5);
        assert!(e5.get("peak").is_none() && e5.get("peak_hours").is_none(),
            "value-substring row gains no key");
        assert!(e5["note"].as_str().unwrap().contains("peak_hours"), "value-substring row note preserved");

        // ⑤ 幂等：再跑一遍，5 行 extra 全部不变
        let before: Vec<serde_json::Value> = (1..=5).map(extra_of).collect();
        let r2 = run_migrations_platform_late(&conn);
        assert!(r2.is_ok(), "second run failed: {:?}", r2);
        for (id, b) in before.iter().enumerate() {
            let after = extra_of(id as i64 + 1);
            assert_eq!(&after, b, "row {} extra must be identical after re-run", id + 1);
        }
    }
}