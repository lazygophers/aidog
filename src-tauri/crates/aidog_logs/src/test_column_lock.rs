#![cfg(test)]
//! issue #56 子票 01：proxy_log 列集行为锁定测试（三库真实文件）。
//!
//! 锁定测试，非先红：全部断言在当前生产代码上应直接绿（票 02 列目录重构前后都须绿）。
//! 锁三条不变量：① 实表每列已分类（穷尽）；② 逐列写后重开读回（捕获 `row_to_proxy_log`
//! 位置下标漂移）；③ CONNECT 行 up/down 字节经列表接口读回非 0。
//! 三库真实文件库走 [`three_file_db`]（db-pool-routing 规则：split 表禁内存库）。

use crate::*;
use aidog_db::models::*;
use aidog_db::now;
use aidog_db::test_support::*;

/// 两侧「原始信息」开关全开（与 `test_proxy_log.rs::enable_raw_logging` 同形）。
async fn enable_raw_logging(db: &aidog_db::Db) {
    let settings = ProxyLogSettings {
        log_user_request: true,
        log_upstream_request: true,
        ..Default::default()
    };
    aidog_db::set_setting(
        db,
        SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::to_value(settings).unwrap(),
        },
    )
    .await
    .unwrap();
}

/// 用户侧「原始信息」5 列（受 `log_user_request`；出处 `strip_raw_sides` doc）。
const USER_SIDE: &[&str] = &[
    "request_headers",
    "request_body",
    "user_response_headers",
    "user_response_body",
    "request_url",
];

/// 上游侧「原始信息」6 列（受 `log_upstream_request`，含 `field_trace` /
/// `upstream_request_url`）。
const UPSTREAM_SIDE: &[&str] = &[
    "upstream_request_headers",
    "upstream_request_body",
    "upstream_response_headers",
    "response_body",
    "field_trace",
    "upstream_request_url",
];

/// DB 列存在但 `ProxyLog` 结构不承载（读走 `ProxyLogSummary`；`row_to_proxy_log`
/// 故意跳过下标 41/42）。
const BYTES_ONLY: &[&str] = &["up_bytes", "down_bytes"];

/// 元数据列全集（不属于任何「原始信息」侧；含 `attempts` / `routing` —— 票 03 拍板：
/// `attempts[].error` 归元数据保留，不受开关与 retention 控制）。
const META: &[&str] = &[
    "id",
    "group_key",
    "model",
    "actual_model",
    "source_protocol",
    "target_protocol",
    "platform_id",
    "upstream_status_code",
    "status_code",
    "duration_ms",
    "input_tokens",
    "output_tokens",
    "cache_tokens",
    "cache_write_tokens",
    "cache_write_1h_tokens",
    "est_cost",
    "is_stream",
    "attempts",
    "retry_count",
    "blocked_by",
    "blocked_reason",
    "created_at",
    "updated_at",
    "deleted_at",
    "done",
    "body_omitted",
    "routing",
    "client",
    "user_agent",
    "session_key",
    "request_class",
    "agent_type",
    "prompt_id",
    "session_id",
    "agent_id",
    "parent_agent_id",
    "compaction",
    "context_compacted",
    "prev_tool_durations",
];

/// 穷尽分类（验收 3，issue #56 票 02 改为直接比对目录）：`PRAGMA table_info(proxy_log)`
/// 列集 == `proxy_log_catalog::PROXY_LOG_CATALOG` 列集；目录分类与票 01 手写清单交叉核对。
/// 收集式：新增列未入目录时一次列出全部差异再失败，不逐条 panic。
#[tokio::test]
async fn every_proxy_log_column_is_classified() {
    use aidog_db::proxy_log_catalog::{PROXY_LOG_CATALOG, Side};
    let (db, _dir) = three_file_db().await;
    let cols: Vec<String> = db
        .call_proxy_log_traced(None, std::panic::Location::caller(), |conn| {
            let mut stmt = conn.prepare("PRAGMA table_info(proxy_log)")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
            rows.collect::<rusqlite::Result<_>>()
        })
        .await
        .unwrap();

    let catalog_names: Vec<&str> = PROXY_LOG_CATALOG.iter().map(|c| c.name).collect();
    let unclassified: Vec<&str> = cols
        .iter()
        .map(|c| c.as_str())
        .filter(|c| !catalog_names.contains(c))
        .collect();
    let stale: Vec<&str> = catalog_names
        .iter()
        .filter(|c| !cols.iter().any(|t| t == **c))
        .copied()
        .collect();
    assert!(
        unclassified.is_empty() && stale.is_empty(),
        "proxy_log 目录与实表不一致（新增列必须先进 proxy_log_catalog，否则 retention / strip 会漏）：\n\
         实表有目录没有: {unclassified:?}\n\
         目录有实表没有: {stale:?}\n\
         实表列: {cols:?}"
    );
    assert_eq!(PROXY_LOG_CATALOG.len(), cols.len(), "目录列数 == 实表列数");

    // 目录分类与票 01 手写清单交叉核对（手写清单保留为语义基准）。
    let side_of = |n: &str| {
        PROXY_LOG_CATALOG
            .iter()
            .find(|c| c.name == n)
            .map(|c| c.side)
    };
    for n in USER_SIDE {
        assert_eq!(side_of(n), Some(Side::User), "{n} 目录分类应为 User");
    }
    for n in UPSTREAM_SIDE {
        assert_eq!(side_of(n), Some(Side::Upstream), "{n} 目录分类应为 Upstream");
    }
    for n in META.iter().chain(BYTES_ONLY) {
        assert_eq!(side_of(n), Some(Side::Meta), "{n} 目录分类应为 Meta");
    }
    // 两侧「原始信息」= 11 列（CLAUDE.md「Proxy 日志」节的机械断言）。
    assert_eq!(USER_SIDE.len() + UPSTREAM_SIDE.len(), 11);
    assert_eq!(
        aidog_db::proxy_log_catalog::raw_cols(Side::User).len()
            + aidog_db::proxy_log_catalog::raw_cols(Side::Upstream).len(),
        11
    );
}

/// 逐列往返（验收 3）：每个字段取互不相同值的 `ProxyLog`，经
/// `insert_proxy_log_columns`（两侧开关全开）写入，重开 DB 后 `get_proxy_log`
/// 读回逐字段相等——捕获 `row_to_proxy_log` 位置下标漂移（40→43 跳位）。
#[tokio::test]
async fn roundtrip_preserves_every_column() {
    let (db, dir) = three_file_db().await;
    enable_raw_logging(&db).await;
    let now_ms = now();
    let mut log = sample_log("rt-col", "grp", now_ms);
    // String 字段逐字段互异、数值字段非 0 且互异（同类型列互换即红）。
    log.group_key = "rt-group".into();
    log.model = "rt-model".into();
    log.actual_model = "rt-actual-model".into();
    log.source_protocol = "rt-src-proto".into();
    log.target_protocol = "rt-tgt-proto".into();
    log.platform_id = 9;
    log.request_headers = "rt-request-headers".into();
    log.request_body = "rt-request-body".into();
    log.upstream_request_headers = "rt-upstream-request-headers".into();
    log.upstream_request_body = "rt-upstream-request-body".into();
    log.response_body = "rt-response-body".into();
    log.request_url = "rt-request-url".into();
    log.upstream_request_url = "rt-upstream-request-url".into();
    log.upstream_response_headers = "rt-upstream-response-headers".into();
    log.upstream_status_code = 201;
    log.user_response_headers = "rt-user-response-headers".into();
    log.user_response_body = "rt-user-response-body".into();
    log.status_code = 200;
    log.duration_ms = 123;
    log.input_tokens = 11;
    log.output_tokens = 22;
    log.cache_tokens = 33;
    log.cache_write_tokens = 44;
    log.cache_write_1h_tokens = 55;
    log.est_cost = 0.0123;
    log.is_stream = true;
    log.attempts = vec![ProxyAttempt {
        chain_stop: None,
        platform_id: 7,
        platform_name: "rt-attempt-platform".into(),
        status_code: 499,
        error: "rt-attempt-error".into(),
        duration_ms: 88,
        ts: now_ms,
        model: Some("rt-attempt-model".into()),
        ..Default::default()
    }];
    log.retry_count = 2;
    log.blocked_by = "rt-blocked-by".into();
    log.blocked_reason = "rt-blocked-reason".into();
    log.done = true;
    log.field_trace = "rt-field-trace".into();
    log.body_omitted = true;
    log.client = "rt-client".into();
    log.user_agent = "rt-user-agent".into();
    log.session_key = "rt-session-key".into();
    log.request_class = "rt-request-class".into();
    log.agent_type = "rt-agent-type".into();
    log.prompt_id = "rt-prompt-id".into();
    log.session_id = "rt-session-id".into();
    log.agent_id = "rt-agent-id".into();
    log.parent_agent_id = "rt-parent-agent-id".into();
    log.compaction = "rt-compaction".into();
    log.context_compacted = "rt-context-compacted".into();
    log.prev_tool_durations = "rt-prev-tool-durations".into();
    log.updated_at = now_ms + 5;
    // deleted_at 恒 0（get_proxy_log 过滤 deleted_at=0）；routing 留 None（空串→None
    // 往返即其锁定路径）；tool_call_counts 不落列（票 16 B1），读回恒空，两侧均保持空。

    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&log, false, false))
        .await
        .unwrap();

    // 重开 DB：同一文件路径新开连接池后再读。
    drop(db);
    let main = dir.path().join("aidog.db");
    let db = aidog_db::Db::new(main.to_str().unwrap()).await.unwrap();

    let row = get_proxy_log(&db, "rt-col")
        .await
        .unwrap()
        .expect("row after reopen");
    assert_eq!(row.id, "rt-col");
    assert_eq!(row.group_key, "rt-group");
    assert_eq!(row.model, "rt-model");
    assert_eq!(row.actual_model, "rt-actual-model");
    assert_eq!(row.source_protocol, "rt-src-proto");
    assert_eq!(row.target_protocol, "rt-tgt-proto");
    assert_eq!(row.platform_id, 9);
    assert_eq!(row.request_headers, "rt-request-headers");
    assert_eq!(row.request_body, "rt-request-body");
    assert_eq!(row.upstream_request_headers, "rt-upstream-request-headers");
    assert_eq!(row.upstream_request_body, "rt-upstream-request-body");
    assert_eq!(row.response_body, "rt-response-body");
    assert_eq!(row.request_url, "rt-request-url");
    assert_eq!(row.upstream_request_url, "rt-upstream-request-url");
    assert_eq!(row.upstream_response_headers, "rt-upstream-response-headers");
    assert_eq!(row.upstream_status_code, 201);
    assert_eq!(row.user_response_headers, "rt-user-response-headers");
    assert_eq!(row.user_response_body, "rt-user-response-body");
    assert_eq!(row.status_code, 200);
    assert_eq!(row.duration_ms, 123);
    assert_eq!(row.input_tokens, 11);
    assert_eq!(row.output_tokens, 22);
    assert_eq!(row.cache_tokens, 33);
    assert_eq!(row.cache_write_tokens, 44);
    assert_eq!(row.cache_write_1h_tokens, 55);
    assert!((row.est_cost - 0.0123).abs() < 1e-9);
    assert!(row.is_stream);
    assert_eq!(row.attempts.len(), 1, "attempts JSON 往返");
    assert_eq!(row.attempts[0].error, "rt-attempt-error");
    assert_eq!(row.attempts[0].status_code, 499);
    assert_eq!(row.attempts[0].platform_name, "rt-attempt-platform");
    assert_eq!(row.retry_count, 2);
    assert_eq!(row.blocked_by, "rt-blocked-by");
    assert_eq!(row.blocked_reason, "rt-blocked-reason");
    assert!(row.done);
    assert_eq!(row.field_trace, "rt-field-trace");
    assert!(row.body_omitted);
    assert_eq!(row.client, "rt-client");
    assert_eq!(row.user_agent, "rt-user-agent");
    assert_eq!(row.session_key, "rt-session-key");
    assert_eq!(row.request_class, "rt-request-class");
    assert_eq!(row.agent_type, "rt-agent-type");
    assert_eq!(row.prompt_id, "rt-prompt-id");
    assert_eq!(row.session_id, "rt-session-id");
    assert_eq!(row.agent_id, "rt-agent-id");
    assert_eq!(row.parent_agent_id, "rt-parent-agent-id");
    assert_eq!(row.compaction, "rt-compaction");
    assert_eq!(row.context_compacted, "rt-context-compacted");
    assert_eq!(row.prev_tool_durations, "rt-prev-tool-durations");
    assert_eq!(row.created_at, now_ms);
    assert_eq!(row.updated_at, now_ms + 5);
    assert_eq!(row.deleted_at, 0);
    assert_eq!(row.routing, None);
    assert!(row.tool_call_counts.is_empty());
}

/// up/down 字节（验收 4）：CONNECT 行路径——直接构造 `ProxyLogColumns`（`from_log`
/// 后覆写；增量路径不携带字节）写入，重开 DB 后经列表接口
/// （`ProxyLogSummary`）读回非 0。
#[tokio::test]
async fn connect_bytes_columns_survive_roundtrip() {
    let (db, dir) = three_file_db().await;
    let mut cols = ProxyLogColumns::from_log(&sample_log("rt-bytes", "grp", now()), false, false);
    cols.up_bytes = 12345;
    cols.down_bytes = 67890;
    insert_proxy_log_columns(&db, cols).await.unwrap();

    drop(db);
    let main = dir.path().join("aidog.db");
    let db = aidog_db::Db::new(main.to_str().unwrap()).await.unwrap();

    let rows = list_proxy_logs(&db, 10, 0).await.unwrap();
    let s = rows
        .iter()
        .find(|r| r.id == "rt-bytes")
        .expect("summary row present");
    assert_eq!(s.up_bytes, 12345, "up_bytes 经列表读回不得为 0");
    assert_eq!(s.down_bytes, 67890, "down_bytes 经列表读回不得为 0");
}

/// 验收 4（issue #56 票 02）：CONNECT 行携带字节数写入后，`upsert_proxy_log` 同 id 重写
/// 必须绑定调用方值落库，不得像修复前那样对这两列写死 0（整行 REPLACE 语义下调用方
/// 重写时携带同一字节数，携带值必须原样落库）。先红测试。
#[tokio::test]
async fn upsert_proxy_log_preserves_connect_bytes() {
    let (db, _dir) = three_file_db().await;
    let log = aidog_db::models::ProxyLog {
        id: "rt-bytes-upsert".into(),
        group_key: "grp".into(),
        up_bytes: 12345,
        down_bytes: 67890,
        ..Default::default()
    };
    upsert_proxy_log(&db, log).await.unwrap();
    // 同 id 重写（CONNECT 行重写场景）：调用方再次携带同一字节数 + 终态元数据。
    let log2 = aidog_db::models::ProxyLog {
        id: "rt-bytes-upsert".into(),
        group_key: "grp".into(),
        status_code: 200,
        done: true,
        up_bytes: 12345,
        down_bytes: 67890,
        ..Default::default()
    };
    upsert_proxy_log(&db, log2).await.unwrap();

    let (up, down) = db
        .call_read_proxy_log_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row(
                "SELECT up_bytes, down_bytes FROM proxy_log WHERE id = 'rt-bytes-upsert'",
                [],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )
        })
        .await
        .unwrap();
    assert_eq!(
        (up, down),
        (12345, 67890),
        "upsert 同 id 重写不得清零 CONNECT 字节数"
    );
}
