#![cfg(test)]
use super::*;
use aidog_test_util::mock_app_with_db;
use tauri::Manager;

fn sample_create(name: &str, auto_group: Option<bool>, join: Option<Vec<u64>>) -> CreatePlatform {
    CreatePlatform {
        name: name.into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://example.invalid".into(),
        api_key: "k".into(),
        extra: String::new(),
        models: None,
        available_models: None,
        endpoints: None,
        manual_budgets: None,
        auto_group,
        join_group_ids: join, default_level_priority: None, expires_at: None,
    }
}

#[tokio::test]
async fn create_list_get_update_delete_flow() {
    let app = mock_app_with_db().await;
    let db = app.state::<aidog_core::gateway::db::Db>();

    // create with auto_group
    let p = platform_create(sample_create("P1", Some(true), None), db.clone()).await.unwrap();
    assert_eq!(p.name, "P1");

    // list (balance_level computed path)
    let list = platform_list(db.clone()).await.unwrap();
    assert_eq!(list.len(), 1);
    assert!(!list[0].balance_level.is_empty());

    // get found + not found
    assert!(platform_get(p.id, db.clone()).await.unwrap().is_some());
    assert!(platform_get(999999, db.clone()).await.unwrap().is_none());

    // update
    let upd = UpdatePlatform {
        id: p.id,
        name: Some("P1-renamed".into()),
        platform_type: None,
        base_url: None,
        api_key: None,
        extra: None,
        models: None,
        available_models: None,
        endpoints: None,
        enabled: None,
        status: None,
        manual_budgets: None,
        join_group_ids: Some(vec![]),
        expires_at: None,
    };
    let p2 = platform_update(upd, db.clone()).await.unwrap();
    assert_eq!(p2.name, "P1-renamed");

    // reorder (single)
    platform_reorder(vec![p.id], db.clone()).await.unwrap();

    // delete
    platform_delete(p.id, db.clone()).await.unwrap();
    assert!(platform_get(p.id, db.clone()).await.unwrap().is_none());
}

#[tokio::test]
async fn create_without_auto_group_and_join_groups() {
    let app = mock_app_with_db().await;
    let db = app.state::<aidog_core::gateway::db::Db>();
    // no auto group + empty join
    let p = platform_create(sample_create("NA", Some(false), Some(vec![])), db.clone()).await.unwrap();
    assert!(p.id > 0);
}

#[tokio::test]
async fn ensure_auto_group_idempotent() {
    let app = mock_app_with_db().await;
    let db = app.state::<aidog_core::gateway::db::Db>();
    // create without auto group, then ensure
    let p = platform_create(sample_create("E1", Some(false), None), db.clone()).await.unwrap();
    platform_ensure_auto_group(p.id, db.clone()).await.unwrap();
    // second call is a no-op (already has auto group)
    platform_ensure_auto_group(p.id, db.clone()).await.unwrap();
    // missing platform errs
    assert!(platform_ensure_auto_group(999999, db.clone()).await.is_err());
}

#[tokio::test]
async fn purge_disabled_returns_result() {
    let app = mock_app_with_db().await;
    let db = app.state::<aidog_core::gateway::db::Db>();
    // no disabled platforms → empty result, global scope
    let res = platform_purge_disabled(None, db.clone()).await.unwrap();
    assert!(res.deleted_ids.is_empty());
}

#[tokio::test]
async fn tray_config_and_today_stats() {
    let app = mock_app_with_db().await;
    let db = app.state::<aidog_core::gateway::db::Db>();
    // default tray config (no config yet)
    let cfg = tray_config_get(db.clone()).await.unwrap();
    let _ = cfg;
    // today stats
    let stats = tray_today_stats(db.clone()).await.unwrap();
    let _ = stats;
}

// ── SharePlatform: skip_serializing_if 空值剔除 (serde 层) ──
//
// 平台分享串 YAML/JSON/Base64 三格式统一在 serde 层剔空值。
// 这里走 serde_yml::to_string 直接验证序列化产物，绕开 DB / tauri command。
// PlatformModels 经 commands/platform.rs 的 `use gateway::models::*` 引入（super::* 链）。

use aidog_core::gateway::models::PlatformModels;
use aidog_core::gateway::models::Protocol;

fn empty_share() -> SharePlatform {
    SharePlatform {
        aidog_platform_share: 1,
        name: "P".into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://example.invalid/v1".into(),
        api_key: "k".into(),
        extra: String::new(),
        models: PlatformModels::default(),
        available_models: vec![],
        endpoints: vec![],
        manual_budgets: vec![],
    }
}

#[test]
fn share_empty_fields_skipped_in_yaml() {
    let s = empty_share();
    let yaml = serde_yml::to_string(&s).expect("serialize");
    // 必保留字段
    assert!(yaml.contains("aidog_platform_share:"), "marker kept: {yaml}");
    assert!(yaml.contains("name: P"), "name kept: {yaml}");
    assert!(yaml.contains("base_url:"), "base_url kept: {yaml}");
    assert!(yaml.contains("api_key:"), "api_key kept (even non-empty here): {yaml}");
    // 空值字段必须从串里消失
    assert!(!yaml.contains("extra:"), "empty extra skipped: {yaml}");
    assert!(!yaml.contains("models:"), "empty models skipped: {yaml}");
    assert!(!yaml.contains("available_models:"), "empty available_models skipped: {yaml}");
    assert!(!yaml.contains("endpoints:"), "empty endpoints skipped: {yaml}");
    assert!(!yaml.contains("manual_budgets:"), "empty manual_budgets skipped: {yaml}");
}

#[test]
fn share_empty_api_key_still_present() {
    // api_key 即便为空串也必须保留（分享核心字段，空 key 便于接收端察觉异常）
    let mut s = empty_share();
    s.api_key = String::new();
    let yaml = serde_yml::to_string(&s).expect("serialize");
    assert!(yaml.contains("api_key:"), "empty api_key still present: {yaml}");
}

#[test]
fn share_nonempty_models_field_kept() {
    // 任一 models 槽位有值 → 整块 models key 保留
    let mut s = empty_share();
    s.models.sonnet = Some("claude-sonnet-4".into());
    let yaml = serde_yml::to_string(&s).expect("serialize");
    assert!(yaml.contains("models:"), "models block kept when slot set: {yaml}");
    assert!(yaml.contains("sonnet: claude-sonnet-4"), "sonnet slot value present: {yaml}");
    // PlatformModels 槽位自身 skip_serializing_if Option::is_none，未设槽位不出现在 models 块里
    assert!(!yaml.contains("default:"), "unset models.default skipped inside block: {yaml}");
    // 其余空字段仍剔除
    assert!(!yaml.contains("extra:"), "extra still skipped: {yaml}");
    assert!(!yaml.contains("available_models:"), "available_models still skipped: {yaml}");
}

#[test]
fn share_roundtrip_empty_equivalent() {
    // round-trip: 导出串 → serde_yml 反序列化 → 缺字段回填 default，语义等价
    let s = empty_share();
    let yaml = serde_yml::to_string(&s).expect("serialize");
    let parsed: SharePlatform = serde_yml::from_str(&yaml).expect("parse");
    assert_eq!(parsed.aidog_platform_share, 1);
    assert_eq!(parsed.name, "P");
    assert_eq!(parsed.api_key, "k");
    // skip 后缺字段回 default
    assert_eq!(parsed.extra, "", "extra back to empty default");
    assert!(parsed.models.is_empty(), "models back to all-None default");
    assert!(parsed.available_models.is_empty());
    assert!(parsed.endpoints.is_empty());
    assert!(parsed.manual_budgets.is_empty());
}

#[test]
fn share_parse_accepts_string_without_optional_keys() {
    // 接收端解析: 仅 marker+必填的极简串（模拟他人转发的清爽串）应成功
    let minimal = r#"
aidog_platform_share: 1
name: P2
platform_type: anthropic
base_url: https://example.invalid/v1
api_key: k2
"#;
    let parsed: SharePlatform = serde_yml::from_str(minimal).expect("parse minimal");
    assert_eq!(parsed.name, "P2");
    assert_eq!(parsed.api_key, "k2");
    assert_eq!(parsed.extra, "");
    assert!(parsed.models.is_empty());
}
