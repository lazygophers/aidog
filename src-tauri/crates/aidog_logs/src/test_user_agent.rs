//! proxy_log.user_agent / client 列：写入 → 详情读回 / 列表摘要读回（三文件库，错路由当场炸）。

use crate::*;
use aidog_db::models::ProxyLogFilter;
use aidog_db::test_support::{ensure_test_master_key, sample_log, three_file_db};

#[tokio::test]
async fn user_agent_roundtrips_in_detail_and_summary() {
    ensure_test_master_key();
    let (db, _dir) = three_file_db().await;
    let mut log = sample_log("ua-1", "g", aidog_db::now());
    log.client = "claude_code".into();
    log.user_agent = "claude-cli/2.0.1 (external, cli)".into();
    upsert_proxy_log(&db, log).await.unwrap();

    let got = get_proxy_log(&db, "ua-1").await.unwrap().unwrap();
    assert_eq!(got.user_agent, "claude-cli/2.0.1 (external, cli)");
    assert_eq!(got.client, "claude_code");

    let page = filtered_list_proxy_logs(&db, &ProxyLogFilter::default(), 10, 0).await.unwrap();
    let row = page.items.iter().find(|r| r.id == "ua-1").expect("summary row");
    assert_eq!(row.user_agent, "claude-cli/2.0.1 (external, cli)");
    assert_eq!(row.client, "claude_code");
}

/// Claude Code 网关提示头列（migration 20261110-01）：写入 → 详情 / 摘要读回；缺省 ''。
#[tokio::test]
async fn claude_hint_columns_roundtrip_in_detail_and_summary() {
    ensure_test_master_key();
    let (db, _dir) = three_file_db().await;
    let mut log = sample_log("ch-1", "g", aidog_db::now());
    log.request_class = "subagent".into();
    log.agent_type = "Explore".into();
    upsert_proxy_log(&db, log).await.unwrap();
    upsert_proxy_log(&db, sample_log("ch-2", "g", aidog_db::now())).await.unwrap();

    let got = get_proxy_log(&db, "ch-1").await.unwrap().unwrap();
    assert_eq!((got.request_class.as_str(), got.agent_type.as_str()), ("subagent", "Explore"));
    let page = filtered_list_proxy_logs(&db, &ProxyLogFilter::default(), 10, 0).await.unwrap();
    let row = page.items.iter().find(|r| r.id == "ch-1").expect("summary row");
    assert_eq!((row.request_class.as_str(), row.agent_type.as_str()), ("subagent", "Explore"));
    let empty = page.items.iter().find(|r| r.id == "ch-2").expect("summary row");
    assert_eq!((empty.request_class.as_str(), empty.agent_type.as_str()), ("", ""));
}
