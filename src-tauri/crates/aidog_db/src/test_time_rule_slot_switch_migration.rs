//! 时段档-模型配置对齐 票 03：migration 20261110-13（已有时段档补本档内置槽开关，取平台级开关，排除自定义槽名）。
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
        Ok(crate::schema_late::backfill_time_rule_slot_switches(c))
    })
    .await
    .unwrap()
}

async fn reopen(db: crate::Db, dir: &tempfile::TempDir) -> crate::Db {
    drop(db);
    crate::Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap()
}

const RULE: &str = r#"{"windows":[],"models":{"default":["n"]}}"#;

/// 缺开关集合的档写入平台级内置槽开关（含旧键，排除自定义槽名）；已有集合（含空数组）不覆盖。
#[tokio::test]
async fn migration_20261110_13_fills_builtin_switches_only() {
    let (db, dir) = three_file_db().await;
    insert_raw(
        &db,
        1,
        &format!(
            r#"{{"disabled_slots":["opus","fast","embedding"],"soft_disabled_slots":["gpt","opus"],
              "time_windows":[{RULE},{{"windows":[],"models":{{}},"disabled_slots":[]}},{{"windows":[],"models":{{}},"disabled_slots":["haiku"]}}]}}"#
        ),
    )
    .await;
    // 平台级无开关：档写入空集合（表示「有集合」）
    insert_raw(&db, 2, &format!(r#"{{"time_windows":[{RULE}]}}"#)).await;
    // 无时段档：不动
    insert_raw(&db, 3, r#"{"disabled_slots":["opus"]}"#).await;

    assert_eq!(run(&db).await, 2, "平台 1、2 被改");

    let db = reopen(db, &dir).await;
    let e1 = read_extra(&db, 1).await;
    assert_eq!(e1["time_windows"][0]["disabled_slots"], json!(["opus", "embedding", "gpt"]), "排除自定义槽 fast");
    assert_eq!(e1["time_windows"][1]["disabled_slots"], json!([]), "已有空集合不覆盖");
    assert_eq!(e1["time_windows"][2]["disabled_slots"], json!(["haiku"]), "已有集合不覆盖");
    assert_eq!(e1["disabled_slots"], json!(["opus", "fast", "embedding"]), "平台级开关原样");
    assert_eq!(read_extra(&db, 2).await["time_windows"][0]["disabled_slots"], json!([]));
    assert_eq!(read_extra(&db, 3).await, json!({"disabled_slots":["opus"]}));
}

/// 幂等：二次运行零变化，档内结果不变。
#[tokio::test]
async fn migration_20261110_13_is_idempotent() {
    let (db, _dir) = three_file_db().await;
    insert_raw(&db, 1, &format!(r#"{{"disabled_slots":["opus"],"time_windows":[{RULE}]}}"#)).await;
    assert_eq!(run(&db).await, 1);
    let first = read_extra(&db, 1).await;
    assert_eq!(run(&db).await, 0, "二次运行无变化");
    assert_eq!(read_extra(&db, 1).await, first);
}

#[tokio::test]
async fn migration_20261110_13_skips_broken_json() {
    let (db, _dir) = three_file_db().await;
    insert_raw(&db, 1, "{broken").await;
    insert_raw(&db, 2, &format!(r#"{{"disabled_slots":["gpt"],"time_windows":[{RULE}]}}"#)).await;
    assert_eq!(run(&db).await, 1, "损坏行跳过，不中断其余行");
    assert_eq!(read_extra(&db, 2).await["time_windows"][0]["disabled_slots"], json!(["gpt"]));
}
