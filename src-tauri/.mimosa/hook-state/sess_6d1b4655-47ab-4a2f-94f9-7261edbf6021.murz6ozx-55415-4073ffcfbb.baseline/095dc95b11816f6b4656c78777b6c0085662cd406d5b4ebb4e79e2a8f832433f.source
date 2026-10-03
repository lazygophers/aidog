use super::*;
use std::time::{Duration, SystemTime};

#[tokio::test]
async fn cleanup_removes_expired_files() {
    // 唯一临时 dir (无 tempfile 依赖)。
    let dir = std::env::temp_dir().join(format!(
        "aidog-backup-test-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // 旧文件 (10 天前 mtime) → 应删。
    let old_path = dir.join("aidog-backup-old.aidogx");
    std::fs::write(&old_path, b"x").unwrap();
    set_mtime_days_ago(&old_path, 10);

    // 新文件 (现在) → 保留。
    let new_path = dir.join("aidog-backup-new.aidogx");
    std::fs::write(&new_path, b"y").unwrap();

    // 非 .aidogx → 不动。
    let other = dir.join("notes.txt");
    std::fs::write(&other, b"z").unwrap();

    let removed = cleanup_expired_in_dir(&dir, 7).await.unwrap();
    assert_eq!(removed, 1);
    assert!(!old_path.exists());
    assert!(new_path.exists());
    assert!(other.exists());

    let _ = std::fs::remove_dir_all(&dir);
}

/// 把文件 mtime 设为 `days_ago` 天前 (std FileTimes, Rust 1.75+)。
fn set_mtime_days_ago(path: &std::path::Path, days_ago: i64) {
    use std::fs::FileTimes;
    let past = SystemTime::now() - Duration::from_secs((days_ago * 86400) as u64);
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    let times = FileTimes::new().set_modified(past).set_accessed(past);
    f.set_times(times).unwrap();
}
