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
