#![cfg(test)]
use super::*;
use aidog_db as db;
use aidog_db::test_support::test_db;

// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
// 故不经 tauri::State 走 command 包装层，直测 command 转发/编排的 db:: 函数
// （create_auto_group_for 等 pub(crate) 编排逻辑经 super::* 直接复用，逻辑等价）。

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
        join_group_ids: join,
        expires_at: None,
    }
}

/// platform_create command 的编排逻辑（auto_group + join_group_ids）内联复刻，绕开 State。
async fn create_platform_via_db(db: &Db, input: CreatePlatform) -> Result<Platform, String> {
    let auto_group = input.auto_group.unwrap_or(true);
    let join_group_ids = input.join_group_ids.clone().unwrap_or_default();
    let platform = db::create_platform(db, input).await?;
    if auto_group {
        create_auto_group_for(db, &platform).await?;
    }
    if !join_group_ids.is_empty() {
        let _ = db::sync_platform_manual_groups(db, platform.id, &join_group_ids).await;
    }
    Ok(platform)
}

#[tokio::test]
async fn create_list_get_update_delete_flow() {
    let db = test_db().await;

    // create with auto_group
    let p = create_platform_via_db(&db, sample_create("P1", Some(true), None))
        .await
        .unwrap();
    assert_eq!(p.name, "P1");

    // list (balance_level computed path)
    let list = db::list_platforms(&db).await.unwrap();
    assert_eq!(list.len(), 1);

    // get found + not found
    assert!(db::get_platform(&db, p.id).await.unwrap().is_some());
    assert!(db::get_platform(&db, 999999).await.unwrap().is_none());

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
    let p2 = db::update_platform(&db, upd).await.unwrap();
    assert_eq!(p2.name, "P1-renamed");

    // reorder (single)
    db::reorder_platforms(&db, &[p.id]).await.unwrap();

    // delete
    db::delete_platform(&db, p.id).await.unwrap();
    assert!(db::get_platform(&db, p.id).await.unwrap().is_none());
}

#[tokio::test]
async fn create_without_auto_group_and_join_groups() {
    let db = test_db().await;
    // no auto group + empty join
    let p = create_platform_via_db(&db, sample_create("NA", Some(false), Some(vec![])))
        .await
        .unwrap();
    assert!(p.id > 0);
}

#[tokio::test]
async fn ensure_auto_group_idempotent() {
    let db = test_db().await;
    // create without auto group, then ensure
    let p = create_platform_via_db(&db, sample_create("E1", Some(false), None))
        .await
        .unwrap();

    async fn ensure_auto_group(db: &Db, id: u64) -> Result<(), String> {
        let platform = match db::get_platform(db, id).await? {
            Some(p) => p,
            None => return Err(format!("platform {id} not found")),
        };
        let groups = db::list_groups(db).await.unwrap_or_default();
        let platform_id_str = platform.id.to_string();
        if groups
            .iter()
            .any(|g| g.auto_from_platform == platform_id_str)
        {
            return Ok(());
        }
        create_auto_group_for(db, &platform).await
    }

    ensure_auto_group(&db, p.id).await.unwrap();
    // second call is a no-op (already has auto group)
    ensure_auto_group(&db, p.id).await.unwrap();
    // missing platform errs
    assert!(ensure_auto_group(&db, 999999).await.is_err());
}

#[tokio::test]
async fn purge_disabled_returns_result() {
    let db = test_db().await;
    // no disabled platforms → empty result, global scope
    let res = db::purge_auto_disabled_platforms(&db, None).await.unwrap();
    assert!(res.deleted_ids.is_empty());
}

#[tokio::test]
async fn tray_config_and_today_stats() {
    let db = test_db().await;
    // default tray config (no config yet)
    let cfg = db::get_tray_config(&db).await.unwrap().unwrap_or_default();
    let _ = cfg;
    // today stats
    let stats = aidog_stats::today_stats(&db).await.unwrap();
    let _ = stats;
}

// ── SharePlatform: skip_serializing_if 空值剔除 (serde 层) ──
//
// 平台分享串 YAML/JSON/Base64 三格式统一在 serde 层剔空值。
// 这里走 serde_yml::to_string 直接验证序列化产物，绕开 DB / tauri command。
// PlatformModels 经 commands/platform.rs 的 `use gateway::models::*` 引入（super::* 链）。

use crate::gateway::models::PlatformModels;
use crate::gateway::models::Protocol;

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
    assert!(
        yaml.contains("aidog_platform_share:"),
        "marker kept: {yaml}"
    );
    assert!(yaml.contains("name: P"), "name kept: {yaml}");
    assert!(yaml.contains("base_url:"), "base_url kept: {yaml}");
    assert!(
        yaml.contains("api_key:"),
        "api_key kept (even non-empty here): {yaml}"
    );
    // 空值字段必须从串里消失
    assert!(!yaml.contains("extra:"), "empty extra skipped: {yaml}");
    assert!(!yaml.contains("models:"), "empty models skipped: {yaml}");
    assert!(
        !yaml.contains("available_models:"),
        "empty available_models skipped: {yaml}"
    );
    assert!(
        !yaml.contains("endpoints:"),
        "empty endpoints skipped: {yaml}"
    );
    assert!(
        !yaml.contains("manual_budgets:"),
        "empty manual_budgets skipped: {yaml}"
    );
}

#[test]
fn share_empty_api_key_still_present() {
    // api_key 即便为空串也必须保留（分享核心字段，空 key 便于接收端察觉异常）
    let mut s = empty_share();
    s.api_key = String::new();
    let yaml = serde_yml::to_string(&s).expect("serialize");
    assert!(
        yaml.contains("api_key:"),
        "empty api_key still present: {yaml}"
    );
}

#[test]
fn share_nonempty_models_field_kept() {
    // 任一 models 槽位有值 → 整块 models key 保留
    let mut s = empty_share();
    s.models.sonnet = Some("claude-sonnet-4".into());
    let yaml = serde_yml::to_string(&s).expect("serialize");
    assert!(
        yaml.contains("models:"),
        "models block kept when slot set: {yaml}"
    );
    assert!(
        yaml.contains("sonnet: claude-sonnet-4"),
        "sonnet slot value present: {yaml}"
    );
    // PlatformModels 槽位自身 skip_serializing_if Option::is_none，未设槽位不出现在 models 块里
    assert!(
        !yaml.contains("default:"),
        "unset models.default skipped inside block: {yaml}"
    );
    // 其余空字段仍剔除
    assert!(!yaml.contains("extra:"), "extra still skipped: {yaml}");
    assert!(
        !yaml.contains("available_models:"),
        "available_models still skipped: {yaml}"
    );
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
