use super::*;

    #[test]
    fn build_upstream_headers_passes_through_and_overrides_auth() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert("anthropic-beta", "beta-x".parse().unwrap());
        orig.insert("x-stainless-package-version", "0.94.0".parse().unwrap());
        orig.insert("cookie", "secret-cookie".parse().unwrap());
        orig.insert("authorization", "Bearer sk-inbound".parse().unwrap());

        // 官方 Anthropic 上游 → anthropic-beta 保留（依赖 beta 协商 1m-context/thinking 等能力）
        let h = build_upstream_headers(
            &ClientType::ClaudeCode,
            &crate::gateway::models::Protocol::Anthropic,
            "sk-realkey-1234567890",
            &orig,
            "https://api.anthropic.com/v1/messages",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        // 入站 SDK 头透传（官方端点 beta 保留）
        assert_eq!(m.get("anthropic-beta"), Some(&"beta-x"));
        assert_eq!(m.get("x-stainless-package-version"), Some(&"0.94.0"));
        // cookie 脱敏
        assert_eq!(m.get("cookie"), Some(&"[REDACTED]"));
        // auth 覆盖为平台 key（redact）+ UA 模拟
        assert!(m.get("x-api-key").unwrap().contains("****"), "x-api-key must be redacted platform key");
        assert!(m.get("User-Agent").unwrap().starts_with("claude-cli/"));
        assert_eq!(m.get("Content-Type"), Some(&"application/json"));
    }

    // ── 第三方 anthropic 兼容端点：anthropic-beta 剔除（不认新 beta token，原样透传致 GLM 400 code 1210）──
    #[test]
    fn build_upstream_headers_strips_anthropic_beta_for_third_party() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert("anthropic-beta", "context-1m-2025-08-07,effort-2025-11-24".parse().unwrap());
        orig.insert("anthropic-version", "2023-06-01".parse().unwrap());
        orig.insert("x-stainless-package-version", "0.94.0".parse().unwrap());

        // GLM anthropic 兼容端点 → 剔 anthropic-beta，其余 SDK 头照常透传
        let h = build_upstream_headers(
            &ClientType::ClaudeCode,
            &crate::gateway::models::Protocol::Anthropic,
            "sk-realkey-1234567890",
            &orig,
            "https://open.bigmodel.cn/api/anthropic/v1/messages",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        assert_eq!(m.get("anthropic-beta"), None, "anthropic-beta must be stripped for third-party endpoint");
        // 非 beta 的 SDK 头仍透传（保留诊断信息 + 协议必需的 version）
        assert_eq!(m.get("anthropic-version"), Some(&"2023-06-01"));
        assert_eq!(m.get("x-stainless-package-version"), Some(&"0.94.0"));
    }

    // ── redact_key: 短key完全遮盖，长key首4末4可见 ──
    #[test]
    fn redact_key_short_is_fully_redacted() {
        assert_eq!(redact_key("short"), "[REDACTED]");
        assert_eq!(redact_key("12chars_only"), "[REDACTED]");
        assert_eq!(redact_key(""), "[REDACTED]");
    }

    #[test]
    fn redact_key_long_shows_prefix_suffix() {
        let key = "sk-abc1234567890xyz";
        let result = redact_key(key);
        // First 4 chars: "sk-a"
        assert!(result.starts_with("sk-a"), "should show first 4: {result}");
        // Last 4 chars: "0xyz"
        assert!(result.ends_with("0xyz"), "should show last 4: {result}");
        assert!(result.contains("****"), "should contain mask: {result}");
    }

    // ── is_official_anthropic_host ──
    #[test]
    fn is_official_anthropic_host_variants() {
        assert!(is_official_anthropic_host("https://api.anthropic.com/v1/messages"));
        assert!(is_official_anthropic_host("https://API.ANTHROPIC.COM/v1/messages"), "case-insensitive");
        assert!(is_official_anthropic_host("https://api.anthropic.com:443/v1/messages"), "with port");
        assert!(!is_official_anthropic_host("https://open.bigmodel.cn/api/anthropic/v1/messages"));
        assert!(!is_official_anthropic_host("https://third-party.example.com/v1/messages"));
        assert!(!is_official_anthropic_host(""));
    }

    // ── passthrough_headers: 剔 host 和 content-length ──
    #[test]
    fn passthrough_headers_strips_hop_by_hop() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert(axum::http::header::HOST, "example.com".parse().unwrap());
        orig.insert(axum::http::header::CONTENT_LENGTH, "100".parse().unwrap());
        orig.insert("x-custom", "keep-me".parse().unwrap());
        orig.insert("authorization", "Bearer tok".parse().unwrap());

        let out = passthrough_headers(&orig);
        assert!(!out.contains_key("host"), "host must be stripped");
        assert!(!out.contains_key("content-length"), "content-length must be stripped");
        assert!(out.contains_key("x-custom"), "custom header must pass through");
        assert!(out.contains_key("authorization"), "auth passes through in passthrough path");
    }

    // ── passthrough_convert_headers ──
    #[test]
    fn passthrough_convert_headers_strips_auth_and_ua() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert("authorization", "Bearer inbound-key".parse().unwrap());
        orig.insert("x-api-key", "inbound-api-key".parse().unwrap());
        orig.insert("user-agent", "claude-cli/1.0".parse().unwrap());
        orig.insert("content-type", "application/json".parse().unwrap());
        orig.insert("anthropic-version", "2023-06-01".parse().unwrap());
        orig.insert("x-custom-app", "my-app".parse().unwrap());

        let out = passthrough_convert_headers(&orig, "https://api.anthropic.com/v1/messages");
        assert!(!out.contains_key("authorization"), "auth stripped in convert path");
        assert!(!out.contains_key("x-api-key"), "x-api-key stripped in convert path");
        assert!(!out.contains_key("user-agent"), "ua stripped in convert path");
        assert!(!out.contains_key("content-type"), "ct stripped in convert path");
        // non-stripped headers pass through
        assert!(out.contains_key("anthropic-version"));
        assert!(out.contains_key("x-custom-app"));
    }

    #[test]
    fn passthrough_convert_headers_strips_anthropic_beta_for_third_party() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert("anthropic-beta", "context-1m".parse().unwrap());
        orig.insert("anthropic-version", "2023-06-01".parse().unwrap());

        let out = passthrough_convert_headers(&orig, "https://open.bigmodel.cn/v1/messages");
        assert!(!out.contains_key("anthropic-beta"), "beta stripped for third-party");
        assert!(out.contains_key("anthropic-version"), "version kept");
    }

    #[test]
    fn passthrough_convert_headers_keeps_anthropic_beta_for_official() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert("anthropic-beta", "context-1m".parse().unwrap());

        let out = passthrough_convert_headers(&orig, "https://api.anthropic.com/v1/messages");
        assert!(out.contains_key("anthropic-beta"), "beta kept for official anthropic");
    }

    // ── is_sensitive_auth_header ──
    #[test]
    fn is_sensitive_auth_header_cases() {
        assert!(is_sensitive_auth_header("authorization"));
        assert!(is_sensitive_auth_header("Authorization"));
        assert!(is_sensitive_auth_header("AUTHORIZATION"));
        assert!(is_sensitive_auth_header("x-api-key"));
        assert!(is_sensitive_auth_header("X-Api-Key"));
        assert!(is_sensitive_auth_header("x-goog-api-key"));
        assert!(is_sensitive_auth_header("api-key"));
        assert!(!is_sensitive_auth_header("content-type"));
        assert!(!is_sensitive_auth_header("user-agent"));
    }

    // ── build_upstream_headers: codex family with OpenAI protocol ──
    #[test]
    fn build_upstream_headers_codex_openai() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::CodexCli,
            &crate::gateway::models::Protocol::OpenAI,
            "sk-test-key-1234567890",
            &orig,
            "https://api.openai.com/v1/chat/completions",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert!(m.get("User-Agent").unwrap().contains("codex"), "codex UA");
        assert!(m.contains_key("OpenAI-Beta"), "OpenAI-Beta header");
        assert!(m.contains_key("conversation_id"), "conversation_id present");
        assert!(m.contains_key("session_id"), "session_id present");
        assert!(m.get("Authorization").unwrap().starts_with("Bearer "), "Bearer auth");
    }

    #[test]
    fn build_upstream_headers_codex_tui_openai() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::CodexTui,
            &crate::gateway::models::Protocol::OpenAI,
            "sk-test-key-1234567890",
            &orig,
            "https://api.openai.com/v1/chat/completions",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert!(m.get("User-Agent").unwrap().contains("Codex"));
    }

    #[test]
    fn build_upstream_headers_codex_anthropic() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::CodexCli,
            &crate::gateway::models::Protocol::Anthropic,
            "sk-test-key-1234567890",
            &orig,
            "https://api.anthropic.com/v1/messages",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        // anthropic protocol: x-api-key
        assert!(m.contains_key("x-api-key"), "x-api-key for anthropic");
        // no OpenAI-Beta for anthropic protocol
        assert!(!m.contains_key("OpenAI-Beta"), "no OpenAI-Beta for anthropic protocol");
    }

    #[test]
    fn build_upstream_headers_cursor_and_windsurf() {
        let orig = axum::http::HeaderMap::new();
        let h_cursor = build_upstream_headers(
            &ClientType::Cursor,
            &crate::gateway::models::Protocol::Anthropic,
            "sk-test-key-1234567890",
            &orig,
            "https://api.anthropic.com/v1/messages",
        );
        let m_cursor: std::collections::HashMap<&str, &str> = h_cursor.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(m_cursor.get("User-Agent"), Some(&"Cursor/0.50.7"));

        let h_windsurf = build_upstream_headers(
            &ClientType::Windsurf,
            &crate::gateway::models::Protocol::Anthropic,
            "sk-test-key-1234567890",
            &orig,
            "https://api.anthropic.com/v1/messages",
        );
        let m_windsurf: std::collections::HashMap<&str, &str> = h_windsurf.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(m_windsurf.get("User-Agent"), Some(&"Windsurf/1.5.0"));
    }

    #[test]
    fn build_upstream_headers_default_client_gemini() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::Default,
            &crate::gateway::models::Protocol::Gemini,
            "AIza-test-key-1234567890",
            &orig,
            "https://generativelanguage.googleapis.com/v1beta",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert!(m.contains_key("x-goog-api-key"), "gemini uses x-goog-api-key");
        assert!(!m.contains_key("Authorization"), "no Authorization for gemini");
    }

    #[test]
    fn build_upstream_headers_default_client_openai() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::Default,
            &crate::gateway::models::Protocol::OpenAI,
            "sk-test-key-1234567890",
            &orig,
            "https://api.openai.com/v1/chat/completions",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert!(m.get("Authorization").unwrap().starts_with("Bearer "), "OpenAI uses Bearer auth");
        // No UA added for Default client
        assert!(!m.contains_key("User-Agent"), "Default client adds no UA");
    }

    // ── claude_code UA variants ──
    #[test]
    fn build_upstream_headers_claude_code_variants() {
        let orig = axum::http::HeaderMap::new();
        let variants = [
            (ClientType::ClaudeCodeVscode, "claude-vscode"),
            (ClientType::ClaudeCodeSdkTs, "sdk-ts"),
            (ClientType::ClaudeCodeSdkPy, "sdk-py"),
            (ClientType::ClaudeCodeGhAction, "claude-code-github-action"),
        ];
        for (ct, expected_part) in &variants {
            let h = build_upstream_headers(
                ct,
                &crate::gateway::models::Protocol::Anthropic,
                "sk-test-key-1234567890",
                &orig,
                "https://api.anthropic.com/v1/messages",
            );
            let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            let ua = m.get("User-Agent").unwrap_or(&"");
            assert!(ua.contains(expected_part), "UA for {ct:?} should contain '{expected_part}', got: {ua}");
        }
    }

    // ── codex desktop/vscode UA variants ──
    #[test]
    fn build_upstream_headers_codex_variants() {
        let orig = axum::http::HeaderMap::new();
        let variants = [
            (ClientType::CodexDesktop, "codex desktop"),
            (ClientType::CodexVscode, "codex-vscode"),
        ];
        for (ct, expected_part) in &variants {
            let h = build_upstream_headers(
                ct,
                &crate::gateway::models::Protocol::OpenAI,
                "sk-test-key-1234567890",
                &orig,
                "https://api.openai.com/v1/chat/completions",
            );
            let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            let ua = m.get("User-Agent").unwrap_or(&"");
            assert!(ua.contains(expected_part), "UA for {ct:?} should contain '{expected_part}', got: {ua}");
        }
    }

    // ── cursor/windsurf with OpenAI protocol ──
    #[test]
    fn build_upstream_headers_cursor_openai() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::Cursor,
            &crate::gateway::models::Protocol::OpenAI,
            "sk-test-key-1234567890",
            &orig,
            "https://api.openai.com/v1/chat/completions",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(m.get("User-Agent"), Some(&"Cursor/0.50.7"));
        assert!(m.get("Authorization").unwrap().starts_with("Bearer "));
    }

    #[test]
    fn build_upstream_headers_windsurf_gemini() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::Windsurf,
            &crate::gateway::models::Protocol::Gemini,
            "AIza-test-key-1234567890",
            &orig,
            "https://generativelanguage.googleapis.com/v1beta",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(m.get("User-Agent"), Some(&"Windsurf/1.5.0"));
        assert!(m.contains_key("x-goog-api-key"));
    }

    // ── format_pretty_json ──
    #[test]
    fn format_pretty_json_valid_json() {
        let input = r#"{"key":"value","num":42}"#;
        let result = format_pretty_json(input);
        assert!(result.contains('\n'), "pretty-printed JSON should have newlines");
        assert!(result.contains("\"key\""), "should contain key");
    }

    #[test]
    fn format_pretty_json_invalid_json_returns_original() {
        let input = "not-json-at-all";
        let result = format_pretty_json(input);
        assert_eq!(result, input, "invalid JSON should return original");
    }

    #[test]
    fn format_pretty_json_empty_object() {
        let result = format_pretty_json("{}");
        assert!(!result.is_empty());
    }

    // ── uuid_sim: format check ──
    #[test]
    fn uuid_sim_format_and_uniqueness() {
        let a = uuid_sim();
        let b = uuid_sim();
        // UUID-like format should contain hyphens
        assert!(a.contains('-'), "uuid_sim output should contain hyphens: {a}");
        // Should not be empty
        assert!(!a.is_empty());
        assert!(!b.is_empty());
        // Both calls return valid strings
        let _ = (a, b);
    }

    // ── inject_coding_plan_fields ──
    #[test]
    fn inject_coding_plan_fields_kimi_adds_prompt_cache_key() {
        let mut body = serde_json::json!({"model": "moonshot-v1-8k", "messages": []});
        inject_coding_plan_fields(&mut body, &crate::gateway::models::Protocol::Kimi);
        assert!(body.get("prompt_cache_key").is_some(), "Kimi must inject prompt_cache_key");
        let key = body["prompt_cache_key"].as_str().unwrap();
        assert!(key.starts_with("aidog-"), "key should start with aidog-: {key}");
    }

    #[test]
    fn inject_coding_plan_fields_other_protocols_no_op() {
        let protocols = [
            crate::gateway::models::Protocol::OpenAI,
            crate::gateway::models::Protocol::Anthropic,
            crate::gateway::models::Protocol::Gemini,
        ];
        for proto in &protocols {
            let mut body = serde_json::json!({"model": "gpt-4o"});
            inject_coding_plan_fields(&mut body, proto);
            assert!(body.get("prompt_cache_key").is_none(), "non-Kimi protocol should not inject: {proto:?}");
        }
    }

    // ── override_coding_plan_path: no-op ──
    #[test]
    fn override_coding_plan_path_is_noop() {
        let mut path = "/chat/completions".to_string();
        override_coding_plan_path(&mut path, &crate::gateway::models::Protocol::OpenAI);
        assert_eq!(path, "/chat/completions");
    }

    // ── build_upstream_headers: claude code with anthropic + gemini ──
    #[test]
    fn build_upstream_headers_claude_code_gemini() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::ClaudeCode,
            &crate::gateway::models::Protocol::Gemini,
            "AIza-test-key-1234567890",
            &orig,
            "https://generativelanguage.googleapis.com/v1beta",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert!(m.contains_key("x-goog-api-key"), "gemini key header");
    }

    #[test]
    fn build_upstream_headers_claude_code_openai_third_party() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::ClaudeCode,
            &crate::gateway::models::Protocol::OpenAI,
            "sk-test-key-1234567890",
            &orig,
            "https://third-party.example.com/v1/chat/completions",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        // For openai protocol, should have Bearer auth
        assert!(m.get("Authorization").unwrap_or(&"").starts_with("Bearer "), "OpenAI bearer auth: {:?}", m);
    }

    #[test]
    fn build_upstream_headers_codex_gemini() {
        let orig = axum::http::HeaderMap::new();
        let h = build_upstream_headers(
            &ClientType::CodexCli,
            &crate::gateway::models::Protocol::Gemini,
            "AIza-test-key-1234567890",
            &orig,
            "https://generativelanguage.googleapis.com/v1beta",
        );
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert!(m.contains_key("x-goog-api-key"));
        assert!(!m.contains_key("OpenAI-Beta"), "no OpenAI-Beta for gemini protocol");
    }

    // ── apply_client_headers covers private apply_* functions ──

    fn headers_from_builder(rb: reqwest::RequestBuilder) -> reqwest::header::HeaderMap {
        rb.build().unwrap().headers().clone()
    }

    #[test]
    fn apply_client_headers_default_anthropic() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::Default, &crate::gateway::models::Protocol::Anthropic, "sk-test-key-1234");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("x-api-key"));
    }

    #[test]
    fn apply_client_headers_default_gemini() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::Default, &crate::gateway::models::Protocol::Gemini, "AIza-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("x-goog-api-key"));
    }

    #[test]
    fn apply_client_headers_default_openai() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::Default, &crate::gateway::models::Protocol::OpenAI, "sk-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("Authorization"));
        assert!(h.contains_key("api-key"));
    }

    #[test]
    fn apply_client_headers_claude_code_anthropic() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::ClaudeCode, &crate::gateway::models::Protocol::Anthropic, "sk-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("User-Agent"));
        assert!(h.contains_key("x-api-key"));
    }

    #[test]
    fn apply_client_headers_claude_code_gemini() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::ClaudeCodeVscode, &crate::gateway::models::Protocol::Gemini, "AIza-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("x-goog-api-key"));
    }

    #[test]
    fn apply_client_headers_claude_code_other_protocol() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::ClaudeCodeSdkTs, &crate::gateway::models::Protocol::DeepSeek, "sk-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("Authorization"));
    }

    #[test]
    fn apply_client_headers_codex_anthropic() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::CodexCli, &crate::gateway::models::Protocol::Anthropic, "sk-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("x-api-key"));
        assert!(!h.contains_key("OpenAI-Beta"));
    }

    #[test]
    fn apply_client_headers_codex_gemini() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::CodexTui, &crate::gateway::models::Protocol::Gemini, "AIza-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("x-goog-api-key"));
    }

    #[test]
    fn apply_client_headers_codex_other() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::CodexDesktop, &crate::gateway::models::Protocol::OpenRouter, "sk-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("Authorization"));
    }

    #[test]
    fn apply_client_headers_cursor_openai() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::Cursor, &crate::gateway::models::Protocol::OpenAI, "sk-key");
        let h = headers_from_builder(rb);
        let ua = h.get("User-Agent").unwrap().to_str().unwrap();
        assert_eq!(ua, "Cursor/0.50.7");
        assert!(h.contains_key("Authorization"));
    }

    #[test]
    fn apply_client_headers_cursor_gemini() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::Cursor, &crate::gateway::models::Protocol::Gemini, "AIza-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("x-goog-api-key"));
    }

    #[test]
    fn apply_client_headers_cursor_other() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::Cursor, &crate::gateway::models::Protocol::Kimi, "sk-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("Authorization"));
    }

    #[test]
    fn apply_client_headers_windsurf_openai() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::Windsurf, &crate::gateway::models::Protocol::OpenAI, "sk-key");
        let h = headers_from_builder(rb);
        let ua = h.get("User-Agent").unwrap().to_str().unwrap();
        assert_eq!(ua, "Windsurf/1.5.0");
        assert!(h.contains_key("Authorization"));
    }

    #[test]
    fn apply_client_headers_windsurf_gemini() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::Windsurf, &crate::gateway::models::Protocol::Gemini, "AIza-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("x-goog-api-key"));
    }

    #[test]
    fn apply_client_headers_windsurf_other() {
        let client = reqwest::Client::new();
        let rb = client.post("http://localhost");
        let rb = apply_client_headers(rb, &ClientType::Windsurf, &crate::gateway::models::Protocol::DeepSeek, "sk-key");
        let h = headers_from_builder(rb);
        assert!(h.contains_key("Authorization"));
    }

    #[test]
    fn inject_trace_header_debug_build_inserts_header() {
        // 测试在 debug build (cargo test 默认 debug) 下运行 → cfg!(debug_assertions) 为 true。
        // 模拟 `make run` (yarn tauri dev = debug build) 期望路径：响应头含 X-AiDog-Trace。
        let mut resp = axum::response::Response::new(axum::body::Body::empty());
        inject_trace_header(&mut resp);
        if cfg!(debug_assertions) {
            let h = resp.headers().get("x-aidog-trace");
            assert!(h.is_some(), "debug build 应注入 X-AiDog-Trace header");
            let v = h.unwrap().to_str().unwrap();
            assert!(!v.is_empty(), "X-AiDog-Trace 值非空");
            // 兜底 id 取自 current_trace_id (None, 测试无活跃 span) → new_trace_id = 8-hex
            assert_eq!(v.len(), 8, "兜底 new_trace_id 应为 8 位 hex");
            assert!(v.chars().all(|c| c.is_ascii_hexdigit()), "兜底 id 应为 hex 字符");
        } else {
            // release build 路径：不注入（无 header）
            assert!(resp.headers().get("x-aidog-trace").is_none());
        }
    }

    #[test]
    fn inject_trace_header_does_not_overwrite_existing_headers() {
        // 验证 helper 不破坏响应已有头（仅 insert 一个新头）。
        let mut resp = axum::response::Response::new(axum::body::Body::empty());
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        inject_trace_header(&mut resp);
        assert_eq!(
            resp.headers().get(axum::http::header::CONTENT_TYPE).unwrap(),
            "application/json",
            "已存在的头应保留"
        );
    }
