use super::*;
use serde_json::json;

#[test]
fn claude_env_extract() {
    let sc = json!({
        "env": {
            "ANTHROPIC_BASE_URL": "https://api.example.com",
            "ANTHROPIC_AUTH_TOKEN": "sk-token-xxx",
            "ANTHROPIC_MODEL": "claude-sonnet-4-6",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5"
        }
    });
    let p = build_provider(
        "id1".into(),
        "claude".into(),
        "Test".into(),
        sc,
        None,
    );
    assert_eq!(p.detected_base_url.as_deref(), Some("https://api.example.com"));
    assert_eq!(p.detected_api_key.as_deref(), Some("sk-token-xxx"));
    assert!(p.codex_config_parsed.is_none());
}

#[test]
fn claude_api_key_fallback_to_anthropic_api_key() {
    let sc = json!({
        "env": {
            "ANTHROPIC_BASE_URL": "https://api.example.com",
            "ANTHROPIC_API_KEY": "sk-ak-xxx"
        }
    });
    let p = build_provider("id".into(), "claude".into(), "N".into(), sc, None);
    assert_eq!(p.detected_api_key.as_deref(), Some("sk-ak-xxx"));
}

#[test]
fn claude_empty_key_is_none() {
    let sc = json!({
        "env": {
            "ANTHROPIC_BASE_URL": "https://api.example.com",
            "ANTHROPIC_AUTH_TOKEN": ""
        }
    });
    let p = build_provider("id".into(), "claude".into(), "N".into(), sc, None);
    assert!(p.detected_api_key.is_none());
}

#[test]
fn claude_empty_settings() {
    // Claude Official preset 模板 = {}。
    let p = build_provider(
        "id".into(),
        "claude".into(),
        "Official".into(),
        json!({}),
        None,
    );
    assert!(p.detected_base_url.is_none());
    assert!(p.detected_api_key.is_none());
}

#[test]
fn codex_settings_config_extract() {
    // 实证样本：本地 cc-switch.db comet codex provider。
    let sc = json!({
        "auth": {"OPENAI_API_KEY": "sk-y21zAr0Mp5UL600I7DyetzQ6kFYITzXDELdoY5vU3tmtZ6o6"},
        "config": "model_provider = \"newapi\"\nmodel = \"gpt-5.4\"\n\n[model_providers]\n[model_providers.newapi]\nname = \"NewAPI\"\nbase_url = \"https://api.cometapi.com/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n"
    });
    let p = build_provider("codex1".into(), "codex".into(), "Comet".into(), sc, None);
    assert_eq!(p.detected_api_key.as_deref(), Some("sk-y21zAr0Mp5UL600I7DyetzQ6kFYITzXDELdoY5vU3tmtZ6o6"));
    let cp = p.codex_config_parsed.expect("codex_config_parsed");
    assert_eq!(cp.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(cp.model_provider.as_deref(), Some("newapi"));
    assert_eq!(cp.base_url.as_deref(), Some("https://api.cometapi.com/v1"));
    assert_eq!(cp.wire_api.as_deref(), Some("responses"));
    assert_eq!(cp.provider_name.as_deref(), Some("NewAPI"));
    assert_eq!(p.detected_base_url.as_deref(), Some("https://api.cometapi.com/v1"));
}

#[test]
fn codex_wire_api_chat() {
    let sc = json!({
        "auth": {"OPENAI_API_KEY": "sk-x"},
        "config": "model = \"gpt-4\"\nmodel_provider = \"p\"\n[model_providers.p]\nbase_url = \"https://x.com/v1\"\nwire_api = \"chat\"\n"
    });
    let p = build_provider("c".into(), "codex".into(), "N".into(), sc, None);
    let cp = p.codex_config_parsed.unwrap();
    assert_eq!(cp.wire_api.as_deref(), Some("chat"));
}

#[test]
fn legacy_json_multi_app_config() {
    // cc-switch 旧 config.json(MultiAppConfig) 形态。
    let v = json!({
        "claudeConfig": {
            "providers": [
                {"id": "c1", "name": "C1", "settingsConfig": {"env": {"ANTHROPIC_BASE_URL": "https://c1.com", "ANTHROPIC_AUTH_TOKEN": "k1"}}},
                {"id": "c2", "name": "C2", "settingsConfig": {}}
            ]
        },
        "codexConfig": {
            "providers": [
                {"id": "x1", "name": "X1", "settingsConfig": {"auth": {"OPENAI_API_KEY": "ok"}, "config": "model = \"g\"\nmodel_provider = \"p\"\n[model_providers.p]\nbase_url = \"https://x.com\"\n"}}
            ]
        },
        "geminiConfig": {
            "providers": [{"id": "g1", "name": "G1", "settingsConfig": {}}]
        }
    });
    let txt = serde_json::to_string(&v).unwrap();
    let tmp = std::env::temp_dir().join("aidog_ccswitch_test.json");
    std::fs::write(&tmp, &txt).unwrap();
    let result = read_json(&tmp).unwrap();
    std::fs::remove_file(&tmp).ok();
    // 仅 claude + codex（gemini 过滤）。
    assert_eq!(result.len(), 3);
    assert_eq!(result.iter().filter(|p| p.app_type == "claude").count(), 2);
    assert_eq!(result.iter().filter(|p| p.app_type == "codex").count(), 1);
    let x1 = result.iter().find(|p| p.id == "x1").unwrap();
    assert_eq!(x1.detected_api_key.as_deref(), Some("ok"));
    assert_eq!(
        x1.codex_config_parsed.as_ref().unwrap().base_url.as_deref(),
        Some("https://x.com")
    );
}

#[test]
fn direct_source_file_path_not_treated_as_dir() {
    // 回归：read() 收到的 path 是 detect 返回的 .db 文件路径。旧逻辑把它
    // 当目录 join 出 `…/cc-switch.db/cc-switch.db`，exists()=false 误报
    // 未检测到。直读路径必须把文件路径识别为 sqlite 源。
    let dir = std::env::temp_dir().join(format!("aidog_ccsw_direct_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_file = dir.join("cc-switch.db");
    std::fs::write(&db_file, b"").unwrap();

    let got = direct_source_if_file(Some(&db_file.to_string_lossy()));
    assert_eq!(
        got,
        Some(("sqlite".to_string(), db_file.to_string_lossy().into_owned()))
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn direct_source_classifies_config_json_as_json() {
    let dir = std::env::temp_dir().join(format!("aidog_ccsw_json_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let json_file = dir.join("config.json");
    std::fs::write(&json_file, b"{}").unwrap();

    let got = direct_source_if_file(Some(&json_file.to_string_lossy()));
    assert_eq!(
        got,
        Some(("json".to_string(), json_file.to_string_lossy().into_owned()))
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn direct_source_returns_none_for_dir_or_missing_or_empty() {
    // 目录路径 → None（须走 detect 探测目录内文件）。
    let dir = std::env::temp_dir().join(format!("aidog_ccsw_none_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(direct_source_if_file(Some(&dir.to_string_lossy())), None);

    // 不存在路径 → None。
    let missing = dir.join("nope.db");
    assert_eq!(direct_source_if_file(Some(&missing.to_string_lossy())), None);

    // 缺省 / 空串 → None。
    assert_eq!(direct_source_if_file(None), None);
    assert_eq!(direct_source_if_file(Some("")), None);
    assert_eq!(direct_source_if_file(Some("   ")), None);

    std::fs::remove_dir_all(&dir).ok();
}
