//! 出站请求整形纯函数（票 09：从 forward.rs 机械迁出，零行为改动）。
//! 含 DB 读取的 `model_forces_thinking` 不是纯函数，留在 forward.rs。

use super::*;
/// 第三方 anthropic 端点不支持字段剔除（仅在已判定为非官方 anthropic 端点时调用）。
///
/// **`context_management`（无条件剔）**：thinking 开启（`thinking.type != "disabled"`）即剔，
/// 独立于 assistant 历史是否齐全。`context_management` 是官方 Anthropic 服务端协商特性
/// （Claude Code adaptive/summarized 模式 `clear_thinking_20251015`，让官方服务端自动清历史 thinking），
/// 第三方 anthropic-compat 端点普遍不实现该协商，保留该字段对第三方无益仅风险。两类复现：
/// 首轮请求（messages 仅 user，无 assistant 历史）GLM 直拒字段 → 400 code 1210 "API 调用参数有误"
/// （旧逻辑 `has_unmatched_assistant`=false 漏剔 → 本次修复根因）；有 assistant 历史时 DeepSeek 认字段
/// 判 thinking mode → 400 "thinking must be passed back"。函数名沿用 `strip_thinking_if_unmatched`
/// （单调用点 forward.rs，最小 diff；context_management 已超越 thinking unmatched 语义，注释说明）。
///
/// **`thinking`（仅 unmatched 时剔）**：thinking 开启且历史任一 assistant 轮缺 thinking block 时剔。
/// 第三方端点严格要求 thinking 模式下每 assistant 轮回传 thinking block，缺失即 400
/// `content[].thinking must be passed back`；官方 Anthropic 的 summarized/adaptive 模式无此约束，
/// Claude Code 故不回传，跨路由到第三方即触发该 400。thinking block 齐全（正常情况）保留直传，
/// 第三方能正常处理。
pub(crate) fn strip_thinking_if_unmatched(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    let thinking_on = obj
        .get("thinking")
        .map(|t| t.get("type").and_then(|v| v.as_str()) != Some("disabled"))
        .unwrap_or(false);
    if !thinking_on {
        return;
    }
    // context_management 无条件剔：第三方端点不认该协商字段（首轮 GLM 1210 + 有历史 DeepSeek 400）
    obj.remove("context_management");
    let has_unmatched_assistant = obj
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|msgs| {
            msgs.iter().any(|m| {
                if m.get("role").and_then(|r| r.as_str()) != Some("assistant") {
                    return false;
                }
                match m.get("content") {
                    // 块数组：非空且无 thinking block → 不匹配
                    Some(Value::Array(blocks)) => {
                        !blocks.is_empty()
                            && !blocks
                                .iter()
                                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("thinking"))
                    }
                    // 纯文本 assistant 轮：thinking 模式下也应携带 thinking，缺失即不匹配
                    Some(Value::String(s)) => !s.is_empty(),
                    _ => false,
                }
            })
        })
        .unwrap_or(false);
    if has_unmatched_assistant {
        obj.remove("thinking");
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
/// [`super::router::cap_max_tokens`]：未传 / 模型无上限 / 未超限一律不动。
/// 返回 `Some((原值, 裁剪后值))` 表示发生了裁剪，`None` 表示 body 未被改动。
pub(crate) fn cap_body_max_tokens(
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
    let (capped, did_cap) = super::router::cap_max_tokens(Some(requested), model_max);
    let capped = capped.filter(|_| did_cap)?;
    *slot = Value::from(capped);
    Some((requested, capped))
}

/// OpenAI wire 出站 body 上把 `max_completion_tokens` 折进 `max_tokens`（票 05）。
///
/// 转换分支上是 no-op（`to_openai` 只写 `max_tokens`）；生效点是同协议透传——新版 OpenAI
/// SDK 与 o 系列模型只发 `max_completion_tokens`，不折就绕过下游的模型上限裁剪。
///
/// 取值规则与入站 `from_openai` 同源：**两键同时存在时取 `max_completion_tokens`**
/// （新键是客户端有意设置的那个，旧键多是 SDK 为兼容老服务端保留的镜像值）。
/// 折完 body 上只剩 `max_tokens`，官方 host 需要的键名由
/// [`rename_openai_max_tokens_key`] 在链路末端改回。
pub(crate) fn fold_openai_max_completion_tokens(body: &mut Value, wire: &Protocol) {
    if !matches!(wire, Protocol::OpenAI) {
        return;
    }
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    if let Some(v) = obj.remove("max_completion_tokens")
        && !v.is_null()
    {
        obj.insert("max_tokens".to_string(), v);
    }
}

/// 剔除多模态图片块里的默认 `image_url.detail: "auto"`（openai wire 家族，透传与转换共用）。
///
/// `auto` 是 OpenAI 侧默认值，省略与显式写等价（<https://platform.openai.com/docs/guides/images-vision>
/// 「detail ... defaults to auto」）；但部分 OpenAI 兼容上游只认 `low` / `high`，见到 `auto`
/// 直接 400：MiniMax 实测 `invalid params, invalid image detail: auto (2013)`
/// （request 2a28c917b08a436f8f4e273b777ece27，客户端为 OpenAI python SDK，SDK 自己填的 auto）。
/// 故只剔默认值，显式 `low` / `high` 原样上送（语义不可省）。
///
/// 返回剔除条数（0 = body 逐字节不变）。
pub(crate) fn strip_default_image_detail(body: &mut Value, wire: &Protocol) -> usize {
    if !wire.same_wire_family(&Protocol::OpenAI) {
        return 0;
    }
    fn walk(v: &mut Value) -> usize {
        match v {
            Value::Array(items) => items.iter_mut().map(walk).sum(),
            Value::Object(obj) => {
                let mut n = 0;
                if let Some(Value::Object(img)) = obj.get_mut("image_url")
                    && img.get("detail").and_then(Value::as_str) == Some("auto")
                {
                    img.remove("detail");
                    n += 1;
                }
                n + obj.values_mut().map(walk).sum::<usize>()
            }
            _ => 0,
        }
    }
    walk(body)
}

/// 官方 OpenAI Chat Completions 的输出长度键改写（票 05）。
///
/// 官方端点已把 `max_tokens` 标为 deprecated，o 系列等新模型直接拒绝该参数，只认
/// `max_completion_tokens`；第三方 OpenAI 兼容端点与 legacy `/v1/completions`
/// 反过来只认 `max_tokens`。故仅 `wire == OpenAI` 且 host 为 `api.openai.com` 时改键，
/// 值不动（此时值已是裁剪后的最终值）。host 判定复用 [`is_official_openai_host`]。
///
/// 返回是否改写过。
pub(crate) fn rename_openai_max_tokens_key(body: &mut Value, wire: &Protocol, upstream_url: &str) -> bool {
    if !matches!(wire, Protocol::OpenAI) || !is_official_openai_host(upstream_url) {
        return false;
    }
    let Some(obj) = body.as_object_mut() else {
        return false;
    };
    let Some(v) = obj.remove("max_tokens") else {
        return false;
    };
    obj.insert("max_completion_tokens".to_string(), v);
    true
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
pub(crate) fn apply_disable_thinking(
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
pub(crate) fn strip_disable_shaped_params(obj: &mut serde_json::Map<String, Value>) {
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
pub(crate) fn is_official_openai_host(upstream_url: &str) -> bool {
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
pub(crate) fn apply_field_passthrough(
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
pub(crate) fn src_gen_config(
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
pub(crate) fn gemini_response_format(gc: &serde_json::Map<String, Value>) -> Option<Value> {
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

/// 把本次出站的三类改写合成 `proxy_log.field_trace` 的 token 串（票 10；票 11 抽成纯函数）。
///
/// - `capped_on_chat_req` / `capped_on_body`：`max_tokens` 被裁到模型上限的两个来源，
///   **合成同一个 token，不重复记**。转换分支裁在 `chat_req` 上（body 层随后是幂等复裁、
///   必不命中），透传分支裁在 body 层。此前只看 body 层的返回值，整条转换路径的裁剪因此不留痕。
/// - `middleware_changed`：middleware 规则改写过透传 body。
/// - `dropped`：被出站允许集合挡掉的字段名（只有名没有值）。
///
/// 无改写无丢弃时返回空串（不产生噪音记录）。
pub(crate) fn build_field_trace(
    capped_on_chat_req: bool,
    capped_on_body: bool,
    middleware_changed: bool,
    dropped: &[String],
) -> String {
    let mut tokens: Vec<String> = Vec::new();
    if capped_on_chat_req || capped_on_body {
        tokens.push("cap:max_tokens".to_string());
    }
    if middleware_changed {
        tokens.push("rewrite:middleware".to_string());
    }
    tokens.extend(dropped.iter().map(|f| format!("drop:{f}")));
    tokens.join(",")
}

/// 票 10 留痕的候选字段集合 = `apply_field_passthrough` 认识的全部顶层键（跨协议并集）。
/// 不在本集合内的键不参与留痕：转换分支的 `system` / `tools` 等是被结构化转换吸收，不是被挡掉。
const TRACED_FIELDS: [&str; 10] = [
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

/// 第三方 anthropic 端点：无条件剥离 messages[].content 内 `redacted_thinking` block。
///
/// **根因（DB 响应体实证）**：Claude 4.x extended thinking 多轮请求含 `redacted_thinking`
/// content block（Claude Code 回传上轮 protected thinking 的加密关联块）。同协议 passthrough
/// （anthropic→anthropic, remap=true）不经 `to_anthropic` 转换（adapter/anthropic.rs 已 filter
/// Unknown 含 redacted_thinking），content 原样透传 → 第三方端点不认该 type → 400 InvalidParameter
/// `"invalid value: 'redacted_thinking', supported values: 'text','thinking','image','tool_use','tool_result'"`。
///
/// **trace 实证**：81dc4466（火山 doubao coding endpoint）+ 87e3c500（deepseek-v4-pro-260425）
/// 同根因 400。所有第三方 anthropic-compat 端点共性。
///
/// **剥离语义**：redacted_thinking 内容为客户端不可解读的加密 opaque blob（仅官方 Anthropic
/// 能关联上轮 protected thinking），第三方必无法处理，无条件剥离安全。仅遍历数组形态 content
/// （字符串形态无 block 可剔）。content 变空数组时保留 message 结构（剥离顺序敏感，下游规整
/// 依赖 message 序列完整）。
pub(crate) fn strip_redacted_thinking_blocks(body: &mut Value) {
    let Some(msgs) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
        return;
    };
    for m in msgs.iter_mut() {
        let Some(blocks) = m.get_mut("content").and_then(|c| c.as_array_mut()) else {
            continue;
        };
        blocks.retain(|b| b.get("type").and_then(|t| t.as_str()) != Some("redacted_thinking"));
    }
}

/// 第三方 anthropic 端点：messages 内非首位的 role=system 规整到顶层 system 数组。
///
/// **根因（DB 全样本取证）**：Claude Code 把 SessionStart/UserPromptSubmit hook 注入的上下文
/// 以 `role=system` 消息插入 messages 数组中段/末尾（官方 Anthropic 接受该非标位置作为客户端
/// 约定）。GLM / DeepSeek 等第三方 anthropic-compat 端点严格执行规范（role=system 仅顶层 system
/// 字段或 messages[0]），多轮 + 非流式场景下拒绝 → 400 code 1210 "API 调用参数有误"。
///
/// **DB 交叉验证**（GLM `open.bigmodel.cn/api/anthropic`，10552 条 200 + 9 条 400 全样本）：
/// 失败全集 = `{no_stream, has_assistant, messages 含 role=system（含中段+末段）}` —— 9/9 命中；
/// 同结构流式 PASS=1166，非流式 PASS=3（GLM 间歇性接受，3 异常样本均为 14-112 msgs 长上下文）。
/// 故仅非流式触发规整：流式同结构当前工作正常（host-gated 但 is_stream=true 不动），避免回归。
///
/// **规整方式**：messages 内 role=system 消息按出现顺序，content 合并到顶层 `system` 数组
/// （顶层 system 已是数组则追加 text block；字符串则升级为数组；缺失则新建）。
/// messages 数组移除该消息，剩余 user/assistant 交替保持原序。仅多轮（含 assistant）才触发：
/// 首轮无 assistant 时 messages 内 role=system 多为客户端约首约定（DeepSeek/GLM 首轮接受），不动。
pub(crate) fn hoist_mid_messages_system(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    let Some(msgs) = obj.get_mut("messages").and_then(|m| m.as_array_mut()) else {
        return;
    };
    // 仅多轮（有 assistant 历史）触发：首轮无 assistant 不动（首轮 role=system 第三方接受）。
    let has_assistant = msgs
        .iter()
        .any(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant"));
    if !has_assistant {
        return;
    }
    // 收集 messages 内 role=system 的 content（保持出现顺序），同时保留非 system 消息原序。
    let mut hoisted_blocks: Vec<Value> = Vec::new();
    let mut kept: Vec<Value> = Vec::with_capacity(msgs.len());
    for m in msgs.drain(..) {
        if m.get("role").and_then(|r| r.as_str()) == Some("system") {
            // system message content：字符串 → text block；数组（blocks） → 逐项取
            match m.get("content") {
                Some(Value::String(s)) => {
                    hoisted_blocks.push(serde_json::json!({"type": "text", "text": s}));
                }
                Some(Value::Array(blocks)) => {
                    for b in blocks {
                        if b.is_object() {
                            hoisted_blocks.push(b.clone());
                        }
                    }
                }
                _ => {}
            }
        } else {
            kept.push(m);
        }
    }
    if hoisted_blocks.is_empty() {
        // 无 system 可规整：还原原 msgs（drain 清空了）
        *msgs = kept;
        return;
    }
    *msgs = kept;
    // 合并到顶层 system 数组：现有数组追加；字符串升级；缺失新建。
    match obj.get_mut("system") {
        Some(Value::Array(arr)) => arr.extend(hoisted_blocks),
        Some(Value::String(s)) => {
            let mut arr = vec![serde_json::json!({"type": "text", "text": s})];
            arr.extend(hoisted_blocks);
            obj.insert("system".to_string(), Value::Array(arr));
        }
        _ => {
            obj.insert("system".to_string(), Value::Array(hoisted_blocks));
        }
    }
}
