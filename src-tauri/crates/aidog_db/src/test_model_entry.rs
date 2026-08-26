#![cfg(test)]
use super::*;
use crate::models::{ModelEntry, PlatformPreset};
use crate::test_support::test_db;

fn entry(platform: &str, model: &str, canonical: &str, official: bool) -> ModelEntry {
    ModelEntry {
        platform_code: platform.to_string(),
        model_id: model.to_string(),
        display_name: String::new(),
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
    ], &Default::default());
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

#[tokio::test]
async fn display_name_roundtrips_and_falls_back_to_model_id() {
    let db = test_db().await;
    let mut named = entry("glm", "glm-4.6", "glm-4.6", true);
    named.display_name = "GLM-4.6".to_string();
    let blank = entry("glm", "glm-4.5", "glm-4.5", true); // 空串
    let mut spaces = entry("glm", "glm-4.4", "glm-4.4", true);
    spaces.display_name = "   ".to_string(); // 纯空白同样算缺省
    upsert_model_entries(&db, vec![named, blank, spaces]).await.unwrap();

    // 有值往返无损。
    let got = get_model_entry(&db, "glm", "glm-4.6").await.unwrap().unwrap();
    assert_eq!(got.display_name, "GLM-4.6");
    // 空串 / 空白回落 model_id，调用方拿到的恒非空。
    assert_eq!(get_model_entry(&db, "glm", "glm-4.5").await.unwrap().unwrap().display_name, "glm-4.5");
    assert_eq!(get_model_entry(&db, "glm", "glm-4.4").await.unwrap().unwrap().display_name, "glm-4.4");
    assert!(list_model_entries(&db, Some("glm")).await.unwrap().iter().all(|e| !e.display_name.is_empty()));

    // 改名后再写即覆盖（同键整行覆盖，展示名不粘旧值）。
    let mut renamed = entry("glm", "glm-4.6", "glm-4.6", true);
    renamed.display_name = "GLM 4.6 (Coding)".to_string();
    upsert_model_entries(&db, vec![renamed]).await.unwrap();
    assert_eq!(get_model_entry(&db, "glm", "glm-4.6").await.unwrap().unwrap().display_name, "GLM 4.6 (Coding)");
}

#[test]
fn group_display_name_comes_from_official_entry() {
    let mut third_party = entry("aihubmix", "glm-4.6", "glm-4.6", false);
    third_party.display_name = "GLM-4.6-preview".to_string();
    let mut official = entry("glm", "glm-4.6", "glm-4.6", true);
    official.display_name = "GLM-4.6".to_string();
    let groups = group_by_canonical(vec![third_party, official], &Default::default());
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].primary_platform, "glm");
    assert_eq!(groups[0].display_name, "GLM-4.6", "聚合行取 official 那条的展示名");
    // 各平台条目展示名各自独立，不被聚合行覆盖。
    assert_eq!(
        groups[0].entries.iter().map(|e| e.display_name.as_str()).collect::<Vec<_>>(),
        vec!["GLM-4.6-preview", "GLM-4.6"]
    );

    // 全员缺展示名 → 聚合行也回落 model_id，不出空白。
    let groups = group_by_canonical(vec![entry("glm", "glm-4.5", "glm-4.5", true)], &Default::default());
    assert_eq!(groups[0].display_name, "glm-4.5");
}

#[test]
fn from_json_reads_display_name_without_write_side_fallback() {
    let e = model_entry_from_json("glm", r#"{"model_id":"glm-4.6","display_name":"GLM-4.6"}"#).expect("parse");
    assert_eq!(e.display_name, "GLM-4.6");
    // 写入路径不回填 model_id：registry 缺省即空串入库，回落留给读取层。
    let e = model_entry_from_json("glm", r#"{"model_id":"glm-4.6"}"#).expect("parse");
    assert_eq!(e.display_name, "");
    // bundled 兜底是读取路径，展示名恒非空。
    assert!(bundled_model_entries().iter().all(|e| !e.display_name.is_empty()));
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
    assert_eq!(protocols["glm"]["name"]["en-US"], "Zhipu Renamed", "同 code 以 DB 行为准");
    // 票 13-C：DB 里没有的协议由 bundled 补齐，不能整篇接管
    assert_eq!(protocols.len(), bundled_platform_presets().len());
    assert!(protocols["anthropic"]["homepage"].is_string(), "DB 缺的协议须由 bundled 补齐");
    assert!(doc["last_updated"].as_i64().unwrap_or(0) > 0, "last_updated 取行 updated_at 最大值（秒）");

    // 同一条链在 list 视图上同样是并集
    let rows = list_platform_presets(&db).await.unwrap();
    assert_eq!(rows.len(), bundled_platform_presets().len());
    assert!(rows.iter().find(|p| p.code == "glm").unwrap().preset_data.contains("Zhipu Renamed"));
}

/// 票 13-H：单协议查询不重建整篇文档，DB 行优先、缺失回落 bundled。
#[tokio::test]
async fn preset_entry_reads_single_row_with_bundled_fallback() {
    let db = test_db().await;
    assert!(preset_entry(&db, "anthropic").await.unwrap().is_some(), "DB 空 → 回落 bundled");
    assert!(preset_entry(&db, "__never_exists__").await.unwrap().is_none());

    upsert_platform_presets(
        &db,
        vec![PlatformPreset { code: "anthropic".into(), preset_data: r#"{"logo_url":"newslug"}"#.into(), updated_at: 0 }],
    )
    .await
    .unwrap();
    let v = preset_entry(&db, "anthropic").await.unwrap().expect("row");
    assert_eq!(v["logo_url"], "newslug");
    // 未同步的协议照旧由 bundled 供给
    assert!(preset_entry(&db, "openai").await.unwrap().is_some());
}

/// 票 13-E：bundled 兜底逐键生效——首轮只成功一部分，失败那批仍读得到内置价，
/// 不会因为「表已非空」就整批掉进 fallback。
#[tokio::test]
async fn bundled_fallback_is_per_key_not_whole_table() {
    let db = test_db().await;
    let sample = &bundled_model_entries()[0];
    // 只同步了一个无关条目 → 表非空
    upsert_model_entries(&db, vec![entry("zzz-only", "zzz-model", "zzz-model", false)]).await.unwrap();
    assert!(count_model_entries(&db).await.unwrap() > 0);

    let got = get_model_entry(&db, &sample.platform_code, &sample.model_id).await.unwrap();
    assert_eq!(got.map(|e| e.model_id), Some(sample.model_id.clone()), "该键 DB 缺失 → 回落 bundled");
    assert!(get_model_entry(&db, "zzz-only", "no-such-model").await.unwrap().is_none());
}

/// 票 13-I：`pricing_only` 来源不能当聚合行的代表平台（用户根本选不到）。
#[test]
fn primary_platform_skips_pricing_only_sources() {
    let pricing_only: std::collections::HashSet<String> = ["mistral".to_string()].into_iter().collect();
    let mut official = entry("mistral", "codestral-2", "codestral-2", true);
    official.display_name = "Codestral".into();
    let groups = group_by_canonical(
        vec![official.clone(), entry("siliconflow", "codestral-2", "codestral-2", false)],
        &pricing_only,
    );
    assert_eq!(groups[0].primary_platform, "siliconflow", "official 但不可选 → 让给可选平台");

    // 全组都是 pricing_only → 只能退回它，否则代表平台空白
    let groups = group_by_canonical(vec![official], &pricing_only);
    assert_eq!(groups[0].primary_platform, "mistral");
}

/// 票 13-F：写入层出问题时返回失败清单而不是把整轮结果换成一个 Err
/// （调用方据此把失败的行并进 `PriceSyncResult.failures`，`last_sync_at` 照写）。
#[tokio::test]
async fn best_effort_upserts_report_failures_instead_of_aborting() {
    let db = test_db().await;
    let (ok, failed) = upsert_model_entries_best_effort(
        &db,
        vec![entry("glm", "glm-4.5", "glm-4.5", true), entry("glm", "glm-4.6", "glm-4.6", true)],
    )
    .await
    .unwrap();
    assert_eq!((ok, failed.len()), (2, 0));
    assert_eq!(count_model_entries(&db).await.unwrap(), 2);

    let (ok, failed) = upsert_platform_presets(
        &db,
        vec![PlatformPreset { code: "glm".into(), preset_data: "{}".into(), updated_at: 0 }],
    )
    .await
    .unwrap();
    assert_eq!((ok, failed.len()), (1, 0));
}

/// 票 13-K：导出走裸行，`display_name` 空串原样带出，不把读取层的回落值写死进备份。
#[tokio::test]
async fn select_returns_raw_display_name_for_export() {
    let db = test_db().await;
    upsert_model_entries(&db, vec![entry("glm", "glm-4.5", "glm-4.5", true)]).await.unwrap();
    let raw = select_model_entries(&db, Some("glm")).await.unwrap();
    assert_eq!(raw[0].display_name, "", "裸行保留空串 = 无展示名");
    let shown = list_model_entries(&db, Some("glm")).await.unwrap();
    assert_eq!(shown[0].display_name, "glm-4.5", "面向 UI 的读取入口才回落");
}
