//! 平台-模型多选 票 01：migration 20261102-01（platform.models / extra.time_windows[].models 单值 → 数组）。
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

async fn read_raw(db: &crate::Db, id: i64) -> (String, Value) {
    db.call_read_platform_traced(None, std::panic::Location::caller(), move |c| {
        let (m, e): (String, String) = c.query_row(
            "SELECT models, extra FROM platform WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((m, serde_json::from_str(&e).unwrap_or(Value::Null)))
    })
    .await
    .unwrap()
}

async fn rerun(db: &crate::Db) {
    crate::schema::init_tables_raw(db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
}

#[tokio::test]
async fn migration_20261102_01_listifies_models_and_time_windows() {
    let (db, dir) = three_file_db().await;
    insert_raw(
        &db,
        1,
        r#"{"default":"glm-5","opus":"glm-5.3","jev":"j"}"#,
        r#"{"breaker":{"threshold":3},"time_windows":[{"windows":[{"start_hour":0,"end_hour":24}],"models":{"default":"night","sonnet":"night-s"}}],"direct_models":["a","b"]}"#,
    )
    .await;
    insert_raw(
        &db,
        2,
        r#"{"default":["x","y"],"haiku":"h"}"#,
        r#"{"time_windows":[{"models":{"default":["t1","t2"]}}]}"#,
    )
    .await;
    insert_raw(&db, 3, "{not json", r#"{"time_windows":[{"models":{"default":"z"}}]}"#).await;
    insert_raw(&db, 4, "", "{}").await;
    insert_raw(&db, 5, r#"{"default":"  ","opus":null,"gpt":"g"}"#, "{broken").await;

    rerun(&db).await;

    // 写后重开 DB 读回：释放连接池，从磁盘文件重新打开
    drop(db);
    let main = dir.path().join("aidog.db");
    let db = crate::Db::new(main.to_str().unwrap()).await.unwrap();

    let (m1, e1) = read_raw(&db, 1).await;
    assert_eq!(
        serde_json::from_str::<Value>(&m1).unwrap(),
        json!({"default":["glm-5"],"opus":["glm-5.3"],"jev":["j"]})
    );
    assert_eq!(
        e1["time_windows"][0]["models"],
        json!({"default":["night"],"sonnet":["night-s"]})
    );
    assert_eq!(e1["time_windows"][0]["windows"][0]["end_hour"], 24, "windows 原样");
    assert_eq!(e1["breaker"]["threshold"], 3, "extra 其余键原样");
    assert_eq!(e1["direct_models"], json!(["a", "b"]), "direct_models 不动");

    // 已是数组的槽位原样；混合行只改字符串槽
    let (m2, e2) = read_raw(&db, 2).await;
    assert_eq!(
        serde_json::from_str::<Value>(&m2).unwrap(),
        json!({"default":["x","y"],"haiku":["h"]})
    );
    assert_eq!(e2["time_windows"][0]["models"]["default"], json!(["t1", "t2"]));

    // 损坏 models 行跳过（原文保留），同行 extra 仍被迁移
    let (m3, e3) = read_raw(&db, 3).await;
    assert_eq!(m3, "{not json");
    assert_eq!(e3["time_windows"][0]["models"]["default"], json!(["z"]));

    // 空串 models 不动
    let (m4, _) = read_raw(&db, 4).await;
    assert_eq!(m4, "");

    // 空白 / null 槽位被丢弃；损坏 extra 跳过不中断
    let (m5, e5) = read_raw(&db, 5).await;
    assert_eq!(serde_json::from_str::<Value>(&m5).unwrap(), json!({"gpt":["g"]}));
    assert_eq!(e5, Value::Null);
}

#[tokio::test]
async fn migration_20261102_01_is_idempotent() {
    let (db, _dir) = three_file_db().await;
    insert_raw(
        &db,
        1,
        r#"{"default":"glm-5"}"#,
        r#"{"time_windows":[{"models":{"default":"night"}}]}"#,
    )
    .await;
    rerun(&db).await;
    let first = read_raw(&db, 1).await;
    assert_eq!(first.1["time_windows"][0]["models"]["default"], json!(["night"]));
    rerun(&db).await;
    rerun(&db).await;
    assert_eq!(read_raw(&db, 1).await, first, "二次及以后运行零变化");
}
