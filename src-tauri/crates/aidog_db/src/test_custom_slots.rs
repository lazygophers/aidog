#![cfg(test)]
use super::test_support::*;
use super::*;

fn with_extra(name: &str, extra: &str) -> CreatePlatform {
    let mut p = sample_platform(name);
    p.extra = extra.to_string();
    p
}

/// 三文件库：计数只数 extra.custom_slots 含该键的平台；清理只删该键、保留其它键，写后重开读回无孤儿。
#[tokio::test]
async fn custom_slot_usage_and_purge_across_reopen() {
    let (db, dir) = three_file_db().await;
    let a = create_platform(&db, with_extra("a", r#"{"custom_slots":{"fast":["m1"],"bg":["m2"]},"breaker":{"open_secs":30}}"#)).await.unwrap();
    let b = create_platform(&db, with_extra("b", r#"{"custom_slots":{"bg":["x"]}}"#)).await.unwrap();
    let c = create_platform(&db, with_extra("c", "")).await.unwrap();
    assert_eq!(count_custom_slot_usage(&db, "fast").await.unwrap(), 1);
    assert_eq!(count_custom_slot_usage(&db, "bg").await.unwrap(), 2);
    assert_eq!(purge_custom_slot(&db, "fast").await.unwrap(), 1);
    drop(db);
    let db = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(()))).await.unwrap();
    assert_eq!(count_custom_slot_usage(&db, "fast").await.unwrap(), 0);
    let got = get_platform(&db, a.id).await.unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_str(&got.extra).unwrap();
    assert!(v["custom_slots"].get("fast").is_none());
    assert_eq!(v["custom_slots"]["bg"], serde_json::json!(["m2"]));
    assert_eq!(v["breaker"]["open_secs"], 30);
    let gb = get_platform(&db, b.id).await.unwrap().unwrap();
    assert_eq!(gb.extra, r#"{"custom_slots":{"bg":["x"]}}"#);
    assert_eq!(get_platform(&db, c.id).await.unwrap().unwrap().extra, "");
}
