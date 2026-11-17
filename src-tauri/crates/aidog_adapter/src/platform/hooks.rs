//! 平台出站差异钩子的默认实现（家族基座）。
//!
//! 均为 aidog_core gateway/proxy 原样搬家的纯函数（行为不变，出处见各函数 doc）：
//! `cap_max_tokens` 原 `gateway/router/mod.rs`；`inject_coding_plan_fields` 原
//! `gateway/proxy/headers.rs`；其余原 `gateway/proxy/forward.rs`。

use aidog_db::models::Protocol;
use serde_json::Value;
/// 出站 max_tokens 裁剪（convert_request 前调用）。
///
/// 保守策略（Q3）：仅当客户端显式传了 max_tokens **且**超过模型上限时裁剪到上限；
/// 未传（None）不注入默认值；模型无上限记录（None）不裁剪。
///
/// 返回 (裁剪后值, 是否发生裁剪)。
pub fn cap_max_tokens(req_max: Option<u32>, model_max: Option<i64>) -> (Option<u32>, bool) {
    match (req_max, model_max) {
        (Some(req), Some(limit)) if limit > 0 && (req as i64) > limit => (Some(limit as u32), true),
        _ => (req_max, false),
    }
}

/// 出站 body 上的 `max_tokens` 裁剪（透传与转换两分支共用）。
///
/// **为什么要在 body 上再裁一次**：`chat_req` 侧的裁剪只喂给 `convert_request`，同协议透传
/// 分支的 body 是客户端原体 `Value`，超限值原样上送 → 上游 400（票 02）。转换分支上此处
/// 幂等（chat_req 已裁到同一上限，不会二次收缩），故不需要按分支分叉。
///
/// 键名按目标 wire 协议分叉：
/// - anthropic / openai / openai_completions → 顶层 `max_tokens`
/// - openai_responses → 顶层 `max_output_tokens`
/// - gemini → `generationConfig.maxOutputTokens`
///
/// 上限口径由调用方给（`get_model_max_output_tokens(actual_model)`），保守语义同
/// [`cap_max_tokens`]：未传 / 模型无上限 / 未超限一律不动。
/// 返回 `Some((原值, 裁剪后值))` 表示发生了裁剪，`None` 表示 body 未被改动。
pub fn cap_body_max_tokens(
    body: &mut Value,
    model_max: Option<i64>,
    wire: &Protocol,
) -> Option<(u32, u32)> {
    let obj = body.as_object_mut()?;
    let slot = match wire {
        Protocol::Anthropic | Protocol::OpenAI | Protocol::OpenAICompletions => {
            obj.get_mut("max_tokens")
        }
        Protocol::OpenAIResponses => obj.get_mut("max_output_tokens"),
        Protocol::Gemini => obj
            .get_mut("generationConfig")
            .and_then(|gc| gc.as_object_mut())
            .and_then(|gc| gc.get_mut("maxOutputTokens")),
        // 其余枚举值不是 wire 协议（平台类型），不会作为 endpoint 协议出现
        _ => None,
    }?;
    // 超 u32 的病态值按 u32::MAX 处理（照样会被上限裁下来）。
    let requested = u32::try_from(slot.as_u64()?).unwrap_or(u32::MAX);
    let (capped, did_cap) = cap_max_tokens(Some(requested), model_max);
    let capped = capped.filter(|_| did_cap)?;
    *slot = Value::from(capped);
    Some((requested, capped))
}

/// `disable_thinking` 请求字段处理（aidog 本地扩展）。字段存在即剥（非标，透传恐 400）；
/// 值为 true 时先剔掉客户端带来的开启型思考参数，再按目标 wire 协议写入**显式禁用参数**。
///
/// **为什么不只剔参数**（用户决策 2026-08-26，request d1c87c9c 实证）：旧实现只剔不写，
/// 上游收不到任何禁用指令 → 按自身默认（思考开启）执行。实测 MiniMax-M2 把 300 max_tokens
/// 全烧在 thinking block 上，正文一个字没输出。语义因此改为「按协议显式告知上游关闭思考」。
///
/// 协议映射：
/// - anthropic → `thinking: {"type": "disabled"}`
/// - gemini → `generationConfig.thinkingConfig.thinkingBudget = 0`
/// - openai_responses → `reasoning: {"effort": "none"}`
/// - openai / openai_completions → 官方 host 用 `reasoning_effort: "none"`（官方拒未知顶层字段），
///   第三方 OpenAI 兼容端点用 `chat_template_kwargs.enable_thinking = false`（vLLM/SGLang/GLM/Qwen 通行写法）
///
/// 上游硬限制不在本函数职责内：MiniMax M2.x 接受 `thinking.type=disabled` 但模型照样思考，
/// 响应侧不做剥离（用户决策：剥了也救不回被思考烧光的 max_tokens）。
///
/// **`disable` 由调用方从客户端原体读**：转换分支的 `req_body` 由强类型 struct 序列化而来
/// （`adapter::convert_request`），`disable_thinking` 这种未建模的 aidog 私有字段在那一步就被丢掉，
/// 从 `body` 读 key 在转换分支恒为 false（审计实证：本仓 707/1016 请求走 anthropic→openai 转换）。
/// 函数内仍剥 `body` 上残留的同名 key，透传分支靠它把非标字段挡在上游之外。
pub fn apply_disable_thinking(
    body: &mut Value,
    disable: bool,
    wire: &Protocol,
    upstream_url: &str,
    forced_thinking: bool,
) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    obj.remove("disable_thinking");
    // 强制思考模型（registry `thinking_toggleable: false`，如 MiniMax-M3.1-Flash）：上游拒绝
    // 任何显式禁用（400 code 2013 "requires adaptive thinking"），只剥禁用参数、一个都不写。
    // 客户端自带的 `thinking.type=disabled` / `reasoning.effort=none` 同样会被上游拒收，一并剥。
    // 响应侧照旧按 disable_thinking 剥 thinking 块（模型反正要思考，客户端 UX 不变）。
    if forced_thinking {
        strip_disable_shaped_params(obj);
        return;
    }
    if !disable {
        return;
    }
    for k in [
        "thinking",
        "context_management",
        "reasoning_effort",
        "reasoning",
        "enable_thinking",
    ] {
        obj.remove(k);
    }
    if let Some(ctk) = obj
        .get_mut("chat_template_kwargs")
        .and_then(|o| o.as_object_mut())
    {
        ctk.remove("enable_thinking");
    }
    if let Some(gc) = obj
        .get_mut("generationConfig")
        .and_then(|o| o.as_object_mut())
    {
        gc.remove("thinkingConfig");
    }

    match wire {
        Protocol::Anthropic => {
            obj.insert(
                "thinking".to_string(),
                serde_json::json!({"type": "disabled"}),
            );
        }
        Protocol::Gemini => {
            let gc = obj
                .entry("generationConfig".to_string())
                .or_insert_with(|| serde_json::json!({}));
            if let Some(gc) = gc.as_object_mut() {
                gc.insert(
                    "thinkingConfig".to_string(),
                    serde_json::json!({"thinkingBudget": 0}),
                );
            }
        }
        Protocol::OpenAIResponses => {
            obj.insert(
                "reasoning".to_string(),
                serde_json::json!({"effort": "none"}),
            );
        }
        Protocol::OpenAI | Protocol::OpenAICompletions => {
            if is_official_openai_host(upstream_url) {
                obj.insert(
                    "reasoning_effort".to_string(),
                    Value::String("none".to_string()),
                );
            } else {
                let ctk = obj
                    .entry("chat_template_kwargs".to_string())
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(ctk) = ctk.as_object_mut() {
                    ctk.insert("enable_thinking".to_string(), Value::Bool(false));
                }
            }
        }
        // 其余枚举值不是 wire 协议（平台类型），不会作为 endpoint 协议出现
        _ => {}
    }
}

/// 剥「显式禁用思考」形态的参数（保留开启型值）：`thinking.type=disabled`、
/// `reasoning.effort=none`、`reasoning_effort=none`。强制思考模型专用。
pub fn strip_disable_shaped_params(obj: &mut serde_json::Map<String, Value>) {
    let is_disabled_thinking = obj
        .get("thinking")
        .and_then(|t| t.get("type"))
        .and_then(Value::as_str)
        == Some("disabled");
    if is_disabled_thinking {
        obj.remove("thinking");
    }
    let effort_none = obj
        .get("reasoning")
        .and_then(|r| r.get("effort"))
        .and_then(Value::as_str)
        == Some("none");
    if effort_none {
        obj.remove("reasoning");
    }
    if obj.get("reasoning_effort").and_then(Value::as_str) == Some("none") {
        obj.remove("reasoning_effort");
    }
}

/// 官方 OpenAI 端点判定（host == api.openai.com）。官方 Chat Completions 对未知顶层字段
/// 返回 400 `unknown_parameter`，因此禁思考只能走官方认的 `reasoning_effort`。
/// host 提取与 `is_official_anthropic_host` 同 idiom。
pub fn is_official_openai_host(upstream_url: &str) -> bool {
    let after_scheme = upstream_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(upstream_url);
    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(authority)
        .split(':')
        .next()
        .unwrap_or("");
    host.eq_ignore_ascii_case("api.openai.com")
}

/// 未建模顶层字段兜底透传：按**目标 wire 协议的允许集合**从客户端原体补齐出站 body（票 01）。
///
/// **为什么需要**：`ChatRequest` 是白名单强类型模型，`stop` / `top_k` / `seed` /
/// `response_format` / `stream_options` / `presence_penalty` / `frequency_penalty` / `n` /
/// `user` 全无对应字段，五个 wire struct 也都是封闭 struct（无 flatten 出口），
/// 转换分支上这些参数在 `parse_incoming_request` → `convert_request` 之间静默消失
/// （全库 grep 实测零命中）。
///
/// **为什么是白名单而不是全量倒出**：官方 OpenAI Chat Completions 对未知顶层字段返回 400
/// `unknown_parameter`（同 `apply_disable_thinking` 的 host gate 理由），全量倒出会把现在
/// 能用的链路打坏。
///
/// 逐协议允许集合：
/// - anthropic → `stop_sequences`（客户端写 `stop` 时换名 + 字符串包成数组）、`top_k`
/// - openai / openai_completions → `stop`（客户端写 `stop_sequences` 时换名）、`seed`、
///   `stream_options`、`presence_penalty`、`frequency_penalty`、`n`、`user`；
///   `response_format` 仅 chat completions（legacy completions 无此参数）；
///   `top_k` 仅第三方 OpenAI 兼容端点（vLLM/SGLang/GLM/Qwen 通行，官方不认）
/// - openai_responses → `user`（Responses API 不收 stop / 惩罚项 / n / seed / response_format）
/// - gemini → 顶层无本组字段（对应键全在 `generationConfig` 下，归票 09，本函数不碰）
///
/// **只补不覆盖、只加不删**：出站 body 已有该键就不动（强类型字段优先）；
/// 透传分支客户端原体的其它字段原样保留（本函数不做剔除，避免打坏现有透传链路）。
///
/// **`src` 是客户端原体**（`req_value`），不是 `req_body`——同 `apply_disable_thinking` 的
/// 调用方约束：转换分支的 `req_body` 由 wire struct 序列化而来，未建模字段在那里已经没了。
/// **返回值（票 10）**：客户端发了、但本函数的允许集合没让它出站的字段名（按 `TRACED_FIELDS`
/// 固定序，只有名没有值——值可能含敏感内容）。调用方拼进 `proxy_log.field_trace`。
/// 判据是「候选集合内、客户端有、最终出站 body 里没有」；`stop` / `stop_sequences` 互为换名，
/// 目标键已出站即视为已承载，不算被挡。
pub fn apply_field_passthrough(
    body: &mut Value,
    src: &Value,
    wire: &Protocol,
    upstream_url: &str,
) -> Vec<String> {
    let Some(src_obj) = src.as_object() else {
        return Vec::new();
    };
    let Some(obj) = body.as_object_mut() else {
        return Vec::new();
    };

    // ① 停止序列：anthropic 用 stop_sequences（只收数组），openai 族用 stop（字符串或数组）
    let stop_key = match wire {
        Protocol::Anthropic => Some("stop_sequences"),
        Protocol::OpenAI | Protocol::OpenAICompletions => Some("stop"),
        _ => None,
    };
    if let Some(key) = stop_key.filter(|k| !obj.contains_key(*k)) {
        // 取值时先认目标协议的写法，再回退另一族的写法，最后回退 Gemini 源的 generationConfig
        let raw = if key == "stop_sequences" {
            src_obj
                .get("stop_sequences")
                .or_else(|| src_obj.get("stop"))
        } else {
            src_obj
                .get("stop")
                .or_else(|| src_obj.get("stop_sequences"))
        }
        .or_else(|| src_gen_config(src_obj).and_then(|g| g.get("stopSequences")));
        let normalized = match raw {
            Some(Value::String(s)) if key == "stop_sequences" => {
                Some(Value::Array(vec![Value::String(s.clone())]))
            }
            Some(v @ (Value::String(_) | Value::Array(_))) => Some(v.clone()),
            // 其它形态（数字 / 对象 / null）不是任何一家的合法值，不写出
            _ => None,
        };
        if let Some(v) = normalized {
            obj.insert(key.to_string(), v);
        }
    }

    // ② 其余顶层键：同名补齐
    const OPENAI_COMMON: [&str; 6] = [
        "seed",
        "stream_options",
        "presence_penalty",
        "frequency_penalty",
        "n",
        "user",
    ];
    let third_party = !is_official_openai_host(upstream_url);
    let allow: Vec<&str> = match wire {
        Protocol::Anthropic => vec!["top_k"],
        Protocol::OpenAI => {
            let mut v = OPENAI_COMMON.to_vec();
            v.push("response_format");
            if third_party {
                v.push("top_k");
            }
            v
        }
        Protocol::OpenAICompletions => {
            let mut v = OPENAI_COMMON.to_vec();
            if third_party {
                v.push("top_k");
            }
            v
        }
        Protocol::OpenAIResponses => vec!["user"],
        // gemini 顶层无本组字段；其余枚举值不是 wire 协议（平台类型），不会作为 endpoint 协议出现
        _ => vec![],
    };
    for k in allow {
        if obj.contains_key(k) {
            continue;
        }
        // Gemini 源的对应值埋在 generationConfig 里（顶层同名键不存在），故取值分两跳（票 11）
        let from_src = src_obj.get(k).cloned().or_else(|| match k {
            "top_k" => src_gen_config(src_obj).and_then(|g| g.get("topK")).cloned(),
            "response_format" => src_gen_config(src_obj).and_then(gemini_response_format),
            _ => None,
        });
        if let Some(v) = from_src {
            obj.insert(k.to_string(), v);
        }
    }

    // ③ 留痕（票 10）：候选集合内、客户端发了但没出站的字段名。
    let stop_carried = stop_key.map(|k| obj.contains_key(k)).unwrap_or(false);
    TRACED_FIELDS
        .iter()
        .filter(|k| src_obj.contains_key(**k) && !obj.contains_key(**k))
        .filter(|k| !(matches!(**k, "stop" | "stop_sequences") && stop_carried))
        .map(|k| (*k).to_string())
        .collect()
}

/// 客户端原体里的 Gemini `generationConfig` 节点（camelCase 为 REST 规范形态，
/// snake_case 是部分 SDK 写法，与 `middleware_body::visit_texts` 的 systemInstruction 同口径）。
///
/// Gemini 把 `stopSequences` / `topK` / `responseMimeType` 全放在这个子对象里，而本函数
/// 其余部分按顶层键取值——不下钻这一跳，gemini 客户端 → openai/anthropic 上游的这三项就必丢
/// （中立层的 `ChatRequest.extra` 有值，但 `to_openai` / `to_anthropic` 的 wire struct 没有槽位）。
pub fn src_gen_config(
    src_obj: &serde_json::Map<String, Value>,
) -> Option<&serde_json::Map<String, Value>> {
    src_obj
        .get("generationConfig")
        .or_else(|| src_obj.get("generation_config"))?
        .as_object()
}

/// Gemini `responseMimeType` / `responseSchema` → OpenAI `response_format`。
/// 与 `adapter::gemini::extra_response_format` 的正向换算互逆，只认 `application/json`
/// （Gemini 另有 `text/plain` = 默认态，OpenAI 无对应显式写法，不写出）。
pub fn gemini_response_format(gc: &serde_json::Map<String, Value>) -> Option<Value> {
    let mime = gc.get("responseMimeType").and_then(|v| v.as_str())?;
    if !mime.eq_ignore_ascii_case("application/json") {
        return None;
    }
    Some(match gc.get("responseSchema") {
        Some(schema) => serde_json::json!({
            "type": "json_schema",
            "json_schema": { "name": "response", "schema": schema }
        }),
        None => serde_json::json!({ "type": "json_object" }),
    })
}

/// 票 10 留痕的候选字段集合 = `apply_field_passthrough` 认识的全部顶层键（跨协议并集）。
/// 不在本集合内的键不参与留痕：转换分支的 `system` / `tools` 等是被结构化转换吸收，不是被挡掉。
pub(crate) const TRACED_FIELDS: [&str; 10] = [
    "stop",
    "stop_sequences",
    "top_k",
    "seed",
    "stream_options",
    "presence_penalty",
    "frequency_penalty",
    "n",
    "user",
    "response_format",
];

/// 为 Coding Plan 端点注入平台特有字段
/// - Kimi Code Plan: 注入 prompt_cache_key（必填，用 group + model hash 作会话标识）
pub fn inject_coding_plan_fields(body: &mut Value, protocol: &aidog_db::models::Protocol) {
    match protocol {
        aidog_db::models::Protocol::Kimi | aidog_db::models::Protocol::KimiCoding => {
            // Kimi Code Plan 要求 prompt_cache_key 以提升缓存命中率
            // 用模型名 + 短随机串生成会话级 cache key
            if let Some(obj) = body.as_object_mut() {
                let model = obj
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let session_id = format!(
                    "aidog-{}-{:06x}",
                    model,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        / 300 // 5-minute window
                );
                obj.insert("prompt_cache_key".to_string(), Value::String(session_id));
            }
        }
        _ => {
            // GLM / MiniMax / 百炼 等 coding plan 暂无额外字段
        }
    }
}
