use super::*;
use std::collections::HashSet;

/// (UA, 期望码)；None = 认不出。每个带规则的条目至少一条（`every_ruled_entry_has_a_sample` 校验）。
const CASES: &[(&str, Option<&str>)] = &[
    ("claude-cli/2.1.204 (external, cli)", Some("claude_code")),
    ("CLAUDE-CLI/2.0.1", Some("claude_code")),
    ("claude-cli/2.1.314 (external, claude-vscode, agent-sdk/0.1.30)", Some("claude_code_vscode")),
    ("claude-cli/2.1.314 (external, sdk-ts, agent-sdk/0.3.314)", Some("claude_code_sdk_ts")),
    ("claude-cli/2.1.314 (external, sdk-py)", Some("claude_code_sdk_py")),
    ("claude-cli/2.1.314 (external, sdk-cli)", Some("claude_code_sdk_cli")),
    ("claude-cli/2.1.314 (external, claude-code-github-action)", Some("claude_code_gh_action")),
    ("claude-cli/2.1.314 (external, claude-desktop)", Some("claude_code_desktop")),
    ("claude-cli/2.1.314 (external, mcp)", Some("claude_code_mcp")),
    ("claude-cli/2.1.314 (external, local-agent)", Some("claude_code_local_agent")),
    ("codex_cli_rs/0.38.0 (Mac OS 15.0; arm64) iTerm", Some("codex_cli")),
    ("codex-tui/0.38.0 (Mac OS 15.0; arm64)", Some("codex_tui")),
    ("codex_vscode/0.38.0 (Mac OS; arm64)", Some("codex_vscode")),
    ("Codex Desktop/0.38.0 (Mac OS; arm64)", Some("codex_desktop")),
    ("codex_exec/0.38.0 (Linux; x86_64)", Some("codex_exec")),
    // 老版本 / 自定义 originator → 大类兜底
    ("Codex/0.38.0", Some("codex")),
    ("pi (darwin; arm64)", Some("pi_cli")),
    ("pi/0.84.2 (darwin; node/v24.0.0; arm64)", Some("pi_cli")),
    ("pi", Some("pi_cli")),
    ("kilo/latest/7.8.18/cli", Some("kilo_code")),
    ("opencode-kilo-provider/1", Some("kilo_code")),
    ("opencode/1.19.5", Some("opencode")),
    ("opencode/dev/2.0.0/cli", Some("opencode")),
    ("QwenCode/0.26.4 (darwin; arm64)", Some("qwen_code")),
    ("RooCode/3.1.0", Some("roo_code")),
    ("roo-code/3.1.0 (darwin 24; arm64) node/v22", Some("roo_code")),
    ("Cline/3.40.0", Some("cline")),
    ("factory-cli/1.0.0", Some("factory_droid")),
    ("Amp-CLI/0.0.1", Some("amp")),
    ("Charm-Crush/0.99.0 (https://charm.land/crush)", Some("crush")),
    ("Continue/IDE", Some("continue")),
    ("Zed/1.26.2 (macos; aarch64)", Some("zed")),
    ("copilot/1.0.105", Some("copilot_cli")),
    ("GeminiCLI/0.66.0/gemini-pro (darwin; arm64; terminal)", Some("gemini_cli")),
    ("GeminiCLI-vscode/0.66.0/m (darwin; arm64)", Some("gemini_cli")),
    ("CloudCodeVSCode/1.0 (aidev_client; proxy_client=geminicli)", Some("gemini_cli")),
    ("Cursor/0.42", Some("cursor")),
    ("Windsurf/1.0", Some("windsurf")),
    ("litellm/1.50.0", Some("litellm")),
    ("langchain-anthropic/0.3.1", Some("langchain")),
    ("Anthropic/Python 0.40.0", Some("anthropic_sdk_py")),
    ("AsyncAnthropic/Python 0.40.0", Some("anthropic_sdk_py")),
    ("AnthropicBedrock/0.40.0", Some("anthropic_sdk_py")),
    ("Anthropic/JS 0.50.0", Some("anthropic_sdk_ts")),
    ("OpenAI/Python 1.60.0", Some("openai_sdk_py")),
    ("AsyncOpenAI/Python 1.60.0", Some("openai_sdk_py")),
    ("OpenAI/JS 4.80.0", Some("openai_sdk_node")),
    ("python-requests/2.32.3", Some("python_requests")),
    ("python-httpx/0.27.0", Some("python_httpx")),
    ("Python/3.12 aiohttp/3.9.5", Some("python_aiohttp")),
    ("node", Some("node_fetch")),
    ("undici", Some("node_fetch")),
    ("axios/1.7.2", Some("node_axios")),
    ("node-fetch/1.0 (+https://github.com/bitinn/node-fetch)", Some("node_node_fetch")),
    ("Go-http-client/1.1", Some("go_http")),
    ("okhttp/4.12.0", Some("java_okhttp")),
    ("Java-http-client/21.0.1", Some("java_httpclient")),
    ("curl/8.7.1", Some("curl")),
    ("Wget/1.21.4 (linux-gnu)", Some("wget")),
    ("HTTPie/3.2.2", Some("httpie")),
    // 顺序：AI 客户端先于 HTTP 库；变体先于大类；pi 的 detailed UA 含 node/ 不被 node_fetch 抢
    ("claude-cli codex", Some("claude_code")),
    ("codex-tui/1 axios/1", Some("codex_tui")),
    // 认不出
    ("Mozilla/5.0 api (x)", None),
    ("rust-thing/1", None),
    ("node/22", None),
    ("\u{0}garbage;;;", None),
    ("", None),
];

#[test]
fn table_driven_identify() {
    for (ua, want) in CASES {
        assert_eq!(identify(ua).map(|e| e.code), *want, "ua={ua:?}");
    }
}

#[test]
fn every_ruled_entry_has_a_sample() {
    let covered: HashSet<_> = CASES.iter().filter_map(|(_, c)| *c).collect();
    for e in CLIENTS.iter().filter(|e| !e.rules.is_empty()) {
        assert!(covered.contains(e.code), "{} 缺样例 UA", e.code);
    }
}

#[test]
fn structure_is_consistent() {
    let mut seen = HashSet::new();
    for e in CLIENTS {
        assert!(seen.insert(e.code), "重复码 {}", e.code);
        assert!(!e.src.is_empty(), "{} 缺来源", e.code);
        if let Some(p) = e.parent {
            let parent = CLIENTS.iter().find(|c| c.code == p).expect("父类存在");
            assert!(parent.parent.is_none(), "{} 的父 {} 不是大类", e.code, p);
            assert!(!e.rules.is_empty(), "变体 {} 必须有规则", e.code);
        }
    }
    // 每个大类要么自身可识别，要么至少有一个变体
    for c in vocab() {
        assert!(c.matchable || !c.variants.is_empty(), "{} 空大类", c.code);
    }
}

#[test]
fn legacy_inbound_codes_stay_valid() {
    for old in ["claude_code", "codex", "pi", "cursor", "windsurf", "gemini_cli"] {
        let e = CLIENTS.iter().find(|e| e.code == old).expect(old);
        assert!(e.parent.is_none(), "{old} 必须仍是大类");
    }
}

#[test]
fn outbound_simulation_codes_align_with_vocab() {
    let doc: serde_json::Value = serde_json::from_str(aidog_db::client_types_const::BUNDLED).unwrap();
    let codes: HashSet<_> = CLIENTS.iter().map(|e| e.code).collect();
    for ct in doc["client_types"].as_array().unwrap() {
        let v = ct["value"].as_str().unwrap();
        if v != "default" {
            assert!(codes.contains(v), "出站模拟值 {v} 不在词表");
        }
    }
}

#[test]
fn every_code_has_name_in_all_locales() {
    let dir = format!("{}/../aidog_i18n/locales", env!("CARGO_MANIFEST_DIR"));
    for l in ["zh-Hans", "en-US", "ar-SA", "fr-FR", "de-DE", "ru-RU", "ja-JP", "es-ES"] {
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(format!("{dir}/{l}.json")).unwrap()).unwrap();
        for e in CLIENTS {
            let k = name_key(e.code);
            let v = doc[&k].as_str().unwrap_or("");
            assert!(!v.trim().is_empty(), "{l} 缺 {k}");
        }
    }
}

/// desktop 展开（直调函数）与 http 展开返回同一结构。
#[tokio::test]
async fn command_desktop_and_http_expansions_agree() {
    let direct = serde_json::to_value(get_client_vocab().await.unwrap()).unwrap();
    let axum::Json(over_http) = get_client_vocab::http(axum::Json(serde_json::json!({}))).await.unwrap();
    assert_eq!(direct, over_http);
    let cc = direct.as_array().unwrap().iter().find(|c| c["code"] == "claude_code").unwrap();
    assert_eq!(cc["nameKey"], "flow.client.claude_code");
    assert_eq!(cc["matchable"], true);
    assert!(cc["variants"].as_array().unwrap().iter().any(|v| v["code"] == "claude_code_vscode"));
}
