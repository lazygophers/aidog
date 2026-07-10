use super::*;

#[test]
fn settings_roundtrip() {
    let s = BackupSettings {
        enabled: true,
        interval_hours: 12,
        retention_days: 14,
        last_backup_at: 1_700_000_000_000,
        last_backup_error: String::new(),
        defaults_version: CURRENT_DEFAULTS_VERSION,
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: BackupSettings = serde_json::from_str(&json).unwrap();
    assert!(back.enabled);
    assert_eq!(back.interval_hours, 12);
    assert_eq!(back.retention_days, 14);
    assert_eq!(back.last_backup_at, 1_700_000_000_000);
    assert_eq!(back.defaults_version, CURRENT_DEFAULTS_VERSION);
}

#[test]
fn settings_default_when_missing_fields() {
    // 缺字段 → serde 默认填充 (defaults_version 缺省 = 0, 标记「老数据」)。
    let json = r#"{"enabled":true}"#;
    let s: BackupSettings = serde_json::from_str(json).unwrap();
    assert!(s.enabled);
    assert_eq!(s.interval_hours, 24); // default
    assert_eq!(s.retention_days, 7); // default
    assert_eq!(s.defaults_version, 0); // serde default, 未走 load 迁移
}

#[test]
fn sanitized_clamps_invalid_values() {
    let s = BackupSettings {
        enabled: true,
        interval_hours: 0,    // 非法 → 默认 24
        retention_days: 999,  // 非法 → 默认 7
        last_backup_at: 0,
        last_backup_error: String::new(),
        defaults_version: CURRENT_DEFAULTS_VERSION,
    };
    let s = s.sanitized();
    assert_eq!(s.interval_hours, 24);
    assert_eq!(s.retention_days, 7);
}

#[tokio::test]
async fn backup_settings_load_save_roundtrip() {
    let db = crate::gateway::db::Db::new(":memory:").await.unwrap();
    db.init_tables().await.unwrap();
    // 默认 → save → load 一致 (sanitized 后)。
    let s = BackupSettings {
        enabled: true,
        interval_hours: 6,
        retention_days: 30,
        last_backup_at: 0,
        last_backup_error: String::new(),
        defaults_version: CURRENT_DEFAULTS_VERSION,
    };
    s.save(&db).await.unwrap();
    let loaded = BackupSettings::load(&db).await;
    assert!(loaded.enabled);
    assert_eq!(loaded.interval_hours, 6);
    assert_eq!(loaded.retention_days, 30);
    assert_eq!(loaded.defaults_version, CURRENT_DEFAULTS_VERSION);
}

#[tokio::test]
async fn migration_flips_enabled_for_legacy_default_false() {
    let db = crate::gateway::db::Db::new(":memory:").await.unwrap();
    db.init_tables().await.unwrap();
    // 老数据: 无 version 字段 + enabled=false (旧默认) → load 后翻 true + version=CURRENT。
    let legacy_json = serde_json::json!({
        "enabled": false,
        "interval_hours": 24,
        "retention_days": 7,
        "last_backup_at": 0,
        "last_backup_error": "",
    });
    db::set_setting(
        &db,
        SetSettingInput {
            scope: SETTING_SCOPE.to_string(),
            key: SETTING_KEY.to_string(),
            value: legacy_json,
        },
    )
    .await
    .unwrap();
    let loaded = BackupSettings::load(&db).await;
    assert!(loaded.enabled, "legacy enabled=false should flip to true");
    assert_eq!(loaded.defaults_version, CURRENT_DEFAULTS_VERSION);
    // 持久化生效: 再读一次仍为迁移后值。
    let again = BackupSettings::load(&db).await;
    assert!(again.enabled);
    assert_eq!(again.defaults_version, CURRENT_DEFAULTS_VERSION);
}

#[tokio::test]
async fn migration_respects_user_disabled_after_confirm() {
    let db = crate::gateway::db::Db::new(":memory:").await.unwrap();
    db.init_tables().await.unwrap();
    // 用户已手动确认关闭: version=CURRENT + enabled=false → load 不翻。
    let s = BackupSettings {
        enabled: false,
        interval_hours: 24,
        retention_days: 7,
        last_backup_at: 0,
        last_backup_error: String::new(),
        defaults_version: CURRENT_DEFAULTS_VERSION,
    };
    s.save(&db).await.unwrap();
    let loaded = BackupSettings::load(&db).await;
    assert!(!loaded.enabled, "confirmed-disabled should stay false");
    assert_eq!(loaded.defaults_version, CURRENT_DEFAULTS_VERSION);
}

#[tokio::test]
async fn migration_idempotent_across_loads() {
    let db = crate::gateway::db::Db::new(":memory:").await.unwrap();
    db.init_tables().await.unwrap();
    // 写老数据 (无 version)。
    let legacy_json = serde_json::json!({"enabled": false, "interval_hours": 48, "retention_days": 14});
    db::set_setting(
        &db,
        SetSettingInput {
            scope: SETTING_SCOPE.to_string(),
            key: SETTING_KEY.to_string(),
            value: legacy_json,
        },
    )
    .await
    .unwrap();
    // 连续 load 两次。
    let first = BackupSettings::load(&db).await;
    let second = BackupSettings::load(&db).await;
    // 迁移结果稳定 (第二次不重复改值)。
    assert_eq!(first.enabled, second.enabled);
    assert_eq!(first.defaults_version, second.defaults_version);
    assert_eq!(first.interval_hours, second.interval_hours);
    assert_eq!(first.retention_days, second.retention_days);
    assert!(second.enabled, "should remain true after second load");
    assert_eq!(second.defaults_version, CURRENT_DEFAULTS_VERSION);
    // db 中 version 已落 CURRENT (二次 load 后仍如此)。
    let row = db::get_setting(&db, SETTING_SCOPE, SETTING_KEY)
        .await
        .unwrap()
        .unwrap();
    let stored: BackupSettings = serde_json::from_value(row).unwrap();
    assert_eq!(stored.defaults_version, CURRENT_DEFAULTS_VERSION);
    assert!(stored.enabled);
}
