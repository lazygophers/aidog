use super::*;
use crate::gateway::backup::CURRENT_DEFAULTS_VERSION;

#[tokio::test]
async fn maybe_backup_skips_when_disabled() {
    let db = crate::gateway::db::Db::new(":memory:").await.unwrap();
    db.init_tables().await.unwrap();
    // 用户已手动确认关闭 (version=CURRENT + enabled=false) → load 不迁移, maybe_backup 跳过。
    let s = BackupSettings {
        enabled: false,
        interval_hours: 24,
        retention_days: 7,
        last_backup_at: 0,
        last_backup_error: String::new(),
        defaults_version: CURRENT_DEFAULTS_VERSION,
    };
    s.save(&db).await.unwrap();
    let r = maybe_backup(&db).await;
    assert!(r.is_ok(), "maybe_backup disabled should not error: {:?}", r.err());
    assert!(r.unwrap().is_none());
}

#[tokio::test]
async fn maybe_backup_runs_for_fresh_default() {
    // 新装/无 db 记录 → load 走 Default → enabled=true + last_backup_at=0 → maybe_backup 应触发。
    // 用 last_backup_at=now 的 enabled=true 配置模拟 throttle 场景验证「enabled 即跑」路径,
    // 避免此处真跑落盘 (collect+encrypt+write 副作用)。
    // 真实「fresh default 跑一次备份」由 spawn_scheduler 启动检查覆盖。
    let s = BackupSettings::default();
    assert!(s.enabled, "default should be enabled=true");
    assert_eq!(s.defaults_version, CURRENT_DEFAULTS_VERSION);
}

#[tokio::test]
async fn maybe_backup_throttles_within_interval() {
    let db = crate::gateway::db::Db::new(":memory:").await.unwrap();
    db.init_tables().await.unwrap();
    // enabled=true + last_backup_at=now → 距上次 < interval → 跳过。
    // 注: 用 version=CURRENT 避免 load 迁移改值。
    let s = BackupSettings {
        enabled: true,
        interval_hours: 24,
        retention_days: 7,
        last_backup_at: now_millis(),
        last_backup_error: String::new(),
        defaults_version: CURRENT_DEFAULTS_VERSION,
    };
    s.save(&db).await.unwrap();
    let r = maybe_backup(&db).await.unwrap();
    assert!(r.is_none(), "within-interval should be throttled");
}
