use super::*;

    #[test]
    fn opencode_zen_fallback_user_key_wins() {
        assert_eq!(opencode_zen_fallback("$realkey", true), "$realkey");
        assert_eq!(opencode_zen_fallback("$realkey", false), "$realkey");
    }

    #[test]
    fn opencode_zen_fallback_empty_zen_to_literal() {
        assert_eq!(opencode_zen_fallback("", true), "$opencode");
        assert_eq!(opencode_zen_fallback("   ", true), "$opencode");
    }

    #[test]
    fn opencode_zen_fallback_non_zen_passthrough_empty() {
        assert_eq!(opencode_zen_fallback("", false), "");
    }

    // ── 透传分支不调 convert_request（结构性确认）──
    // ClaudeCode 命中拦截分支后直接 return handle_passthrough，
    // handle_passthrough 不引用 convert_request / build_upstream_headers / apply_client_headers。
    #[test]
    fn same_protocol_passthrough_condition() {
        // 入站 anthropic + 平台显式 anthropic 端点 → 透传
        let source = "anthropic";
        let matched: Option<super::Protocol> = Some(super::Protocol::Anthropic);
        let pass = matched
            .as_ref()
            .map(|p| format!("{:?}", p).to_lowercase() == source)
            .unwrap_or(false);
        assert!(pass, "exact-protocol endpoint must passthrough");

        // 入站 openai_responses 回退到 openai 端点 → 跨协议，不透传
        let source = "openai_responses";
        let matched: Option<super::Protocol> = Some(super::Protocol::OpenAI);
        let pass = matched
            .as_ref()
            .map(|p| format!("{:?}", p).to_lowercase() == source)
            .unwrap_or(false);
        assert!(!pass, "openai_responses→openai fallback must NOT passthrough (needs conversion)");

        // 无匹配端点 → 不透传（走 convert_request 转 platform_type）
        let matched: Option<super::Protocol> = None;
        let pass = matched
            .as_ref()
            .map(|p| format!("{:?}", p).to_lowercase() == "openai")
            .unwrap_or(false);
        assert!(!pass, "no matched endpoint must NOT passthrough");
    }

    // ── coding-plan 平台端点选择：anthropic 入站不得落到非 coding endpoint(coding key→401) ──
    #[test]
    fn select_endpoint_coding_plan_exclusivity() {
        use super::select_endpoint_for_protocol as sel;
        use super::super::models::{PlatformEndpoint, Protocol};
        let ep = |proto: Protocol, url: &str, cp: bool| PlatformEndpoint {
            protocol: proto,
            base_url: url.to_string(),
            client_type: "claude_code".to_string(),
            coding_plan: cp,
        };

        // ── Kimi coding plan：唯一 openai coding endpoint，anthropic 入站须选 coding(转换) ──
        let kimi_cp = vec![ep(Protocol::OpenAI, "https://api.kimi.com/coding/v1", true)];
        let m = sel(&kimi_cp, "anthropic").expect("anthropic inbound must resolve to coding endpoint");
        assert_eq!(m.base_url, "https://api.kimi.com/coding/v1");
        assert!(m.coding_plan, "selected endpoint must be the coding endpoint");
        // openai 入站同样落 coding endpoint
        let m = sel(&kimi_cp, "openai").unwrap();
        assert!(m.coding_plan);

        // ── Kimi 跨 host 防 401（核心约束）：openai coding host=api.kimi.com，
        //    anthropic endpoint host=api.moonshot.cn（cp=false，需另一把常规 key）。
        //    两 host 不同 → 加固后**不采纳**该 anthropic 端点，anthropic 入站回退 openai coding 转换。
        //    coding key 绝不打到 moonshot.cn（否则 401 连累整个平台 auto_disabled）。 ──
        let kimi_cp_legacy = vec![
            ep(Protocol::OpenAI, "https://api.kimi.com/coding/v1", true),
            ep(Protocol::Anthropic, "https://api.moonshot.cn/anthropic", false),
        ];
        let m = sel(&kimi_cp_legacy, "anthropic").unwrap();
        assert_eq!(
            m.base_url, "https://api.kimi.com/coding/v1",
            "anthropic inbound on coding platform must NOT pick the cross-host non-coding anthropic endpoint"
        );
        assert!(m.coding_plan);

        // ── 非 coding-plan 平台(GLM 常规双端点)：anthropic 入站正常选 anthropic endpoint(行为不变) ──
        let glm = vec![
            ep(Protocol::OpenAI, "https://open.bigmodel.cn/api/paas/v4", false),
            ep(Protocol::Anthropic, "https://open.bigmodel.cn/api/anthropic", false),
        ];
        let m = sel(&glm, "anthropic").unwrap();
        assert_eq!(m.base_url, "https://open.bigmodel.cn/api/anthropic");
        assert!(!m.coding_plan);
        // openai 入站选 openai endpoint
        let m = sel(&glm, "openai").unwrap();
        assert_eq!(m.base_url, "https://open.bigmodel.cn/api/paas/v4");

        // ── GLM Coding Plan(openai coding cp=true + anthropic cp=true，同 host)：「同协议优先于转换」──
        // anthropic(Claude Code)入站 → 选 anthropic coding 端点原协议直发，不得回退 openai coding 转换。
        let glm_cp = vec![
            ep(Protocol::OpenAI, "https://open.bigmodel.cn/api/coding/paas/v4", true),
            ep(Protocol::Anthropic, "https://open.bigmodel.cn/api/anthropic", true),
        ];
        let m = sel(&glm_cp, "anthropic")
            .expect("anthropic inbound must resolve to the anthropic coding endpoint");
        assert_eq!(
            m.base_url, "https://open.bigmodel.cn/api/anthropic",
            "GLM coding plan: anthropic inbound must use anthropic coding endpoint (no openai conversion)"
        );
        assert!(m.coding_plan, "selected endpoint must be the coding endpoint");
        // openai 入站仍选 openai coding 端点
        let m = sel(&glm_cp, "openai").unwrap();
        assert_eq!(m.base_url, "https://open.bigmodel.cn/api/coding/paas/v4");
        assert!(m.coding_plan);

        // ── GLM 形态（真实 DB 数据）：openai coding cp=true + anthropic cp=FALSE，同 host ──
        // 加固后：anthropic 入站凭「与 openai coding 端点同 host(open.bigmodel.cn)、同一把 key 通用」
        // 采纳该 cp=false anthropic 端点原协议直发，**无需 migration 把它标 cp=true**。
        let glm_cp_real = vec![
            ep(Protocol::OpenAI, "https://open.bigmodel.cn/api/coding/paas/v4", true),
            ep(Protocol::Anthropic, "https://open.bigmodel.cn/api/anthropic", false),
        ];
        let m = sel(&glm_cp_real, "anthropic")
            .expect("anthropic inbound must resolve to the same-host anthropic endpoint");
        assert_eq!(
            m.base_url, "https://open.bigmodel.cn/api/anthropic",
            "GLM (anthropic ep cp=false, same host): anthropic inbound must use anthropic endpoint, no conversion"
        );
        // openai 入站仍走 openai coding 端点
        let m = sel(&glm_cp_real, "openai").unwrap();
        assert_eq!(m.base_url, "https://open.bigmodel.cn/api/coding/paas/v4");
        assert!(m.coding_plan);

        // ── 非 coding-plan：openai_responses 无 Responses endpoint → 回退 openai(行为不变) ──
        let openai_only = vec![ep(Protocol::OpenAI, "https://api.deepseek.com/v1", false)];
        let m = sel(&openai_only, "openai_responses").unwrap();
        assert_eq!(m.base_url, "https://api.deepseek.com/v1");
        // 无任何匹配且非 openai_responses → None
        assert!(sel(&openai_only, "gemini").is_none());
    }

    // ── endpoint_host：scheme/端口/路径/userinfo/大小写 边界 ──
    #[test]
    fn endpoint_host_extraction() {
        use super::endpoint_host as host;
        assert_eq!(host("https://open.bigmodel.cn/api/anthropic").as_deref(), Some("open.bigmodel.cn"));
        assert_eq!(host("https://open.bigmodel.cn/api/coding/paas/v4").as_deref(), Some("open.bigmodel.cn"));
        // 端口被剥离
        assert_eq!(host("http://localhost:8080/v1").as_deref(), Some("localhost"));
        // 大小写归一
        assert_eq!(host("https://API.Kimi.COM/coding/v1").as_deref(), Some("api.kimi.com"));
        // userinfo 被剥离
        assert_eq!(host("https://user:pass@api.moonshot.cn/anthropic").as_deref(), Some("api.moonshot.cn"));
        // 无 scheme 也能取 host
        assert_eq!(host("api.moonshot.cn/anthropic").as_deref(), Some("api.moonshot.cn"));
        // 跨 host 判定：GLM 同 host，Kimi 异 host
        assert_eq!(
            host("https://open.bigmodel.cn/api/coding/paas/v4"),
            host("https://open.bigmodel.cn/api/anthropic")
        );
        assert_ne!(
            host("https://api.kimi.com/coding/v1"),
            host("https://api.moonshot.cn/anthropic")
        );
        // 空 / 不可解析 → None（保守，不视为同 host）
        assert_eq!(host(""), None);
        assert_eq!(host("https://"), None);
    }

    // ── UA → 透传协议推断：claude-cli→anthropic / codex→openai_responses / 其它→None ──
    #[test]
    fn infer_passthrough_protocol_from_ua_mapping() {
        use super::infer_passthrough_protocol_from_ua as infer;
        // Claude Code 家族（全部含 claude-cli 前缀）
        assert_eq!(infer("claude-cli/1.0.117 (external, cli)"), Some("anthropic"));
        assert_eq!(infer("claude-cli/1.0.117 (external, claude-vscode, agent-sdk/0.1.30)"), Some("anthropic"));
        // Codex 家族（codex_cli_rs / Codex/ / codex desktop / codex-vscode；大小写不敏感）
        assert_eq!(infer("codex_cli_rs/0.38.0 (MacOS; arm64) Terminal"), Some("openai_responses"));
        assert_eq!(infer("Codex/0.38.0"), Some("openai_responses"));
        assert_eq!(infer("codex desktop/0.38.0"), Some("openai_responses"));
        assert_eq!(infer("codex-vscode/0.38.0"), Some("openai_responses"));
        // 不识别（Cursor / Windsurf / gemini-cli / 未知 / 空）→ None
        assert_eq!(infer("Cursor/0.50.7"), None);
        assert_eq!(infer("Windsurf/1.5.0"), None);
        assert_eq!(infer("gemini-cli/0.1.0"), None);
        assert_eq!(infer("curl/8.0"), None);
        assert_eq!(infer(""), None);
    }

    // ── UA 透传三级回退分支判定（镜像插入点逻辑：matched_ep==None 时按 UA 推断）──
    // 级别 1：UA 命中 + 平台有该协议 endpoint → 透传 wire = UA 协议。
    // 级别 2：UA 命中 + 平台无该协议 endpoint → 回退（不透传）。
    // 级别 3：UA 不识别 → 回退（不透传）。
    #[test]
    fn ua_passthrough_three_level_fallback() {
        use super::infer_passthrough_protocol_from_ua as infer;
        // 模拟平台端点协议集合（小写名）
        let try_passthrough = |ua: &str, platform_protos: &[&str], source_matched: bool| -> Option<&'static str> {
            // path 已被支持（source_matched=true）→ 不介入
            if source_matched {
                return None;
            }
            // matched_ep == None → 尝试 UA 推断
            let p = infer(ua)?;
            // 平台需确有该协议 endpoint
            if platform_protos.contains(&p) {
                Some(p)
            } else {
                None
            }
        };
        // 级别 1：codex UA + 平台有 openai_responses → 透传该协议
        assert_eq!(
            try_passthrough("codex_cli_rs/0.38.0", &["openai_responses", "anthropic"], false),
            Some("openai_responses")
        );
        // 级别 1：claude-cli + 平台有 anthropic → 透传 anthropic
        assert_eq!(
            try_passthrough("claude-cli/1.0.117 (external, cli)", &["anthropic"], false),
            Some("anthropic")
        );
        // 级别 2：codex UA 但平台只有 openai（无 openai_responses）→ 回退
        assert_eq!(
            try_passthrough("codex_cli_rs/0.38.0", &["openai", "anthropic"], false),
            None
        );
        // 级别 3：UA 不识别 → 回退
        assert_eq!(
            try_passthrough("Cursor/0.50.7", &["anthropic", "openai_responses"], false),
            None
        );
        // 级别 0：path 已被平台支持（source_matched）→ 不介入，UA 不参与
        assert_eq!(
            try_passthrough("codex_cli_rs/0.38.0", &["openai_responses"], true),
            None
        );
    }

    // ── 透传 model remap：仅 patch model 字段，messages/tools 结构原样保留 ──
