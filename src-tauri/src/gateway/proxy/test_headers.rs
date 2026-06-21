use super::*;

    #[test]
    fn build_upstream_headers_passes_through_and_overrides_auth() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert("anthropic-beta", "beta-x".parse().unwrap());
        orig.insert("x-stainless-package-version", "0.94.0".parse().unwrap());
        orig.insert("cookie", "secret-cookie".parse().unwrap());
        orig.insert("authorization", "Bearer sk-inbound".parse().unwrap());

        let h = build_upstream_headers(&ClientType::ClaudeCode, &crate::gateway::models::Protocol::Anthropic, "sk-realkey-1234567890", &orig);
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        // 入站 SDK 头透传
        assert_eq!(m.get("anthropic-beta"), Some(&"beta-x"));
        assert_eq!(m.get("x-stainless-package-version"), Some(&"0.94.0"));
        // cookie 脱敏
        assert_eq!(m.get("cookie"), Some(&"[REDACTED]"));
        // auth 覆盖为平台 key（redact）+ UA 模拟
        assert!(m.get("x-api-key").unwrap().contains("****"), "x-api-key must be redacted platform key");
        assert!(m.get("User-Agent").unwrap().starts_with("claude-cli/"));
        assert_eq!(m.get("Content-Type"), Some(&"application/json"));
    }

    // ── OpenCode Zen api_key 兜底（决策核单测）──
