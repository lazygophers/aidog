use super::*;
use rusqlite::{params, OptionalExtension, Result as SqlResult};

/// 主库迁出的 notification 行（migration 049）。由 init_tables 在主库闭包内读出 + DROP 主库
/// 残留表后，传入 proxy_log_late 写入 log.db.notification。空 Vec = 主库表已不存在（已迁移过）。
type NotifRow = (String, String, String, i64);

/// 迁移期间读出的 4 表行数据（config-db-split）。由 init_tables Phase 1 主库闭包读出（保 id 全列），
/// Phase 3 platform.db 闭包 INSERT OR IGNORE 写入。列名 + 值均用 `rusqlite::types::Value` 动态承载，
/// 避免对 4 表 80+ 列各自建 tuple 类型（列漂移时自动跟随 SELECT *）。
type TableRows = (Vec<String>, Vec<Vec<rusqlite::types::Value>>);

/// Migration 049: `notification` 表归属 log.db。主库残留表读出全部行（**不 DROP**，由 Phase 1
/// 主库闭包独立 DROP，避免 notification 049 的 read+DROP→INSERT 顺序在 crash 时丢数据）。
/// 幂等：表已不存在 → SELECT 报错吞空 Vec。
fn migrate_main_notification_out(conn: &rusqlite::Connection) -> Vec<NotifRow> {
    conn.prepare("SELECT notif_type, title, body, created_at FROM notification")
        .and_then(|mut s| {
            s.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map(|iter| iter.filter_map(Result::ok).collect())
        })
        .unwrap_or_default()
}

/// 查主库 platform 表中 CPA 平台 ID（migration 046 清理用）。跨库不能子查询，由 init_tables
/// 在主库闭包内预查后传入 proxy_log_late。无 CPA 行返空 Vec（proxy_log_late for-loop 空转）。
/// 保留在 Phase 1 主库闭包：首次迁移主库仍有 platform 存量数据；二次启动主库已 DROP → 返空 Vec
/// （此时 CPA proxy_log 清理已无意义，046 DELETE 幂等空转）。
fn fetch_cpa_platform_ids(conn: &rusqlite::Connection) -> Vec<i64> {
    conn.prepare("SELECT id FROM platform WHERE platform_type LIKE '\"cpa-%'")
        .and_then(|mut s| {
            s.query_map([], |r| r.get::<_, i64>(0))
                .map(|iter| iter.filter_map(Result::ok).collect())
        })
        .unwrap_or_default()
}

/// CPA（CLIProxyAPI）平台聚合行清理 —— stats-agg-to-main-db s5 补 Mig 046 在主库的缺失。
///
/// 背景：Mig 046 的 CPA 清理在 `run_migrations_proxy_log_late`（log.db 写连接）内跑，含
/// `DELETE FROM stats_agg_hourly`。s1 把 stats_agg_hourly DDL 迁回主库后，log.db 不再有此表，
/// 那条 DELETE 报 no such table 被 `let _ =` 吞 → CPA stats_agg 残留行不再清理。
///
/// 本函数在 Phase 1 主库连接上补做：对每个 cpa pid 删 stats_agg_hourly 残留行。
/// 幂等：DELETE 无匹配行 0 影响；每次启动跑无副作用。
fn cleanup_cpa_stats_agg(conn: &rusqlite::Connection, cpa_pids: &[i64]) {
    if cpa_pids.is_empty() {
        return;
    }
    let mut deleted = 0u64;
    for pid in cpa_pids {
        match conn.execute(
            "DELETE FROM stats_agg_hourly WHERE platform_id = ?1",
            rusqlite::params![pid],
        ) {
            Ok(n) => deleted += n as u64,
            Err(e) => {
                tracing::warn!(
                    pid,
                    error = %e,
                    "cleanup_cpa_stats_agg: DELETE failed for pid (stats_agg_hourly DDL 预期已存在)"
                );
            }
        }
    }
    if deleted > 0 {
        tracing::info!(deleted, "cleanup_cpa_stats_agg: 主库 CPA 残留聚合行清理完成");
    }
}

/// 读主库 4 表（platform / "group" / group_platform / cli_proxy_provider）全行（**不 DROP**）。
/// config-db-split crash-safe 四阶段迁移的 Phase 1 read：仅读不删，Phase 3 成功后才由 Phase 4 DROP。
/// 表不存在（已迁过 / 新装主库从未建）→ 返空 TableRows，Phase 3 INSERT for 空转。
/// ponytail: 全列 SELECT * + Value 动态类型，比硬编码 80+ 列 tuple 短得多且抗列漂移；保 id 列在首位。
fn read_platform_tables_out(conn: &rusqlite::Connection, table: &str) -> TableRows {
    let sql = format!("SELECT * FROM {table}");
    match conn.prepare(&sql) {
        Ok(mut stmt) => {
            let cols: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
            let col_count = stmt.column_count();
            match stmt.query_map([], move |r| {
                (0..col_count)
                    .map(|i| r.get::<_, rusqlite::types::Value>(i))
                    .collect()
            }) {
                Ok(iter) => {
                    let rows: Vec<Vec<rusqlite::types::Value>> = iter.filter_map(Result::ok).collect();
                    (cols, rows)
                }
                Err(_) => (Vec::new(), Vec::new()),
            }
        }
        Err(_) => (Vec::new(), Vec::new()),
    }
}

/// 把 4 表行数据写入 platform.db（Phase 3 INSERT OR IGNORE，保 id）。
/// 列名取自 Phase 1 SELECT * 的源列集 —— 源 / 目的同名同序，id 列在首位保留原主键值。
/// 源缺列（老库未跑全 migration）→ INSERT 列集是源子集，目标剩余列按 schema DEFAULT 填充。
/// INSERT OR IGNORE：目标 id 已存在则跳过（重放幂等，防 Phase 3 重试翻倍）。
fn insert_platform_table_rows(
    conn: &rusqlite::Connection,
    table: &str,
    cols: &[String],
    rows: &[Vec<rusqlite::types::Value>],
) -> SqlResult<()> {
    if cols.is_empty() || rows.is_empty() {
        return Ok(());
    }
    let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "INSERT OR IGNORE INTO {table} ({cols}) VALUES ({ph})",
        cols = cols.join(", "),
        ph = placeholders.join(", "),
    );
    let mut stmt = conn.prepare(&sql)?;
    for row in rows {
        stmt.execute(rusqlite::params_from_iter(row.iter()))?;
    }
    Ok(())
}

impl Db {
    #[track_caller]
    pub fn init_tables(&self) -> impl std::future::Future<Output = Result<(), String>> + '_ {
        let __db_caller = std::panic::Location::caller();
        async move {
            // Phase 1: 主库 migration（不含 4 表 DDL）+ 读 4 表全部行 + 读 proxy_log 阶段所需预数据。
            // crash-safe：仅读不 DROP。auto_map 读主库 "group" 表（首次迁移仍在；二次启动空表 →
            // backfill_stats_agg_if_empty 跳过，无回归）。cpa_pids / notif_rows 同 Phase 2 消费。
            let (auto_map, cpa_pids, notif_rows, plat_rows, grp_rows, gp_rows, cpa_rows) = self
                .call_traced(None, __db_caller, |conn| {
                    run_migrations_early(conn)?;
                    run_migrations_late(conn)?;
                    let auto_map = load_auto_from_map(conn)?;
                    let cpa_pids = fetch_cpa_platform_ids(conn);
                    // stats-agg-to-main-db s5：CPA stats_agg_hourly 清理（Mig 046 在 log.db 上的
                    // `DELETE FROM stats_agg_hourly` 因表已迁主库而 no-op，被 `let _ =` 吞）。
                    // 此处主库补做：每次启动幂等 DELETE CPA 残留聚合行（platform_type='"cpa-%'）。
                    // ponytail: 不改 run_migrations_late 签名透传 cpa_pids，避免波及 s1/s2 已锁的
                    // migration 逻辑；post-migration 一次性清理等价、幂等、零回归。
                    cleanup_cpa_stats_agg(conn, &cpa_pids);
                    let notif_rows = migrate_main_notification_out(conn);
                    // 读 4 表全行（保 id）。首次迁移主库仍有存量；二次启动主库已 DROP → 空 TableRows。
                    let plat_rows = read_platform_tables_out(conn, "platform");
                    let grp_rows = read_platform_tables_out(conn, "\"group\"");
                    let gp_rows = read_platform_tables_out(conn, "group_platform");
                    let cpa_rows = read_platform_tables_out(conn, "cli_proxy_provider");
                    // 主库残留 notification 表 DROP（migration 049：notif_rows 已读出待 Phase 2 落 log.db）。
                    let _ = conn.execute("DROP TABLE IF EXISTS notification", []);
                    if !plat_rows.1.is_empty() || !grp_rows.1.is_empty() {
                        tracing::info!(
                            platform_rows = plat_rows.1.len(),
                            group_rows = grp_rows.1.len(),
                            group_platform_rows = gp_rows.1.len(),
                            cli_proxy_rows = cpa_rows.1.len(),
                            "config-db-split: 主库 4 表数据读出待迁 platform.db",
                        );
                    }
                    Ok((auto_map, cpa_pids, notif_rows, plat_rows, grp_rows, gp_rows, cpa_rows))
                })
                .await
                .map_err(|e| e.to_string())?;

            // Phase 2: log.db migration（proxy_log + notification 建表/索引/回填）。
            // stats-agg-to-main-db：stats_agg_hourly 已迁主库（Phase 1 run_migrations_late Mig 051）。
            // 内存库 fallback 下 proxy_log handle = 主内存连接 clone，两阶段同物理库，行为不变。
            self.call_proxy_log_traced(None, __db_caller, move |conn| {
                run_migrations_proxy_log_early(conn)?;
                run_migrations_proxy_log_late(conn, &auto_map, &cpa_pids, &notif_rows)?;
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?;

            // Phase 3: platform.db migration（建 4 表 DDL + 历史 ALTER + INSERT OR IGNORE 保 id 回填）。
            // crash-safe：INSERT OR IGNORE 可任意重放。内存库 fallback 下 platform handle = 主内存连接
            // clone，与 Phase 1 同物理库，4 表数据仍在（Phase 1 未 DROP），INSERT OR IGNORE 全部 id 冲突跳过。
            self.call_platform_traced(None, __db_caller, move |conn| {
                run_migrations_platform_early(conn)?;
                run_migrations_platform_late(conn)?;
                insert_platform_table_rows(conn, "platform", &plat_rows.0, &plat_rows.1)?;
                insert_platform_table_rows(conn, "\"group\"", &grp_rows.0, &grp_rows.1)?;
                insert_platform_table_rows(conn, "group_platform", &gp_rows.0, &gp_rows.1)?;
                insert_platform_table_rows(conn, "cli_proxy_provider", &cpa_rows.0, &cpa_rows.1)?;
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?;

            // Phase 4: 主库 DROP × 4（仅 Phase 3 成功后达）。crash 前未达 Phase 4 → 下次启动 Phase 1
            // 仍能读到 4 表（read 幂等）+ Phase 3 INSERT OR IGNORE 跳过已回填行（id 冲突），无重复无丢失。
            // 内存库 fallback：platform handle = 主内存 conn clone，DROP 会清掉共享物理连接上的 4 表
            // 致后续 call_platform_traced 访问失败 → 内存库跳过 Phase 4（main 与 platform 同 conn，
            // DROP main 等于 DROP platform；文件库才有「main 残留待清 + platform 独立存在」语义）。
            if !self.is_memory() {
                self.call_traced(None, __db_caller, |conn| {
                    let _ = conn.execute("DROP TABLE IF EXISTS platform", []);
                    let _ = conn.execute("DROP TABLE IF EXISTS \"group\"", []);
                    let _ = conn.execute("DROP TABLE IF EXISTS group_platform", []);
                    let _ = conn.execute("DROP TABLE IF EXISTS cli_proxy_provider", []);
                    Ok(())
                })
                .await
                .map_err(|e| e.to_string())?;
            }

            Ok(())
        }
    }
}

/// 内置预设手机号正则（中国大陆 11 位 + 通用国际 E.164 形式）。
/// C2 无内置手机检测器，故此规则走显式 regex（content_filter match_type=regex），
/// 与 C2 的密钥/邮箱内置检测器（content_filter 空 pattern）互补不冲突。
pub(crate) const BUILTIN_PHONE_PATTERN: &str =
    r"(?:\+?\d{1,3}[\s\-]?)?1[3-9]\d{9}|\+\d{6,15}";

/// 单条内置规则种子定义。INSERT 时按 (name, is_builtin=1) 幂等。
pub(crate) struct BuiltinRuleSpec {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) rule_type: &'static str,
    pub(crate) match_type: &'static str,
    /// 空 pattern → content_filter 类复用 C2 内置密钥/邮箱检测器（BUILTIN_SECRET/EMAIL_PATTERN）。
    pub(crate) pattern: &'static str,
    pub(crate) action: &'static str,
    pub(crate) config: &'static str,
    pub(crate) priority: i64,
}

/// 内置预设规则清单（密钥/邮箱/手机脱敏 + 默认 error_rules 分类）。
/// 密钥/邮箱用 content_filter 空 pattern 复用 C2 内置检测器；手机用显式 regex。
/// error_rules 覆盖 research category 集，pattern 用 (?i) 不区分大小写匹配上游错误消息。
pub(crate) fn builtin_rule_specs() -> &'static [BuiltinRuleSpec] {
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
        // ── 日期格式改写防检测（redaction，action=mask，global）──
        // Claude Code system prompt 注入斜杠日期 YYYY/MM/DD（中文区惯用格式），
        // 易被上游针对性检测识别为中文用户 → 封禁风险。改 ISO 横杠 YYYY-MM-DD。
        // 复用 redaction 引擎 regex capture（$1-$2-$3），不改 forward.rs。
        BuiltinRuleSpec {
            name: "内置·日期格式改写防检测",
            description: "将 body 中斜杠日期 YYYY/MM/DD 改写为 ISO 横杠 YYYY-MM-DD，防中文用户针对性检测。",
            rule_type: "redaction",
            match_type: "regex",
            pattern: r"(\d{4})/(\d{1,2})/(\d{1,2})",
            action: "mask",
            config: r#"{"replacement":"$1-$2-$3","fields":["messages","system"]}"#,
            priority: 13,
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
///
/// 薄 wrapper：调 [`seed_builtin_middleware_rules_counted`] 忽略计数，保 migration 015 调用点签名不破。
pub(crate) fn seed_builtin_middleware_rules(conn: &rusqlite::Connection) -> SqlResult<()> {
    let (inserted, _skipped) = seed_builtin_middleware_rules_counted(conn)?;
    if inserted > 0 {
        tracing::info!(inserted, "migration 015: seeded builtin middleware rules");
    }
    Ok(())
}

/// 内置规则 seed 核心：返回 (inserted, skipped) 计数。
///
/// 抽出为独立 pub 入口，供 migration 015（经 [`seed_builtin_middleware_rules`] wrapper）
/// 与 `middleware_import_default_rules` command 共用——禁抄第二份 builtin_rule_specs。
///
/// 语义：按 (name, is_builtin=1) 幂等判定，已存在 → skip（不 update enabled，
/// 尊重用户禁用态）；不存在 → INSERT (enabled=1, is_builtin=1, scope=global)。
pub fn seed_builtin_middleware_rules_counted(
    conn: &rusqlite::Connection,
) -> SqlResult<(u32, u32)> {
    let ts = now();
    let mut inserted = 0u32;
    let mut skipped = 0u32;
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
            skipped += 1;
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
    Ok((inserted, skipped))
}
