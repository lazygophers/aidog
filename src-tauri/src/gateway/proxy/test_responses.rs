
    #[test]
    fn responses_subendpoint_detection() {
        use super::is_responses_subendpoint;
        // create（裸 /v1/responses，无尾段）→ false（关键回归：不被新分流拦）
        assert!(!is_responses_subendpoint("/proxy/v1/responses"), "create bare path must NOT be subendpoint");
        assert!(!is_responses_subendpoint("/proxy/v1/responses/"), "create with trailing slash must NOT be subendpoint");
        assert!(!is_responses_subendpoint("/v1/responses"), "create (no proxy prefix) must NOT be subendpoint");
        // 子端点 → true
        assert!(is_responses_subendpoint("/proxy/v1/responses/resp_123"), "retrieve must be subendpoint");
        assert!(is_responses_subendpoint("/proxy/v1/responses/resp_123/cancel"), "cancel must be subendpoint");
        assert!(is_responses_subendpoint("/proxy/v1/responses/resp_123/input_items"), "input_items must be subendpoint");
        assert!(is_responses_subendpoint("/proxy/v1/responses/compact"), "compact must be subendpoint");
        assert!(is_responses_subendpoint("/v1/responses/resp_123"), "subendpoint without proxy prefix must be true");
        // 无 /v1/ 前缀 / 非 responses → false
        assert!(!is_responses_subendpoint("/proxy/v1/chat/completions"), "chat must NOT be subendpoint");
        assert!(!is_responses_subendpoint("/proxy/responses/resp_123"), "missing /v1/ prefix must NOT be subendpoint");
        assert!(!is_responses_subendpoint("/v1/messages"), "anthropic must NOT be subendpoint");
    }

    // ── 子端点上游 URL 构造：base_url(含 /v1) + 子路径(去 /v1)，禁重复拼版本 ──
    #[test]
    fn responses_subendpoint_url_construction() {
        // 镜像 handle_responses_subendpoint 的 URL 拼接逻辑
        let build = |base_url: &str, path: &str| -> String {
            let api_path = match path.find("/v1/") {
                Some(idx) => &path[idx..],
                None => path,
            };
            let sub_path = api_path.strip_prefix("/v1").unwrap_or(api_path);
            format!("{}{}", base_url.trim_end_matches('/'), sub_path)
        };
        // openai 标准 base_url 含 /v1 → 不重复拼
        assert_eq!(
            build("https://api.openai.com/v1", "/proxy/v1/responses/resp_abc/cancel"),
            "https://api.openai.com/v1/responses/resp_abc/cancel"
        );
        assert_eq!(
            build("https://api.openai.com/v1", "/proxy/v1/responses/resp_abc"),
            "https://api.openai.com/v1/responses/resp_abc"
        );
        assert_eq!(
            build("https://api.openai.com/v1", "/proxy/v1/responses/compact"),
            "https://api.openai.com/v1/responses/compact"
        );
        // base_url 末尾带斜杠 → trim 后正确
        assert_eq!(
            build("https://api.openai.com/v1/", "/proxy/v1/responses/resp_abc/input_items"),
            "https://api.openai.com/v1/responses/resp_abc/input_items"
        );
    }

    // ── count_tokens 端点识别：尾段 /v1/messages/count_tokens 才命中，普通 /v1/messages 不命中 ──
