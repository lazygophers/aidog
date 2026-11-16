//! 票 09：出站整形纯函数的测试，随函数机械迁出 forward.rs（测试体零改动）。

use super::*;
use super::outbound_shape::*;

#[cfg(test)]
mod test_openai_max_completion_tokens {
    use super::{
        Protocol, fold_openai_max_completion_tokens,
        rename_openai_max_tokens_key, strip_default_image_detail,
    };
    use serde_json::json;

    const OFFICIAL: &str = "https://api.openai.com/v1";
    const THIRD_PARTY: &str = "https://open.bigmodel.cn/api/paas/v4";

    /// 透传 body 上两键同时存在 → 取 `max_completion_tokens`（与入站 `from_openai` 同规则）。
    #[test]
    fn both_keys_present_prefers_max_completion_tokens() {
        let mut b = json!({"model": "m", "max_tokens": 100, "max_completion_tokens": 9000});
        fold_openai_max_completion_tokens(&mut b, &Protocol::OpenAI);
        assert_eq!(b["max_tokens"], json!(9000));
        assert!(
            b.get("max_completion_tokens").is_none(),
            "折叠后旧键不残留: {b}"
        );
    }

    /// 回归防线：只发 `max_tokens` 时 body 逐字节不变。
    #[test]
    fn max_tokens_only_body_unchanged() {
        let original = json!({"model": "m", "max_tokens": 777});
        let mut b = original.clone();
        fold_openai_max_completion_tokens(&mut b, &Protocol::OpenAI);
        assert_eq!(b, original, "无新键时 body 逐字节不变");
    }


    /// 出站键名按目标分叉：官方 OpenAI host 改成 `max_completion_tokens`，第三方保持 `max_tokens`。
    #[test]
    fn official_host_renames_third_party_keeps_max_tokens() {
        let mut b = json!({"model": "m", "max_tokens": 8192, "temperature": 0.5});
        assert!(rename_openai_max_tokens_key(
            &mut b,
            &Protocol::OpenAI,
            OFFICIAL
        ));
        assert_eq!(b["max_completion_tokens"], json!(8192));
        assert!(b.get("max_tokens").is_none(), "官方 host 不应残留旧键: {b}");
        assert_eq!(b["temperature"], json!(0.5), "同级其它字段不动");

        let original = json!({"model": "m", "max_tokens": 8192});
        let mut b = original.clone();
        assert!(!rename_openai_max_tokens_key(
            &mut b,
            &Protocol::OpenAI,
            THIRD_PARTY
        ));
        assert_eq!(b, original, "第三方端点 body 逐字节不变");
    }

    /// 其它 wire 协议与「没有 max_tokens」时不动（anthropic 只认 max_tokens）。
    #[test]
    fn other_wires_and_absent_key_are_noop() {
        for wire in [
            Protocol::Anthropic,
            Protocol::OpenAICompletions,
            Protocol::Gemini,
        ] {
            let original = json!({"model": "m", "max_tokens": 8192});
            let mut b = original.clone();
            assert!(!rename_openai_max_tokens_key(&mut b, &wire, OFFICIAL));
            assert_eq!(b, original, "{wire:?}: 非 openai wire 不应改键");
            fold_openai_max_completion_tokens(&mut b, &wire);
            assert_eq!(b, original, "{wire:?}: 非 openai wire 不应折叠");
        }
        let mut b = json!({"model": "m"});
        assert!(!rename_openai_max_tokens_key(
            &mut b,
            &Protocol::OpenAI,
            OFFICIAL
        ));
        assert_eq!(b, json!({"model": "m"}), "未传 max_tokens 时不产键");
    }

    /// `detail:"auto"` 是 SDK 填的默认值 → 剔除；显式 low/high 与非 openai wire 不动。
    #[test]
    fn strips_only_default_image_detail() {
        let mk = || {
            json!({"model": "m", "messages": [{"role": "user", "content": [
                {"type": "text", "text": "hi"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,AA", "detail": "auto"}},
                {"type": "image_url", "image_url": {"url": "u2", "detail": "high"}}
            ]}]})
        };
        let mut b = mk();
        assert_eq!(strip_default_image_detail(&mut b, &Protocol::OpenAI), 1);
        let blocks = &b["messages"][0]["content"];
        assert!(
            blocks[1]["image_url"].get("detail").is_none(),
            "auto 应被剔除: {b}"
        );
        assert_eq!(
            blocks[1]["image_url"]["url"],
            json!("data:image/png;base64,AA")
        );
        assert_eq!(
            blocks[2]["image_url"]["detail"],
            json!("high"),
            "显式值不动"
        );

        let mut b = mk();
        assert_eq!(strip_default_image_detail(&mut b, &Protocol::Anthropic), 0);
        assert_eq!(b, mk(), "非 openai wire body 逐字节不变");
    }
}

#[cfg(test)]
mod test_disable_thinking {
    use aidog_adapter::platform::hooks::{apply_disable_thinking, is_official_openai_host};
    use super::Protocol;
    use serde_json::json;

    /// anthropic：客户端开启型参数剔干净后，写入显式 `thinking.type=disabled`
    /// （request d1c87c9c 的 minimax anthropic 端点场景）。
    #[test]
    fn anthropic_emits_explicit_disabled() {
        let mut b = json!({
            "model": "m", "max_tokens": 300, "disable_thinking": true,
            "thinking": {"type": "enabled", "budget_tokens": 1024},
            "context_management": {"edits": []},
        });
        apply_disable_thinking(
            &mut b,
            true,
            &Protocol::Anthropic,
            "https://api.minimax.io/anthropic",
            false,
        );
        assert!(b.get("disable_thinking").is_none());
        assert!(b.get("context_management").is_none());
        assert_eq!(b["thinking"], json!({"type": "disabled"}));
    }

    /// 强制思考模型（registry `thinking_toggleable: false`，MiniMax-M3.1-Flash 实证 400
    /// code 2013 "requires adaptive thinking"）：只剥禁用参数，出站不写任何禁用指令。
    #[test]
    fn forced_thinking_strips_disable_and_writes_nothing() {
        let mut b = json!({
            "model": "m", "max_tokens": 300, "disable_thinking": true,
            "thinking": {"type": "disabled"},
        });
        apply_disable_thinking(
            &mut b,
            true,
            &Protocol::Anthropic,
            "https://api.minimaxi.com/anthropic",
            true,
        );
        assert!(b.get("disable_thinking").is_none());
        assert!(b.get("thinking").is_none());
    }

    /// 客户端自带显式禁用（未设 aidog 扩展字段）在强制思考模型下同样剥掉——
    /// 上游对这些形态一视同仁返 2013。
    #[test]
    fn forced_thinking_strips_client_disabled_even_when_disable_false() {
        let mut b = json!({
            "model": "m",
            "thinking": {"type": "disabled"},
            "reasoning_effort": "none",
        });
        apply_disable_thinking(
            &mut b,
            false,
            &Protocol::Anthropic,
            "https://api.minimaxi.com/anthropic",
            true,
        );
        assert!(b.get("thinking").is_none());
        assert!(b.get("reasoning_effort").is_none());
    }

    /// 开启型 thinking 不受强制分支影响（只剥禁用形态，不改语义）。
    #[test]
    fn forced_thinking_keeps_enabled_params() {
        let mut b = json!({
            "model": "m",
            "thinking": {"type": "enabled", "budget_tokens": 1024},
        });
        apply_disable_thinking(
            &mut b,
            false,
            &Protocol::Anthropic,
            "https://api.minimaxi.com/anthropic",
            true,
        );
        assert_eq!(b["thinking"]["type"], json!("enabled"));
    }

    /// 第三方 OpenAI 兼容端点：`chat_template_kwargs.enable_thinking=false`，开启型参数不残留。
    #[test]
    fn third_party_openai_emits_chat_template_kwargs() {
        let mut b = json!({
            "model": "m", "disable_thinking": true, "reasoning_effort": "medium",
            "chat_template_kwargs": {"enable_thinking": true, "other": 1},
        });
        apply_disable_thinking(
            &mut b,
            true,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
            false,
        );
        assert!(b.get("reasoning_effort").is_none());
        assert_eq!(b["chat_template_kwargs"]["enable_thinking"], json!(false));
        assert_eq!(
            b["chat_template_kwargs"]["other"],
            json!(1),
            "同级其它字段不动"
        );
    }

    /// 官方 OpenAI：拒未知顶层字段，只能发 `reasoning_effort="none"`。
    #[test]
    fn official_openai_emits_reasoning_effort() {
        let mut b = json!({"model": "gpt-5", "disable_thinking": true});
        apply_disable_thinking(
            &mut b,
            true,
            &Protocol::OpenAI,
            "https://api.openai.com/v1",
            false,
        );
        assert_eq!(b["reasoning_effort"], json!("none"));
        assert!(b.get("chat_template_kwargs").is_none());
    }

    /// openai_responses：`reasoning.effort="none"`。
    #[test]
    fn openai_responses_emits_reasoning_effort_object() {
        let mut b =
            json!({"model": "m", "disable_thinking": true, "reasoning": {"effort": "high"}});
        apply_disable_thinking(
            &mut b,
            true,
            &Protocol::OpenAIResponses,
            "https://api.openai.com/v1",
            false,
        );
        assert_eq!(b["reasoning"], json!({"effort": "none"}));
    }

    /// gemini：`generationConfig.thinkingConfig.thinkingBudget=0`，generationConfig 缺省时补建。
    #[test]
    fn gemini_emits_zero_thinking_budget() {
        let mut b = json!({
            "disable_thinking": true,
            "generationConfig": {"thinkingConfig": {"thinkingBudget": 128}, "temperature": 0.5},
        });
        apply_disable_thinking(
            &mut b,
            true,
            &Protocol::Gemini,
            "https://generativelanguage.googleapis.com",
            false,
        );
        assert_eq!(
            b["generationConfig"]["thinkingConfig"],
            json!({"thinkingBudget": 0})
        );
        assert_eq!(b["generationConfig"]["temperature"], json!(0.5));

        let mut b = json!({"disable_thinking": true});
        apply_disable_thinking(
            &mut b,
            true,
            &Protocol::Gemini,
            "https://generativelanguage.googleapis.com",
            false,
        );
        assert_eq!(
            b["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            json!(0)
        );
    }

    #[test]
    fn false_strips_field_only_and_absent_is_noop() {
        let mut b = json!({"disable_thinking": false, "thinking": {"type": "enabled"}});
        apply_disable_thinking(&mut b, false, &Protocol::Anthropic, "https://example.com", false);
        assert!(b.get("disable_thinking").is_none(), "非标字段仍剥");
        assert_eq!(
            b["thinking"],
            json!({"type": "enabled"}),
            "false 不动思考参数"
        );

        let mut b = json!({"model": "m"});
        apply_disable_thinking(&mut b, false, &Protocol::Anthropic, "https://example.com", false);
        assert_eq!(b, json!({"model": "m"}), "未请求禁用时整体不动");
    }

    /// 转换分支：`req_body` 由强类型 struct 序列化，body 里根本没有 `disable_thinking` key，
    /// 禁用意图只能靠调用方从客户端原体读出的 flag 传进来（审计发现的漏修路径）。
    #[test]
    fn converted_body_without_key_still_disables() {
        let mut b = json!({"model": "m", "messages": [], "reasoning_effort": "high"});
        apply_disable_thinking(
            &mut b,
            true,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
            false,
        );
        assert_eq!(b["chat_template_kwargs"]["enable_thinking"], json!(false));
        assert!(b.get("reasoning_effort").is_none());
    }

    #[test]
    fn official_openai_host_matches_host_only() {
        assert!(is_official_openai_host(
            "https://api.openai.com/v1/chat/completions"
        ));
        assert!(is_official_openai_host("https://API.OpenAI.com:443/v1"));
        assert!(!is_official_openai_host(
            "https://api.openai.com.evil.dev/v1"
        ));
        assert!(!is_official_openai_host(
            "https://open.bigmodel.cn/api/paas/v4"
        ));
    }
}

#[cfg(test)]
mod test_strip_thinking {
    use super::{strip_redacted_thinking_blocks, strip_thinking_if_unmatched};
    use serde_json::json;

    #[test]
    fn strips_when_assistant_turn_lacks_thinking_block() {
        // 复现 0cea9d32 真因：thinking 开启 + assistant 轮仅 tool_use 无 thinking → 第三方 400
        let mut body = json!({
            "thinking": {"type": "adaptive", "display": "summarized"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "hi"}]},
                {"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "x", "input": {}}]},
            ],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_none(), "应剔除 thinking");
        assert!(
            body.get("context_management").is_none(),
            "应剔除 context_management"
        );
    }

    #[test]
    fn strips_context_management_with_adaptive_thinking_no_assistant_block() {
        // 复现 request_id=1658bb4b 真因：Claude Code adaptive/summarized 模式
        // (thinking adaptive + context_management clear_thinking_20251015) → assistant 轮不回传 thinking block
        // → 跨路由到第三方 anthropic 端点(DeepSeek)。单剔 thinking 不够，context_management 保留仍判 thinking mode → 400。
        // 修复：两字段皆剔。
        let mut body = json!({
            "thinking": {"type": "adaptive", "display": "summarized"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q1"}]},
                {"role": "assistant", "content": [{"type": "text", "text": "a1"}]},
                {"role": "user", "content": [{"type": "text", "text": "q2"}]},
                {"role": "assistant", "content": [{"type": "text", "text": "a2"}]},
            ],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_none(), "应剔除 thinking");
        assert!(
            body.get("context_management").is_none(),
            "应剔除 context_management"
        );
    }

    #[test]
    fn keeps_thinking_when_blocks_present() {
        let mut body = json!({
            "thinking": {"type": "adaptive"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "...", "signature": "s"},
                    {"type": "tool_use", "id": "t1", "name": "x", "input": {}},
                ]},
            ],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_some(), "thinking 齐全应保留");
        // context_management 无条件剔（第三方不认该协商字段）
        assert!(
            body.get("context_management").is_none(),
            "thinking 开启即无条件剔 context_management（即使 thinking 齐全）"
        );
    }

    #[test]
    fn noop_when_thinking_off() {
        let mut body = json!({
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [{"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "x", "input": {}}]}],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("messages").is_some());
        assert!(
            body.get("context_management").is_some(),
            "thinking off 不命中 unmatched，context_management 保留"
        );
    }

    #[test]
    fn keeps_thinking_first_turn_no_assistant_but_strips_context_management() {
        // 复现 request_id=3a76c297 真因（GLM 1210）：首轮请求 messages 仅 user，无 assistant 历史，
        // thinking adaptive + context_management clear_thinking_20251015。
        // 旧逻辑 has_unmatched_assistant=false → 两字段皆保留 → GLM 不认 context_management 报 1210。
        // 修复：context_management 无条件剔（thinking_on 即剔，独立于 has_unmatched）；thinking 无 unmatched 故保留。
        let mut body = json!({
            "thinking": {"type": "adaptive", "display": "summarized"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(
            body.get("thinking").is_some(),
            "首轮无 assistant → has_unmatched=false，thinking 保留"
        );
        assert!(
            body.get("context_management").is_none(),
            "thinking 开启即无条件剔 context_management（首轮 GLM 1210 根因）"
        );
    }

    #[test]
    fn test_strip_redacted_thinking_blocks_filters_only_redacted() {
        // 复现 trace 81dc4466（火山 doubao coding endpoint 400 InvalidParameter）
        // + 87e3c500（deepseek-v4-pro-260425 同根因）：redacted_thinking block 透传致第三方端点 400。
        let mut body = json!({
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q"}]},
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "sig-data", "signature": "s"},
                    {"type": "redacted_thinking", "data": "encrypted-opaque-blob"},
                    {"type": "text", "text": "a"},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "t1", "content": "r"},
                    {"type": "redacted_thinking", "data": "encrypted-opaque-blob-2"},
                ]},
            ],
        });
        strip_redacted_thinking_blocks(&mut body);
        let msgs = body.get("messages").and_then(|m| m.as_array()).unwrap();
        // assistant 轮：仅剩 thinking + text
        let asst = msgs[1].get("content").and_then(|c| c.as_array()).unwrap();
        assert_eq!(
            asst.len(),
            2,
            "应仅剥离 redacted_thinking，保留 thinking + text"
        );
        assert!(
            asst.iter()
                .all(|b| b.get("type").and_then(|t| t.as_str()) != Some("redacted_thinking")),
            "无残留 redacted_thinking"
        );
        // user 轮：仅剩 tool_result
        let u2 = msgs[2].get("content").and_then(|c| c.as_array()).unwrap();
        assert_eq!(u2.len(), 1, "tool_result 保留，仅剔 redacted_thinking");
        assert_eq!(
            u2[0].get("type").and_then(|t| t.as_str()),
            Some("tool_result")
        );
    }

    #[test]
    fn test_strip_redacted_thinking_blocks_all_redacted_keeps_empty_message() {
        // 全 redacted_thinking 的 message → 剥离后 content 为空数组，但 message 结构保留
        // （剥离顺序敏感，禁删整条 message）。
        let mut body = json!({
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q"}]},
                {"role": "assistant", "content": [
                    {"type": "redacted_thinking", "data": "blob-1"},
                    {"type": "redacted_thinking", "data": "blob-2"},
                ]},
            ],
        });
        strip_redacted_thinking_blocks(&mut body);
        let msgs = body.get("messages").and_then(|m| m.as_array()).unwrap();
        assert_eq!(msgs.len(), 2, "message 数量不变（结构保留）");
        let asst = msgs[1].get("content").and_then(|c| c.as_array()).unwrap();
        assert!(
            asst.is_empty(),
            "全 redacted_thinking 剥离后 content 为空数组"
        );
        // user 轮 text block 保留
        let u = msgs[0].get("content").and_then(|c| c.as_array()).unwrap();
        assert_eq!(u.len(), 1);
    }

    #[test]
    fn test_strip_redacted_thinking_blocks_noop_on_string_content() {
        // 字符串形态 content 无 block 可剔，不动。
        let mut body = json!({
            "messages": [
                {"role": "user", "content": "plain text"},
                {"role": "assistant", "content": [{"type": "redacted_thinking", "data": "x"}]},
            ],
        });
        strip_redacted_thinking_blocks(&mut body);
        let msgs = body.get("messages").and_then(|m| m.as_array()).unwrap();
        // 字符串 content 原样
        assert_eq!(
            msgs[0].get("content").and_then(|c| c.as_str()),
            Some("plain text")
        );
        // assistant 数组内 redacted_thinking 已剔
        let asst = msgs[1].get("content").and_then(|c| c.as_array()).unwrap();
        assert!(asst.is_empty());
    }
}

#[cfg(test)]
mod test_hoist_mid_messages_system {
    use super::hoist_mid_messages_system;
    use serde_json::json;

    #[test]
    fn hoists_mid_system_to_top_level_when_multiturn() {
        // 复现 GLM 1210 真因（request_id=7c8629eadb074648a71858ae388ea550 等 9 例）：
        // CC 注入 role=system 进 messages 中段+末段，多轮 + 非流式下 GLM 拒绝 → 400 code 1210。
        // 规整：messages 内 role=system 合并到顶层 system 数组，messages 仅留 user/assistant。
        let mut body = json!({
            "system": [{"type": "text", "text": "base system"}],
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q1"}]},
                {"role": "system", "content": "mid reminder 1"},
                {"role": "assistant", "content": [{"type": "text", "text": "a1"}]},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": "mid reminder 2"},
                {"role": "assistant", "content": [{"type": "text", "text": "a2"}]},
                {"role": "user", "content": "q3"},
                {"role": "system", "content": "trailing reminder"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let msgs = body["messages"].as_array().unwrap();
        // messages 内不再有 role=system
        assert!(
            !msgs.iter().any(|m| m["role"] == "system"),
            "messages 内不应再有 role=system"
        );
        // user/assistant 交替保留
        let roles: Vec<&str> = msgs.iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(
            roles,
            vec!["user", "assistant", "user", "assistant", "user"]
        );
        // 顶层 system 数组追加 3 个 text block（原 1 + 合并 3 = 4）
        let sys = body["system"].as_array().unwrap();
        assert_eq!(sys.len(), 4, "顶层 system 数组应含原 1 + 合并 3 = 4 块");
        assert_eq!(sys[0]["text"], "base system");
        assert_eq!(sys[1]["text"], "mid reminder 1");
        assert_eq!(sys[2]["text"], "mid reminder 2");
        assert_eq!(sys[3]["text"], "trailing reminder");
    }

    #[test]
    fn noop_when_no_assistant_first_turn() {
        // 首轮无 assistant 历史：messages 内 role=system 多为客户端首约定，第三方接受，不动。
        let mut body = json!({
            "system": [{"type": "text", "text": "base"}],
            "messages": [
                {"role": "user", "content": "hi"},
                {"role": "system", "content": "ctx"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        // messages 保持原样（含 role=system）
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2, "首轮无 assistant 不应规整");
        assert_eq!(msgs[1]["role"], "system");
        assert_eq!(
            body["system"].as_array().unwrap().len(),
            1,
            "顶层 system 不变"
        );
    }

    #[test]
    fn noop_when_no_mid_system() {
        // 多轮但 messages 内无 role=system：无需规整
        let mut body = json!({
            "system": [{"type": "text", "text": "base"}],
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        assert_eq!(
            body["messages"].as_array().unwrap().len(),
            3,
            "无 mid-system 不动"
        );
        assert_eq!(body["system"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn upgrades_top_system_str_to_array() {
        // 顶层 system 是字符串时：升级为数组并追加 mid-system
        let mut body = json!({
            "system": "base string",
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": "injected"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let sys = body["system"].as_array().expect("顶层 system 应升级为数组");
        assert_eq!(sys.len(), 2);
        assert_eq!(sys[0]["text"], "base string");
        assert_eq!(sys[1]["text"], "injected");
    }

    #[test]
    fn creates_top_system_when_absent() {
        // 顶层无 system 字段：mid-system 合并新建
        let mut body = json!({
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": [{"type": "text", "text": "block1"}]},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let sys = body["system"].as_array().expect("应新建顶层 system 数组");
        assert_eq!(sys.len(), 1);
        assert_eq!(sys[0]["text"], "block1");
    }

    #[test]
    fn preserves_array_block_content_from_mid_system() {
        // mid-system content 是数组（blocks）时：逐项合并到顶层 system 数组
        let mut body = json!({
            "system": [{"type": "text", "text": "base"}],
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": [
                    {"type": "text", "text": "block a"},
                    {"type": "text", "text": "block b"},
                ]},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let sys = body["system"].as_array().unwrap();
        assert_eq!(sys.len(), 3, "原 1 + mid-system 2 block = 3");
        assert_eq!(sys[1]["text"], "block a");
        assert_eq!(sys[2]["text"], "block b");
    }
}

#[cfg(test)]
mod test_dropped_media_fields {
    use super::outbound_shape::{build_field_trace, dropped_media_fields, dropped_server_tools};
    use aidog_adapter::types::{ChatRequest, ContentBlock, Message, MessageContent, Role};
    use super::Protocol;

    fn media_request(media_type: &str, data: Option<&str>) -> ChatRequest {
        ChatRequest {
            model: "m".into(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Blocks(vec![ContentBlock::Media {
                    media_type: media_type.into(),
                    data: data.map(str::to_owned),
                    url: None,
                }]),
            }],
            system: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stream: None,
            tools: None,
            tool_choice: None,
            thinking_budget: None,
            thinking_mode: None,
            extra: None,
        }
    }

    #[test]
    fn anthropic_media_drops_are_recorded() {
        let req = media_request("audio/wav", Some("AAAA"));
        assert_eq!(build_field_trace(false, false, false, &dropped_media_fields(&req, &Protocol::Anthropic)), "drop:audio");
    }

    #[test]
    fn responses_media_drops_are_recorded() {
        let req = media_request("video/mp4", Some("AAAA"));
        assert_eq!(build_field_trace(false, false, false, &dropped_media_fields(&req, &Protocol::OpenAIResponses)), "drop:video");
    }

    #[test]
    fn openai_url_audio_drop_is_recorded() {
        let req = media_request("audio/wav", None);
        assert_eq!(build_field_trace(false, false, false, &dropped_media_fields(&req, &Protocol::OpenAI)), "drop:audio");
    }

    #[test]
    fn openai_video_drop_is_recorded() {
        let req = media_request("video/mp4", Some("AAAA"));
        assert_eq!(build_field_trace(false, false, false, &dropped_media_fields(&req, &Protocol::OpenAI)), "drop:video");
    }

    /// 三类改写按固定序拼接；全无改写时空串（不产生噪音记录）。（master forward.rs 测试移植）
    #[test]
    fn fa11_field_trace_composition_and_empty() {
        assert_eq!(
            build_field_trace(false, true, true, &["seed".to_string(), "n".to_string()]),
            "cap:max_tokens,rewrite:middleware,drop:seed,drop:n"
        );
        assert_eq!(build_field_trace(false, false, false, &[]), "");
    }

    // ── issue #74：服务端工具丢弃留痕（master forward.rs 测试移植）──

    fn tools_request(tools: serde_json::Value) -> ChatRequest {
        let mut req = media_request("text/plain", Some("hi"));
        req.tools = serde_json::from_value(tools).expect("tools parse");
        req
    }

    #[test]
    fn anthropic_wire_has_no_server_tool_drops() {
        let req = tools_request(serde_json::json!([
            {"type": "web_search_20250305", "name": "web_search"},
            {"type": "bash_20250124", "name": "bash"}
        ]));
        assert!(dropped_server_tools(&req, &Protocol::Anthropic).is_empty());
    }

    #[test]
    fn openai_wire_drops_all_server_tools() {
        let req = tools_request(serde_json::json!([
            {"name": "get_weather", "input_schema": {"type": "object"}},
            {"type": "web_search_20250305", "name": "web_search"}
        ]));
        assert_eq!(
            dropped_server_tools(&req, &Protocol::OpenAI),
            vec!["web_search".to_string()]
        );
    }

    #[test]
    fn gemini_wire_drops_only_unmappable() {
        let req = tools_request(serde_json::json!([
            {"type": "web_search_20250305", "name": "web_search"},
            {"type": "web_fetch_20250910", "name": "web_fetch"},
            {"type": "bash_20250124", "name": "bash"}
        ]));
        assert_eq!(
            dropped_server_tools(&req, &Protocol::Gemini),
            vec!["bash".to_string()]
        );
        let req = tools_request(serde_json::json!([
            {"type": "web_search_20250305", "name": "web_search"},
            {"type": "web_fetch_20250910", "name": "web_fetch"}
        ]));
        assert_eq!(
            dropped_server_tools(&req, &Protocol::OpenAIResponses),
            vec!["web_fetch".to_string()],
            "Responses 只映射 web_search，web_fetch 落留痕"
        );
    }

    #[test]
    fn server_tool_drops_compose_into_field_trace() {
        let req = tools_request(serde_json::json!([
            {"name": "get_weather", "input_schema": {"type": "object"}},
            {"type": "bash_20250124", "name": "bash"}
        ]));
        assert_eq!(
            build_field_trace(
                false,
                false,
                false,
                &dropped_server_tools(&req, &Protocol::OpenAI)
            ),
            "drop:bash"
        );
    }
}
