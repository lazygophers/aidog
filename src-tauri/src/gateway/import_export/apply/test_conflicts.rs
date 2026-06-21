//! conflicts.rs 测试：detect_conflicts 不为 platform scope 报冲突。

use super::detect_conflicts;
use crate::gateway::db::Db;
use crate::gateway::import_export::{Manifest, Payload};
use serde_json::json;

/// 内存库（同 db.rs test 约定）。
async fn test_db() -> Db {
    let db = Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    db
}

fn platform_payload(name: &str, base_url: &str) -> serde_json::Value {
    json!({
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

fn payload(platforms: Vec<serde_json::Value>) -> Payload {
    Payload {
        manifest: Manifest {
            format_version: 1,
            aidog_version: "test".to_string(),
            created_at: "2026-06-17T00:00:00Z".to_string(),
            source_machine: "test".to_string(),
            scopes: vec![crate::gateway::import_export::SCOPE_PLATFORM.to_string()],
            checksum: String::new(),
        },
        platform: platforms,
        group: Vec::new(),
        group_platform: Vec::new(),
        setting: Vec::new(),
        codex_global: None,
        codex_profiles: Vec::new(),
        claude_code_global: None,
        claude_code_group_settings: Vec::new(),
        skills: Vec::new(),
    }
}

/// platform.name 非唯一（数据模型不变量，见 db.rs init_tables 内联 platform 表定义）。
/// upsert_platform_row 已改为 always-INSERT（删 SELECT-by-name→UPDATE）。
/// （runtime 多行验证受 tokio_rusqlite `:memory:` 多-call ConnectionClosed harness 限制，
///  留 dev 验收；schema 不变量 + always-insert 代码路径已覆盖诉求。）
///
/// detect_conflicts 不再为 platform scope 报冲突（name 非唯一，无覆盖语义）。
/// 即使 payload 含 platform 且 db 预置同 name，detect_conflicts 也不扫 platform → 输出无 platform 项。
#[tokio::test]
async fn detect_conflicts_no_platform_conflict() {
    let db = test_db().await;
    // 预置一个同名 platform（裸 INSERT，避开 apply 事务路径）。
    db.0
        .call(|conn| {
            conn.execute(
                "INSERT INTO platform (name, created_at, updated_at) VALUES (?1, 0, 0)",
                rusqlite::params!["Dup"],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    // 扫一个同 name 的 incoming platform payload → 不应报 platform 冲突。
    let conflicts = detect_conflicts(&payload(vec![platform_payload("Dup", "https://b.example.com")]), &db).await.unwrap();
    let platform_conflicts: Vec<_> = conflicts.iter().filter(|c| c.scope == crate::gateway::import_export::SCOPE_PLATFORM).collect();
    assert!(platform_conflicts.is_empty(), "platform scope 不应再报 name 冲突");
}
