#![cfg(test)]
//! issue #56 子票 03：`attempts[].error` 归元数据保留的锁定测试（三库真实文件）。
//!
//! 口径（2026-11-13 拍板）：`attempts[].error`（上游错误体摘要，≤500 字符）归元数据，
//! 不受 `log_upstream_request` 开关与 upstream retention 控制，始终保留。
//! 回归护栏，非先红：断言在当前生产代码上应直接绿；红 = 现状与口径不符。
//! 三库真实文件库走 [`aidog_db::test_support::three_file_db`]（db-pool-routing 规则：
//! proxy_log 的 call 站点禁内存库）。

use crate::*;
use aidog_db::models::*;
use aidog_db::now;
use aidog_db::test_support::*;

/// 两侧「原始信息」开关全关（`upsert_proxy_log` 的 strip 门禁据此清原始列）。
async fn disable_raw_logging(db: &aidog_db::Db) {
    let settings = ProxyLogSettings {
        log_user_request: false,
        log_upstream_request: false,
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

const ATTEMPT_ERROR: &str = "t3-attempt-error: upstream error body digest (<=500 chars)";

fn sample_log(id: &str, created_ms: i64) -> ProxyLog {
    let mut log = ProxyLog {
        id: id.into(),
        group_key: "t3-grp".into(),
        status_code: 429,
        created_at: created_ms,
        ..Default::default()
    };
    log.request_headers = r#"{"x":"t3-user-h"}"#.into();
    log.request_body = "t3-user-body".into();
    log.user_response_headers = "t3-user-resp-h".into();
    log.user_response_body = "t3-user-resp-b".into();
    log.request_url = "http://127.0.0.1/v1/messages".into();
    log.upstream_request_headers = "t3-up-h".into();
    log.upstream_request_body = "t3-up-body".into();
    log.upstream_response_headers = "t3-up-resp-h".into();
    log.response_body = "t3-up-resp-b".into();
    log.field_trace = "t3-trace".into();
    log.upstream_request_url = "https://up.invalid/chat/completions".into();
    log.attempts = vec![ProxyAttempt {
        platform_id: 3,
        platform_name: "t3-platform".into(),
        status_code: 429,
        error: ATTEMPT_ERROR.into(),
        duration_ms: 120,
        ts: created_ms,
        ..Default::default()
    }];
    log
}

/// 口径锁定：两侧开关全关写入含 `error` 的 attempts → 重开 DB 读回 `error` 原样保留；
/// upstream retention（`cleanup_upstream_request_fields`，1 小时窗）跑过后 `error` 仍在、
/// 上游侧 6 列「原始信息」按既有口径被清（SET '' 不删行）。
#[tokio::test]
async fn attempt_error_is_metadata_survives_switch_and_upstream_retention() {
    let (db, dir) = three_file_db().await;
    disable_raw_logging(&db).await;
    // created_at 落在 1 小时保留窗之外，retention 才会清它的上游侧原始列
    let created = now() - 90 * 60 * 1000;
    upsert_proxy_log(&db, sample_log("t3-err", created))
        .await
        .unwrap();

    // 重开 DB：同一文件路径新开连接池后再读（捕获写入侧遗漏 strip 的不一致）
    drop(db);
    let main = dir.path().join("aidog.db");
    let db = aidog_db::Db::new(main.to_str().unwrap()).await.unwrap();

    let row = get_proxy_log(&db, "t3-err")
        .await
        .unwrap()
        .expect("row exists after reopen");
    assert_eq!(row.attempts.len(), 1, "attempts 关开关后仍保留");
    assert_eq!(row.attempts[0].error, ATTEMPT_ERROR, "error 原样读回");
    assert_eq!(
        row.upstream_request_body, "",
        "开关关 → 上游侧原始列被清（既有口径，对照项）"
    );

    // upstream retention 跑过后：error 仍在，上游侧原始列保持被清
    aidog_db::cleanup_upstream_request_fields(&db, 1, RetentionUnit::Hour)
        .await
        .unwrap();
    let row = get_proxy_log(&db, "t3-err")
        .await
        .unwrap()
        .expect("retention SET '' 不删行");
    assert_eq!(row.attempts.len(), 1, "retention 不动 attempts");
    assert_eq!(
        row.attempts[0].error, ATTEMPT_ERROR,
        "error 不受 upstream retention 控制"
    );
    for (col, val) in [
        ("upstream_request_headers", row.upstream_request_headers.as_str()),
        ("upstream_request_body", row.upstream_request_body.as_str()),
        ("upstream_response_headers", row.upstream_response_headers.as_str()),
        ("response_body", row.response_body.as_str()),
        ("field_trace", row.field_trace.as_str()),
        ("upstream_request_url", row.upstream_request_url.as_str()),
    ] {
        assert_eq!(val, "", "{col} 应被 upstream retention 清空");
    }
    assert_eq!(row.status_code, 429, "retention 只清原始列，元数据不动");
}
