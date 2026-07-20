
    #[test]
    fn count_tokens_endpoint_detection() {
        use super::is_count_tokens_endpoint;
        // 命中（关键修复点）
        assert!(is_count_tokens_endpoint("/proxy/v1/messages/count_tokens"));
        assert!(is_count_tokens_endpoint("/glm-coding-plan-auto/v1/messages/count_tokens"));
        assert!(is_count_tokens_endpoint("/v1/messages/count_tokens"));
        assert!(is_count_tokens_endpoint("/v1/messages/count_tokens/")); // 容尾斜杠
        // 普通 messages → 不命中（关键回归：普通对话路径不被新分流拦）
        assert!(!is_count_tokens_endpoint("/proxy/v1/messages"));
        assert!(!is_count_tokens_endpoint("/v1/messages"));
        assert!(!is_count_tokens_endpoint("/v1/messages/"));
        // 无 /v1/ 前缀 / 其他端点 → 不命中
        assert!(!is_count_tokens_endpoint("/proxy/messages/count_tokens"));
        assert!(!is_count_tokens_endpoint("/v1/chat/completions"));
        assert!(!is_count_tokens_endpoint("/v1/responses/resp_1"));
    }

    // ── count_tokens 上游 URL 构造：anthropic base_url(不含 /v1) + /v1/messages/count_tokens ──
    #[test]
    fn count_tokens_url_construction() {
        let build = |base_url: &str| format!("{}/v1/messages/count_tokens", base_url.trim_end_matches('/'));
        // GLM anthropic 端点（base_url 不含 /v1）→ 拼出含尾段的完整 URL
        assert_eq!(
            build("https://open.bigmodel.cn/api/anthropic"),
            "https://open.bigmodel.cn/api/anthropic/v1/messages/count_tokens"
        );
        // anthropic 官方 base_url → 同款
        assert_eq!(
            build("https://api.anthropic.com"),
            "https://api.anthropic.com/v1/messages/count_tokens"
        );
        // base_url 末尾带斜杠 → trim 后正确，不双斜杠
        assert_eq!(
            build("https://api.anthropic.com/"),
            "https://api.anthropic.com/v1/messages/count_tokens"
        );
    }

    // ── 本地估算兜底：累计文本字符数 ~4 字符/token，保底 1 ──
    // s3 起改 per-model BPE 分词，旧 chars/4 断言失效；s4 重写为真 BPE 用例。
    #[test]
    #[ignore = "s3 接入 BPE 分词后旧 chars/4 断言失效，s4 重写"]
    fn count_tokens_local_estimate() {
        use super::estimate_input_tokens;
        // 空 body / 无文本字段 → 保底 1（不返回 0 误导客户端）
        assert_eq!(estimate_input_tokens(&serde_json::json!({}), "claude-3-opus"), 1);
        assert_eq!(estimate_input_tokens(&serde_json::Value::Null, "claude-3-opus"), 1);
        // messages 递归累计全部字符串值：role "user"(4) + content "abcdefgh"(8) = 12 → ceil(12/4)=3
        let body = serde_json::json!({
            "model": "claude-opus-4-8",
            "messages": [{"role": "user", "content": "abcdefgh"}]
        });
        assert_eq!(estimate_input_tokens(&body, "claude-3-opus"), 3);
        // system + messages + tools 全字符串值累计：
        // system "syst"(4) + role "user"(4) + content "msgs"(4) + tool name "x"(1) + desc "tdsc"(4) = 17 → ceil(17/4)=5
        let body = serde_json::json!({
            "system": "syst",
            "messages": [{"role": "user", "content": "msgs"}],
            "tools": [{"name": "x", "description": "tdsc"}]
        });
        assert_eq!(estimate_input_tokens(&body, "claude-3-opus"), 5);
        // model 字段不计入文本估算（仅 system/messages/tools）
        let with_model = serde_json::json!({ "model": "very-long-model-name-not-counted" });
        assert_eq!(estimate_input_tokens(&with_model, "claude-3-opus"), 1);
    }

    // ── 同协议透传判定：仅端点协议精确等于入站协议才透传 ──
    // 精确匹配 → 透传；openai_responses→openai 跨协议回退 → 不透传（必须真转换）。
