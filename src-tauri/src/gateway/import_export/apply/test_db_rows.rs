//! db_rows.rs 测试：breaker 迁入 extra + ensure_group_and_attach 幂等。

use super::{effective_extra_with_breaker, ensure_group_and_attach, snapshot_platform_ids};
use crate::gateway::db::Db;

/// 内存库（同 db.rs test 约定）。
async fn test_db() -> Db {
    let db = Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    db
}

fn platform_payload(name: &str, base_url: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "platform_type": "anthropic",
        "base_url": base_url,
        "api_key": "sk-test",
        "extra": "{}",
        "models": "{}",
        "available_models": "[]",
        "endpoints": "[]",
        "enabled": true,
        "status": "enabled",
        "auto_disabled_until": 0,
        "auto_disable_strikes": 0,
        "breaker_failure_threshold": 0,
        "breaker_open_secs": 0,
        "breaker_half_open_max": 0,
        "est_balance_remaining": 0.0,
        "est_coding_plan": "",
        "last_real_query_at": 0,
        "estimate_count": 0,
        "show_in_tray": false,
        "tray_display": "balance",
        "sort_order": 0,
        "manual_budgets": "[]"
    })
}

/// 旧格式导入（breaker 在顶层）→ 无损迁入 extra.breaker。
#[test]
fn legacy_top_level_breaker_migrates_into_extra() {
    let mut row = platform_payload("Old", "https://a.example.com");
    row["breaker_failure_threshold"] = serde_json::json!(6);
    row["breaker_open_secs"] = serde_json::json!(180);
    row["breaker_half_open_max"] = serde_json::json!(3);
    let extra = effective_extra_with_breaker(&row);
    let b = crate::gateway::models::parse_breaker(&extra);
    assert_eq!((b.failure_threshold, b.open_secs, b.half_open_max), (6, 180, 3));
}

/// 直插一个 platform（绕过 apply 事务），返回 rowid。
async fn insert_test_platform(db: &Db, name: &str) -> i64 {
    let name = name.to_string();
    db.0
        .call(move |conn| {
            conn.execute(
                "INSERT INTO platform (name, created_at, updated_at) VALUES (?1, 0, 0)",
                rusqlite::params![name],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .unwrap()
}

async fn group_id_by_name(db: &Db, name: &str) -> Option<i64> {
    let name = name.to_string();
    db.0
        .call(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT id FROM \"group\" WHERE name = ?1 AND deleted_at = 0",
                    [&name],
                    |r| r.get::<_, i64>(0),
                )
                .ok())
        })
        .await
        .unwrap()
}

async fn link_count(db: &Db, gid: i64) -> i64 {
    db.0
        .call(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM group_platform WHERE group_id = ?1 AND deleted_at = 0",
                    [gid],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0))
        })
        .await
        .unwrap()
}

/// 组不存在 → 按 name 建组（生成 group_key）+ 关联本次新建 platform。
#[tokio::test]
async fn ensure_group_creates_when_absent() {
    let db = test_db().await;
    // 预置一个旧平台（before 快照含它，不应被关联）。
    insert_test_platform(&db, "old").await;
    let before = snapshot_platform_ids(&db).await.unwrap();
    // 本次"导入"新建两个平台。
    insert_test_platform(&db, "new1").await;
    insert_test_platform(&db, "new2").await;

    ensure_group_and_attach(&db, "sub2api", &before).await.unwrap();

    let gid = group_id_by_name(&db, "sub2api").await.expect("group created");
    // 校验 group_key 生成。
    let gkey: String = db
        .0
        .call(move |conn| {
            Ok(conn
                .query_row("SELECT group_key FROM \"group\" WHERE id = ?1", [gid], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap())
        })
        .await
        .unwrap();
    assert!(gkey.starts_with("gk_"), "group_key 应生成 gk_ 前缀");
    // 仅关联本次新建的 2 个平台（old 不在内）。
    assert_eq!(link_count(&db, gid).await, 2);
}

/// 同名组已存在 → 不重复建组，仅 attach（ON CONFLICT 幂等）。
#[tokio::test]
async fn ensure_group_idempotent() {
    let db = test_db().await;
    let before1 = snapshot_platform_ids(&db).await.unwrap();
    insert_test_platform(&db, "p1").await;
    ensure_group_and_attach(&db, "sub2api", &before1).await.unwrap();
    let gid = group_id_by_name(&db, "sub2api").await.unwrap();
    assert_eq!(link_count(&db, gid).await, 1);

    // 第二次导入：组已存在 → 复用同 id，不重复建组。
    let before2 = snapshot_platform_ids(&db).await.unwrap();
    insert_test_platform(&db, "p2").await;
    ensure_group_and_attach(&db, "sub2api", &before2).await.unwrap();
    let gid2 = group_id_by_name(&db, "sub2api").await.unwrap();
    assert_eq!(gid, gid2, "同名组不应重复创建");
    // 组数确认只有一个。
    let group_count: i64 = db
        .0
        .call(|conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM \"group\" WHERE name = 'sub2api' AND deleted_at = 0",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap())
        })
        .await
        .unwrap();
    assert_eq!(group_count, 1);
    // 第二次关联追加 p2 → 共 2 个关联。
    assert_eq!(link_count(&db, gid).await, 2);
}

/// auto_group=false 等价于不调 ensure → 不建组（行为契约：import 跳过 ensure）。
#[tokio::test]
async fn no_ensure_means_no_group() {
    let db = test_db().await;
    insert_test_platform(&db, "p").await;
    // 不调用 ensure_group_and_attach（模拟 auto_group=false）。
    assert!(group_id_by_name(&db, "sub2api").await.is_none());
}

/// 新格式导入（breaker 已在 extra）→ 原样保留，不被顶层 0 覆盖。
#[test]
fn new_format_extra_breaker_preserved() {
    let mut row = platform_payload("New", "https://a.example.com");
    row["extra"] = serde_json::json!(crate::gateway::models::merge_breaker_into_extra(
        "{}",
        &crate::gateway::models::PlatformBreaker { failure_threshold: 9, open_secs: 30, half_open_max: 1 },
    ));
    // 顶层全 0（新导出不再含顶层 breaker）。
    let extra = effective_extra_with_breaker(&row);
    let b = crate::gateway::models::parse_breaker(&extra);
    assert_eq!((b.failure_threshold, b.open_secs, b.half_open_max), (9, 30, 1));
}

/// 新格式导出 extra 为 JSON object（非 string）→ 导入兼容，breaker 迁入仍生效。
/// `json_str` 对 Object 值走 `other.to_string()` 兜底 → 序列化字符串 → parse_breaker 正常解析。
#[test]
fn new_format_extra_as_object_breaker_preserved() {
    let mut row = platform_payload("ObjExtra", "https://a.example.com");
    // 模拟新导出：extra 是 obj（非 string），含 breaker。
    row["extra"] = serde_json::json!({
        "breaker": { "failure_threshold": 4, "open_secs": 120, "half_open_max": 2 }
    });
    // 新格式顶层无 breaker_* 字段 → 全 0。
    let extra = effective_extra_with_breaker(&row);
    let b = crate::gateway::models::parse_breaker(&extra);
    assert_eq!((b.failure_threshold, b.open_secs, b.half_open_max), (4, 120, 2));
}

/// 新格式导出省略 extra（空）→ 导入回空字符串（分享语义：平台像全新，无 breaker 覆盖）。
#[test]
fn new_format_extra_missing_yields_empty() {
    let mut row = platform_payload("NoExtra", "https://a.example.com");
    row.as_object_mut().unwrap().remove("extra");
    let extra = effective_extra_with_breaker(&row);
    assert!(extra.is_empty(), "缺失 extra 应回空字符串");
    let b = crate::gateway::models::parse_breaker(&extra);
    assert_eq!((b.failure_threshold, b.open_secs, b.half_open_max), (0, 0, 0));
}

/// 新格式导出省略 models/available_models/endpoints（空配置）→ 导入写库写标准空 JSON
/// （`{}` / `[]`），而非空串。避免 read 端 parse_models 等对空串刷 warn 日志淹没真实问题。
/// 锁定 insert_platform_row 的缺失字段补默认行为（db.rs create_platform serialize_* 默认对齐）。
#[tokio::test]
async fn new_format_missing_config_fields_write_default_json() {
    let db = test_db().await;
    // 模拟新格式导出 payload：仅核心字段，省略 models/available_models/endpoints。
    let row = serde_json::json!({
        "name": "Clean",
        "platform_type": "anthropic",
        "base_url": "https://x.example.com",
        "api_key": "sk",
    });
    let now = 0_i64;
    db.0
        .call(move |conn| {
            let tx = conn.transaction()?;
            super::insert_platform_row(&tx, "Clean", &row, now)?;
            tx.commit()?;
            Ok(())
        })
        .await
        .unwrap();

    // 读回 DB 列值，校验写的是标准空 JSON（非空串）。
    let (models, available, endpoints): (String, String, String) = db
        .0
        .call(move |conn| {
            Ok(conn.query_row(
                "SELECT models, available_models, endpoints FROM platform WHERE name = 'Clean'",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(models, "{}", "缺失 models 应写 '{{}}' 非空串");
    assert_eq!(available, "[]", "缺失 available_models 应写 '[]' 非空串");
    assert_eq!(endpoints, "[]", "缺失 endpoints 应写 '[]' 非空串");
}
