
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

    // 测试辅助：从 test_count_tokens（child of count_tokens）拿 proxy::tokenizer 的 per-model BPE 入口。
    // ponytail: 直接走 super::super 而非 `crate::gateway::proxy::tokenizer`，避开私有 mod 路径限制。
    fn bpe_count(text: &str, model: &str) -> usize {
        super::super::tokenizer::count_tokens(text, model)
    }

    // ── per-model 分流：gpt-4o (o200k) vs gpt-4 (cl100k) 同文本 token 数不等（核心分流）──
    // 文本含 CJK + emoji + 长 token，最大化两类 BPE 词表差异（o200k 收 CJK/emoji 更激进）。
    #[test]
    fn per_model_gpt4o_vs_gpt4_diverges() {
        let text = "你好世界🌟 计算语言学 tokenizer 分词测试 longcompoundword";
        let o200k = bpe_count(text, "gpt-4o");
        let cl100k = bpe_count(text, "gpt-4");
        assert_ne!(
            o200k, cl100k,
            "o200k and cl100k must diverge on CJK+emoji mix; got o200k={o200k} cl100k={cl100k}"
        );
    }

    // ── claude-* 走 cl100k：与 gpt-4 (cl100k) 完全相等（同 BPE 实例）──
    #[test]
    fn per_model_claude_equals_gpt4_cl100k() {
        let text = "The quick brown fox jumps over 13 lazy dogs. 你好世界🌟";
        let claude = bpe_count(text, "claude-3-opus-20240229");
        let gpt4 = bpe_count(text, "gpt-4-turbo");
        assert_eq!(claude, gpt4, "claude-* and gpt-4 both route to cl100k, must be equal");
        // claude-opus-4-x 等不同 claude 变体也应一致
        let claude2 = bpe_count(text, "claude-opus-4-8");
        assert_eq!(claude, claude2);
    }

    // ── 未知模型走 cl100k 兜底：deepseek-chat / foobar 与 gpt-4 token 数相等 ──
    #[test]
    fn per_model_unknown_falls_back_to_cl100k() {
        let text = "Mixed CJK 你好 + emoji 🚀 + English word: tokenizer.";
        let gpt4 = bpe_count(text, "gpt-4");
        for unknown in ["deepseek-chat", "foobar", "some-future-model-xyz", "kimi-k2"] {
            assert_eq!(
                bpe_count(text, unknown),
                gpt4,
                "unknown model {unknown} should fall back to cl100k == gpt-4"
            );
        }
    }

    // ── glm-* 走 HF glm-4.json：与 gpt-4 (cl100k) 大概率不等（不同 BPE 词表/归一化）──
    #[test]
    fn per_model_glm_diverges_from_cl100k() {
        // HF glm tokenizer 对 CJK 有独立词表，cl100k 走 byte-fallback；混合文本二者分词几乎必不等。
        let text = "你好世界，这是一段中文测试。Hello world 🌟 tokenizer vocabulary divergence.";
        let glm = bpe_count(text, "glm-4-plus");
        let gpt4 = bpe_count(text, "gpt-4");
        assert_ne!(
            glm, gpt4,
            "glm-4 BPE should diverge from cl100k on CJK+emoji mix; got glm={glm} gpt4={gpt4}"
        );
    }

    // ── qwen-* 走 HF qwen2.json：与 gpt-4 (cl100k) 大概率不等 ──
    #[test]
    fn per_model_qwen_diverges_from_cl100k() {
        let text = "通义千问 Qwen2 分词器测试 Hello world 🚀 中文混合 English vocabulary.";
        let qwen = bpe_count(text, "qwen2-7b-instruct");
        let gpt4 = bpe_count(text, "gpt-4");
        assert_ne!(
            qwen, gpt4,
            "qwen2 BPE should diverge from cl100k on CJK+emoji mix; got qwen={qwen} gpt4={gpt4}"
        );
    }

    // ── estimate_input_tokens 端到端：anthropic body 累计 system+messages+tools 文本 → per-model 分流 ──
    #[test]
    fn estimate_input_tokens_per_model_routing() {
        use super::estimate_input_tokens;
        let body = serde_json::json!({
            "system": "你是中文助手。Hello system prompt 🌟.",
            "messages": [
                {"role": "user", "content": "请帮我分词：tokenizer 测试 你好世界"},
                {"role": "assistant", "content": "好的，我开始分析。"}
            ],
            "tools": [{"name": "count_tokens", "description": "估算输入 token 数"}]
        });
        let gpt4o = estimate_input_tokens(&body, "gpt-4o-2024-08-06");
        let gpt4 = estimate_input_tokens(&body, "gpt-4-turbo");
        let claude = estimate_input_tokens(&body, "claude-opus-4-8");
        let glm = estimate_input_tokens(&body, "glm-4.6");
        let qwen = estimate_input_tokens(&body, "qwen-max");
        let unknown = estimate_input_tokens(&body, "deepseek-chat");

        // 保底：所有模型返回正值
        for (n, label) in [(gpt4o, "gpt-4o"), (gpt4, "gpt-4"), (claude, "claude"), (glm, "glm"), (qwen, "qwen"), (unknown, "unknown")] {
            assert!(n > 0, "{label} should return positive token count, got {n}");
        }
        // claude 与 gpt-4 同 cl100k → 相等
        assert_eq!(claude, gpt4, "claude == gpt-4 (same cl100k)");
        // 未知 == gpt-4 (cl100k 兜底)
        assert_eq!(unknown, gpt4, "unknown falls back to cl100k == gpt-4");
        // o200k 与 cl100k 在混合 CJK+emoji 文本上分流不同
        assert_ne!(gpt4o, gpt4, "gpt-4o (o200k) should diverge from gpt-4 (cl100k)");
    }

    // ── estimate_input_tokens 空 body / 无文本字段 → 保底 1（不返回 0 误导客户端）──
    #[test]
    fn estimate_input_tokens_empty_floor() {
        use super::estimate_input_tokens;
        assert_eq!(estimate_input_tokens(&serde_json::json!({}), "claude-3-opus"), 1);
        assert_eq!(estimate_input_tokens(&serde_json::Value::Null, "claude-3-opus"), 1);
        // model 字段不计入文本估算（仅 system/messages/tools）
        let with_model = serde_json::json!({ "model": "very-long-model-name-not-counted" });
        assert_eq!(estimate_input_tokens(&with_model, "claude-3-opus"), 1);
        // 空字符串字段 → buf 为空 → 保底 1
        let empty = serde_json::json!({ "system": "", "messages": [] });
        assert_eq!(estimate_input_tokens(&empty, "claude-3-opus"), 1);
    }

    // ── estimate_input_tokens 递归累计：嵌套数组/对象所有字符串值都被收集 ──
    #[test]
    fn estimate_input_tokens_recursive_collect() {
        use super::estimate_input_tokens;
        // content array (anthropic 多段格式)：每段 {type,text} 的 text 都要被累计
        let body = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "第一段 Hello"},
                    {"type": "text", "text": "第二段 World 🌟"}
                ]
            }]
        });
        let n = estimate_input_tokens(&body, "gpt-4");
        assert!(n > 2, "multi-segment content should accumulate both texts, got {n}");
    }

    // ── BPE 真分词 vs chars/4 启发式：纯中文 gpt-4o token 数远 > chars/4 低估 ──
    // chars/4 对 4 个中文字符估算 1 token；真 cl100k 每个 CJK 字符约 1-2 token。
    #[test]
    fn bpe_not_chars_heuristic() {
        let zh = "你好世界明天会更好"; // 9 个 CJK 字符
        let n = bpe_count(zh, "gpt-4");
        // chars/4 = ceil(9/4) = 3；真 BPE 应 >= 9（每字至少 1 token）
        assert!(
            n >= 9,
            "real BPE must not collapse to chars/4; 9 CJK chars should yield >=9 tokens, got {n}"
        );
        // 英文 ASCII 短词：BPE 分词数应远低于纯中文（验证 BPE 真分词，非简单 chars/4）
        let en = "abcdefghi"; // 同样 9 字符但 ASCII
        let en_n = bpe_count(en, "gpt-4");
        assert!(
            en_n < n,
            "ASCII 9 chars ({en_n} tokens) should tokenize fewer than 9 CJK chars ({n} tokens)"
        );
    }

    // ── 同协议透传判定：仅端点协议精确等于入站协议才透传 ──
    // 精确匹配 → 透传；openai_responses→openai 跨协议回退 → 不透传（必须真转换）。
