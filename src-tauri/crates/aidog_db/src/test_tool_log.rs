#![cfg(test)]
use super::test_support::*;
use super::*;
use crate::models::{ToolLog, ToolLogFilter};
use rusqlite::params;

fn row(tool: &str, trigger: &str, group: &str, query: &str) -> ToolLog {
    ToolLog {
        tool: tool.into(),
        trigger: trigger.into(),
        group_name: group.into(),
        platform: "glm".into(),
        query: query.into(),
        result_count: 3,
        engines: r#"[{"engine":"bing","count":3,"latency_ms":12,"error":null}]"#.into(),
        duration_ms: 40,
        success: true,
        error: String::new(),
        result: "R".into(),
        ..Default::default()
    }
}

/// 三文件库：写后重开读回；列表不带 result、详情带；按 tool/trigger/group 筛选；倒序分页。
#[tokio::test]
async fn tool_log_roundtrip_filter_and_detail_across_reopen() {
    let (db, dir) = three_file_db().await;
    let a = insert_tool_log(&db, &row("web_search", "gateway", "g1", "rust")).await.unwrap();
    insert_tool_log(&db, &row("web_fetch", "mcp", "g2", "http://x")).await.unwrap();
    let mut failed = row("web_search", "mcp", "g1", "boom");
    failed.success = false;
    failed.error = "all engines failed".into();
    insert_tool_log(&db, &failed).await.unwrap();
    drop(db);
    let db = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(()))).await.unwrap();

    let all = list_tool_logs(&db, &ToolLogFilter::default(), 10, 0).await.unwrap();
    assert_eq!(all.total, 3);
    assert_eq!(all.rows[0].query, "boom", "倒序：最新在前");
    assert!(!all.rows[0].success);
    assert_eq!(all.rows[0].error, "all engines failed");
    assert!(all.rows.iter().all(|r| r.result.is_empty()), "列表不带 result");

    let f = ToolLogFilter { tool: Some("web_search".into()), ..Default::default() };
    assert_eq!(list_tool_logs(&db, &f, 10, 0).await.unwrap().total, 2);
    let f = ToolLogFilter { trigger: Some("gateway".into()), ..Default::default() };
    let g = list_tool_logs(&db, &f, 10, 0).await.unwrap();
    assert_eq!((g.total, g.rows.len(), g.rows[0].id), (1, 1, a));
    let f = ToolLogFilter { group_name: Some("g2".into()), ..Default::default() };
    assert_eq!(list_tool_logs(&db, &f, 10, 0).await.unwrap().rows[0].tool, "web_fetch");
    let page2 = list_tool_logs(&db, &ToolLogFilter::default(), 2, 2).await.unwrap();
    assert_eq!((page2.total, page2.rows.len()), (3, 1));

    let d = get_tool_log(&db, a).await.unwrap().expect("detail");
    assert_eq!(d.result, "R");
    assert_eq!(d.platform, "glm");
    assert!(d.engines.contains("bing"));
    assert!(d.created_at > 0);
    assert!(get_tool_log(&db, 9999).await.unwrap().is_none());
}

/// 迁移幂等：重复跑 log.db early migration 不报错、不丢行。
#[tokio::test]
async fn tool_log_migration_idempotent() {
    let (db, _dir) = three_file_db().await;
    insert_tool_log(&db, &row("web_search", "mcp", "g", "q")).await.unwrap();
    for _ in 0..2 {
        db.call_proxy_log_traced(None, std::panic::Location::caller(), |c| {
            crate::schema_early::run_migrations_proxy_log_early(c)?;
            Ok(())
        })
        .await
        .unwrap();
    }
    assert_eq!(list_tool_logs(&db, &ToolLogFilter::default(), 10, 0).await.unwrap().total, 1);
}

/// 清理：超过 days 的删，days==0 不清。
#[tokio::test]
async fn tool_log_cleanup_respects_days_and_zero() {
    let (db, _dir) = three_file_db().await;
    insert_tool_log(&db, &row("web_search", "mcp", "g", "fresh")).await.unwrap();
    let old = now() - 40 * 86_400_000;
    db.call_proxy_log_traced(None, std::panic::Location::caller(), move |c| {
        c.execute("INSERT INTO tool_log (created_at, tool, query) VALUES (?1,'web_search','old')", params![old])?;
        Ok(())
    })
    .await
    .unwrap();
    cleanup_tool_logs(&db, 0).await.unwrap();
    assert_eq!(list_tool_logs(&db, &ToolLogFilter::default(), 10, 0).await.unwrap().total, 2, "0=永久保留");
    cleanup_tool_logs(&db, 30).await.unwrap();
    let rows = list_tool_logs(&db, &ToolLogFilter::default(), 10, 0).await.unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].query, "fresh");
}
