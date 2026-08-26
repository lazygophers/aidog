#![cfg(test)]
use super::*;
use crate::models::{ModelEntry, PlatformPreset};
use crate::test_support::test_db;

fn entry(platform: &str, model: &str, canonical: &str, official: bool) -> ModelEntry {
    ModelEntry {
        platform_code: platform.to_string(),
        model_id: model.to_string(),
        canonical_model: canonical.to_string(),
        family: "glm".to_string(),
        version: "4.6".to_string(),
        predecessor: "glm-4.5".to_string(),
        capabilities: vec!["text".to_string(), "tool_use".to_string()],
        builtin_tools_excluded: vec!["WebSearch".to_string()],
        max_input_tokens: Some(131072),
        max_output_tokens: Some(32768),
        context_window: Some(131072),
        official,
        price_data: r#"{"model_id":"glm-4.6","input_cost_per_token":1.1e-6}"#.to_string(),
        updated_at: 0,
    }
}

#[tokio::test]
async fn upsert_roundtrip_preserves_all_columns() {
    let db = test_db().await;
    upsert_model_entries(&db, vec![entry("glm", "glm-4.6", "glm-4.6", true)]).await.unwrap();

    let got = get_model_entry(&db, "glm", "glm-4.6").await.unwrap().expect("row");
    assert_eq!(got.canonical_model, "glm-4.6");
    assert_eq!(got.family, "glm");
    assert_eq!(got.version, "4.6");
    assert_eq!(got.predecessor, "glm-4.5");
    assert_eq!(got.capabilities, vec!["text".to_string(), "tool_use".to_string()]);
    assert_eq!(got.builtin_tools_excluded, vec!["WebSearch".to_string()]);
    assert_eq!(got.max_input_tokens, Some(131072));
    assert_eq!(got.max_output_tokens, Some(32768));
    assert_eq!(got.context_window, Some(131072));
    assert!(got.official);
    assert!(got.price_data.contains("glm-4.6"));
    assert!(got.updated_at > 0, "updated_at 由写入层填当前时间");
}

#[tokio::test]
async fn upsert_is_idempotent_on_composite_key() {
    let db = test_db().await;
    let e = entry("glm", "glm-4.6", "glm-4.6", true);
    upsert_model_entries(&db, vec![e.clone(), e.clone()]).await.unwrap();
    upsert_model_entries(&db, vec![e]).await.unwrap();
    assert_eq!(count_model_entries(&db).await.unwrap(), 1);

    // 同 model_id 换平台 = 另一行，不覆盖。
    upsert_model_entries(&db, vec![entry("aihubmix", "glm-4.6", "glm-4.6", false)]).await.unwrap();
    assert_eq!(count_model_entries(&db).await.unwrap(), 2);

    // 同键再写，字段整行覆盖。
    let mut changed = entry("glm", "glm-4.6", "glm-4.6", true);
    changed.max_output_tokens = Some(65536);
    changed.capabilities = vec!["text".to_string()];
    upsert_model_entries(&db, vec![changed]).await.unwrap();
    let got = get_model_entry(&db, "glm", "glm-4.6").await.unwrap().unwrap();
    assert_eq!(got.max_output_tokens, Some(65536));
    assert_eq!(got.capabilities, vec!["text".to_string()]);
    assert_eq!(count_model_entries(&db).await.unwrap(), 2);
}

#[tokio::test]
async fn list_filters_by_platform_and_orders_stably() {
    let db = test_db().await;
    upsert_model_entries(
        &db,
        vec![
            entry("glm", "glm-4.6", "glm-4.6", true),
            entry("glm", "glm-4.5", "glm-4.5", true),
            entry("aihubmix", "glm-4.6", "glm-4.6", false),
        ],
    )
    .await
    .unwrap();

    let all = list_model_entries(&db, None).await.unwrap();
    assert_eq!(
        all.iter().map(|e| (e.platform_code.as_str(), e.model_id.as_str())).collect::<Vec<_>>(),
        vec![("aihubmix", "glm-4.6"), ("glm", "glm-4.5"), ("glm", "glm-4.6")]
    );

    let one = list_model_entries(&db, Some("glm")).await.unwrap();
    assert_eq!(one.len(), 2);
    // 表非空但该平台无条目 → 照实返回空，不回落 bundled。
    assert!(list_model_entries(&db, Some("no-such-platform")).await.unwrap().is_empty());
}

#[tokio::test]
async fn group_by_canonical_picks_official_as_primary() {
    let groups = group_by_canonical(vec![
        entry("zzz", "glm-4.6", "glm-4.6", false),
        entry("glm", "glm-4.6", "glm-4.6", true),
        entry("aihubmix", "glm-4.5", "glm-4.5", false),
    ]);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].canonical_model, "glm-4.5");
    // 无 official → 取 platform_code 字典序第一条
    assert_eq!(groups[0].primary_platform, "aihubmix");
    assert_eq!(groups[1].canonical_model, "glm-4.6");
    assert_eq!(groups[1].primary_platform, "glm");
    assert_eq!(
        groups[1].entries.iter().map(|e| e.platform_code.as_str()).collect::<Vec<_>>(),
        vec!["glm", "zzz"]
    );
}

#[tokio::test]
async fn platform_preset_upsert_overwrites_whole_json() {
    let db = test_db().await;
    upsert_platform_presets(&db, vec![PlatformPreset { code: "glm".into(), preset_data: r##"{"color":"#111111"}"##.into(), updated_at: 0 }]).await.unwrap();
    upsert_platform_presets(&db, vec![PlatformPreset { code: "glm".into(), preset_data: r##"{"color":"#222222"}"##.into(), updated_at: 0 }]).await.unwrap();
    let rows = list_platform_presets(&db).await.unwrap();
    let glm = rows.iter().find(|p| p.code == "glm").expect("glm preset");
    assert_eq!(glm.preset_data, r##"{"color":"#222222"}"##);
    // 未在本次 upsert 传入的平台不受影响（票 12 best-effort 语义的底座）。
    upsert_platform_presets(&db, vec![PlatformPreset { code: "kimi".into(), preset_data: "{}".into(), updated_at: 0 }]).await.unwrap();
    let rows = list_platform_presets(&db).await.unwrap();
    assert_eq!(rows.iter().find(|p| p.code == "glm").unwrap().preset_data, r##"{"color":"#222222"}"##);
}

#[tokio::test]
async fn bundled_fallback_serves_empty_db() {
    let db = test_db().await;
    assert_eq!(count_model_entries(&db).await.unwrap(), 0);

    let all = list_model_entries(&db, None).await.unwrap();
    assert!(!all.is_empty(), "DB 空时应回落编译期内置 registry");
    assert_eq!(all.len(), bundled_model_entries().len());

    let presets = list_platform_presets(&db).await.unwrap();
    assert_eq!(presets.len(), bundled_platform_presets().len());
    assert!(presets.iter().any(|p| p.code == "anthropic"));

    let sample = &bundled_model_entries()[0];
    let got = get_model_entry(&db, &sample.platform_code, &sample.model_id).await.unwrap();
    assert_eq!(got.map(|e| e.model_id), Some(sample.model_id.clone()));

    let snap = model_info_snapshot(&db).await.unwrap();
    assert!(snap.bundled);
    assert!(!snap.groups.is_empty());
    assert!(!snap.platforms.is_empty());
}

#[tokio::test]
async fn db_rows_win_over_bundled() {
    let db = test_db().await;
    upsert_model_entries(&db, vec![entry("glm", "glm-4.6", "glm-4.6", true)]).await.unwrap();
    let all = list_model_entries(&db, None).await.unwrap();
    assert_eq!(all.len(), 1, "DB 非空即完全接管，不与 bundled 合并");

    let snap = model_info_snapshot(&db).await.unwrap();
    assert!(!snap.bundled);
    assert_eq!(snap.groups.len(), 1);
    // 平台预设表仍为空 → 该维度独立回落 bundled。
    assert_eq!(snap.platforms.len(), bundled_platform_presets().len());
}

#[test]
fn from_json_defaults_canonical_to_model_id() {
    let e = model_entry_from_json("glm", r#"{"model_id":"glm-4.6"}"#).expect("parse");
    assert_eq!(e.canonical_model, "glm-4.6");
    assert!(e.capabilities.is_empty());
    assert!(!e.official);
    assert_eq!(e.max_output_tokens, None);
    assert_eq!(e.price_data, r#"{"model_id":"glm-4.6"}"#);

    assert!(model_entry_from_json("glm", r#"{"canonical_model":"x"}"#).is_none(), "缺 model_id 跳过");
    assert!(model_entry_from_json("glm", "not json").is_none());
}

#[test]
fn bundled_registry_entries_are_wellformed() {
    let entries = bundled_model_entries();
    assert!(entries.len() > 500, "registry 现有 900+ 模型条目，实测 {}", entries.len());
    assert!(entries.iter().all(|e| !e.model_id.is_empty() && !e.canonical_model.is_empty()));
    assert!(entries.iter().all(|e| !e.platform_code.is_empty()));
    let presets = bundled_platform_presets();
    assert!(presets.iter().all(|p| p.preset_data.starts_with('{')));
    assert!(presets.windows(2).all(|w| w[0].code < w[1].code), "code 升序且无重复");
}

/// `get_defaults_json` 的数据源：DB 有同步数据即以 DB 为准，
/// 从未同步过才回落编译期内置那份（`~/.aidog/platform-presets.json` 本地文件层已彻底移除）。
#[tokio::test]
async fn presets_doc_json_prefers_db_over_bundled() {
    let db = test_db().await;
    let empty = presets_doc_json(&db).await.unwrap();
    assert_eq!(empty, crate::registry::presets_json(), "DB 空 → 原样回落 bundled");

    upsert_platform_presets(
        &db,
        vec![PlatformPreset {
            code: "glm".into(),
            preset_data: r#"{"name":{"en-US":"Zhipu Renamed"}}"#.into(),
            updated_at: 0,
        }],
    )
    .await
    .unwrap();

    let doc: serde_json::Value = serde_json::from_str(&presets_doc_json(&db).await.unwrap()).unwrap();
    let protocols = doc["protocols"].as_object().expect("protocols");
    assert_eq!(protocols.len(), 1, "DB 非空即完全接管，不与 bundled 合并");
    assert_eq!(protocols["glm"]["name"]["en-US"], "Zhipu Renamed");
    assert!(doc["last_updated"].as_i64().unwrap_or(0) > 0, "last_updated 取行 updated_at 最大值（秒）");
}
