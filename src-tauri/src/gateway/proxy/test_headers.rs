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

    // ── OpenCode Zen api_key 兜底（决策核单测）──
