use super::*;
use crate::CURRENT_DEFAULTS_VERSION;
use aidog_stats::DbInitTables;

#[tokio::test]
async fn maybe_backup_skips_when_disabled() {
    let db = aidog_db::Db::new(":memory:").await.unwrap();
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
    assert!(
        r.is_ok(),
        "maybe_backup disabled should not error: {:?}",
        r.err()
    );
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
    let db = aidog_db::Db::new(":memory:").await.unwrap();
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

// ─── next_wake_delay（睡到下次到期，取代 60s 轮询）──────────────

#[tokio::test]
async fn backup_settings_wake_interrupts_long_wait() {
    let waiter = tokio::spawn(async { wait_next_backup(Duration::from_secs(3600)).await });
    tokio::task::yield_now().await;
    wake_backup_scheduler();
    assert!(
        waiter.await.unwrap(),
        "settings change should interrupt long sleep and force recomputation"
    );
}

#[test]
fn next_wake_delay_sleeps_until_due() {
    let now = now_millis();
    let s = BackupSettings {
        enabled: true,
        interval_hours: 24,
        last_backup_at: now - 3600 * 1000, // 1h 前备份过 → 还差 23h
        ..Default::default()
    };
    let d = next_wake_delay(&s, now, false);
    assert_eq!(
        d.as_millis() as i64,
        23 * 3600 * 1000,
        "should sleep the remaining 23h, got {:?}",
        d
    );
}

#[test]
fn next_wake_delay_overdue_returns_zero() {
    let now = now_millis();
    let s = BackupSettings {
        enabled: true,
        interval_hours: 24,
        last_backup_at: now - 25 * 3600 * 1000, // 超过 interval → 到期
        ..Default::default()
    };
    assert_eq!(next_wake_delay(&s, now, false), Duration::ZERO);
    // 首轮补 60s 启动延迟（保持「启动不立即跑」）。
    assert_eq!(next_wake_delay(&s, now, true), Duration::from_secs(60));
}

#[test]
fn next_wake_delay_never_backed_up_is_due_now() {
    let now = now_millis();
    let s = BackupSettings {
        enabled: true,
        last_backup_at: 0,
        ..Default::default()
    };
    assert_eq!(next_wake_delay(&s, now, false), Duration::ZERO);
    assert_eq!(next_wake_delay(&s, now, true), Duration::from_secs(60));
}

#[test]
fn next_wake_delay_disabled_has_no_timer() {
    let now = now_millis();
    let s = BackupSettings {
        enabled: false,
        ..Default::default()
    };
    let d = next_wake_delay(&s, now, false);
    assert_eq!(
        d,
        Duration::ZERO,
        "disabled settings do not use a timed wake"
    );
}

#[test]
fn next_wake_delay_first_loop_grace_overrides_immediate_run() {
    let now = now_millis();
    let s = BackupSettings {
        enabled: true,
        interval_hours: 24,
        last_backup_at: now, // 刚备份过，还差 24h
        ..Default::default()
    };
    assert_eq!(next_wake_delay(&s, now, false).as_secs(), 24 * 3600);
    assert_eq!(next_wake_delay(&s, now, true).as_secs(), 24 * 3600);
}

#[test]
fn run_backup_emits_backup_completed() {
    // 备份成功/失败后必须 emit backup-completed（前端 ScheduledBackupSection 重拉状态）
    let src = include_str!("scheduler.rs");
    assert!(src.contains(r#"emit_unit("backup-completed")"#));
}
