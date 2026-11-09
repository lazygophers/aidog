//! 时段档-模型配置对齐 票 02：migration 20261110-11（已有时段档补齐平台默认 embedding / rerank，一次性）。
#![cfg(test)]
use super::test_support::*;
use serde_json::{Value, json};

async fn insert_raw(db: &crate::Db, id: i64, models: &str, extra: &str) {
    let (models, extra) = (models.to_string(), extra.to_string());
    db.call_platform_traced(None, std::panic::Location::caller(), move |c| {
        c.execute(
            "INSERT INTO platform (id, name, models, extra) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, format!("p{id}"), models, extra],
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

/// 直接跑迁移本体；`reset` = 先清一次性标记（三文件库建库时已对空表打过标）。
async fn run(db: &crate::Db, reset: bool) -> u64 {
    db.call_platform_traced(None, std::panic::Location::caller(), move |c| {
        if reset {
            c.execute("DELETE FROM migration_done", [])?;
        }
        Ok(crate::schema_late::backfill_time_rule_dedicated_models(c))
    })
    .await
    .unwrap()
}

async fn reopen(db: crate::Db, dir: &tempfile::TempDir) -> crate::Db {
    drop(db);
    crate::Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap()
}

const DEFAULTS: &str = r#"{"default":["glm"],"embedding":["emb-d"],"rerank":["rr-d"]}"#;

#[tokio::test]
async fn migration_20261110_11_fills_missing_and_keeps_existing() {
    let (db, dir) = three_file_db().await;
    insert_raw(
        &db,
        1,
        DEFAULTS,
        r#"{"breaker":{"threshold":3},"time_windows":[
            {"windows":[{"start_hour":0,"end_hour":24}],"models":{"default":["night"]}},
            {"windows":[{"start_hour":1,"end_hour":2}],"models":{"default":["x"],"embedding":["emb-own"],"rerank":[]}}]}"#,
    )
    .await;
    // 平台默认无 embedding：不凭空写
    insert_raw(&db, 2, r#"{"default":["a"]}"#, r#"{"time_windows":[{"windows":[],"models":{"default":["n"]}}]}"#).await;
    // 无时段档 / 空 extra：不动
    insert_raw(&db, 3, DEFAULTS, "{}").await;

    assert_eq!(run(&db, true).await, 1, "只有平台 1 被改");

    let db = reopen(db, &dir).await;
    let e1 = read_extra(&db, 1).await;
    assert_eq!(e1["time_windows"][0]["models"], json!({"default":["night"],"embedding":["emb-d"],"rerank":["rr-d"]}));
    assert_eq!(
        e1["time_windows"][1]["models"],
        json!({"default":["x"],"embedding":["emb-own"],"rerank":["rr-d"]}),
        "已有 embedding 不覆盖；空 rerank 视为缺失补齐"
    );
    assert_eq!(e1["breaker"]["threshold"], 3, "extra 其余键原样");
    assert_eq!(read_extra(&db, 2).await["time_windows"][0]["models"], json!({"default":["n"]}));
    assert_eq!(read_extra(&db, 3).await, json!({}));
}

/// 一次性：二次运行零变化；用户之后有意清空档内 embedding，重启（再跑 init）不得被补回。
#[tokio::test]
async fn migration_20261110_11_is_one_shot() {
    let (db, _dir) = three_file_db().await;
    insert_raw(&db, 1, DEFAULTS, r#"{"time_windows":[{"windows":[],"models":{"default":["n"]}}]}"#).await;
    assert_eq!(run(&db, true).await, 1);
    let first = read_extra(&db, 1).await;
    assert_eq!(run(&db, false).await, 0, "二次运行无变化");
    assert_eq!(read_extra(&db, 1).await, first);

    // 用户清空档内 embedding 后重启
    db.call_platform_traced(None, std::panic::Location::caller(), |c| {
        c.execute(
            "UPDATE platform SET extra = ?1 WHERE id = 1",
            [r#"{"time_windows":[{"windows":[],"models":{"default":["n"]}}]}"#],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(()))).await.unwrap();
    assert_eq!(
        read_extra(&db, 1).await["time_windows"][0]["models"],
        json!({"default":["n"]}),
        "有意留空的槽不得被重启补回"
    );
}

#[tokio::test]
async fn migration_20261110_11_skips_broken_json() {
    let (db, _dir) = three_file_db().await;
    insert_raw(&db, 1, "{broken", r#"{"time_windows":[{"windows":[],"models":{"default":["n"]}}]}"#).await;
    insert_raw(&db, 2, DEFAULTS, "{broken").await;
    insert_raw(&db, 3, DEFAULTS, r#"{"time_windows":[{"windows":[],"models":{"default":["n"]}}]}"#).await;
    assert_eq!(run(&db, true).await, 1, "损坏行跳过，不中断其余行");
    assert_eq!(read_extra(&db, 1).await["time_windows"][0]["models"], json!({"default":["n"]}));
    assert_eq!(read_extra(&db, 3).await["time_windows"][0]["models"]["embedding"], json!(["emb-d"]));
}
