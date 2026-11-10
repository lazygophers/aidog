//! 限频规则-合并 票 01：migration 20261110-14（extra.rate_limits 旧 models / windows → rules）。
#![cfg(test)]
use super::test_support::*;
use serde_json::{Value, json};

async fn insert_raw(db: &crate::Db, id: i64, extra: &str) {
    let extra = extra.to_string();
    db.call_platform_traced(None, std::panic::Location::caller(), move |c| {
        c.execute(
            "INSERT INTO platform (id, name, models, extra) VALUES (?1, ?2, '{}', ?3)",
            rusqlite::params![id, format!("p{id}"), extra],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

async fn read_extra(db: &crate::Db, id: i64) -> Value {
    db.call_read_platform_traced(None, std::panic::Location::caller(), move |c| {
        let e: String = c.query_row("SELECT extra FROM platform WHERE id = ?1", [id], |r| r.get(0))?;
        Ok(serde_json::from_str(&e).unwrap_or(Value::Null))
    })
    .await
    .unwrap()
}

async fn run(db: &crate::Db) -> u64 {
    db.call_platform_traced(None, std::panic::Location::caller(), move |c| {
        Ok(crate::schema_late::migrate_rate_limits_to_rules(c))
    })
    .await
    .unwrap()
}

async fn reopen(db: crate::Db, dir: &tempfile::TempDir) -> crate::Db {
    drop(db);
    crate::Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap()
}

fn rl(extra: &Value) -> &Value {
    &extra["rate_limits"]
}

/// 仅 models：每个模型一条「只选模型」规则；旧键删除；其它键保留。
#[tokio::test]
async fn models_only_become_model_rules() {
    let (db, _d) = three_file_db().await;
    insert_raw(
        &db,
        1,
        r#"{"x":1,"rate_limits":{"rpm":60,"source_policy_id":"v1","models":{"a":{"rpm":5},"b":{"concurrency":2,"tpm":9},"zero":{"rpm":0}}}}"#,
    )
    .await;
    assert_eq!(run(&db).await, 1);
    let e = read_extra(&db, 1).await;
    assert_eq!(e["x"], 1);
    assert_eq!(
        rl(&e),
        &json!({"rpm":60,"source_policy_id":"v1","rules":[{"models":["a"],"rpm":5},{"models":["b"],"concurrency":2,"tpm":9}]})
    );
}

/// 无模型筛选的窗口：转「只选时段」，未填项从平台级抄入，multiplier 去掉。
#[tokio::test]
async fn plain_window_copies_unfilled_from_base() {
    let (db, _d) = three_file_db().await;
    insert_raw(
        &db,
        1,
        r#"{"rate_limits":{"rpm":60,"concurrency":5,"windows":[{"start_hour":9,"end_hour":18,"timezone":"Asia/Shanghai","days_of_week":[1,2],"multiplier":1.0,"concurrency":10}]}}"#,
    )
    .await;
    assert_eq!(run(&db).await, 1);
    let e = read_extra(&db, 1).await;
    assert_eq!(
        rl(&e),
        &json!({"rpm":60,"concurrency":5,"rules":[
            {"start_hour":9,"end_hour":18,"timezone":"Asia/Shanghai","days_of_week":[1,2],"concurrency":10,"rpm":60}]})
    );
}

/// 带 models 的窗口：转「模型 + 时段」，模型名原样搬，未填项不抄平台级。
#[tokio::test]
async fn window_with_models_becomes_model_time_rule_without_copy() {
    let (db, _d) = three_file_db().await;
    insert_raw(
        &db,
        1,
        r#"{"rate_limits":{"rpm":60,"windows":[{"start_hour":0,"end_hour":24,"models":["m1","m2*"],"tpm":7}]}}"#,
    )
    .await;
    assert_eq!(run(&db).await, 1);
    let e = read_extra(&db, 1).await;
    assert_eq!(
        rl(&e)["rules"],
        json!([{"start_hour":0,"end_hour":24,"models":["m1","m2*"],"tpm":7}])
    );
}

/// 两者都有：旧窗口在前、旧模型级在后。
#[tokio::test]
async fn windows_precede_model_rules() {
    let (db, _d) = three_file_db().await;
    insert_raw(
        &db,
        1,
        r#"{"rate_limits":{"rpm":6,"models":{"a":{"rpm":1}},"windows":[{"start_hour":1,"end_hour":2,"rpm":2},{"start_hour":3,"end_hour":4,"models":["a"],"rpm":3}]}}"#,
    )
    .await;
    assert_eq!(run(&db).await, 1);
    let e = read_extra(&db, 1).await;
    assert_eq!(
        rl(&e)["rules"],
        json!([
            {"start_hour":1,"end_hour":2,"rpm":2},
            {"start_hour":3,"end_hour":4,"models":["a"],"rpm":3},
            {"models":["a"],"rpm":1}
        ])
    );
    assert!(rl(&e).get("models").is_none() && rl(&e).get("windows").is_none());
}

/// 空 / 新形状 / 无 rate_limits / 损坏 JSON：不动，损坏行不中断其余行。
#[tokio::test]
async fn untouched_cases_and_broken_json_skipped() {
    let (db, _d) = three_file_db().await;
    insert_raw(&db, 1, "{broken rate_limits").await;
    insert_raw(&db, 2, r#"{"rate_limits":{}}"#).await;
    insert_raw(&db, 3, r#"{"rate_limits":{"rpm":1,"rules":[{"models":["a"],"rpm":2}]}}"#).await;
    insert_raw(&db, 4, "").await;
    insert_raw(&db, 5, r#"{"rate_limits":{"models":{"a":{"rpm":1}}}}"#).await;
    assert_eq!(run(&db).await, 1, "只有第 5 行被改");
    assert_eq!(read_extra(&db, 2).await, json!({"rate_limits":{}}));
    assert_eq!(read_extra(&db, 3).await, json!({"rate_limits":{"rpm":1,"rules":[{"models":["a"],"rpm":2}]}}));
    assert_eq!(rl(&read_extra(&db, 5).await)["rules"], json!([{"models":["a"],"rpm":1}]));
}

/// 幂等：二次运行零变化。
#[tokio::test]
async fn idempotent() {
    let (db, _d) = three_file_db().await;
    insert_raw(&db, 1, r#"{"rate_limits":{"rpm":6,"models":{"a":{"rpm":1}},"windows":[{"start_hour":1,"end_hour":2,"rpm":2}]}}"#).await;
    assert_eq!(run(&db).await, 1);
    let first = read_extra(&db, 1).await;
    assert_eq!(run(&db).await, 0);
    assert_eq!(read_extra(&db, 1).await, first);
}

/// 启动链路已挂入：旧形状落盘后重开 DB（启动迁移）读回已是 rules。
#[tokio::test]
async fn runs_on_startup_and_reads_back_after_reopen() {
    let (db, dir) = three_file_db().await;
    insert_raw(&db, 1, r#"{"rate_limits":{"models":{"a":{"rpm":1}}}}"#).await;
    let db = reopen(db, &dir).await;
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(()))).await.unwrap();
    let e = read_extra(&db, 1).await;
    assert_eq!(rl(&e)["rules"], json!([{"models":["a"],"rpm":1}]));
    assert!(rl(&e).get("models").is_none());
}
