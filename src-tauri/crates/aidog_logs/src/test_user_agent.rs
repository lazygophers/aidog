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

/// Claude Code 网关提示头七列（migration 20261110-03）：写入 → 详情读回；prompt_id / session_id 可筛选。
#[tokio::test]
async fn claude_hint_ids_roundtrip_and_filter() {
    ensure_test_master_key();
    let (db, _dir) = three_file_db().await;
    let mut a = sample_log("ci-1", "g", aidog_db::now());
    a.prompt_id = "p-1".into();
    a.session_id = "s-1".into();
    a.agent_id = "ag-1".into();
    a.parent_agent_id = "ag-0".into();
    a.compaction = "auto".into();
    a.context_compacted = "manual".into();
    a.prev_tool_durations = "Bash=742;Read=9".into();
    upsert_proxy_log(&db, a).await.unwrap();
    let mut b = sample_log("ci-2", "g", aidog_db::now());
    b.prompt_id = "p-2".into();
    b.session_id = "s-1".into();
    upsert_proxy_log(&db, b).await.unwrap();
    upsert_proxy_log(&db, sample_log("ci-3", "g", aidog_db::now())).await.unwrap();

    let got = get_proxy_log(&db, "ci-1").await.unwrap().unwrap();
    assert_eq!((got.prompt_id.as_str(), got.session_id.as_str()), ("p-1", "s-1"));
    assert_eq!((got.agent_id.as_str(), got.parent_agent_id.as_str()), ("ag-1", "ag-0"));
    assert_eq!((got.compaction.as_str(), got.context_compacted.as_str()), ("auto", "manual"));
    assert_eq!(got.prev_tool_durations, "Bash=742;Read=9");
    let none = get_proxy_log(&db, "ci-3").await.unwrap().unwrap();
    assert_eq!(none.prompt_id, "");

    let ids = |f: ProxyLogFilter| {
        let db = &db;
        async move {
            let mut v: Vec<String> = filtered_list_proxy_logs(db, &f, 10, 0).await.unwrap().items.into_iter().map(|r| r.id).collect();
            v.sort();
            v
        }
    };
    assert_eq!(ids(ProxyLogFilter { prompt_id: Some("p-1".into()), ..Default::default() }).await, ["ci-1"]);
    assert_eq!(ids(ProxyLogFilter { session_id: Some(" s-1 ".into()), ..Default::default() }).await, ["ci-1", "ci-2"]);
    assert_eq!(ids(ProxyLogFilter { prompt_id: Some("p-2".into()), session_id: Some("s-1".into()), ..Default::default() }).await, ["ci-2"]);
    assert_eq!(ids(ProxyLogFilter { prompt_id: Some("".into()), ..Default::default() }).await.len(), 3);
}
