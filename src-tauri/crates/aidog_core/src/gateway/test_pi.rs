use super::*;
use aidog_db::test_support::HomeGuard;

fn group(key: &str, models: &[&str]) -> PiGroup {
    PiGroup {
        group_key: key.to_string(),
        models: models.iter().map(|m| m.to_string()).collect(),
        api: PiApi::AnthropicMessages,
    }
}

fn group_with_api(key: &str, api: PiApi) -> PiGroup {
    PiGroup { api, ..group(key, &["m"]) }
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
    let config = build_pi_config(&empty(), &empty(), &groups, 8787, &PiSettings::default());
    let providers = providers_of(&config);

    assert_eq!(providers.len(), 2);
    assert!(providers.contains_key("aidog-teamA"));
    assert!(providers.contains_key("aidog-teamB"));
}

#[test]
fn group_key_becomes_bearer_credential() {
    // token 不走 env：apiKey 写字面分组名 + authHeader，pi 因此发 Authorization: Bearer <group>。
    // 这是绕开 auth.json 覆盖 env 的关键（ADR 0001）。
    let config = build_pi_config(&empty(), &empty(), &[group("teamA", &["m1"])], 8787, &PiSettings::default());
    let p = &providers_of(&config)["aidog-teamA"];

    assert_eq!(p["apiKey"], "teamA");
    assert_eq!(p["authHeader"], true);
}

#[test]
fn anthropic_base_url_has_no_version_suffix() {
    // pi 的 anthropic 内置 provider 常量是裸 host，`/v1/messages` 由 SDK 自己补。
    // pi 官方文档的代理示例带 `/v1`，照抄会打到 `/v1/v1/messages`。以源码为准，勿「修正」。
    let config = build_pi_config(&empty(), &empty(), &[group("g", &["m"])], 8787, &PiSettings::default());
    let p = &providers_of(&config)["aidog-g"];

    assert_eq!(p["baseUrl"], "http://127.0.0.1:8787/proxy");
    assert_eq!(p["api"], "anthropic-messages");
}

#[test]
fn version_suffix_rule_is_inverted_between_anthropic_and_openai() {
    // ⚠️ 反直觉且 pi 官方文档写错了：anthropic 线路要**裸根地址**（SDK 自己补
    // `/v1/messages`，内置常量 `providers/anthropic.ts:47` 就是裸 host），openai 线路要
    // **带 `/v1`**（内置常量 `providers/openai.ts:11`）。`models.md:300-329` 的 anthropic
    // 示例带了 `/v1`，照抄会打到 `/v1/v1/messages`。以源码常量为准，勿把本断言「修正」回去。
    let cases = [
        (PiApi::AnthropicMessages, "http://127.0.0.1:8787/proxy", "anthropic-messages"),
        (PiApi::OpenaiCompletions, "http://127.0.0.1:8787/proxy/v1", "openai-completions"),
        (PiApi::OpenaiResponses, "http://127.0.0.1:8787/proxy/v1", "openai-responses"),
        (PiApi::GoogleGenerativeAi, "http://127.0.0.1:8787/proxy/v1beta", "google-generative-ai"),
    ];
    for (api, want_url, want_api) in cases {
        let config = build_pi_config(&empty(), &empty(), &[group_with_api("g", api)], 8787, &PiSettings::default());
        let p = &providers_of(&config)["aidog-g"];
        assert_eq!(p["baseUrl"], want_url, "baseUrl for {want_api}");
        assert_eq!(p["api"], want_api);
    }
}

#[test]
fn group_models_become_provider_models() {
    // pi 规定非内置 provider 必须自带 models 才能在 /model 里选到。
    let config = build_pi_config(&empty(), &empty(), &[group("g", &["a", "b"])], 8787, &PiSettings::default());
    let models = providers_of(&config)["aidog-g"]["models"]
        .as_array()
        .expect("models array");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0]["id"], "a");
    assert_eq!(models[1]["id"], "b");
}

#[test]
fn provider_turns_off_pi_only_request_extras() {
    // 关掉后 pi 不发 eager tool input streaming / 长 ttl 缓存 / session affinity 头 ——
    // 上游不认这些字段时会 400。键名出自 pi `docs/models.md` 的 compat 表。
    let config = build_pi_config(&empty(), &empty(), &[group("g", &["m"])], 8787, &PiSettings::default());
    let compat = &providers_of(&config)["aidog-g"]["compat"];

    assert_eq!(compat["supportsEagerToolInputStreaming"], false);
    assert_eq!(compat["supportsLongCacheRetention"], false);
    assert_eq!(compat["sendSessionAffinityHeaders"], false);
}

#[test]
fn provider_identifies_itself_as_pi() {
    // 自定义 provider 下 pi 不设自己的 UA（只有内置 kimi-coding 设），不写就成匿名 SDK 默认值。
    let config = build_pi_config(&empty(), &empty(), &[group("g", &["m"])], 8787, &PiSettings::default());
    let ua = providers_of(&config)["aidog-g"]["headers"]["User-Agent"]
        .as_str()
        .expect("User-Agent header");

    assert!(ua.starts_with("pi ("), "UA should read as pi: {ua}");
    assert!(ua.ends_with(')'), "UA should keep pi's parenthesised form: {ua}");
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
    let config = build_pi_config(&existing, &empty(), &[group("g", &["m"])], 8787, &PiSettings::default());
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
    let config = build_pi_config(&existing, &empty(), &[group("keep", &["m"])], 8787, &PiSettings::default());
    let providers = providers_of(&config);

    assert!(!providers.contains_key("aidog-stale"));
    assert!(providers.contains_key("aidog-keep"));
    assert!(providers.contains_key("anthropic"));
}

#[test]
fn no_groups_and_no_other_providers_leaves_no_empty_table() {
    let config = build_pi_config(&empty(), &empty(), &[], 8787, &PiSettings::default());
    assert!(config.models_json.get("providers").is_none());
}

#[test]
fn dollar_in_group_key_is_escaped_to_literal() {
    // pi 在 apiKey 里做环境变量插值，且插值在长字面量内部也生效。不转义则分组名
    // `$HOME` 会被解析成环境变量值，路由 token 完全错位。
    let config = build_pi_config(&empty(), &empty(), &[group("a$HOME-b", &["m"])], 8787, &PiSettings::default());
    assert_eq!(providers_of(&config)["aidog-a$HOME-b"]["apiKey"], "a$$HOME-b");
}

#[test]
fn bang_prefixed_group_key_is_escaped_to_literal() {
    // 开头 `!` 在 pi 里是「执行这条 shell 命令取 stdout」。不转义等于任意命令执行。
    let config = build_pi_config(&empty(), &empty(), &[group("!whoami", &["m"])], 8787, &PiSettings::default());
    assert_eq!(providers_of(&config)["aidog-!whoami"]["apiKey"], "$!whoami");
}

#[test]
fn group_extra_carries_the_protocol_choice() {
    assert_eq!(parse_group_api(r#"{"pi_api":"openai-responses"}"#), PiApi::OpenaiResponses);
    // 老分组：extra 为空串 / 无该键 / 值非法，一律 anthropic（向后兼容，不报错）。
    assert_eq!(parse_group_api(""), PiApi::AnthropicMessages);
    assert_eq!(parse_group_api(r#"{"_ui_collapsed":true}"#), PiApi::AnthropicMessages);
    assert_eq!(parse_group_api(r#"{"pi_api":"nonsense"}"#), PiApi::AnthropicMessages);
}

fn settings_of(existing: &Value, settings: &PiSettings) -> Map<String, Value> {
    build_pi_config(&empty(), existing, &[group("g", &["m"])], 8787, settings)
        .settings_json
        .as_object()
        .expect("settings object")
        .clone()
}

#[test]
fn default_group_becomes_pi_default_provider() {
    let s = settings_of(
        &empty(),
        &PiSettings { default_group: Some("teamA".into()), ..PiSettings::default() },
    );
    assert_eq!(s["defaultProvider"], "aidog-teamA");
}

#[test]
fn clearing_default_group_removes_only_aidogs_own_value() {
    let ours = serde_json::json!({ "defaultProvider": "aidog-teamA" });
    assert!(!settings_of(&ours, &PiSettings::default()).contains_key("defaultProvider"));

    // 用户手设的默认 provider 不是 aidog 写的，取消默认组时必须留着。
    let theirs = serde_json::json!({ "defaultProvider": "anthropic" });
    assert_eq!(settings_of(&theirs, &PiSettings::default())["defaultProvider"], "anthropic");
}

#[test]
fn outbound_proxy_goes_to_pis_native_setting() {
    let s = settings_of(
        &empty(),
        &PiSettings { http_proxy: Some("http://127.0.0.1:7890".into()), ..PiSettings::default() },
    );
    assert_eq!(s["httpProxy"], "http://127.0.0.1:7890");

    // aidog 没配代理就不碰这个键：分不清用户手填的值，删了等于吞掉用户配置。
    let theirs = serde_json::json!({ "httpProxy": "http://user-proxy:1080" });
    assert_eq!(settings_of(&theirs, &PiSettings::default())["httpProxy"], "http://user-proxy:1080");
}

#[test]
fn unrelated_settings_keys_survive_the_write() {
    let existing = serde_json::json!({ "theme": "dark", "quietStartup": true, "somethingAidogNeverHeardOf": 42 });
    let s = settings_of(
        &existing,
        &PiSettings { default_group: Some("g".into()), http_proxy: Some("http://p:1".into()) },
    );
    assert_eq!(s["theme"], "dark");
    assert_eq!(s["quietStartup"], true);
    assert_eq!(s["somethingAidogNeverHeardOf"], 42);
}

#[test]
fn sync_writes_then_skips_unchanged_then_rewrites_on_port_change() {
    let _g = HomeGuard::new();
    let groups = [group("grp", &["m"])];

    let first = sync_groups(&groups, 9000, &PiSettings::default()).unwrap();
    assert!(!first.is_empty(), "first sync must write models.json");

    let again = sync_groups(&groups, 9000, &PiSettings::default()).unwrap();
    assert!(again.is_empty(), "unchanged content must not rewrite");

    let changed = sync_groups(&groups, 9001, &PiSettings::default()).unwrap();
    assert!(!changed.is_empty(), "port change must rewrite");
}

#[test]
fn settings_read_write_round_trip_keeps_keys_the_schema_does_not_cover() {
    let _g = HomeGuard::new();
    let original = serde_json::json!({
        "theme": "dark",
        "somethingAidogNeverHeardOf": { "nested": [1, 2, 3] }
    });

    pi_settings_write(original.clone()).unwrap();
    let read_back = pi_settings_read().unwrap();
    pi_settings_write(read_back.clone()).unwrap();

    assert_eq!(read_back, original);
    assert_eq!(pi_settings_read().unwrap(), original);
}

#[test]
fn settings_read_returns_empty_object_when_file_is_missing() {
    // 页面据此填推荐默认，而不是报错。
    let _g = HomeGuard::new();
    assert_eq!(pi_settings_read().unwrap(), empty());
}

#[test]
fn read_corrupt_models_json_errors_with_filename() {
    let _g = HomeGuard::new();
    let path = models_path().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{\"providers\": {,}").unwrap();

    let err = sync_groups(&[group("g", &["m"])], 8787, &PiSettings::default()).expect_err("corrupt JSON must error");
    assert!(err.starts_with("parse "), "err should mark parse stage: {err}");
}
