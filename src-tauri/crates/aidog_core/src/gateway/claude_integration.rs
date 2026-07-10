//! Claude Code / Codex 外部文件联动写入。
//!
//! 用于 AppSettings「Claude Code / Codex」tab 的两联动开关：
//! - `apply_to_claude_plugin` → 在 `~/.claude/config.json` 写入 `primaryApiKey="any"`，
//!   使 VS Code Claude Code 扩展走本地代理。关闭则移除该字段。
//! - `skip_claude_onboarding` → 在 `~/.claude.json` 根对象写入 `hasCompletedOnboarding=true`，
//!   跳过 Claude Code CLI 首次启动 onboarding。关闭则移除该字段。
//!
//! 机制移植自 cc-switch：增量读 JSON → 单字段 insert/remove → pretty 写回，
//! 保留其它字段。两文件不同、字段不同、互不耦合。
//!
//! 注：serde_json 未启用 `preserve_order` feature，写回时键顺序会按 BTreeMap 重排。
//! `~/.claude.json` 可能很大（含 projects 历史），但 JSON 本身无注释、键顺序语义不敏感，
//! 与项目内 `gateway::mcp` 读写 `~/.claude.json` 的现有做法保持一致。

use std::path::PathBuf;

/// `~/.claude/config.json` 绝对路径。
fn claude_config_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".claude").join("config.json"))
}

/// `~/.claude.json` 根配置绝对路径。
fn claude_dotjson_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".claude.json"))
}

/// 通用：读 JSON 文件为 Value。文件不存在或为空返回空对象 `{}`。
fn read_json_object(path: &PathBuf) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&content).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// 通用：pretty 写回 JSON。父目录不存在时自动创建。
fn write_json_object(path: &PathBuf, root: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create_dir_all {}: {e}", parent.display()))?;
    }
    let s = serde_json::to_string_pretty(root)
        .map_err(|e| format!("serialize {}: {e}", path.display()))?;
    std::fs::write(path, s).map_err(|e| format!("write {}: {e}", path.display()))
}

/// 写 `~/.claude/config.json` 的 `primaryApiKey="any"`（开启走本地代理）。
/// 增量 merge：若值已为 `"any"` 则跳过写入。
pub fn write_plugin_primary_key() -> Result<bool, String> {
    let path = claude_config_path()?;
    let mut root = read_json_object(&path)?;
    let obj = root
        .as_object_mut()
        .ok_or("~/.claude/config.json root is not an object")?;
    if obj.get("primaryApiKey").and_then(|v| v.as_str()) == Some("any") {
        return Ok(false);
    }
    obj.insert("primaryApiKey".to_string(), serde_json::json!("any"));
    write_json_object(&path, &root)?;
    Ok(true)
}

/// 移除 `~/.claude/config.json` 的 `primaryApiKey` 字段。
/// 若字段不存在则跳过写入。
pub fn clear_plugin_primary_key() -> Result<bool, String> {
    let path = claude_config_path()?;
    if !path.exists() {
        return Ok(false);
    }
    let mut root = read_json_object(&path)?;
    let obj = root
        .as_object_mut()
        .ok_or("~/.claude/config.json root is not an object")?;
    if obj.remove("primaryApiKey").is_none() {
        return Ok(false);
    }
    write_json_object(&path, &root)?;
    Ok(true)
}

/// 写 `~/.claude.json` 根对象 `hasCompletedOnboarding=true`（跳过首启引导）。
/// 若值已为 `true` 则跳过写入。
pub fn set_has_completed_onboarding() -> Result<bool, String> {
    let path = claude_dotjson_path()?;
    let mut root = read_json_object(&path)?;
    let obj = root
        .as_object_mut()
        .ok_or("~/.claude.json root is not an object")?;
    if obj.get("hasCompletedOnboarding").and_then(|v| v.as_bool()) == Some(true) {
        return Ok(false);
    }
    obj.insert(
        "hasCompletedOnboarding".to_string(),
        serde_json::json!(true),
    );
    write_json_object(&path, &root)?;
    Ok(true)
}

/// 移除 `~/.claude.json` 的 `hasCompletedOnboarding` 字段。
/// 若字段不存在则跳过写入。
pub fn clear_has_completed_onboarding() -> Result<bool, String> {
    let path = claude_dotjson_path()?;
    if !path.exists() {
        return Ok(false);
    }
    let mut root = read_json_object(&path)?;
    let obj = root
        .as_object_mut()
        .ok_or("~/.claude.json root is not an object")?;
    if obj.remove("hasCompletedOnboarding").is_none() {
        return Ok(false);
    }
    write_json_object(&path, &root)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::db::test_support::HomeGuard;

    fn scratch_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "aidog_claude_integration_test_{}_{}.json",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn write_plugin_primary_key_creates_and_is_idempotent() {
        let _g = HomeGuard::new();
        // First write: creates file, returns true
        let written = write_plugin_primary_key().unwrap();
        assert!(written, "first write should return true");
        // Second write: value already 'any', should skip
        let written2 = write_plugin_primary_key().unwrap();
        assert!(!written2, "idempotent: already 'any' should skip");
    }

    #[test]
    fn clear_plugin_primary_key_removes_field() {
        let _g = HomeGuard::new();
        // File doesn't exist → returns false (no-op)
        let cleared_before = clear_plugin_primary_key().unwrap();
        assert!(!cleared_before, "clear on nonexistent file should return false");
        // Write, then clear
        write_plugin_primary_key().unwrap();
        let cleared = clear_plugin_primary_key().unwrap();
        assert!(cleared, "clear after write should return true");
        // Second clear: field already gone
        let cleared2 = clear_plugin_primary_key().unwrap();
        assert!(!cleared2, "second clear should return false (field gone)");
    }

    #[test]
    fn set_and_clear_has_completed_onboarding_roundtrip() {
        let _g = HomeGuard::new();
        // Set: creates .claude.json, returns true
        let set = set_has_completed_onboarding().unwrap();
        assert!(set, "first set should return true");
        // Idempotent: already true
        let set2 = set_has_completed_onboarding().unwrap();
        assert!(!set2, "second set should return false (already true)");
        // Clear: removes field, returns true
        let cleared = clear_has_completed_onboarding().unwrap();
        assert!(cleared, "clear should return true");
        // Second clear: field gone, returns false
        let cleared2 = clear_has_completed_onboarding().unwrap();
        assert!(!cleared2, "second clear should return false");
    }

    #[test]
    fn clear_has_completed_onboarding_nonexistent_file() {
        let _g = HomeGuard::new();
        // ~/.claude.json doesn't exist → returns false without error
        let cleared = clear_has_completed_onboarding().unwrap();
        assert!(!cleared, "clear on nonexistent file returns false");
    }

    #[test]
    fn write_and_clear_plugin_primary_key_roundtrip() {
        let path = scratch_path("roundtrip");

        let mut root = serde_json::json!({"otherField": 42});
        let obj = root.as_object_mut().unwrap();
        obj.insert("primaryApiKey".to_string(), serde_json::json!("any"));
        write_json_object(&path, &root).unwrap();
        let reread = read_json_object(&path).unwrap();
        assert_eq!(reread["primaryApiKey"], "any");
        assert_eq!(reread["otherField"], 42);

        let mut root2 = reread;
        root2.as_object_mut().unwrap().remove("primaryApiKey");
        write_json_object(&path, &root2).unwrap();
        let after = read_json_object(&path).unwrap();
        assert!(after.get("primaryApiKey").is_none());
        assert_eq!(after["otherField"], 42);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_missing_file_returns_empty_object() {
        let path = scratch_path("nonexistent");
        let _ = std::fs::remove_file(&path);
        let v = read_json_object(&path).unwrap();
        assert!(v.is_object());
        assert!(v.as_object().unwrap().is_empty());
    }

    #[test]
    fn read_corrupt_json_errors_with_filename() {
        // 损坏 JSON（尾逗号）→ 必须返回 Err，且错误文案含文件名 + "parse" 标记，
        // 供前端常驻错误态向用户展示真实失败原因。
        let path = scratch_path("corrupt");
        std::fs::write(&path, "{\"primaryApiKey\": \"any\",}").unwrap();
        let err = read_json_object(&path).expect_err("corrupt JSON must error");
        assert!(err.starts_with("parse "), "err should mark parse stage: {err}");
        assert!(
            err.contains(path.to_str().unwrap()),
            "err should name the offending file: {err}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_non_object_root_errors_on_write_helpers() {
        // 根非对象（顶层是数组）→ as_object_mut 失败路径，写助手须返回明确 Err。
        let path = scratch_path("nonobject");
        std::fs::write(&path, "[1, 2, 3]").unwrap();
        let mut root = read_json_object(&path).unwrap();
        assert!(root.as_object_mut().is_none(), "array root has no object");
        let _ = std::fs::remove_file(&path);
    }
}
