use super::*;
use crate::gateway::db::test_support::{HomeGuard, ENV_LOCK};
use std::collections::HashSet;



#[test]
fn build_group_profile_toml_shape() {
    let toml = build_group_profile_toml(8787).unwrap();
    assert!(toml.contains("model_provider = \"aidog\""));
    assert!(toml.contains("[model_providers.aidog]"));
    assert!(toml.contains("http://127.0.0.1:8787/proxy"));
    assert!(toml.contains("wire_api = \"responses\""));
    assert!(toml.contains("env_key = \"AIDOG_KEY\""));
}

#[test]
fn home_and_profile_path_accessors() {
    let _g = HomeGuard::new();
    let home = codex_home_public().unwrap();
    assert!(home.ends_with(home.file_name().unwrap()));
    let pp = profile_path_public("teamA").unwrap();
    assert!(pp.to_string_lossy().ends_with("teamA.config.toml"));
}

#[test]
fn write_group_profile_creates_and_skips_unchanged() {
    let _g = HomeGuard::new();
    let written = write_group_profile("grp", 9000).unwrap();
    assert!(written.is_some());
    // second write, identical content → None (skip)
    let again = write_group_profile("grp", 9000).unwrap();
    assert!(again.is_none());
    // changed port → writes again
    let changed = write_group_profile("grp", 9001).unwrap();
    assert!(changed.is_some());
}

#[test]
fn cleanup_group_profiles_removes_stale() {
    let _g = HomeGuard::new();
    write_group_profile("keep", 9000).unwrap();
    write_group_profile("stale", 9000).unwrap();
    // user-level baseline must never be removed
    let dir = codex_home_public().unwrap();
    std::fs::write(dir.join("config.toml"), "x = 1").unwrap();

    let mut keep = HashSet::new();
    keep.insert("keep".to_string());
    cleanup_group_profiles(&keep).unwrap();

    assert!(dir.join("keep.config.toml").exists());
    assert!(!dir.join("stale.config.toml").exists());
    assert!(dir.join("config.toml").exists());
}

#[test]
fn cleanup_missing_dir_is_ok() {
    // 不用 HomeGuard（它会把 CODEX_HOME 指向存在的 tempdir）；此测需要 CODEX_HOME 指向不存在路径。
    // 串行用中心 ENV_LOCK，env 手动 save/restore。
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("CODEX_HOME").ok();
    unsafe {
        std::env::set_var("CODEX_HOME", "/nonexistent/aidog-codex-test-xyz");
    }
    let keep = HashSet::new();
    assert!(cleanup_group_profiles(&keep).is_ok());
    unsafe {
        match &prev {
            Some(v) => std::env::set_var("CODEX_HOME", v),
            None => std::env::remove_var("CODEX_HOME"),
        }
    }
}

#[test]
fn config_read_write_roundtrip() {
    let _g = HomeGuard::new();
    // empty (no file) → {}
    let empty = codex_config_read().unwrap();
    assert!(empty.as_object().unwrap().is_empty());

    // write JSON with null values stripped
    let val = serde_json::json!({
        "model": "gpt-5",
        "removed": null,
        "model_providers": {"aidog": {"name": "aidog proxy"}}
    });
    codex_config_write(val).unwrap();

    let read = codex_config_read().unwrap();
    assert_eq!(read.get("model").and_then(|v| v.as_str()), Some("gpt-5"));
    assert!(read.get("removed").is_none());
    assert!(read.get("model_providers").is_some());

    // path accessor
    let p = codex_config_path().unwrap();
    assert!(p.ends_with("config.toml"));
}

#[test]
fn config_write_rejects_non_object() {
    let _g = HomeGuard::new();
    assert!(codex_config_write(serde_json::json!([1, 2, 3])).is_err());
}

#[test]
fn default_profile_inject_and_remove() {
    let _g = HomeGuard::new();
    // inject default profile
    let injected = write_default_profile_to_config(8080).unwrap();
    assert!(injected.is_some());
    let cfg = codex_config_read().unwrap();
    assert_eq!(cfg.get("model_provider").and_then(|v| v.as_str()), Some("aidog"));
    assert!(cfg.pointer("/model_providers/aidog").is_some());

    // re-inject identical → None (no change)
    let again = write_default_profile_to_config(8080).unwrap();
    assert!(again.is_none());

    // remove default profile
    let removed = remove_default_profile_from_config().unwrap();
    assert!(removed.is_some());
    let cfg2 = codex_config_read().unwrap();
    assert!(cfg2.get("model_provider").is_none());
    assert!(cfg2.get("model_providers").is_none()); // empty providers table cleaned

    // remove again → nothing to change → None
    let removed2 = remove_default_profile_from_config().unwrap();
    assert!(removed2.is_none());
}

#[test]
fn remove_keeps_user_provider_and_non_aidog_model_provider() {
    let _g = HomeGuard::new();
    let val = serde_json::json!({
        "model_provider": "openai",
        "model_providers": {"openai": {"name": "x"}, "aidog": {"name": "aidog proxy"}}
    });
    codex_config_write(val).unwrap();
    remove_default_profile_from_config().unwrap();
    let cfg = codex_config_read().unwrap();
    // user's model_provider "openai" preserved (not aidog)
    assert_eq!(cfg.get("model_provider").and_then(|v| v.as_str()), Some("openai"));
    // aidog provider removed but openai provider kept
    assert!(cfg.pointer("/model_providers/aidog").is_none());
    assert!(cfg.pointer("/model_providers/openai").is_some());
}

// ── strip_nulls ──
#[test]
fn strip_nulls_removes_null_values_from_object() {
    let v = serde_json::json!({ "a": "keep", "b": null, "c": 42 });
    let out = strip_nulls(v);
    assert!(out.get("a").is_some());
    assert!(out.get("b").is_none(), "null field should be removed");
    assert!(out.get("c").is_some());
}

#[test]
fn strip_nulls_removes_nulls_from_nested_objects() {
    let v = serde_json::json!({ "outer": { "keep": "x", "drop": null } });
    let out = strip_nulls(v);
    assert!(out.pointer("/outer/keep").is_some());
    assert!(out.pointer("/outer/drop").is_none(), "nested null removed");
}

#[test]
fn strip_nulls_removes_nulls_from_arrays() {
    let v = serde_json::json!([1, null, "three", null]);
    let out = strip_nulls(v);
    let arr = out.as_array().unwrap();
    assert_eq!(arr.len(), 2, "nulls in array removed: {arr:?}");
    assert_eq!(arr[0], serde_json::json!(1));
    assert_eq!(arr[1], serde_json::json!("three"));
}

#[test]
fn strip_nulls_primitives_pass_through() {
    assert_eq!(strip_nulls(serde_json::json!(42)), serde_json::json!(42));
    assert_eq!(strip_nulls(serde_json::json!("text")), serde_json::json!("text"));
    assert_eq!(strip_nulls(serde_json::json!(true)), serde_json::json!(true));
}

// ── set_obj_path ──
#[test]
fn set_obj_path_single_key() {
    let mut base = serde_json::json!({});
    set_obj_path(&mut base, &["k"], serde_json::json!("v"));
    assert_eq!(base["k"], "v");
}

#[test]
fn set_obj_path_nested_path() {
    let mut base = serde_json::json!({});
    set_obj_path(&mut base, &["a", "b", "c"], serde_json::json!(99));
    assert_eq!(base["a"]["b"]["c"], 99);
}

#[test]
fn set_obj_path_overwrites_existing() {
    let mut base = serde_json::json!({ "x": "old" });
    set_obj_path(&mut base, &["x"], serde_json::json!("new"));
    assert_eq!(base["x"], "new");
}

#[test]
fn set_obj_path_empty_path_replaces_entire_value() {
    let mut base = serde_json::json!({ "x": 1 });
    set_obj_path(&mut base, &[], serde_json::json!("replaced"));
    assert_eq!(base, serde_json::json!("replaced"));
}

#[test]
fn set_obj_path_non_object_base_upgraded() {
    let mut base = serde_json::json!("scalar");
    set_obj_path(&mut base, &["k"], serde_json::json!(1));
    assert_eq!(base["k"], 1);
}
