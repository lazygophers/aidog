#![cfg(test)]
//! 测试共享 helper（自原 db.rs 单一 tests 模块拆出，pub(crate) 供各 tests_* 子模块复用）。
use super::*;
use rusqlite::{params};


    /// 创建一个初始化好的内存库
    pub(crate) async fn test_db() -> Db {
        let db = Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");
        db
    }


    pub(crate) fn sample_platform(name: &str) -> CreatePlatform {
        CreatePlatform {
            name: name.to_string(),
            platform_type: Protocol::Anthropic,
            base_url: "https://example.com".to_string(),
            api_key: "sk-test".to_string(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
        }
    }


    pub(crate) fn sample_group(name: &str, mappings: Vec<ModelMapping>) -> CreateGroup {
        CreateGroup {
            name: name.to_string(),
            group_key: Some(name.to_string()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: 2,
            model_mappings: mappings,
        }
    }


    pub(crate) fn sample_log(id: &str, group_key: &str, created_at: i64) -> ProxyLog {
        ProxyLog {
            id: id.to_string(),
            group_key: group_key.to_string(),
            model: "claude-sonnet-4".to_string(),
            actual_model: "glm-4-plus".to_string(),
            source_protocol: "anthropic".to_string(),
            target_protocol: "openai".to_string(),
            platform_id: 1,
            request_headers: String::new(),
            request_body: String::new(),
            upstream_request_headers: String::new(),
            upstream_request_body: String::new(),
            response_body: String::new(),
            request_url: String::new(),
            upstream_request_url: String::new(),
            upstream_response_headers: String::new(),
            upstream_status_code: 200,
            user_response_headers: String::new(),
            user_response_body: String::new(),
            status_code: 200,
            duration_ms: 100,
            input_tokens: 10,
            output_tokens: 20,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: false,
            attempts: Vec::new(),
            retry_count: 0,
            blocked_by: String::new(),
            blocked_reason: String::new(),
            created_at,
            updated_at: created_at,
            deleted_at: 0,
        }
    }


    // ─── DB Vacuum/Hard-delete (Tier 1 + Tier 2) ───────────────

    /// 辅助：插入一行 proxy_log（指定 created_at），返回 id。
    pub(crate) async fn insert_proxy_log_at(db: &Db, created_at: i64) -> String {
        let id = format!("test-{created_at}");
        let id_clone = id.clone();
        db
            .call_traced(None, std::panic::Location::caller(), move |conn| {
                conn.execute(
                    "INSERT INTO proxy_log (id, platform_id, group_key, model, source_protocol, \
                     status_code, input_tokens, output_tokens, cache_tokens, est_cost, is_stream, \
                     created_at, deleted_at) \
                     VALUES (?1, 0, '', 'm', 'anthropic', 200, 10, 5, 0, 0.001, 0, ?2, 0)",
                    params![id_clone, created_at],
                )?;
                Ok(())
            })
            .await
            .expect("insert test proxy_log");
        id
    }


    /// 辅助：COUNT(*) FROM proxy_log（含 tombstone，不过滤 deleted_at）。
    pub(crate) async fn count_all_proxy_logs(db: &Db) -> i64 {
        db
            .call_traced(None, std::panic::Location::caller(), |conn| Ok(conn.query_row("SELECT COUNT(*) FROM proxy_log", [], |r| r.get(0))?))
            .await
            .unwrap()
    }
