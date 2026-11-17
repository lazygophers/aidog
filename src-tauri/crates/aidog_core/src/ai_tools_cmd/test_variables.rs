use super::*;

use serde_json::Value;

fn var(key: &str, value: &str) -> Variable {
    Variable {
        key: key.into(),
        value: value.into(),
    }
}

fn applied(before: Option<&str>, after: &str) -> VarApplied {
    VarApplied {
        before: before.map(|b| Value::String(b.into())),
        after: Value::String(after.into()),
    }
}

// ─── Claude env 覆盖应用 ──────────────────────────────────────

#[test]
fn env_apply_overrides_same_name_and_records_before() {
    let mut env = Map::new();
    env.insert("KEEP".into(), Value::String("keep".into()));
    env.insert("OVERRIDE".into(), Value::String("user".into()));
    let (env, ledger) = env_apply(env, &AppliedLedger::new(), &[
        var("OVERRIDE", "aidog"),
        var("NEW", "v1"),
    ])
    .unwrap();
    assert_eq!(env.get("OVERRIDE"), Some(&Value::String("aidog".into())));
    assert_eq!(env.get("NEW"), Some(&Value::String("v1".into())));
    assert_eq!(env.get("KEEP"), Some(&Value::String("keep".into())));
    assert_eq!(ledger.get("OVERRIDE"), Some(&applied(Some("user"), "aidog")));
    assert_eq!(ledger.get("NEW"), Some(&applied(None, "v1")));
}

#[test]
fn env_apply_restores_overridden_value_on_removal_and_keeps_user_edit() {
    let mut env = Map::new();
    env.insert("UNTOUCHED".into(), Value::String("aidog".into()));
    env.insert("EDITED".into(), Value::String("user-edit".into()));
    let mut ledger = AppliedLedger::new();
    ledger.insert("UNTOUCHED".into(), applied(Some("orig"), "aidog"));
    ledger.insert("EDITED".into(), applied(Some("orig"), "aidog"));
    let (env, next) = env_apply(env, &ledger, &[]).unwrap();
    assert_eq!(env.get("UNTOUCHED"), Some(&Value::String("orig".into())));
    assert_eq!(env.get("EDITED"), Some(&Value::String("user-edit".into())));
    assert!(!next.contains_key("UNTOUCHED"));
    assert!(!next.contains_key("EDITED"));
}

#[test]
fn env_apply_repeated_save_keeps_first_before() {
    let mut env = Map::new();
    env.insert("K".into(), Value::String("orig".into()));
    let (env, l1) = env_apply(env, &AppliedLedger::new(), &[var("K", "v1")]).unwrap();
    let (env, l2) = env_apply(env, &l1, &[var("K", "v2")]).unwrap();
    assert_eq!(env.get("K"), Some(&Value::String("v2".into())));
    assert_eq!(l2.get("K"), Some(&applied(Some("orig"), "v2")));
}

#[test]
fn env_apply_empty_value_unapplies_previous() {
    let mut env = Map::new();
    env.insert("K".into(), Value::String("aidog".into()));
    let mut ledger = AppliedLedger::new();
    ledger.insert("K".into(), applied(Some("orig"), "aidog"));
    let (env, next) = env_apply(env, &ledger, &[var("K", "")]).unwrap();
    assert_eq!(env.get("K"), Some(&Value::String("orig".into())));
    assert!(next.is_empty());
}

#[test]
fn env_apply_rejects_invalid_name_and_overlong_value() {
    let env = Map::new();
    assert!(env_apply(env.clone(), &AppliedLedger::new(), &[var("BAD-NAME", "x")]).is_err());
    assert!(env_apply(env, &AppliedLedger::new(), &[var("K", &"x".repeat(8193))]).is_err());
}

// ─── Codex 覆盖应用 ───────────────────────────────────────────

#[test]
fn codex_apply_overrides_same_name_and_records_before() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        "model = \"gpt-5\"\n\n[shell_environment_policy.set]\nUSER_VALUE = \"keep\"\nMANAGED = \"old\"\n",
    )
    .unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let (next, ledger) = codex_apply(&source, &AppliedLedger::new(), &[
        var("MANAGED", "new"),
        var("USER_VALUE", "overwrite"),
        var("ADDED", "yes\"\nmodel = \"bad"),
    ])
    .unwrap();
    assert!(next.contains("USER_VALUE = \"overwrite\""));
    assert!(next.contains("MANAGED = \"new\""));
    assert!(next.contains("ADDED = \"yes\\\"\\nmodel = \\\"bad\""));
    assert_eq!(ledger.get("USER_VALUE"), Some(&applied(Some("keep"), "overwrite")));
    assert_eq!(ledger.get("MANAGED"), Some(&applied(Some("old"), "new")));
    assert_eq!(
        ledger.get("ADDED"),
        Some(&applied(None, "yes\"\nmodel = \"bad"))
    );
    // 写后内容仍可解析
    let _: toml::Value = toml::from_str(&next).unwrap();
}

#[test]
fn codex_apply_restores_overridden_value_on_removal_and_keeps_user_edit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        "[shell_environment_policy.set]\nUNTOUCHED = \"aidog\"\nEDITED = \"user-edit\"\n",
    )
    .unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let mut ledger = AppliedLedger::new();
    ledger.insert("UNTOUCHED".into(), applied(Some("orig"), "aidog"));
    ledger.insert("EDITED".into(), applied(Some("orig"), "aidog"));
    let (next, restored) = codex_apply(&source, &ledger, &[]).unwrap();
    assert!(next.contains("UNTOUCHED = \"orig\""));
    assert!(next.contains("EDITED = \"user-edit\""));
    assert!(!restored.contains_key("UNTOUCHED"));
    assert!(!restored.contains_key("EDITED"));
}

#[test]
fn codex_apply_legacy_flat_ledger_removes_line() {
    // 旧形状（key → aidog 写入值，无 before）：撤销 = 删行（旧行为，无用户原值可恢复）。
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "[shell_environment_policy.set]\nOLD = \"aidog\"\n").unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let legacy_json = serde_json::json!({ "OLD": "aidog" });
    let ledger = parse_ledger(Some(legacy_json));
    let (next, restored) = codex_apply(&source, &ledger, &[]).unwrap();
    assert!(!next.contains("OLD"));
    assert!(restored.is_empty());
}

#[test]
fn codex_apply_new_ledger_shape_round_trips() {
    let ledger_json = serde_json::json!({
        "K": { "before": "user", "after": "aidog" },
        "L": { "before": null, "after": "aidog2" },
        "OLD": "flat"
    });
    let ledger = parse_ledger(Some(ledger_json));
    assert_eq!(ledger.get("K"), Some(&applied(Some("user"), "aidog")));
    assert_eq!(ledger.get("L"), Some(&applied(None, "aidog2")));
    assert_eq!(ledger.get("OLD"), Some(&applied(None, "flat")));
    assert_eq!(parse_ledger(None), AppliedLedger::new());
    assert_eq!(parse_ledger(Some(serde_json::json!("garbage"))), AppliedLedger::new());
}

#[test]
fn codex_apply_refuses_invalid_toml_and_invalid_variable_names() {
    let mut ledger = AppliedLedger::new();
    ledger.insert("K".into(), applied(None, "v"));
    assert!(codex_apply("bad = [\n", &ledger, &[]).is_err());
    assert!(codex_apply("", &ledger, &[var("BAD-NAME", "x")]).is_err());
    assert!(
        codex_apply(
            "",
            &ledger,
            &[var("K", &"x".repeat(8193))]
        )
        .is_err()
    );
}
