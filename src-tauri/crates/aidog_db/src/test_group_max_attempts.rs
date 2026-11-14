//! 请求尝试-逐次重算 票 01：最大尝试次数 1–10 校验 + migration 20261110-12（全部分组置 10，一次性）。
#![cfg(test)]
use super::test_support::*;
use super::*;

async fn set_raw(db: &Db, name: &str, n: i64) {
    let name = name.to_string();
    db.call_platform_traced(None, std::panic::Location::caller(), move |c| {
        c.execute("UPDATE \"group\" SET max_retries = ?1 WHERE name = ?2", rusqlite::params![n, name])?;
        Ok(())
    })
    .await
    .unwrap();
}

async fn run(db: &Db, reset: bool) -> u64 {
    db.call_platform_traced(None, std::panic::Location::caller(), move |c| {
        if reset {
            c.execute("DELETE FROM migration_done", [])?;
        }
        Ok(crate::schema_late::reset_group_max_attempts(c))
    })
    .await
    .unwrap()
}

async fn max_of(db: &Db, name: &str) -> u32 {
    let name = name.to_string();
    db.call_read_platform_traced(None, std::panic::Location::caller(), move |c| {
        c.query_row("SELECT max_retries FROM \"group\" WHERE name = ?1", [name], |r| r.get::<_, u32>(0))
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn create_and_update_reject_out_of_range() {
    let db = test_db().await;
    for bad in [0u32, 11, 100] {
        let mut input = sample_group(&format!("bad{bad}"), vec![]);
        input.max_retries = bad;
        assert!(create_group(&db, input).await.is_err(), "create {bad}");
    }
    for ok in [1u32, 10] {
        let mut input = sample_group(&format!("ok{ok}"), vec![]);
        input.max_retries = ok;
        assert_eq!(create_group(&db, input).await.unwrap().max_retries, ok);
    }
    let g = create_group(&db, sample_group("upd", vec![])).await.unwrap();
    for bad in [0u32, 11] {
        let r = update_group(&db, UpdateGroup {
            id: g.id, name: None, routing_mode: None, request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: None, max_retries: Some(bad), model_mappings: vec![], env_vars: vec![], is_default: None,
        })
        .await;
        assert!(r.is_err(), "update {bad}");
    }
    assert_eq!(get_group(&db, g.id).await.unwrap().unwrap().max_retries, 10, "校验失败不落库");
}

#[tokio::test]
async fn migration_sets_all_groups_to_10_and_is_one_shot() {
    let (db, dir) = three_file_db().await;
    for n in ["a", "b", "c"] {
        create_group(&db, sample_group(n, vec![])).await.unwrap();
    }
    for (n, v) in [("a", 2i64), ("b", 5), ("c", 0)] {
        set_raw(&db, n, v).await;
    }
    assert_eq!(run(&db, true).await, 3);

    drop(db);
    let db = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    for n in ["a", "b", "c"] {
        assert_eq!(max_of(&db, n).await, 10, "{n}");
    }

    // 一次性：用户之后改成 3，二次运行 / 重启都不得再覆盖
    set_raw(&db, "a", 3).await;
    assert_eq!(run(&db, false).await, 0);
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(()))).await.unwrap();
    assert_eq!(max_of(&db, "a").await, 3);
}
