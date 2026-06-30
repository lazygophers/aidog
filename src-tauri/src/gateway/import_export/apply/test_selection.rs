//! apply 白名单（Selection）过滤测试：仅勾选条目被写入，未勾选条目不落库。

use super::apply;
use crate::gateway::db::{list_all_settings_raw, Db};
use crate::gateway::import_export::{
    Manifest, Payload, Selection, SCOPE_PLATFORM, SCOPE_SETTING,
};

async fn test_db() -> Db {
    let db = Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    db
}

fn platform_value(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        // platform_type 列存 JSON 序列化字符串（含引号），与 create_platform 一致。
        "platform_type": "\"anthropic\"",
        "base_url": "https://a.example.com",
        "api_key": "sk-test",
        "extra": "{}",
        "models": "{}",
        "available_models": "[]",
        "endpoints": "[]",
        "enabled": true,
        "status": "enabled",
        "auto_disabled_until": 0,
        "auto_disable_strikes": 0,
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

fn payload_with(platforms: &[&str], settings: &[(&str, &str, &str)]) -> Payload {
    Payload {
        manifest: Manifest {
            format_version: 1,
            aidog_version: "test".into(),
            created_at: "now".into(),
            source_machine: "test".into(),
            scopes: vec![SCOPE_PLATFORM.into(), SCOPE_SETTING.into()],
            checksum: String::new(),
        },
        platform: platforms.iter().map(|n| platform_value(n)).collect(),
        group: Vec::new(),
        group_platform: Vec::new(),
        setting: settings
            .iter()
            .map(|(s, k, v)| [s.to_string(), k.to_string(), v.to_string()])
            .collect(),
        codex_global: None,
        codex_profiles: Vec::new(),
        claude_code_global: None,
        claude_code_group_settings: Vec::new(),
        skills: Vec::new(),
        mcp: Vec::new(),
        middleware: Vec::new(),
        model_price: Vec::new(),
    }
}

/// 直查未删除平台名（按 sort_order, id），避开 list_platforms 的 platform_type 反序列化。
async fn platform_names(db: &Db) -> Vec<String> {
    db.0
        .call(|conn| {
            let mut stmt = conn
                .prepare("SELECT name FROM platform WHERE deleted_at = 0 ORDER BY id")?;
            let v = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<String>, _>>()?;
            Ok(v)
        })
        .await
        .unwrap()
}

/// 白名单仅含 platform idx:0 + setting "ui:theme" → 仅这两条落库；idx:1 与另一 setting 被跳过。
#[tokio::test]
async fn apply_selection_filters_unchecked_items() {
    let db = test_db().await;
    let payload = payload_with(
        &["alpha", "beta"],
        &[("ui", "theme", "\"dark\""), ("ui", "locale", "\"zh-CN\"")],
    );

    let mut sel: Selection = Selection::new();
    sel.insert((SCOPE_PLATFORM.to_string(), "idx:0".to_string()));
    sel.insert((SCOPE_SETTING.to_string(), "ui:theme".to_string()));

    let report = apply(payload, &[], Some(&sel), &db).await.expect("apply ok");
    assert!(report.errors.is_empty(), "no errors: {:?}", report.errors);

    // 平台：只导入了 alpha（idx:0），beta 被过滤。
    assert_eq!(platform_names(&db).await, vec!["alpha".to_string()], "只导入勾选的 idx:0");

    // 设置：只导入了 ui:theme，ui:locale 被过滤。
    let settings = list_all_settings_raw(&db).await.unwrap();
    let keys: Vec<String> = settings
        .iter()
        .map(|(s, k, _)| format!("{s}:{k}"))
        .filter(|k| k.starts_with("ui:"))
        .collect();
    assert_eq!(keys, vec!["ui:theme".to_string()], "只导入勾选的 ui:theme");
}

/// selection = None → 不过滤，全部导入（旧行为 / 异源路径契约）。
#[tokio::test]
async fn apply_none_selection_imports_all() {
    let db = test_db().await;
    let payload = payload_with(&["alpha", "beta"], &[]);
    let report = apply(payload, &[], None, &db).await.expect("apply ok");
    assert!(report.errors.is_empty());
    assert_eq!(platform_names(&db).await.len(), 2, "None selection 导入全部平台");
}
