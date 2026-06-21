//! `codex.rs` 单测（1:1）：Codex notify 注入/移除。

use super::*;
use serde_json::json;

#[test]
fn codex_notify_inject_and_remove() {
    let mut cfg = json!({ "model_provider": "aidog" });
    inject_codex_notify(&mut cfg, "/a/aidog-notify-complete.sh");
    assert_eq!(cfg["notify"][0], "/a/aidog-notify-complete.sh");
    // 其他键不动
    assert_eq!(cfg["model_provider"], "aidog");
    remove_codex_notify(&mut cfg);
    assert!(cfg.get("notify").is_none());
}

#[test]
fn codex_remove_preserves_user_notify() {
    let mut cfg = json!({ "notify": ["/usr/bin/user-notify.sh"] });
    remove_codex_notify(&mut cfg);
    // 非 aidog notify 保留
    assert_eq!(cfg["notify"][0], "/usr/bin/user-notify.sh");
}
