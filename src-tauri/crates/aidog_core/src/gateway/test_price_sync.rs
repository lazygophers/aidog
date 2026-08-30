//! registry 同步的 mock 上游覆盖：index 驱动、逐文件 best-effort、两源回退、分类计数。
//!
//! stub server 用一张 `path -> body` 表当 registry 快照，缺表即 404——
//! 「某个文件 404」正是 best-effort 要守的分支（DB 旧行必须原样留着）。
use super::*;
use aidog_db::test_support::test_db;
use std::collections::BTreeMap;

/// 只认清单内路径的 registry stub，返回可直接当 base 用的 URL。
async fn spawn_registry(files: BTreeMap<String, String>) -> String {
    use axum::extract::Path;
    use axum::routing::get;
    let files = Arc::new(files);
    let app = axum::Router::new().route(
        "/{*path}",
        get(move |Path(path): Path<String>| {
            let files = files.clone();
            async move {
                match files.get(path.as_str()) {
                    Some(body) => (axum::http::StatusCode::OK, body.clone()),
                    None => (axum::http::StatusCode::NOT_FOUND, "not found".to_string()),
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    format!("http://{addr}")
}

fn index_with(last_updated: u64) -> String {
    format!(
        r#"{{"last_updated": {last_updated},
  "platforms": [
    {{"code": "alpha", "platform_file": "platforms/alpha/platform.json", "models_dir": "platforms/alpha/models", "models": ["a-1.json"]}},
    {{"code": "beta", "platform_file": "platforms/beta/platform.json", "models_dir": "platforms/beta/models", "models": ["b-1.json"]}}
  ],
  "pricing_only": [
    {{"code": "litellm", "models_dir": "platforms/litellm/models", "models": ["a-1.json"]}}
  ]}}"#
    )
}

const ALPHA_PLATFORM: &str = r##"{"name":{"en-US":"Alpha"},"logo_url":"alpha","color":"#111111"}"##;
const BETA_PLATFORM: &str = r##"{"name":{"en-US":"Beta"},"logo_url":"beta","color":"#222222"}"##;
const A1: &str = r#"{"model_id":"a-1","canonical_model":"a-1","official":true,"capabilities":["text"],"max_input_tokens":100}"#;
const B1: &str = r#"{"model_id":"b-1","canonical_model":"a-1","official":false,"capabilities":["text"]}"#;
const A1_LITELLM: &str = r#"{"model_id":"a-1","canonical_model":"a-1","official":false,"capabilities":["text"]}"#;

/// 完整快照：4 个文件全在（index.last_updated 可配，模拟上游发版）。
fn full_with(last_updated: u64) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("index.json".to_string(), index_with(last_updated)),
        ("platforms/alpha/platform.json".to_string(), ALPHA_PLATFORM.to_string()),
        ("platforms/beta/platform.json".to_string(), BETA_PLATFORM.to_string()),
        ("platforms/alpha/models/a-1.json".to_string(), A1.to_string()),
        ("platforms/beta/models/b-1.json".to_string(), B1.to_string()),
        ("platforms/litellm/models/a-1.json".to_string(), A1_LITELLM.to_string()),
    ])
}

fn full() -> BTreeMap<String, String> {
    full_with(1)
}

/// 同 `full()` 但 index.last_updated 更新：上游发布了新版本的模拟。
fn full_newer() -> BTreeMap<String, String> {
    full_with(2)
}

#[tokio::test]
async fn full_sync_upserts_every_file_from_index() {
    let db = test_db().await;
    let base = spawn_registry(full()).await;
    let r = sync_registry_from(&db, &[&base]).await.unwrap();

    // 2 platform.json + 3 模型文件（含 pricing_only 的 litellm）
    assert_eq!(r.total, 5);
    assert_eq!(r.added, 5);
    assert_eq!((r.updated, r.unchanged, r.failed), (0, 0, 0));
    assert!(r.failures.is_empty());

    let presets = aidog_db::select_platform_presets(&db).await.unwrap();
    assert_eq!(presets.iter().map(|p| p.code.as_str()).collect::<Vec<_>>(), ["alpha", "beta"]);
    let entries = aidog_db::select_model_entries(&db, None).await.unwrap();
    assert_eq!(entries.len(), 3);
    let a1 = entries.iter().find(|e| e.platform_code == "alpha").unwrap();
    assert_eq!(a1.model_id, "a-1");
    assert_eq!(a1.max_input_tokens, Some(100));
    assert!(a1.official);
    // 同步写库后 last_sync_at 落地，周期判定才会歇下来
    assert!(get_sync_settings(&db).await.last_sync_at > 0);
}

#[tokio::test]
async fn partial_failure_keeps_existing_rows_and_reports_files() {
    let db = test_db().await;
    // 先来一轮完整同步，DB 里有 beta 的品牌字段与模型条目
    let base = spawn_registry(full()).await;
    sync_registry_from(&db, &[&base]).await.unwrap();

    // 第二轮：beta 的两个文件从上游消失（404），alpha 的 platform.json 改了名字（index 更新）
    let mut broken = full_newer();
    broken.remove("platforms/beta/platform.json");
    broken.remove("platforms/beta/models/b-1.json");
    broken.insert("platforms/alpha/platform.json".to_string(), r##"{"name":{"en-US":"Alpha Renamed"},"logo_url":"alpha2","color":"#111111"}"##.to_string());
    let base = spawn_registry(broken).await;
    let r = sync_registry_from(&db, &[&base]).await.unwrap();

    assert_eq!(r.total, 5);
    assert_eq!(r.failed, 2);
    assert_eq!(r.updated, 1, "只有 alpha 的 platform.json 变了");
    assert_eq!(r.unchanged, 2, "alpha/litellm 两个模型条目内容未变");
    assert_eq!(r.added, 0);
    let mut files: Vec<&str> = r.failures.iter().map(|f| f.file.as_str()).collect();
    files.sort_unstable();
    assert_eq!(files, ["platforms/beta/models/b-1.json", "platforms/beta/platform.json"]);
    assert!(r.failures.iter().all(|f| f.error.contains("404")));

    // best-effort：失败平台的品牌字段与模型条目原样保留，没被清空也没被部分覆盖
    let presets = aidog_db::select_platform_presets(&db).await.unwrap();
    let beta = presets.iter().find(|p| p.code == "beta").expect("beta 行不可消失");
    assert_eq!(beta.preset_data, BETA_PLATFORM);
    let alpha = presets.iter().find(|p| p.code == "alpha").unwrap();
    assert!(alpha.preset_data.contains("Alpha Renamed"), "成功平台的新名字要生效");
    assert!(aidog_db::select_model_entries(&db, Some("beta")).await.unwrap().len() == 1);
}

/// 品牌字段整份随 `preset_data` 入库：8 locale 名字、logo slug、色值、keywords 数组、
/// source_urls 对象一个不丢，且模型条目的 `display_name` 同轮落地。
#[tokio::test]
async fn sync_carries_brand_fields_and_display_name() {
    const RICH_PLATFORM: &str = r##"{"name":{"en-US":"Alpha Inc","zh-Hans":"阿尔法"},"logo_url":"alpha","color":"#111111","homepage":"https://alpha.example.com","keywords":["alpha","阿尔法","aerfa"],"source_urls":{"docs":"https://alpha.example.com/docs","pricing":"https://alpha.example.com/pricing"}}"##;
    const RICH_MODEL: &str = r#"{"model_id":"a-1","display_name":"Alpha 1","canonical_model":"a-1","official":true,"capabilities":["text"]}"#;

    let db = test_db().await;
    let mut files = full();
    files.insert("platforms/alpha/platform.json".to_string(), RICH_PLATFORM.to_string());
    files.insert("platforms/alpha/models/a-1.json".to_string(), RICH_MODEL.to_string());
    let base = spawn_registry(files).await;
    sync_registry_from(&db, &[&base]).await.unwrap();

    let presets = aidog_db::select_platform_presets(&db).await.unwrap();
    let alpha = presets.iter().find(|p| p.code == "alpha").unwrap();
    let v: serde_json::Value = serde_json::from_str(&alpha.preset_data).unwrap();
    assert_eq!(v["name"]["zh-Hans"], "阿尔法");
    assert_eq!(v["logo_url"], "alpha");
    assert_eq!(v["color"], "#111111");
    assert_eq!(v["keywords"].as_array().unwrap().len(), 3, "keywords 数组不得被截断");
    assert_eq!(v["source_urls"]["pricing"], "https://alpha.example.com/pricing");

    let a1 = aidog_db::select_model_entries(&db, Some("alpha")).await.unwrap();
    assert_eq!(a1[0].display_name, "Alpha 1", "模型展示名随同步入库");
    // 上游没写 display_name 的条目在读取层回落 model_id，不留空单元格
    let b1 = aidog_db::list_model_entries(&db, Some("beta")).await.unwrap();
    assert_eq!(b1[0].display_name, "b-1");
}

/// 失败平台在 DB 里的品牌字段（含名字与 logo slug）逐字段原样保留，不被清空也不被部分覆盖。
#[tokio::test]
async fn failed_platform_keeps_brand_fields_intact() {
    const RICH_BETA: &str = r##"{"name":{"en-US":"Beta Inc","ja-JP":"ベータ"},"logo_url":"beta","color":"#222222","keywords":["beta"],"source_urls":{"docs":"https://beta.example.com/docs"}}"##;

    let db = test_db().await;
    let mut files = full();
    files.insert("platforms/beta/platform.json".to_string(), RICH_BETA.to_string());
    let base = spawn_registry(files.clone()).await;
    sync_registry_from(&db, &[&base]).await.unwrap();

    // 第二轮 beta 的 platform.json 404（index 更新才会真正拉文件）
    let mut broken = files;
    broken.insert("index.json".to_string(), index_with(2));
    broken.remove("platforms/beta/platform.json");
    let base = spawn_registry(broken).await;
    let r = sync_registry_from(&db, &[&base]).await.unwrap();
    assert_eq!(r.failed, 1);
    assert_eq!(r.failures[0].file, "platforms/beta/platform.json");

    let presets = aidog_db::select_platform_presets(&db).await.unwrap();
    let beta = presets.iter().find(|p| p.code == "beta").expect("beta 行不可消失");
    let v: serde_json::Value = serde_json::from_str(&beta.preset_data).unwrap();
    assert_eq!(v["name"]["en-US"], "Beta Inc");
    assert_eq!(v["name"]["ja-JP"], "ベータ");
    assert_eq!(v["logo_url"], "beta", "logo slug 不因一次网络抖动变空");
    assert_eq!(v["color"], "#222222");
    assert_eq!(v["source_urls"]["docs"], "https://beta.example.com/docs");
}

/// index 不比 DB 新 → 整轮跳过一个文件都不拉；last_updated 变了但内容没变 → 全部 unchanged。
#[tokio::test]
async fn second_identical_sync_skips_then_bumped_index_is_all_unchanged() {
    let db = test_db().await;
    let base = spawn_registry(full()).await;
    sync_registry_from(&db, &[&base]).await.unwrap();

    let r = sync_registry_from(&db, &[&base]).await.unwrap();
    assert_eq!((r.added, r.updated, r.unchanged, r.failed, r.total), (0, 0, 0, 0, 0),
        "index last_updated 未变 → 整轮跳过");
    assert_eq!(aidog_db::select_model_entries(&db, None).await.unwrap().len(), 3, "DB 数据原样");
    assert!(get_sync_settings(&db).await.last_sync_at > 0, "跳过也写 last_sync_at，周期判定不空转");

    let base = spawn_registry(full_newer()).await;
    let r = sync_registry_from(&db, &[&base]).await.unwrap();
    assert_eq!(r.total, 5);
    assert_eq!((r.added, r.updated), (0, 0), "内容没变不该被记成 updated");
    assert_eq!(r.unchanged, 5);
}

/// index 拉不到 = 不知道该拉什么，整轮放弃，DB 一行不动。
#[tokio::test]
async fn index_fetch_failure_aborts_round() {
    let db = test_db().await;
    let base = spawn_registry(full()).await;
    sync_registry_from(&db, &[&base]).await.unwrap();

    let empty = spawn_registry(BTreeMap::new()).await;
    let err = sync_registry_from(&db, &[&empty]).await.unwrap_err();
    assert!(err.starts_with("index.json:"), "{err}");
    assert_eq!(aidog_db::select_model_entries(&db, None).await.unwrap().len(), 3);
}

/// 主源整体不可达时逐文件回退到第二源（jsDelivr → raw 的本地等价）。
#[tokio::test]
async fn falls_back_to_secondary_source_per_file() {
    let db = test_db().await;
    let dead = spawn_registry(BTreeMap::new()).await;
    let alive = spawn_registry(full()).await;
    let r = sync_registry_from(&db, &[&dead, &alive]).await.unwrap();
    assert_eq!(r.added, 5);
    assert!(r.failures.is_empty());
}

/// 上游给出读不出 `model_id` 的脏 JSON：该文件记失败，不写进库污染读取层。
#[tokio::test]
async fn invalid_payload_counts_as_failure_without_writing() {
    let db = test_db().await;
    let mut dirty = full();
    dirty.insert("platforms/beta/models/b-1.json".to_string(), r#"{"no_model_id":true}"#.to_string());
    dirty.insert("platforms/beta/platform.json".to_string(), "{not json".to_string());
    let base = spawn_registry(dirty).await;
    let r = sync_registry_from(&db, &[&base]).await.unwrap();

    assert_eq!(r.failed, 2);
    assert_eq!(r.added, 3);
    assert!(r.failures.iter().any(|f| f.error == "invalid model json"));
    assert!(r.failures.iter().any(|f| f.error == "invalid platform json"));
    assert!(aidog_db::select_platform_presets(&db).await.unwrap().iter().all(|p| p.code != "beta"));
    assert!(aidog_db::select_model_entries(&db, Some("beta")).await.unwrap().is_empty());
}

/// 票 13-F：一轮里有文件失败时，成功的行照样落库、`last_sync_at` 照写、
/// failures 清单原样回给前端——不能因为写入层一处出错就把整轮结果换成一个字符串错误。
#[tokio::test]
async fn partial_round_still_commits_and_stamps_last_sync_at() {
    let db = test_db().await;
    let mut broken = full();
    broken.remove("platforms/beta/models/b-1.json"); // 404
    let base = spawn_registry(broken).await;

    let r = sync_registry_from(&db, &[&base]).await.unwrap();
    assert_eq!(r.failed, 1);
    assert_eq!(r.failures.len(), 1);
    assert_eq!(r.added, 4, "拉到的 4 个文件必须落库");
    assert_eq!(aidog_db::select_model_entries(&db, None).await.unwrap().len(), 2);
    assert!(get_sync_settings(&db).await.last_sync_at > 0, "last_sync_at 不能因部分失败而不写");
}

#[tokio::test]
async fn get_sync_settings_returns_default_when_absent() {
    let db = test_db().await;
    let s = get_sync_settings(&db).await;
    assert!(!s.auto_sync_enabled);
    assert_eq!(s.sync_interval_secs, 86400);
    assert_eq!(s.last_sync_at, 0);
}

#[tokio::test]
async fn save_and_get_sync_settings_roundtrip() {
    let db = test_db().await;
    let settings = super::super::models::PriceSyncSettings {
        auto_sync_enabled: true,
        sync_interval_secs: 3600,
        last_sync_at: 1234567890,
        registry_last_updated: 0,
        fallback_input_price: 5.0,
        fallback_output_price: 7.0,
    };
    save_sync_settings(&db, &settings).await;
    let got = get_sync_settings(&db).await;
    assert!(got.auto_sync_enabled);
    assert_eq!(got.sync_interval_secs, 3600);
    assert_eq!(got.last_sync_at, 1234567890);
    assert!((got.fallback_input_price - 5.0).abs() < 1e-9);
}

#[tokio::test]
async fn maybe_auto_sync_returns_none_when_disabled() {
    let db = test_db().await;
    // auto_sync_enabled = false (default) → 直接返回，不碰网络
    assert!(maybe_auto_sync(&db).await.unwrap().is_none());
}

#[tokio::test]
async fn maybe_auto_sync_returns_none_when_not_due() {
    let db = test_db().await;
    let settings = super::super::models::PriceSyncSettings {
        auto_sync_enabled: true,
        sync_interval_secs: 86400,
        last_sync_at: aidog_db::now() - 100, // 100ms 前刚同步过
        registry_last_updated: 0,
        fallback_input_price: 3.0,
        fallback_output_price: 3.0,
    };
    save_sync_settings(&db, &settings).await;
    assert!(maybe_auto_sync(&db).await.unwrap().is_none(), "未到间隔不该同步");
}
