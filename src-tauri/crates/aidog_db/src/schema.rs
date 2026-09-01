use rusqlite::Connection;
use crate::schema_early::*;
use crate::schema_late::*;
use crate::{Db, now, load_auto_from_map};
use rusqlite::{params, OptionalExtension, Result as SqlResult};

/// 主库迁出的 notification 行（migration 20260727-20, 原 049）。由 init_tables 在主库闭包内读出 + DROP 主库
/// 残留表后，传入 proxy_log_late 写入 log.db.notification。空 Vec = 主库表已不存在（已迁移过）。
type NotifRow = (String, String, String, i64);

/// 迁移期间读出的 4 表行数据（config-db-split）。由 init_tables Phase 1 主库闭包读出（保 id 全列），
/// Phase 3 platform.db 闭包 INSERT OR IGNORE 写入。列名 + 值均用 `rusqlite::types::Value` 动态承载，
/// 避免对 4 表 80+ 列各自建 tuple 类型（列漂移时自动跟随 SELECT *）。
type TableRows = (Vec<String>, Vec<Vec<rusqlite::types::Value>>);

/// 2026-08-28 registry 下架的 20 家 newapi 类中转站（用户决策：「newapi 的那种第三方站，
/// 都不要作为独立的平台存在」；registry preset 已删，Protocol 枚举与 adapter 有意保留——
/// DB 里填过 key 的存量条目照常工作，wire 转换不受下架影响）。
const DELISTED_PLATFORM_CODES: &[&str] = &[
    "aicodemirror", "aigocode", "ccsub", "relaxycode", "ctok", "cubence", "rightcode",
    "micu", "lemondata", "apikeyfun", "claudeapi", "claudecn", "eflowcode", "packycode",
    "runapi", "sudocode", "sssaicode", "pateway", "dmxapi", "cherryin",
];

/// 下架平台启动期清理（platform.db 侧，Phase 3 回填后跑，幂等）：
/// `api_key` 空的行 = 用户从未配置（只是点过预设），软删 + 清 group_platform 成员关系
/// （与 delete_platform 同语义，拆两步而非单事务——deleted_at 置位后不再命中，重放安全）。
/// 填过 key 的行不动：那是在用的平台，删了直接断用户请求。
fn cleanup_delisted_platform_rows(conn: &rusqlite::Connection) {
    let mut removed = 0usize;
    for code in DELISTED_PLATFORM_CODES {
        // platform_type 两种历史形态都认（from_db_str 同口径）：带引号 JSON 串 + 裸 wire 名。
        let quoted = format!("\"{code}\"");
        let ids: Vec<i64> = conn
            .prepare(
                "SELECT id FROM platform
                 WHERE deleted_at = 0 AND TRIM(api_key) = ''
                   AND platform_type IN (?1, ?2)",
            )
            .and_then(|mut s| {
                s.query_map(params![code, quoted], |r| r.get::<_, i64>(0))
                    .map(|iter| iter.filter_map(Result::ok).collect())
            })
            .unwrap_or_default();
        for id in ids {
            if conn
                .execute(
                    "UPDATE platform SET deleted_at = ?1 WHERE id = ?2 AND deleted_at = 0",
                    params![now(), id],
                )
                .map(|n| n > 0)
                .unwrap_or(false)
            {
                conn.execute("DELETE FROM group_platform WHERE platform_id = ?1", params![id])
                    .unwrap_or_default();
                removed += 1;
            }
        }
    }
    if removed > 0 {
        tracing::info!(removed, "registry 下架平台清理：api_key 空的存量行已软删（填过 key 的行保留）");
    }
}

/// 主库镜像表（platform_preset / model_entry）对应行无条件清：镜像数据无用户态，registry
/// 真值源已删，留着只会在模型维度列表里继续展示下架平台。幂等：无匹配行 0 影响。
/// Phase 1 主库闭包在 run_migrations_late 之后跑（两表 DDL 在 20260826-01 内）。
fn cleanup_delisted_registry_mirror_rows(conn: &rusqlite::Connection) {
    let mut removed = 0usize;
    for code in DELISTED_PLATFORM_CODES {
        let _ = conn
            .execute("DELETE FROM platform_preset WHERE code = ?1", params![code])
            .map(|n| removed += n);
        let _ = conn
            .execute("DELETE FROM model_entry WHERE platform_code = ?1", params![code])
            .map(|n| removed += n);
    }
    if removed > 0 {
        tracing::info!(removed, "registry 下架平台镜像行清理（platform_preset / model_entry）");
    }
}

/// Migration 20260727-20 (原 049): `notification` 表归属 log.db。主库残留表读出全部行（**不 DROP**，由 Phase 1
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

/// 查主库 platform 表中 CPA 平台 ID（原 migration 046 清理用）。跨库不能子查询，由 init_tables
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

/// CPA（CLIProxyAPI）平台聚合行清理 —— stats-agg-to-main-db s5 补原 046 在主库的缺失。
///
/// 背景：原 046 的 CPA 清理在 `run_migrations_proxy_log_late`（log.db 写连接）内跑，含
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

/// 建表/迁移编排（backfill 由 caller 注入：aidog_core 传 aidog_stats::backfill_stats_agg_if_empty，
/// 避免 aidog_db → aidog_stats 循环依赖）。
pub type BackfillFn = std::sync::Arc<dyn Fn(&Connection, &std::collections::HashMap<String, i64>) -> rusqlite::Result<()> + Send + Sync>;

#[track_caller]
pub fn init_tables_raw(
    db: &Db,
    backfill: BackfillFn,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
        let __db_caller = std::panic::Location::caller();
        async move {
            // Phase 1: 主库 migration（不含 4 表 DDL）+ 读 4 表全部行 + 读 proxy_log 阶段所需预数据。
            // crash-safe：仅读不 DROP。auto_map 读主库 "group" 表（首次迁移仍在；二次启动空表 →
            // backfill_stats_agg_if_empty 跳过，无回归）。cpa_pids / notif_rows 同 Phase 2 消费。
            let (auto_map, cpa_pids, notif_rows, plat_rows, grp_rows, gp_rows, cpa_rows) = db
                .call_traced(None, __db_caller, {
                    let backfill = backfill.clone();
                    move |conn| {
                    run_migrations_early(conn)?;
                    run_migrations_late(conn, backfill.clone())?;
                    cleanup_delisted_registry_mirror_rows(conn);
                    let auto_map = load_auto_from_map(conn)?;
                    let cpa_pids = fetch_cpa_platform_ids(conn);
                    // stats-agg-to-main-db s5：CPA stats_agg_hourly 清理（原 Mig 046 在 log.db 上的
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
                    // 主库残留 notification 表 DROP（20260727-20，原 049：notif_rows 已读出待 Phase 2 落 log.db）。
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
                    }
                })
                .await
                .map_err(|e| e.to_string())?;

            // Phase 2: log.db migration（proxy_log + notification 建表/索引/回填）。
            // stats-agg-to-main-db：stats_agg_hourly 已迁主库（Phase 1 run_migrations_late 20260727-16）。
            // 内存库 fallback 下 proxy_log handle = 主内存连接 clone，两阶段同物理库，行为不变。
            db.call_proxy_log_traced(None, __db_caller, move |conn| {
                run_migrations_proxy_log_early(conn)?;
                run_migrations_proxy_log_late(conn, &auto_map, &cpa_pids, &notif_rows)?;
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?;

            // Phase 3: platform.db migration（建 4 表 DDL + 历史 ALTER + INSERT OR IGNORE 保 id 回填）。
            // crash-safe：INSERT OR IGNORE 可任意重放。内存库 fallback 下 platform handle = 主内存连接
            // clone，与 Phase 1 同物理库，4 表数据仍在（Phase 1 未 DROP），INSERT OR IGNORE 全部 id 冲突跳过。
            db.call_platform_traced(None, __db_caller, move |conn| {
                run_migrations_platform_early(conn)?;
                run_migrations_platform_late(conn)?;
                insert_platform_table_rows(conn, "platform", &plat_rows.0, &plat_rows.1)?;
                insert_platform_table_rows(conn, "\"group\"", &grp_rows.0, &grp_rows.1)?;
                insert_platform_table_rows(conn, "group_platform", &gp_rows.0, &gp_rows.1)?;
                insert_platform_table_rows(conn, "cli_proxy_provider", &cpa_rows.0, &cpa_rows.1)?;
                // 回填之后再清：首次迁移（主库存量 → platform.db）若先清，回填会把待删行原样搬回。
                cleanup_delisted_platform_rows(conn);
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?;

            // Phase 4: 主库 DROP × 4（仅 Phase 3 成功后达）。crash 前未达 Phase 4 → 下次启动 Phase 1
            // 仍能读到 4 表（read 幂等）+ Phase 3 INSERT OR IGNORE 跳过已回填行（id 冲突），无重复无丢失。
            // 内存库 fallback：platform handle = 主内存 conn clone，DROP 会清掉共享物理连接上的 4 表
            // 致后续 call_platform_traced 访问失败 → 内存库跳过 Phase 4（main 与 platform 同 conn，
            // DROP main 等于 DROP platform；文件库才有「main 残留待清 + platform 独立存在」语义）。
            if !db.is_memory() {
                db.call_traced(None, __db_caller, |conn| {
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

/// 内置规则正则集（票 03：硬编码检测器迁为内置规则，单一真值于此）。
///
/// 密钥模式分三层（覆盖任意平台的 key 形态，不止 sk-/ghp_/AKIA/AIza/xox）：
/// ① 显式厂商形态：`sk-` 系（含 `sk-ant-` / `sk-or-` / `sk-ws-H.xxx.yyy` 这类带 `.`/`-` 分节的）、
///    GitHub / GitLab / Slack / AWS / Google API key、Google OAuth（`ya29.` / `AQ.`）、JWT。
/// ② 短前缀厂商 token：`bfl_` / `gsk_` / `hf_` / `r8_` / `nvapi-` / `ark-` / `tp-` 等，前缀已足够独特，
///    尾串仅要求长度 ≥ 8。
/// ③ 通用兜底（未知厂商）：`<前缀><_|-><尾串>`，尾串**必须含数字**且足够长——
///    通用词前缀（key/token/auth/sess/cred/bearer）尾串 ≥ 12，任意前缀尾串 ≥ 16。
///    「必须含数字」用两段式写法表达（Rust regex 无 lookahead）：数字左边或右边至少有 N 个 token 字符。
///    这条约束是防误伤的关键：`token_expiration_check` / `run_migrations_proxy_log_late` /
///    `some-file-name-that-is-long.tsx` 这类普通标识符不含数字，不会被当密钥抹掉。
pub const BUILTIN_SECRET_PATTERN: &str = r"(?i)\b(?:sk-[A-Za-z0-9._\-]{16,}|(?:gh[pousr]_|github_pat_)[A-Za-z0-9_]{20,}|glpat-[A-Za-z0-9._\-]{16,}|xox[baprs]-[A-Za-z0-9\-]{10,}|(?:AKIA|ASIA)[0-9A-Z]{16}|AIza[A-Za-z0-9_\-]{20,}|(?:ya29|AQ)\.[A-Za-z0-9._\-]{16,}|eyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}|(?:bfl|gsk|hf|fal|r8|nvapi|pplx|tvly|csk|ark|tp|xai|dop_v1|rk|pk|ak)[_\-][A-Za-z0-9._\-]{8,}|(?:key|token|auth|sess|session|cred|bearer)[_\-](?:[A-Za-z0-9._\-]{11,}[0-9][A-Za-z0-9._\-]*|[A-Za-z0-9._\-]*[0-9][A-Za-z0-9._\-]{11,})|[A-Za-z][A-Za-z0-9]{1,11}[_\-](?:[A-Za-z0-9._\-]{15,}[0-9][A-Za-z0-9._\-]*|[A-Za-z0-9._\-]*[0-9][A-Za-z0-9._\-]{15,}))";
pub const BUILTIN_EMAIL_PATTERN: &str = r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}";
pub const BUILTIN_PHONE_PATTERN: &str =
    r"(?:\+?\d{1,3}[\s\-]?)?1[3-9]\d{9}";
/// DB/Redis 等连接串中的明文凭据（scheme://user:pass@host 形式）。
pub const BUILTIN_DB_URI_PATTERN: &str = r#"(?i)\b(?:mysql|postgres(?:ql)?|redis|mssql|mongodb(?:\+srv)?|amqp)://[^\s@/:"']*(?::[^\s/@'"]+)?@"#;
/// 环境变量/配置文件式明文密钥（password/secret/api_key 等显式 key = value 形式）。
pub const BUILTIN_KEY_VALUE_PATTERN: &str = r#"(?i)\b(password|passwd|pwd|secret|api_key|apikey|access_token)\s*["']?\s*[=:]\s*["']?[A-Za-z0-9_\-./+=]{8,}"#;

/// 单条内置规则种子定义（统一引擎模型：conditions/actions 为 ConditionNode/ActionStep JSON）。
pub struct BuiltinRuleSpec {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) conditions: &'static str,
    pub(crate) actions: &'static str,
    pub(crate) priority: i64,
}

/// 密钥脱敏规则的 conditions JSON：pattern 由 [`BUILTIN_SECRET_PATTERN`] 单点生成，
/// 禁在此再抄一份正则字面量（抄第二份必漂移；schema_late 迁移路径同样引用该常量）。
static SECRET_RULE_CONDITIONS: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    serde_json::json!({
        "kind": "any",
        "children": [{
            "kind": "leaf",
            "target": "request_body",
            "field": "",
            "match_type": "regex",
            "pattern": BUILTIN_SECRET_PATTERN,
        }],
    })
    .to_string()
});

/// 内置预设规则清单（票 03：密钥/邮箱/手机/DB-Redis 凭据脱敏 + 日期改写 + 默认错误分类）。
/// 全部显式 regex 条件（无空 pattern 隐藏兜底，ADR 0003）；error 分类按 response_body 命中。
pub fn builtin_rule_specs() -> &'static [BuiltinRuleSpec] {
    static SPECS: std::sync::LazyLock<Vec<BuiltinRuleSpec>> = std::sync::LazyLock::new(|| vec![
        BuiltinRuleSpec {
            name: "内置·密钥脱敏",
            description: "脱敏各平台 API 密钥（sk- 系 / ghp_ / AKIA / AIza / bfl_ / key_ / AQ. 等，含未知厂商的长随机 token）。",
            conditions: SECRET_RULE_CONDITIONS.as_str(),
            actions: r#"[{"kind":"mask","params":{"replacement":"****","fields":["messages","system"]}}]"#,
            priority: 10,
        },
        BuiltinRuleSpec {
            name: "内置·邮箱脱敏",
            description: "脱敏邮箱地址。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"request_body","field":"","match_type":"regex","pattern":"[a-zA-Z0-9._%+\\-]+@[a-zA-Z0-9.\\-]+\\.[a-zA-Z]{2,}"}]}"#,
            actions: r#"[{"kind":"mask","params":{"replacement":"****","fields":["messages","system"]}}]"#,
            priority: 11,
        },
        BuiltinRuleSpec {
            name: "内置·手机号脱敏",
            description: "脱敏手机号（中国大陆 11 位 + E.164 国际形式）。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"request_body","field":"","match_type":"regex","pattern":"(?:\\+?\\d{1,3}[\\s\\-]?)?1[3-9]\\d{9}|\\+\\d{6,15}"}]}"#,
            actions: r#"[{"kind":"mask","params":{"replacement":"****","fields":["messages","system"]}}]"#,
            priority: 12,
        },
        BuiltinRuleSpec {
            name: "内置·数据库/Redis 凭据脱敏",
            description: "脱敏连接串中的明文凭据（mysql/postgres/redis/mongodb 等 scheme://user:pass@host）。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"request_body","field":"","match_type":"regex","pattern":"(?i)\\b(?:mysql|postgres(?:ql)?|redis|mssql|mongodb(?:\\+srv)?|amqp)://[^\\s@/:\"']+(?::[^\\s/'\"]+)?@"}]}"#,
            actions: r#"[{"kind":"mask","params":{"replacement":"****","fields":["messages","system"]}}]"#,
            priority: 13,
        },
        BuiltinRuleSpec {
            name: "内置·配置式密钥脱敏",
            description: "脱敏 password/secret/api_key 等显式 key=value 形式的明文密钥。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"request_body","field":"","match_type":"regex","pattern":"(?i)\\b(password|passwd|pwd|secret|api_key|apikey|access_token)\\s*[=:]\\s*[\"']?[A-Za-z0-9_\\-./+=]{8,}"}]}"#,
            actions: r#"[{"kind":"mask","params":{"replacement":"****","fields":["messages","system"]}}]"#,
            priority: 14,
        },
        // ── 日期格式改写防检测（request_body 改写，regex capture $1-$2-$3）──
        // Claude Code system prompt 注入斜杠日期 YYYY/MM/DD（中文区惯用格式），
        // 易被上游针对性检测识别为中文用户 → 封禁风险。改 ISO 横杠 YYYY-MM-DD。
        BuiltinRuleSpec {
            name: "内置·日期格式改写防检测",
            description: "将请求文本中斜杠日期 YYYY/MM/DD 改写为 ISO 横杠 YYYY-MM-DD，防中文用户针对性检测。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"request_body","field":"","match_type":"regex","pattern":"(\\d{4})/(\\d{1,2})/(\\d{1,2})"}]}"#,
            actions: r#"[{"kind":"mask","params":{"replacement":"$1-$2-$3","fields":["messages","system"]}}]"#,
            priority: 15,
        },
        // ── 默认错误分类（response_body 条件 + classify 动作，retryable=false）──
        BuiltinRuleSpec {
            name: "内置·上下文超限",
            description: "上游报上下文/prompt 过长 → prompt_limit（不可重试，换候选无益）。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"response_body","field":"","match_type":"regex","pattern":"(?i)(context length|context window|maximum context|prompt is too long|too many tokens|reduce the length|maximum.*tokens)"}]}"#,
            actions: r#"[{"kind":"classify","params":{"category":"prompt_limit","retryable":false}}]"#,
            priority: 20,
        },
        BuiltinRuleSpec {
            name: "内置·内容审查拦截",
            description: "上游内容安全过滤拦截 → content_filter（不可重试）。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"response_body","field":"","match_type":"regex","pattern":"(?i)(content filter|content_filter|content policy|safety|flagged|moderation|responsible_ai_policy)"}]}"#,
            actions: r#"[{"kind":"classify","params":{"category":"content_filter","retryable":false}}]"#,
            priority: 21,
        },
        BuiltinRuleSpec {
            name: "内置·PDF/文件超限",
            description: "上游报 PDF/文件页数或大小超限 → pdf_limit（不可重试）。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"response_body","field":"","match_type":"regex","pattern":"(?i)(pdf.*(too many pages|exceed|too large|limit)|too many pages|file.*too large|maximum.*pages)"}]}"#,
            actions: r#"[{"kind":"classify","params":{"category":"pdf_limit","retryable":false}}]"#,
            priority: 22,
        },
        BuiltinRuleSpec {
            name: "内置·思考链错误",
            description: "上游报 thinking/reasoning 字段错误 → thinking_error（不可重试）。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"response_body","field":"","match_type":"regex","pattern":"(?i)(thinking|reasoning).*(not (supported|allowed|enabled)|invalid|must be|required|error)"}]}"#,
            actions: r#"[{"kind":"classify","params":{"category":"thinking_error","retryable":false}}]"#,
            priority: 23,
        },
        BuiltinRuleSpec {
            name: "内置·参数错误",
            description: "上游报参数非法 → parameter_error（不可重试，换候选同样会失败）。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"response_body","field":"","match_type":"regex","pattern":"(?i)(invalid.*parameter|unsupported parameter|unknown parameter|parameter.*(invalid|not supported)|unexpected.*field)"}]}"#,
            actions: r#"[{"kind":"classify","params":{"category":"parameter_error","retryable":false}}]"#,
            priority: 24,
        },
        BuiltinRuleSpec {
            name: "内置·非法请求",
            description: "上游报 invalid_request → invalid_request（不可重试）。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"response_body","field":"","match_type":"regex","pattern":"(?i)(invalid_request_error|invalid request|bad request|malformed)"}]}"#,
            actions: r#"[{"kind":"classify","params":{"category":"invalid_request","retryable":false}}]"#,
            priority: 25,
        },
        BuiltinRuleSpec {
            name: "内置·缓存超限",
            description: "上游报 prompt cache 写入/数量超限 → cache_limit（不可重试）。",
            conditions: r#"{"kind":"any","children":[{"kind":"leaf","target":"response_body","field":"","match_type":"regex","pattern":"(?i)(cache.*(limit|exceed|too many)|prompt cache|cache_control.*(limit|exceed|maximum))"}]}"#,
            actions: r#"[{"kind":"classify","params":{"category":"cache_limit","retryable":false}}]"#,
            priority: 26,
        },
    ]);
    &SPECS
}

/// 首启/升级 seed 内置预设中间件规则。幂等：按 (name, is_builtin=1) 判定；
/// 已存在 → **强制覆盖内容**（description/conditions/actions/priority），保留 enabled
/// （用户禁用态不被升级重置）；不存在 → INSERT (enabled=1, is_builtin=1)。
/// 在 [`Db::init_tables`] migration 末尾、同一 connection 闭包内同步调用。
pub fn seed_builtin_middleware_rules(conn: &rusqlite::Connection) -> SqlResult<()> {
    let (inserted, updated) = seed_builtin_middleware_rules_counted(conn)?;
    if inserted + updated > 0 {
        tracing::info!(inserted, updated, "migration: seeded builtin middleware rules");
    }
    Ok(())
}

/// 内置规则 seed 核心：返回 (inserted, updated) 计数。
///
/// 按 (name, is_builtin=1) 幂等判定，已存在 → UPDATE 内容（保留 enabled/failed=0 重置）；
/// 不存在 → INSERT。升级时内置规格变化（如票 03 新增 DB/Redis 脱敏）靠此路径落地。
pub fn seed_builtin_middleware_rules_counted(
    conn: &rusqlite::Connection,
) -> SqlResult<(u32, u32)> {
    // 守卫：旧 8 类 schema（无 conditions 列，升级库 early 阶段）→ 跳过，
    // 由 run_migrations_late 20260824-02 完成表迁移后重新 seed。
    let has_conditions = conn
        .prepare("PRAGMA table_info(middleware_rule)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(Result::ok)
        .any(|c| c == "conditions");
    if !has_conditions {
        return Ok((0, 0));
    }
    // failed 内置行（旧模型残留翻译失败或 conditions JSON 损坏）自动清除：内置内容由
    // 本 seed 全权管理，失效即删 + 重新 INSERT 干净版本，不留给用户手动清理。
    // 失效判定与读时对齐（DB 列或 JSON 解析失败）。
    {
        let stale: Vec<i64> = conn
            .prepare("SELECT id, failed, conditions FROM middleware_rule WHERE is_builtin = 1")?
            .query_map([], |r| {
                let id: i64 = r.get(0)?;
                let failed: i64 = r.get(1)?;
                let cond: String = r.get(2)?;
                Ok(if crate::is_effective_failed(failed, &cond) { Some(id) } else { None })
            })?
            .filter_map(Result::ok)
            .flatten()
            .collect();
        for id in stale {
            conn.execute("DELETE FROM middleware_rule WHERE id = ?1", params![id])?;
        }
    }
    let ts = now();
    let mut inserted = 0u32;
    let mut updated = 0u32;
    for spec in builtin_rule_specs() {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM middleware_rule WHERE name = ?1 AND is_builtin = 1 LIMIT 1",
                params![spec.name],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = exists {
            conn.execute(
                "UPDATE middleware_rule SET
                   description = ?2, conditions = ?3, actions = ?4, priority = ?5,
                   failed = 0, updated_at = ?6
                 WHERE id = ?1",
                params![id, spec.description, spec.conditions, spec.actions, spec.priority, ts],
            )?;
            updated += 1;
            continue;
        }
        conn.execute(
            "INSERT INTO middleware_rule
               (name, description, conditions, actions, applies_to, priority, enabled, is_builtin, failed, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, '{}', ?5, 1, 1, 0, ?6, ?6)",
            params![
                spec.name,
                spec.description,
                spec.conditions,
                spec.actions,
                spec.priority,
                ts,
            ],
        )?;
        inserted += 1;
    }
    Ok((inserted, updated))
}


#[cfg(test)]
#[path = "test_delisted_cleanup.rs"]
mod test_delisted_cleanup;
