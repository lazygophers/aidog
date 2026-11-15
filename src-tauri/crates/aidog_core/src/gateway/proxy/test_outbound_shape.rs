//! 票 09：出站整形纯函数的测试，随函数机械迁出 forward.rs（测试体零改动）。

use super::*;
use super::outbound_shape::*;
#[cfg(test)]
mod test_cap_body_max_tokens {
    use super::{Protocol, cap_body_max_tokens};
    use serde_json::json;

    /// anthropic 透传 body：超模型上限的 max_tokens 被裁到上限（票 02 的 400 根因）。
    #[test]
    fn anthropic_caps_top_level_max_tokens() {
        let mut b = json!({"model": "m", "max_tokens": 200_000, "messages": []});
        assert_eq!(
            cap_body_max_tokens(&mut b, Some(8192), &Protocol::Anthropic),
            Some((200_000, 8192))
        );
        assert_eq!(b["max_tokens"], json!(8192));
        assert_eq!(b["messages"], json!([]), "同级其它字段不动");
    }

    /// openai / completions 同样是顶层 `max_tokens`。
    #[test]
    fn openai_and_completions_cap_top_level_max_tokens() {
        for wire in [Protocol::OpenAI, Protocol::OpenAICompletions] {
            let mut b = json!({"model": "m", "max_tokens": 9999});
            assert_eq!(
                cap_body_max_tokens(&mut b, Some(4096), &wire),
                Some((9999, 4096))
            );
            assert_eq!(b["max_tokens"], json!(4096), "{wire:?}");
        }
    }

    /// openai_responses 的键名是 `max_output_tokens`。
    #[test]
    fn responses_caps_max_output_tokens() {
        let mut b = json!({"model": "m", "max_output_tokens": 100_000, "max_tokens": 100_000});
        assert_eq!(
            cap_body_max_tokens(&mut b, Some(16384), &Protocol::OpenAIResponses),
            Some((100_000, 16384))
        );
        assert_eq!(b["max_output_tokens"], json!(16384));
        assert_eq!(
            b["max_tokens"],
            json!(100_000),
            "responses 不碰顶层 max_tokens"
        );
    }

    /// gemini 的键名嵌在 `generationConfig.maxOutputTokens`。
    #[test]
    fn gemini_caps_generation_config_max_output_tokens() {
        let mut b = json!({"generationConfig": {"maxOutputTokens": 65536, "temperature": 0.5}});
        assert_eq!(
            cap_body_max_tokens(&mut b, Some(8192), &Protocol::Gemini),
            Some((65536, 8192))
        );
        assert_eq!(b["generationConfig"]["maxOutputTokens"], json!(8192));
        assert_eq!(
            b["generationConfig"]["temperature"],
            json!(0.5),
            "同级其它字段不动"
        );
    }

    /// 未超限 / 模型无上限 / 未传 max_tokens：body 整体不动（保守语义，与 chat_req 侧一致）。
    #[test]
    fn under_limit_or_no_limit_or_absent_is_noop() {
        let cases = [
            (json!({"model": "m", "max_tokens": 100}), Some(8192)),
            (json!({"model": "m", "max_tokens": 100}), None),
            (json!({"model": "m"}), Some(8192)),
            (json!({"model": "m", "max_tokens": null}), Some(8192)),
        ];
        for (original, model_max) in cases {
            let mut b = original.clone();
            assert_eq!(
                cap_body_max_tokens(&mut b, model_max, &Protocol::Anthropic),
                None
            );
            assert_eq!(b, original, "未超限/无上限/未传时 body 必须逐字节不变");
        }
    }

    /// 非 wire 协议（平台类型枚举）不改写。
    #[test]
    fn non_wire_protocol_is_noop() {
        let mut b = json!({"max_tokens": 999_999});
        assert_eq!(cap_body_max_tokens(&mut b, Some(8), &Protocol::Mock), None);
        assert_eq!(b["max_tokens"], json!(999_999));
    }
}

/// 票 10：`apply_field_passthrough` 的留痕返回值——只记被挡掉的字段名，绝不记值。
#[cfg(test)]
mod test_field_trace {
    use super::{Protocol, apply_field_passthrough};
    use serde_json::json;

    /// 被允许集合挡掉的字段：名进留痕，值不进。
    #[test]
    fn blocked_field_name_recorded_without_its_value() {
        let src = json!({
            "model": "m",
            "messages": [],
            "top_k": 40,
            "user": "tenant-42-secret-id",
            "response_format": {"type": "json_object"}
        });
        let mut b = json!({"model": "m", "messages": [], "max_tokens": 100});
        // anthropic 允许集合只有 top_k → user / response_format 被挡。
        let dropped = apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::Anthropic,
            "https://api.anthropic.com/v1/messages",
        );
        assert_eq!(
            dropped,
            vec!["user".to_string(), "response_format".to_string()]
        );
        let trace = dropped.join(",");
        assert!(
            !trace.contains("tenant-42-secret-id"),
            "留痕不得含字段值：{trace}"
        );
        assert!(!trace.contains("json_object"), "留痕不得含字段值：{trace}");
        assert!(!trace.contains("top_k"), "已出站的字段不算被挡：{trace}");
    }

    /// 官方 OpenAI host 上 `top_k` 被 host gate 挡掉 → 记名。
    #[test]
    fn official_openai_host_records_top_k_drop() {
        let src = json!({"model": "m", "messages": [], "top_k": 40, "seed": 7});
        let mut b = json!({"model": "m", "messages": []});
        let dropped = apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://api.openai.com/v1/chat/completions",
        );
        assert_eq!(dropped, vec!["top_k".to_string()]);
        assert_eq!(b["seed"], json!(7), "允许集合内的字段照常出站");
    }

    /// 无丢弃时不产生噪音：全部字段都出站（或本就不在候选集合内）→ 留痕为空。
    #[test]
    fn no_drop_produces_no_trace() {
        let src = json!({"model": "m", "messages": [], "max_tokens": 100, "temperature": 0.7, "top_k": 40});
        let mut b = json!({"model": "m", "messages": [], "max_tokens": 100});
        let dropped = apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::Anthropic,
            "https://open.bigmodel.cn/api/anthropic",
        );
        assert!(dropped.is_empty(), "无字段被挡时不应产生留痕：{dropped:?}");
    }

    /// 换名承载不算被挡：客户端 `stop` 以 `stop_sequences` 出站 → 留痕为空。
    #[test]
    fn renamed_stop_is_not_reported_as_dropped() {
        let src = json!({"model": "m", "messages": [], "stop": ["\n\n"]});
        let mut b = json!({"model": "m", "messages": []});
        let dropped = apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::Anthropic,
            "https://api.anthropic.com/v1/messages",
        );
        assert_eq!(b["stop_sequences"], json!(["\n\n"]));
        assert!(dropped.is_empty(), "换名出站的字段不算被挡：{dropped:?}");
    }

    /// gemini 顶层无本组字段 → 客户端发的采样参数全被挡，逐个记名。
    #[test]
    fn gemini_reports_all_candidates_as_dropped() {
        let src = json!({"model": "m", "contents": [], "stop": ["x"], "seed": 1, "n": 2});
        let mut b = json!({"model": "m", "contents": []});
        let dropped = apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::Gemini,
            "https://generativelanguage.googleapis.com/v1beta",
        );
        assert_eq!(
            dropped,
            vec!["stop".to_string(), "seed".to_string(), "n".to_string()]
        );
    }
}

/// 票 11 对账补的两处：Gemini 源的生成参数下钻取值、`cap:max_tokens` 两来源合一。
#[cfg(test)]
mod test_fa11_reconciliation {
    use super::{Protocol, apply_field_passthrough, build_field_trace};
    use serde_json::json;

    /// Gemini 客户端 → OpenAI 上游：stop / topK / responseMimeType 埋在 `generationConfig` 里，
    /// 按顶层键取值取不到。下钻一跳后三项都要到达出站 body。
    #[test]
    fn fa11_gemini_source_generation_config_reaches_openai_target() {
        let src = json!({
            "contents": [],
            "generationConfig": {
                "stopSequences": ["END"],
                "topK": 40,
                "responseMimeType": "application/json",
                "temperature": 0.3
            }
        });
        let mut b = json!({"model": "m", "messages": []});
        let dropped = apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
        );
        assert_eq!(b["stop"], json!(["END"]));
        assert_eq!(b["top_k"], json!(40));
        assert_eq!(b["response_format"], json!({"type": "json_object"}));
        assert!(dropped.is_empty(), "全部承载，不应留痕：{dropped:?}");
    }

    /// 带 responseSchema 时换算成 OpenAI 的 json_schema 形态（`name` 必填，上游缺它判 400）。
    #[test]
    fn fa11_gemini_response_schema_maps_to_openai_json_schema() {
        let src = json!({"contents": [], "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": {"type": "object", "properties": {"a": {"type": "string"}}}
        }});
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
        );
        assert_eq!(b["response_format"]["type"], json!("json_schema"));
        assert_eq!(
            b["response_format"]["json_schema"]["name"],
            json!("response")
        );
        assert_eq!(
            b["response_format"]["json_schema"]["schema"]["type"],
            json!("object")
        );
    }

    /// Gemini 源 → Anthropic 上游：stopSequences 直接同名承载，topK 换名 top_k。
    #[test]
    fn fa11_gemini_source_reaches_anthropic_target() {
        let src = json!({"contents": [], "generationConfig": {"stopSequences": ["X"], "topK": 5}});
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::Anthropic,
            "https://api.anthropic.com/v1/messages",
        );
        assert_eq!(b["stop_sequences"], json!(["X"]));
        assert_eq!(b["top_k"], json!(5));
    }

    /// `responseMimeType: text/plain` 是 Gemini 默认态，OpenAI 无对应显式写法 → 不写出。
    #[test]
    fn fa11_gemini_text_plain_mime_writes_no_response_format() {
        let src = json!({"contents": [], "generationConfig": {"responseMimeType": "text/plain"}});
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
        );
        assert!(
            b.get("response_format").is_none(),
            "默认 mime 不该产出 response_format"
        );
    }

    /// 顶层写法优先于 generationConfig（客户端两处都写时以显式顶层为准）。
    #[test]
    fn fa11_top_level_wins_over_generation_config() {
        let src = json!({"stop": ["TOP"], "top_k": 1, "generationConfig": {"stopSequences": ["NESTED"], "topK": 99}});
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
        );
        assert_eq!(b["stop"], json!(["TOP"]));
        assert_eq!(b["top_k"], json!(1));
    }

    /// 转换分支：裁剪发生在 `chat_req` 上，body 层是幂等复裁不再命中 —— 留痕仍须记
    /// `cap:max_tokens`（此前只看 body 层，整条转换路径的裁剪不留痕）。
    #[test]
    fn fa11_cap_trace_covers_both_branches_without_duplicating() {
        // (转换分支, 透传分支, 两者同时) 三种组合都记且只记一个 token
        for (on_chat_req, on_body) in [(true, false), (false, true), (true, true)] {
            let trace = build_field_trace(on_chat_req, on_body, false, &[]);
            assert_eq!(
                trace, "cap:max_tokens",
                "chat_req={on_chat_req} body={on_body}"
            );
            assert_eq!(trace.matches("cap:max_tokens").count(), 1);
        }
        assert_eq!(
            build_field_trace(false, false, false, &[]),
            "",
            "未裁剪不记"
        );
    }

    /// 三类改写按固定序拼接；全无改写时空串（不产生噪音记录）。
    #[test]
    fn fa11_field_trace_composition_and_empty() {
        assert_eq!(
            build_field_trace(false, true, true, &["seed".to_string(), "n".to_string()]),
            "cap:max_tokens,rewrite:middleware,drop:seed,drop:n"
        );
        assert_eq!(build_field_trace(false, false, false, &[]), "");
    }
}

#[cfg(test)]
mod test_field_passthrough {
    use super::{Protocol, apply_field_passthrough};
    use serde_json::json;

    /// 客户端原体：一份同时带 openai 族与 anthropic 族写法的采样参数。
    fn client_body() -> serde_json::Value {
        json!({
            "model": "m",
            "messages": [],
            "stop": ["\n\n"],
            "top_k": 40,
            "seed": 7,
            "response_format": {"type": "json_object"},
            "stream_options": {"include_usage": true},
            "presence_penalty": 0.5,
            "frequency_penalty": -0.25,
            "n": 2,
            "user": "u-1",
            "wild_unknown_field": "should-never-be-forwarded"
        })
    }

    /// anthropic 目标：`stop` 换名 `stop_sequences`，`top_k` 原名补齐；
    /// 允许集合外的 openai 专属参数（seed / n / 惩罚项 / response_format）一个都不出现。
    #[test]
    fn anthropic_renames_stop_and_keeps_only_its_allow_set() {
        let src = client_body();
        let mut b = json!({"model": "m", "messages": [], "max_tokens": 100});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::Anthropic,
            "https://api.anthropic.com/v1/messages",
        );
        assert_eq!(b["stop_sequences"], json!(["\n\n"]));
        assert_eq!(b["top_k"], json!(40));
        for k in [
            "stop",
            "seed",
            "response_format",
            "stream_options",
            "presence_penalty",
            "frequency_penalty",
            "n",
            "user",
            "wild_unknown_field",
        ] {
            assert!(b.get(k).is_none(), "anthropic 允许集合外字段 {k} 不应出现");
        }
    }

    /// anthropic 只收数组形态的 stop_sequences：客户端写字符串要包成单元素数组。
    #[test]
    fn anthropic_wraps_string_stop_into_array() {
        let src = json!({"stop": "END"});
        let mut b = json!({"model": "m"});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::Anthropic,
            "https://api.anthropic.com/v1/messages",
        );
        assert_eq!(b["stop_sequences"], json!(["END"]));
    }

    /// 第三方 OpenAI 兼容端点：openai 族允许集合全量补齐，`top_k` 也放行（vLLM/GLM 通行）。
    #[test]
    fn third_party_openai_fills_full_allow_set() {
        let src = client_body();
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
        );
        assert_eq!(b["stop"], json!(["\n\n"]));
        assert_eq!(b["top_k"], json!(40));
        assert_eq!(b["seed"], json!(7));
        assert_eq!(b["response_format"], json!({"type": "json_object"}));
        assert_eq!(b["stream_options"], json!({"include_usage": true}));
        assert_eq!(b["presence_penalty"], json!(0.5));
        assert_eq!(b["frequency_penalty"], json!(-0.25));
        assert_eq!(b["n"], json!(2));
        assert_eq!(b["user"], json!("u-1"));
        assert!(
            b.get("wild_unknown_field").is_none(),
            "允许集合外字段不透传"
        );
    }

    /// anthropic 客户端 → openai 上游：`stop_sequences` 换名成 `stop`。
    #[test]
    fn openai_renames_stop_sequences_to_stop() {
        let src = json!({"stop_sequences": ["\n\nHuman:"]});
        let mut b = json!({"model": "m"});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
        );
        assert_eq!(b["stop"], json!(["\n\nHuman:"]));
        assert!(b.get("stop_sequences").is_none());
    }

    /// 官方 OpenAI host 回归防线：`top_k` 不是官方参数（未知顶层字段 → 400），必须挡住。
    #[test]
    fn official_openai_excludes_top_k() {
        let src = client_body();
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://api.openai.com/v1/chat/completions",
        );
        assert!(b.get("top_k").is_none(), "官方 OpenAI 允许集合不含 top_k");
        assert!(b.get("wild_unknown_field").is_none());
        assert_eq!(b["seed"], json!(7), "官方文档有的参数照常补齐");
        assert_eq!(b["user"], json!("u-1"));
    }

    /// legacy completions 无 `response_format` 参数，不写出。
    #[test]
    fn completions_excludes_response_format() {
        let src = client_body();
        let mut b = json!({"model": "m", "prompt": "x"});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAICompletions,
            "https://api.openai.com/v1/completions",
        );
        assert!(b.get("response_format").is_none());
        assert_eq!(b["stop"], json!(["\n\n"]));
    }

    /// Responses API 只认 `user`，stop / 惩罚项 / n / seed / response_format 全挡。
    #[test]
    fn responses_allows_only_user() {
        let src = client_body();
        let mut b = json!({"model": "m", "input": []});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAIResponses,
            "https://api.openai.com/v1/responses",
        );
        assert_eq!(b["user"], json!("u-1"));
        for k in [
            "stop",
            "stop_sequences",
            "top_k",
            "seed",
            "response_format",
            "stream_options",
            "presence_penalty",
            "frequency_penalty",
            "n",
        ] {
            assert!(b.get(k).is_none(), "responses 允许集合外字段 {k} 不应出现");
        }
    }

    /// gemini 顶层无本组字段（对应键在 generationConfig 下，归票 09），本函数整体 no-op。
    #[test]
    fn gemini_top_level_is_noop() {
        let src = client_body();
        let mut b = json!({"contents": [], "generationConfig": {"temperature": 0.3}});
        let before = b.clone();
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::Gemini,
            "https://generativelanguage.googleapis.com/v1beta",
        );
        assert_eq!(b, before);
    }

    /// 已建模字段不被覆盖：出站 body 已有该键时保持强类型字段的值。
    #[test]
    fn modeled_fields_are_not_overwritten() {
        let src = json!({"stop": ["from-client"], "user": "from-client", "top_k": 40});
        let mut b = json!({"model": "m", "stop": ["already-modeled"], "user": "already-modeled", "top_k": 1});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
        );
        assert_eq!(b["stop"], json!(["already-modeled"]));
        assert_eq!(b["user"], json!("already-modeled"));
        assert_eq!(b["top_k"], json!(1));
    }

    /// anthropic→anthropic 透传分支上 `safeguards` 必须原样到上游。
    ///
    /// Claude Code 的 auto mode 把安全审查请求放在请求体顶层 `safeguards` 字段，配对 beta 头
    /// `dangerous-tool-use-2026-09-03`；上游把结论放回 `safeguard_results`。网关删掉、改写其中任何
    /// 一半，客户端整个 session 降级并改为自费分类器请求。透传分支上本函数是 no-op（出站 body 就是
    /// 客户端原体，见本文件 `same_protocol_passthrough` 分支的 `req_value.clone()`），本测试守住这点。
    ///
    /// 出处：<https://code.claude.com/docs/en/llm-gateway-protocol#feature-pass-through>
    #[test]
    fn anthropic_passthrough_keeps_safeguards_field() {
        let src = json!({
            "model": "claude-opus-4-8",
            "messages": [],
            "safeguards": [{"type": "dangerous_tool_use", "classifier_context": {"v": 1}}]
        });
        let mut b = src.clone();
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::Anthropic,
            "https://api.anthropic.com/v1/messages",
        );
        assert_eq!(b["safeguards"], src["safeguards"]);
        assert_eq!(b, src, "透传分支上本函数是 no-op");
    }

    /// 透传分支：客户端原体即出站 body，本函数不得删掉任何既有字段（只补不删）。
    #[test]
    fn passthrough_body_keeps_its_own_unknown_fields() {
        let src = client_body();
        let mut b = src.clone();
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
        );
        assert_eq!(b["wild_unknown_field"], json!("should-never-be-forwarded"));
        assert_eq!(b, src, "透传分支上本函数是 no-op");
    }

    /// 客户端没设这些参数时不凭空造键。
    #[test]
    fn absent_client_fields_are_not_invented() {
        let src = json!({"model": "m", "messages": []});
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(
            &mut b,
            &src,
            &Protocol::OpenAI,
            "https://open.bigmodel.cn/api/paas/v4",
        );
        assert_eq!(b, json!({"model": "m", "messages": []}));
    }
}

#[cfg(test)]
mod test_openai_max_completion_tokens {
    use super::{
        Protocol, cap_body_max_tokens, fold_openai_max_completion_tokens,
        rename_openai_max_tokens_key, strip_default_image_detail,
    };
    use aidog_adapter::converter::{convert_request, parse_incoming_request};
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

    /// 与 cap 链路衔接：入站只发 `max_completion_tokens` 且超模型上限 →
    /// 裁剪作用在识别后的值上（转换分支 chat_req 侧口径由 body 侧幂等复裁锚住）。
    #[test]
    fn cap_applies_to_recognized_value() {
        let body = json!({
            "model": "gpt-5", "max_completion_tokens": 200_000,
            "messages": [{"role": "user", "content": "hi"}]
        });
        let req = parse_incoming_request(&Protocol::OpenAI, &body, None).expect("parse");
        let (mut out, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
        assert_eq!(
            cap_body_max_tokens(&mut out, Some(8192), &Protocol::OpenAI),
            Some((200_000, 8192)),
            "cap 没作用在归一后的值上: {out}"
        );
        assert_eq!(out["max_tokens"], json!(8192));

        // 透传分支：原体只有新键，折叠后同样被裁
        let mut b = json!({"model": "gpt-5", "max_completion_tokens": 200_000});
        fold_openai_max_completion_tokens(&mut b, &Protocol::OpenAI);
        assert_eq!(
            cap_body_max_tokens(&mut b, Some(8192), &Protocol::OpenAI),
            Some((200_000, 8192))
        );
        assert_eq!(b["max_tokens"], json!(8192));
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
    use super::{Protocol, apply_disable_thinking, is_official_openai_host};
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
