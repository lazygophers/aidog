//! 代理日志-MITM隧道 票 01：一次性迁移删 proxy_log 中非 AI 隧道行
//! （source_protocol='http-connect' AND platform_id=0）；AI 隧道行与普通请求行不动。
#![cfg(test)]
use super::test_support::*;

#[tokio::test]
async fn migration_purges_only_non_ai_connect_rows() {
    let (db, _dir) = three_file_db().await;
    db.call_proxy_log_traced(None, std::panic::Location::caller(), |c| {
        c.execute(
            "INSERT INTO proxy_log (id, platform_id, group_key, model, source_protocol, \
             status_code, est_cost, blocked_by, blocked_reason, created_at, deleted_at) VALUES \
             ('non_ai', 0, '', '', 'http-connect', 200, 0, '', 'mitm_opaque', 1, 0), \
             ('ai', 7, '', '', 'http-connect', 200, 0, '', 'mitm_opaque', 1, 0), \
             ('chat', 0, '', 'm', 'anthropic', 200, 0, '', '', 1, 0)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    // 重跑 init（= 重启）触发 log.db 迁移，再读回
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    let ids: Vec<String> = db
        .call_read_proxy_log_traced(None, std::panic::Location::caller(), |c| {
            let mut s = c.prepare("SELECT id FROM proxy_log ORDER BY id")?;
            s.query_map([], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()
        })
        .await
        .unwrap();
    assert_eq!(ids, vec!["ai".to_string(), "chat".to_string()]);
}
