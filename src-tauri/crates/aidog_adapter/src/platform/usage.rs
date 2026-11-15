//! 非流式响应 usage 抽取（issue 68 迁入：usage 抽取接口的默认实现）。
//! 原 `aidog_core::gateway::proxy::stream.rs` 同名项原样搬家（行为不变）。

use serde_json::Value;

/// 按次工具计数提取（spec-按次计费 B2 非流式路）：anthropic `usage.server_tool_use`
/// 子段 → `(工具键, 次数)`，键从子段名派生（`web_search_requests` → `web_search`）。
/// 开放键集：未知子字段容错提取，未在 tools_pricing 声明的键由计费侧（tools_cost）记 0。
pub fn server_tool_counts(usage: &Value) -> Vec<(String, i64)> {
    match usage.get("server_tool_use").and_then(|s| s.as_object()) {
        Some(obj) => obj
            .iter()
            .filter_map(|(k, v)| {
                let n = v.as_i64()?;
                let key = k.strip_suffix("_requests").unwrap_or(k);
                (n > 0).then(|| (key.to_string(), n))
            })
            .collect(),
        None => Vec::new(),
    }
}

/// B1/B2 arm 绑定提取缓存字段。返回 (cache_read, cache_write_5m, cache_write_1h, input 内含的缓存量)。
/// 第 2/3 项是 5m / 1h TTL 写入桶（B3）：仅 anthropic `cache_creation.ephemeral_*_input_tokens`
/// 拆两桶；其余 arm 的写入全记 5m 桶，1h 恒 None。扁平 `cache_creation_input_tokens` 回落全记
/// 5m 桶（与拆桶前等价）。
/// 第 4 项是命中 arm 对应协议的 input 已包含的缓存 token 数，调用方据它扣减 input
/// （Responses 官方公式 ordinaryInputTokens = input_tokens − cached − cache_write），
/// 避免缓存部分在 input 与 cache_read/cache_write 两处双重计价。
/// 不扣的 arm：anthropic `cache_read_input_tokens` / `cache_creation_input_tokens`
/// （input_tokens 天然不含缓存）；裸 `cache_tokens`（schema 不可判定，保守不扣）。
pub fn usage_cache_arms(
    usage: &Value,
) -> (Option<i64>, Option<i64>, Option<i64>, i64) {
    let detail = |outer: &str, inner: &str| {
        usage
            .get(outer)
            .and_then(|d| d.get(inner))
            .and_then(|v| v.as_i64())
    };
    let plain = |k: &str| usage.get(k).and_then(|v| v.as_i64());
    // 读侧优先级：anthropic → openai chat completions → responses → 裸 cache_tokens → deepseek
    // B3：anthropic `cache_creation.ephemeral_5m/1h_input_tokens` 优先拆桶；
    // 扁平 `cache_creation_input_tokens` 回落全记 5m 桶。
    // arm 进入条件是任一 anthropic 缓存字段在场（不能只认 cache_read——上游可能
    // 只报 cache_creation，此时仍须走 5m/1h 拆桶，B3 断言形状即此）。
    let creation = usage.get("cache_creation");
    let e5m = creation
        .and_then(|c| c.get("ephemeral_5m_input_tokens"))
        .and_then(|v| v.as_i64());
    let e1h = creation
        .and_then(|c| c.get("ephemeral_1h_input_tokens"))
        .and_then(|v| v.as_i64());
    let cache_read = plain("cache_read_input_tokens");
    let flat_write = plain("cache_creation_input_tokens");
    if cache_read.is_some() || e5m.is_some() || e1h.is_some() || flat_write.is_some() {
        let (cache_write, cache_write_1h) = if e5m.is_some() || e1h.is_some() {
            (e5m, e1h)
        } else {
            (flat_write, None)
        };
        return (cache_read, cache_write, cache_write_1h, 0);
    }
    if let Some(c) = detail("prompt_tokens_details", "cached_tokens") {
        return (Some(c), None, None, c);
    }
    if let Some(c) = detail("input_tokens_details", "cached_tokens") {
        let cache_write = detail("input_tokens_details", "cache_write_tokens");
        let in_input = c + cache_write.unwrap_or(0);
        return (Some(c), cache_write, None, in_input);
    }
    if let Some(c) = plain("cache_tokens") {
        return (Some(c), None, None, 0);
    }
    if let Some(c) = plain("prompt_cache_hit_tokens") {
        return (Some(c), None, None, c);
    }
    (None, None, None, 0)
}

pub fn extract_usage(body: &str) -> (i32, i32, i32, i32, i32, Vec<(String, i64)>) {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return (0, 0, 0, 0, 0, Vec::new()),
    };
    let usage = match v.get("usage").or_else(|| v.get("usageMetadata")) {
        Some(u) => u,
        None => return (0, 0, 0, 0, 0, Vec::new()),
    };
    if let Some(gemini) = crate::parse_gemini_usage_metadata(usage) {
        return (
            gemini.input_tokens() as i32,
            gemini.output_tokens() as i32,
            gemini.cached_content_tokens as i32,
            0,
            0,
            Vec::new(),
        );
    }
    // Cache tokens（arm 绑定见 usage_cache_arms）：Anthropic (cache_read_input_tokens),
    // OpenAI (prompt_tokens_details.cached_tokens), responses (input_tokens_details.*),
    // DeepSeek (prompt_cache_hit_tokens), generic (cache_tokens)
    let (cache, cache_write, cache_write_1h, in_input) = usage_cache_arms(usage);
    let input = (usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        - in_input)
        .max(0) as i32;
    let output = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    (
        input,
        output,
        cache.unwrap_or(0) as i32,
        cache_write.unwrap_or(0) as i32,
        cache_write_1h.unwrap_or(0) as i32,
        server_tool_counts(usage),
    )
}

