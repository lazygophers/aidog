use super::*;
use aidog_db::test_support::HomeGuard;

fn group(key: &str, models: &[&str]) -> PiGroup {
    PiGroup {
        group_key: key.to_string(),
        models: models.iter().map(|m| m.to_string()).collect(),
    }
}

fn empty() -> Value {
    Value::Object(Map::new())
}

fn providers_of(config: &PiConfig) -> &Map<String, Value> {
    config.models_json["providers"]
        .as_object()
        .expect("providers object")
}

#[test]
fn one_provider_per_group_named_by_prefix() {
    let groups = [group("teamA", &["m1"]), group("teamB", &["m2"])];
    let config = build_pi_config(&empty(), &empty(), &groups, 8787);
    let providers = providers_of(&config);

    assert_eq!(providers.len(), 2);
    assert!(providers.contains_key("aidog-teamA"));
    assert!(providers.contains_key("aidog-teamB"));
}

#[test]
fn group_key_becomes_bearer_credential() {
    // token 不走 env：apiKey 写字面分组名 + authHeader，pi 因此发 Authorization: Bearer <group>。
    // 这是绕开 auth.json 覆盖 env 的关键（ADR 0001）。
    let config = build_pi_config(&empty(), &empty(), &[group("teamA", &["m1"])], 8787);
    let p = &providers_of(&config)["aidog-teamA"];

    assert_eq!(p["apiKey"], "teamA");
    assert_eq!(p["authHeader"], true);
}

#[test]
fn anthropic_base_url_has_no_version_suffix() {
    // pi 的 anthropic 内置 provider 常量是裸 host，`/v1/messages` 由 SDK 自己补。
    // pi 官方文档的代理示例带 `/v1`，照抄会打到 `/v1/v1/messages`。以源码为准，勿「修正」。
    let config = build_pi_config(&empty(), &empty(), &[group("g", &["m"])], 8787);
    let p = &providers_of(&config)["aidog-g"];

    assert_eq!(p["baseUrl"], "http://127.0.0.1:8787/proxy");
    assert_eq!(p["api"], "anthropic-messages");
}

#[test]
fn group_models_become_provider_models() {
    // pi 规定非内置 provider 必须自带 models 才能在 /model 里选到。
    let config = build_pi_config(&empty(), &empty(), &[group("g", &["a", "b"])], 8787);
    let models = providers_of(&config)["aidog-g"]["models"]
        .as_array()
        .expect("models array");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0]["id"], "a");
    assert_eq!(models[1]["id"], "b");
}

#[test]
fn builtin_and_user_providers_survive() {
    let existing = serde_json::json!({
        "providers": {
            "anthropic": { "baseUrl": "https://my-proxy.example.com" },
            "my-gateway": { "baseUrl": "https://gw.example.com", "api": "openai-completions" }
        },
        "someUnknownTopLevelKey": 42
    });
    let config = build_pi_config(&existing, &empty(), &[group("g", &["m"])], 8787);
    let providers = providers_of(&config);

    assert_eq!(providers["anthropic"]["baseUrl"], "https://my-proxy.example.com");
    assert_eq!(providers["my-gateway"]["api"], "openai-completions");
    assert!(providers.contains_key("aidog-g"));
    assert_eq!(config.models_json["someUnknownTopLevelKey"], 42);
}

#[test]
fn deleted_group_provider_is_swept() {
    let existing = serde_json::json!({
        "providers": {
            "aidog-stale": { "baseUrl": "http://127.0.0.1:1/proxy" },
            "anthropic": { "baseUrl": "https://api.anthropic.com" }
        }
    });
    let config = build_pi_config(&existing, &empty(), &[group("keep", &["m"])], 8787);
    let providers = providers_of(&config);

    assert!(!providers.contains_key("aidog-stale"));
    assert!(providers.contains_key("aidog-keep"));
    assert!(providers.contains_key("anthropic"));
}

#[test]
fn no_groups_and_no_other_providers_leaves_no_empty_table() {
    let config = build_pi_config(&empty(), &empty(), &[], 8787);
    assert!(config.models_json.get("providers").is_none());
}

#[test]
fn dollar_in_group_key_is_escaped_to_literal() {
    // pi 在 apiKey 里做环境变量插值，且插值在长字面量内部也生效。不转义则分组名
    // `$HOME` 会被解析成环境变量值，路由 token 完全错位。
    let config = build_pi_config(&empty(), &empty(), &[group("a$HOME-b", &["m"])], 8787);
    assert_eq!(providers_of(&config)["aidog-a$HOME-b"]["apiKey"], "a$$HOME-b");
}

#[test]
fn bang_prefixed_group_key_is_escaped_to_literal() {
    // 开头 `!` 在 pi 里是「执行这条 shell 命令取 stdout」。不转义等于任意命令执行。
    let config = build_pi_config(&empty(), &empty(), &[group("!whoami", &["m"])], 8787);
    assert_eq!(providers_of(&config)["aidog-!whoami"]["apiKey"], "$!whoami");
}

#[test]
fn settings_json_passes_through_untouched() {
    let existing = serde_json::json!({ "theme": "dark", "defaultProvider": "anthropic" });
    let config = build_pi_config(&empty(), &existing, &[group("g", &["m"])], 8787);
    assert_eq!(config.settings_json, existing);
}

#[test]
fn sync_writes_then_skips_unchanged_then_rewrites_on_port_change() {
    let _g = HomeGuard::new();
    let groups = [group("grp", &["m"])];

    let first = sync_groups(&groups, 9000).unwrap();
    assert!(!first.is_empty(), "first sync must write models.json");

    let again = sync_groups(&groups, 9000).unwrap();
    assert!(again.is_empty(), "unchanged content must not rewrite");

    let changed = sync_groups(&groups, 9001).unwrap();
    assert!(!changed.is_empty(), "port change must rewrite");
}

#[test]
fn read_corrupt_models_json_errors_with_filename() {
    let _g = HomeGuard::new();
    let path = models_path().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{\"providers\": {,}").unwrap();

    let err = sync_groups(&[group("g", &["m"])], 8787).expect_err("corrupt JSON must error");
    assert!(err.starts_with("parse "), "err should mark parse stage: {err}");
}
