//! proxy_log 列目录（issue #56 子票 02，形状 A）：每列的归属（元数据 / 用户侧 / 上游侧）、
//! 大字段位、响应侧只在此一处声明；retention SQL、strip、队列携带、首写位掩码、
//! 列清单（旧 `PROXY_LOG_COLUMNS` + 去重标记列）与 INSERT 占位符全部由它派生。
//!
//! 消费者：aidog_logs（`strip_raw_sides` / 写入 SQL / 首写掩码）、本 crate `maintenance`
//! （retention 两条 UPDATE）、aidog_core `gateway::proxy::log`（队列携带与降级清空）。
//!
//! 不变量（本模块 tests + aidog_logs `test_column_lock` 锁定）：
//! ① 目录列集 == `PRAGMA table_info(proxy_log)` 列集（穷尽，三库真实文件）；
//! ② `Side::User` ∪ `Side::Upstream` == 11 个「原始信息」列；
//! ③ `large` 位下标 0..7 唯一连续，响应侧恰 4 列（终态强制携带 / 覆盖写）；
//! ④ 派生 SQL 运行期文本不变（`prepare_cached` 按 SQL 文本命中，文本必须稳定）。
//!
//! 目录与实表不一致只会在测试里红（编译期无法证明：列由迁移添加）；运行期无新错误路径。

use std::sync::LazyLock;

use crate::models::ProxyLog;

/// 列归属：决定受哪个开关（User → `log_user_request` / Upstream → `log_upstream_request`）
/// 与哪条 retention；Meta 恒保留（token / cost / 状态 / attempts 等）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Meta,
    User,
    Upstream,
}

/// 单列声明（目录列序 = 表定义列序 = 旧 `PROXY_LOG_COLUMNS` 列序）。
pub struct Col {
    pub name: &'static str,
    pub side: Side,
    /// `Some((bit, response_side))` = 8 个大字段之一；bit 即首写位掩码
    /// （`ProxyLogColumns::raw_written_mask`）下标；response_side = true 的列在终态
    /// 强制携带 / 强制覆盖写（空值也写，覆盖中间态部分响应）。
    pub large: Option<(u8, bool)>,
}

/// 两侧去重标记列（migration 20261122-01）：上游侧与用户侧逐字节相同且非空时上游侧存 ''、
/// 标记 1。归 Meta（技术标记，非原始内容）；retention 上游侧 SET/GUARD 附加重置，用户侧
/// retention 先写回再清用户侧（写回分支在 `maintenance` 手写，行为由 aidog_logs
/// `dedup_*_expires_first_*` 测试锁定）。
pub const DEDUP_FLAG_COLS: &[&str] = &["upstream_request_body_dedup", "response_body_dedup"];

/// proxy_log 全列目录：54 列，按表定义顺序。
pub const PROXY_LOG_CATALOG: &[Col] = &[
    Col { name: "id", side: Side::Meta, large: None },
    Col { name: "group_key", side: Side::Meta, large: None },
    Col { name: "model", side: Side::Meta, large: None },
    Col { name: "actual_model", side: Side::Meta, large: None },
    Col { name: "source_protocol", side: Side::Meta, large: None },
    Col { name: "target_protocol", side: Side::Meta, large: None },
    Col { name: "platform_id", side: Side::Meta, large: None },
    Col { name: "request_headers", side: Side::User, large: Some((0, false)) },
    Col { name: "request_body", side: Side::User, large: Some((1, false)) },
    Col { name: "upstream_request_headers", side: Side::Upstream, large: Some((2, false)) },
    Col { name: "upstream_request_body", side: Side::Upstream, large: Some((3, false)) },
    Col { name: "response_body", side: Side::Upstream, large: Some((4, true)) },
    Col { name: "request_url", side: Side::User, large: None },
    Col { name: "upstream_request_url", side: Side::Upstream, large: None },
    Col { name: "upstream_response_headers", side: Side::Upstream, large: Some((5, true)) },
    Col { name: "upstream_status_code", side: Side::Meta, large: None },
    Col { name: "user_response_headers", side: Side::User, large: Some((6, true)) },
    Col { name: "user_response_body", side: Side::User, large: Some((7, true)) },
    Col { name: "status_code", side: Side::Meta, large: None },
    Col { name: "duration_ms", side: Side::Meta, large: None },
    Col { name: "input_tokens", side: Side::Meta, large: None },
    Col { name: "output_tokens", side: Side::Meta, large: None },
    Col { name: "cache_tokens", side: Side::Meta, large: None },
    Col { name: "cache_write_tokens", side: Side::Meta, large: None },
    Col { name: "cache_write_1h_tokens", side: Side::Meta, large: None },
    Col { name: "est_cost", side: Side::Meta, large: None },
    Col { name: "is_stream", side: Side::Meta, large: None },
    Col { name: "attempts", side: Side::Meta, large: None },
    Col { name: "retry_count", side: Side::Meta, large: None },
    Col { name: "blocked_by", side: Side::Meta, large: None },
    Col { name: "blocked_reason", side: Side::Meta, large: None },
    Col { name: "created_at", side: Side::Meta, large: None },
    Col { name: "updated_at", side: Side::Meta, large: None },
    Col { name: "deleted_at", side: Side::Meta, large: None },
    Col { name: "done", side: Side::Meta, large: None },
    Col { name: "field_trace", side: Side::Upstream, large: None },
    Col { name: "body_omitted", side: Side::Meta, large: None },
    Col { name: "routing", side: Side::Meta, large: None },
    Col { name: "client", side: Side::Meta, large: None },
    Col { name: "user_agent", side: Side::Meta, large: None },
    Col { name: "session_key", side: Side::Meta, large: None },
    Col { name: "up_bytes", side: Side::Meta, large: None },
    Col { name: "down_bytes", side: Side::Meta, large: None },
    Col { name: "request_class", side: Side::Meta, large: None },
    Col { name: "agent_type", side: Side::Meta, large: None },
    Col { name: "prompt_id", side: Side::Meta, large: None },
    Col { name: "session_id", side: Side::Meta, large: None },
    Col { name: "agent_id", side: Side::Meta, large: None },
    Col { name: "parent_agent_id", side: Side::Meta, large: None },
    Col { name: "compaction", side: Side::Meta, large: None },
    Col { name: "context_compacted", side: Side::Meta, large: None },
    Col { name: "prev_tool_durations", side: Side::Meta, large: None },
    Col { name: "upstream_request_body_dedup", side: Side::Meta, large: None },
    Col { name: "response_body_dedup", side: Side::Meta, large: None },
];

// ---- 派生：列清单与写入 SQL ------------------------------------------------

/// 全 54 列 CSV（`LazyLock` 派生，运行期文本不变）。替代旧 52 列 `PROXY_LOG_COLUMNS`
/// + 2 列去重标记常量的手写拼接；列序 == 目录序 == 表定义序（tests 锁定逐字相等）。
static COLUMNS_CSV: LazyLock<String> = LazyLock::new(|| {
    PROXY_LOG_CATALOG
        .iter()
        .map(|c| c.name)
        .collect::<Vec<_>>()
        .join(", ")
});

pub fn columns_csv() -> &'static str {
    COLUMNS_CSV.as_str()
}

/// `?1..?54` 占位符串（列数派生）。
static VALUES_PLACEHOLDERS: LazyLock<String> = LazyLock::new(|| {
    (1..=PROXY_LOG_CATALOG.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",")
});

/// 整行写入 SQL（INSERT / INSERT OR REPLACE，列序 = 目录序）。
/// 固定文本（`&'static str`）→ `prepare_cached` 按文本命中 statement cache。
static INSERT_SQL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "INSERT INTO proxy_log ({}) VALUES ({})",
        columns_csv(),
        values_placeholders()
    )
});
static INSERT_OR_REPLACE_SQL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "INSERT OR REPLACE INTO proxy_log ({}) VALUES ({})",
        columns_csv(),
        values_placeholders()
    )
});

pub fn values_placeholders() -> &'static str {
    VALUES_PLACEHOLDERS.as_str()
}

pub fn insert_sql(or_replace: bool) -> &'static str {
    if or_replace {
        INSERT_OR_REPLACE_SQL.as_str()
    } else {
        INSERT_SQL.as_str()
    }
}

// ---- 派生：两侧「原始信息」列集 ----------------------------------------------

static USER_RAW_COLS: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| side_cols(Side::User));
static UPSTREAM_RAW_COLS: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| side_cols(Side::Upstream));

fn side_cols(side: Side) -> Vec<&'static str> {
    PROXY_LOG_CATALOG
        .iter()
        .filter(|c| c.side == side)
        .map(|c| c.name)
        .collect()
}

/// 侧「原始信息」列集（User 5 列 / Upstream 6 列，合计 11）。retention 与 strip 共用。
pub fn raw_cols(side: Side) -> &'static [&'static str] {
    match side {
        Side::User => &USER_RAW_COLS,
        Side::Upstream => &UPSTREAM_RAW_COLS,
        Side::Meta => &[],
    }
}

fn set_empty(cols: &[&str]) -> String {
    cols.iter()
        .map(|c| format!("{c} = ''"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn nonempty_guard(cols: &[&str]) -> String {
    cols.iter()
        .map(|c| format!("{c} != ''"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn dedup_reset_set() -> String {
    DEDUP_FLAG_COLS
        .iter()
        .map(|c| format!("{c} = 0"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn dedup_reset_guard() -> String {
    DEDUP_FLAG_COLS
        .iter()
        .map(|c| format!("{c} != 0"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// retention 清理 UPDATE（proxy_log 部分；`?1` = cutoff）。上游侧 SET/GUARD 附带
/// 去重标记重置（上游正文清空后标记必须归零，否则读回层会用已清空的用户侧内容
/// 错误回填；行为由 aidog_logs `dedup_upstream_side_expires_first_clears_flags` 锁定）。
static USER_RETENTION_SQL: LazyLock<String> = LazyLock::new(|| {
    let cols = raw_cols(Side::User);
    format!(
        "UPDATE proxy_log SET {} WHERE created_at < ?1 AND ({})",
        set_empty(&cols),
        nonempty_guard(&cols)
    )
});
static UPSTREAM_RETENTION_SQL: LazyLock<String> = LazyLock::new(|| {
    let cols = raw_cols(Side::Upstream);
    format!(
        "UPDATE proxy_log SET {}, {} WHERE created_at < ?1 AND ({} OR {})",
        set_empty(&cols),
        dedup_reset_set(),
        nonempty_guard(&cols),
        dedup_reset_guard()
    )
});

pub fn retention_sql(side: Side) -> &'static str {
    match side {
        Side::User => USER_RETENTION_SQL.as_str(),
        Side::Upstream => UPSTREAM_RETENTION_SQL.as_str(),
        Side::Meta => "",
    }
}

// ---- ProxyLog「原始信息」字段访问器 ------------------------------------------

/// 列名 → `ProxyLog` 可变 String 字段（仅 11 个「原始信息」列，与 [`raw_cols`] 同集）。
/// strip / 队列携带 / 降级清空的派生消费者共用；目录新增原始信息列而此处漏加时，
/// `test_column_lock` 的穷尽 / 往返测试红。
pub fn proxy_log_raw_field_mut<'a>(log: &'a mut ProxyLog, col: &str) -> Option<&'a mut String> {
    Some(match col {
        "request_headers" => &mut log.request_headers,
        "request_body" => &mut log.request_body,
        "user_response_headers" => &mut log.user_response_headers,
        "user_response_body" => &mut log.user_response_body,
        "request_url" => &mut log.request_url,
        "upstream_request_headers" => &mut log.upstream_request_headers,
        "upstream_request_body" => &mut log.upstream_request_body,
        "upstream_response_headers" => &mut log.upstream_response_headers,
        "response_body" => &mut log.response_body,
        "field_trace" => &mut log.field_trace,
        "upstream_request_url" => &mut log.upstream_request_url,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 目录列序逐字锁定旧手写列清单（row_to_proxy_log 位置下标依赖此序，漂移即红）。
    #[test]
    fn columns_csv_matches_legacy_handwritten_order() {
        const LEGACY_52: &str = "id, group_key, model, actual_model, source_protocol, target_protocol, platform_id, request_headers, request_body, upstream_request_headers, upstream_request_body, response_body, request_url, upstream_request_url, upstream_response_headers, upstream_status_code, user_response_headers, user_response_body, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, cache_write_tokens, cache_write_1h_tokens, est_cost, is_stream, attempts, retry_count, blocked_by, blocked_reason, created_at, updated_at, deleted_at, done, field_trace, body_omitted, routing, client, user_agent, session_key, up_bytes, down_bytes, request_class, agent_type, prompt_id, session_id, agent_id, parent_agent_id, compaction, context_compacted, prev_tool_durations";
        const LEGACY_DEDUP: &str = "upstream_request_body_dedup, response_body_dedup";
        assert_eq!(columns_csv(), format!("{LEGACY_52}, {LEGACY_DEDUP}"));
    }

    /// 派生 SQL 运行期文本不变（验收 2）：连续两次取到同一 `&'static str` 指针。
    #[test]
    fn derived_sql_text_is_stable_across_calls() {
        assert!(std::ptr::eq(insert_sql(true), insert_sql(true)));
        assert!(std::ptr::eq(insert_sql(false), insert_sql(false)));
        assert!(std::ptr::eq(retention_sql(Side::User), retention_sql(Side::User)));
        assert!(std::ptr::eq(retention_sql(Side::Upstream), retention_sql(Side::Upstream)));
        assert!(std::ptr::eq(columns_csv(), columns_csv()));
        assert!(std::ptr::eq(values_placeholders(), values_placeholders()));
        assert_ne!(insert_sql(true), insert_sql(false));
        assert!(insert_sql(true).starts_with("INSERT OR REPLACE INTO proxy_log ("));
        assert!(insert_sql(false).starts_with("INSERT INTO proxy_log ("));
        assert!(insert_sql(true).ends_with(&format!("VALUES ({})", values_placeholders())));
    }

    /// 占位符数 = 目录列数 = 54，编号连续。
    #[test]
    fn placeholders_match_catalog_len() {
        let p = values_placeholders();
        assert_eq!(p, (1..=54).map(|i| format!("?{i}")).collect::<Vec<_>>().join(","));
        assert_eq!(PROXY_LOG_CATALOG.len(), 54);
    }

    /// 大字段位 0..7 唯一连续；响应侧恰 4 列且与终态强制携带清单一致（不变量 ③）。
    #[test]
    fn large_bits_contiguous_and_response_side_fixed() {
        let mut bits = Vec::new();
        let mut response = Vec::new();
        for c in PROXY_LOG_CATALOG.iter().filter_map(|c| c.large.map(|l| (c.name, l))) {
            let (name, (bit, resp)) = (c.0, c.1);
            bits.push(bit);
            if resp {
                response.push(name);
            }
        }
        bits.sort_unstable();
        assert_eq!(bits, (0..8).collect::<Vec<_>>(), "large 位必须 0..7 唯一连续");
        assert_eq!(
            response,
            [
                "response_body",
                "upstream_response_headers",
                "user_response_headers",
                "user_response_body"
            ],
            "响应侧大字段（终态强制携带 / 覆盖写）恰 4 列"
        );
    }

    /// User ∪ Upstream == 11「原始信息」列；去重标记列在目录内且归 Meta（不变量 ②）。
    #[test]
    fn raw_sides_are_eleven_and_dedup_flags_are_meta() {
        assert_eq!(raw_cols(Side::User).len() + raw_cols(Side::Upstream).len(), 11);
        for f in DEDUP_FLAG_COLS {
            let col = PROXY_LOG_CATALOG.iter().find(|c| c.name == *f).expect("dedup flag in catalog");
            assert_eq!(col.side, Side::Meta, "去重标记是技术标记，归 Meta");
        }
        assert!(raw_cols(Side::Meta).is_empty());
    }

    /// retention SQL 文本锁定：与原手写 UPDATE 同语义（列集 + 去重重置），文本自此由本测试锁定。
    #[test]
    fn retention_sql_text_locked() {
        assert_eq!(
            retention_sql(Side::User),
            "UPDATE proxy_log SET request_headers = '', request_body = '', request_url = '', user_response_headers = '', user_response_body = '' WHERE created_at < ?1 AND (request_headers != '' OR request_body != '' OR request_url != '' OR user_response_headers != '' OR user_response_body != '')"
        );
        assert_eq!(
            retention_sql(Side::Upstream),
            "UPDATE proxy_log SET upstream_request_headers = '', upstream_request_body = '', response_body = '', upstream_request_url = '', upstream_response_headers = '', field_trace = '', upstream_request_body_dedup = 0, response_body_dedup = 0 WHERE created_at < ?1 AND (upstream_request_headers != '' OR upstream_request_body != '' OR response_body != '' OR upstream_request_url != '' OR upstream_response_headers != '' OR field_trace != '' OR upstream_request_body_dedup != 0 OR response_body_dedup != 0)"
        );
    }
}
