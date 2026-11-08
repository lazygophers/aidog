#![cfg(test)]
use super::*;
use aidog_db::test_support::test_db;

fn settings(enabled: bool) -> PrivacyScrubSettings {
    PrivacyScrubSettings {
        enabled,
        secrets: true,
        personal: true,
        model_enabled: false,
        model_endpoint: String::new(),
        per_kind: Default::default(),
    }
}

/// 读写往返：默认全关 → 存 true → 读回 true。
#[tokio::test]
async fn scrub_settings_roundtrip() {
    let db = test_db().await;
    assert!(!privacy_scrub_settings(&db).await.enabled);
    save_privacy_scrub_settings(&db, &settings(true)).await.unwrap();
    assert!(privacy_scrub_settings(&db).await.enabled);
}

/// 总开关关：预览原样返回，不命中。
#[tokio::test]
async fn preview_disabled_returns_original() {
    let db = test_db().await;
    save_privacy_scrub_settings(&db, &settings(false)).await.unwrap();
    let p = scrub_preview_text(&db, "mail bob@example.com").await;
    assert_eq!(p.scrubbed, "mail bob@example.com");
    assert_eq!(p.hits, 0);
}

/// 总开关开：email 命中规则占位符，hits=1。
#[tokio::test]
async fn preview_scrubs_email() {
    let db = test_db().await;
    save_privacy_scrub_settings(&db, &settings(true)).await.unwrap();
    let p = scrub_preview_text(&db, "mail bob@example.com").await;
    assert!(p.scrubbed.contains("{{PII_EMAIL_"), "got: {}", p.scrubbed);
    assert_eq!(p.hits, 1);
}
