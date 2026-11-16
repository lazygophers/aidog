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

/// 出站协议转换丢弃的音视频字段。OpenAI Chat 仅接受 base64 audio；其余不支持的块记入 field_trace。
/// 2026-11-15 官方契约：Anthropic `ContentBlockParam` 无 audio/video（https://platform.claude.com/docs/en/api/messages）；
/// Responses `input` 的内容块无 audio（https://developers.openai.com/api/reference/resources/responses/methods/create）；
/// Chat Completions 支持 base64 input_audio（https://developers.openai.com/api/docs/guides/audio-chat-completions）。
/// 上述目标均未提供 video content 输入形状。
pub(crate) fn dropped_media_fields(req: &aidog_adapter::types::ChatRequest, wire: &Protocol) -> Vec<String> {
    req.messages
        .iter()
        .flat_map(|message| message.content.blocks())
        .filter_map(|block| match block {
            aidog_adapter::types::ContentBlock::Media { media_type, data, .. } => {
                let kind = media_type.split_once('/')?.0;
                if !matches!(kind, "audio" | "video") {
                    return None;
                }
                let dropped = match wire {
                    &Protocol::OpenAI => kind != "audio" || data.is_none(),
                    &Protocol::Anthropic | &Protocol::OpenAIResponses => true,
                    _ => false,
                };
                dropped.then(|| kind.to_string())
            }
            _ => None,
        })
        .collect()
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
