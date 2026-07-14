use super::*;

/// 聚合 SSE body 的上限（字节）。OOM 止血：512MB → 16MB（N 并发 × 上限 = 物理内存预算）。
/// 超限截断 + 标记，禁 panic / OOM。完整上游响应正文落库不依赖此上限（DB schema 列仍在）。
const STREAM_BODY_MAX_BYTES: usize = 16 * 1024 * 1024;

/// 非流式响应 body 落库上限（对齐 STREAM_BODY_MAX_BYTES）。仅落库 String 经此截断 + 标记；
/// 转发客户端的原文不受影响（与流式「转发全量、聚合上限」语义对称）。
pub(crate) const NONSTREAM_BODY_MAX_BYTES: usize = 16 * 1024 * 1024;

/// 非流式 body cap：超 NONSTREAM_BODY_MAX_BYTES 截断并追加 truncation 标记（同 join_stream_body idiom）。
/// ponytail: 与 join_stream_body 同 ceiling 16MB，落库侧用，转发原文照旧全量。
pub(crate) fn cap_nonstream_body(bytes: &[u8]) -> String {
    if bytes.len() > NONSTREAM_BODY_MAX_BYTES {
        let mut s = String::from_utf8_lossy(&bytes[..NONSTREAM_BODY_MAX_BYTES]).into_owned();
        s.push_str("\n[truncated: non-stream body exceeded size limit]");
        s
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// 把聚合的 SSE chunk（Vec<Bytes>）拼接为字符串，超上限则截断并加标记。
/// 旁路累积零阻塞转发，此处一次性拼接（仅 flush 时调用，非 chunk 热路径）。
/// ponytail: 不预分配大 Vec —— 截断分支按需 grow（避免每次 flush 预占 16MB），非截断分支
/// total 已是实际字节和（≤16MB）可用 with_capacity。
fn join_stream_body(chunks: &[Bytes]) -> String {
    let total: usize = chunks.iter().map(|c| c.len()).sum();
    if total > STREAM_BODY_MAX_BYTES {
        let mut buf: Vec<u8> = Vec::new();
        for c in chunks {
            if buf.len() >= STREAM_BODY_MAX_BYTES {
                break;
            }
            let remaining = STREAM_BODY_MAX_BYTES - buf.len();
            let take = remaining.min(c.len());
            buf.extend_from_slice(&c[..take]);
        }
        let mut s = String::from_utf8_lossy(&buf).into_owned();
        s.push_str("\n[truncated: stream body exceeded size limit]");
        s
    } else {
        let mut buf: Vec<u8> = Vec::with_capacity(total);
        for c in chunks {
            buf.extend_from_slice(c);
        }
        String::from_utf8_lossy(&buf).into_owned()
    }
}

/// 流式日志聚合状态：旁路累积 token + 上游响应原文 + 转换后下发客户端的 SSE。
/// 闭包内对其加锁是同步短临界区（push），**禁持锁跨 await**。
pub(crate) struct StreamAggregator {
    pub(crate) upstream_body: std::sync::Mutex<Vec<Bytes>>,
    pub(crate) client_body: std::sync::Mutex<Vec<Bytes>>,
    tokens_in: std::sync::atomic::AtomicI32,
    tokens_out: std::sync::atomic::AtomicI32,
    tokens_cache: std::sync::atomic::AtomicI32,
    // SSE 行重组缓冲：网络 chunk 边界与 SSE event 边界不对齐，单个 `data:` 行可能被
    // 切到两个 reqwest chunk。逐 chunk `.lines()` 解析会把尾部不完整行喂给 serde 解析失败
    // 静默丢弃 usage（尤其 anthropic 尾部 message_delta 携带最终 input/output_tokens 时）。
    // 此缓冲保留每个 chunk 末尾未以换行结束的残行，拼到下个 chunk 头部，保证 usage 解析始终见完整行。
    sse_line_buf: std::sync::Mutex<String>,
}

impl StreamAggregator {
    pub(crate) fn new() -> Self {
        Self {
            upstream_body: std::sync::Mutex::new(Vec::new()),
            client_body: std::sync::Mutex::new(Vec::new()),
            tokens_in: std::sync::atomic::AtomicI32::new(0),
            tokens_out: std::sync::atomic::AtomicI32::new(0),
            tokens_cache: std::sync::atomic::AtomicI32::new(0),
            sse_line_buf: std::sync::Mutex::new(String::new()),
        }
    }

    /// 从一个网络 chunk 的文本累计 SSE usage，跨 chunk 边界重组 `data:` 行。
    /// 仅用于 usage 提取，不影响向客户端 relay 的原始字节。
    /// 缓冲未以换行结束的尾部残行，拼到后续 chunk；遇 `[DONE]`/解析失败的行静默跳过。
    pub(crate) fn feed_sse_usage(&self, text: &str) {
        let mut buf = match self.sse_line_buf.lock() {
            Ok(b) => b,
            Err(_) => return,
        };
        buf.push_str(text);
        // ponytail: 末尾若无换行 → 残行留到下次。按最后换行位置 split_off 切分：
        // 前段（已完整行）用 lines() 借用迭代（&str，零分配）；后段（残行）保留为 String 写回 buf。
        // 比 split('\n').map(to_string).collect::<Vec<String>>() 少 N 次 String 分配 / chunk。
        // 无换行（split_pos=0）→ 整段作 remainder，不迭代；末尾换行（split_pos=len）→ 全段迭代。
        let split_pos = if buf.ends_with('\n') {
            buf.len()
        } else {
            buf.rfind('\n').map(|p| p + 1).unwrap_or(0)
        };
        let remainder = buf.split_off(split_pos);
        for line in buf.lines() {
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();
                if data == "[DONE]" {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    accumulate_sse_usage(&json, &self.tokens_in, &self.tokens_out, &self.tokens_cache);
                }
            }
        }
        *buf = remainder;
    }
}

/// 流式日志最终回写 guard：[DONE] 正常结束 或 客户端断连 Drop 时，
/// 用聚合的 token + body 回写日志（INSERT OR REPLACE 覆盖返回前的占位 upsert）。
/// flush 幂等（est_fired 守卫），[DONE] 与 Drop 只触发一次。
/// Drop 内不可 await → 用 tokio::spawn fire-and-forget 落库 + 后台预估。
pub(crate) struct StreamLogGuard {
    pub(crate) agg: Arc<StreamAggregator>,
    pub(crate) est_fired: Arc<std::sync::atomic::AtomicBool>,
    // 日志回写上下文
    pub(crate) log: ProxyLog,
    pub(crate) state: Arc<ProxyState>,
    pub(crate) settings: ProxyLogSettings,
    pub(crate) start: std::time::Instant,
    pub(crate) record_upstream_body: bool,
    pub(crate) record_client_body: bool,
    pub(crate) req_span: tracing::Span,
    // 后台预估上下文（None = 不做预估，如透传分支）
    pub(crate) est: Option<StreamEstCtx>,
}

/// 流式 flush 时触发的后台预估上下文。
pub(crate) struct StreamEstCtx {
    pub(crate) platform_id: u64,
    pub(crate) platform_type: Protocol,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) extra: String,
    pub(crate) coding_plan: bool,
}

impl StreamLogGuard {
    /// 若 chunk 文本含 SSE 终止标记则触发 flush（确定性回写，不依赖 Drop 兜底）。
    /// 覆盖两类协议终止符：
    ///   - OpenAI / 兼容：`data: [DONE]`
    ///   - Anthropic：`event: message_stop`（含 `data: {"type":"message_stop"}`）—— 原生
    ///     Anthropic 流**不发 `[DONE]`**，仅以 message_stop 收尾。漏检此标记会使 anthropic→anthropic
    ///     透传流仅靠 Drop 兜底回写；Drop 内 `tokio::spawn` 在连接 abort 时序下偶发丢写，
    ///     导致 response_body 永久停在 `[stream]` 占位（见修复）。
    ///
    /// 正常结束走此路径回写（token 已累加完整）；仍未命中（如上游中途断裂无终止符）由 Drop 兜底。
    pub(crate) fn flush_if_done(&self, text: &str) {
        for line in text.lines() {
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();
                if data == "[DONE]" {
                    self.flush();
                    return;
                }
                // Anthropic message_stop 也可能以 data 行携带 type 字段出现
                if data.contains("\"type\":\"message_stop\"")
                    || data.contains("\"type\": \"message_stop\"")
                {
                    self.flush();
                    return;
                }
            }
            // SSE event 行形式：`event: message_stop`
            if let Some(ev) = line.strip_prefix("event: ") {
                if ev.trim() == "message_stop" {
                    self.flush();
                    return;
                }
            }
        }
    }

    /// 用聚合结果回写日志 + 触发后台预估。幂等：仅首次调用生效。
    pub(crate) fn flush(&self) {
        use std::sync::atomic::Ordering::Relaxed;
        if self.est_fired.swap(true, Relaxed) {
            return;
        }
        let input_tokens = self.agg.tokens_in.load(Relaxed);
        let output_tokens = self.agg.tokens_out.load(Relaxed);
        let cache_tokens = self.agg.tokens_cache.load(Relaxed);

        let mut final_log = self.log.clone();
        final_log.input_tokens = input_tokens;
        final_log.output_tokens = output_tokens;
        final_log.cache_tokens = cache_tokens;
        final_log.status_code = 200;
        final_log.duration_ms = self.start.elapsed().as_millis() as i32;
        // 聚合真实 SSE 内容写入 body（受 record 开关控制；upsert_log 仍按 settings 二次过滤）。
        // 无论是否记录正文，都把 response_body 从 "[stream]" 占位改写为真实内容 / 空串，
        // 使 upsert_log 的终态判定（response_body != "[stream]"）识别本次为流式终态 —— 否则
        // 关日志正文时占位 "[stream]" 会残留，导致聚合统计漏计流式请求。
        if self.record_upstream_body {
            if let Ok(chunks) = self.agg.upstream_body.lock() {
                final_log.response_body = join_stream_body(&chunks);
            }
        } else {
            final_log.response_body = String::new();
        }
        if self.record_client_body {
            if let Ok(chunks) = self.agg.client_body.lock() {
                final_log.user_response_body = join_stream_body(&chunks);
            }
        }

        tracing::info!(
            platform_id = final_log.platform_id, model = %final_log.actual_model,
            status = 200, stream = true, duration_ms = final_log.duration_ms,
            input_tokens, output_tokens, cache_tokens, "stream request completed (flush)"
        );

        let upsert_state = self.state.clone();
        let upsert_settings = self.settings.clone();
        let span = self.req_span.clone();
        let task = async move {
            let id = final_log.id.clone();
            upsert_log(&upsert_state, &final_log, &upsert_settings).await;
            // 流式终态：移除 in-flight 列快照，防 map 无限增长。
            remove_log_snapshot(&upsert_state, &id);
        }
        .instrument(span);
        // 经显式 runtime handle 落库：Drop（含客户端 abort / 连接 teardown）路径下
        // 裸 `tokio::spawn` 可能不在 runtime 上下文 → panic 被 Drop 吞掉、最终态丢写
        // （response_body 停在 `[stream]` 占位）。捕获 handle 后 spawn 始终落到 runtime，
        // 保证 flush 在所有收尾路径（[DONE] / message_stop / Drop 兜底）确定性回写。
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(task);
        } else {
            tracing::warn!(
                "stream flush: no tokio runtime in scope, final log write skipped (response_body may stay placeholder)"
            );
        }

        if let Some(est) = &self.est {
            spawn_estimate(
                &self.state,
                est.platform_id,
                &est.platform_type,
                est.base_url.clone(),
                est.api_key.clone(),
                est.model.clone(),
                est.extra.clone(),
                input_tokens,
                output_tokens,
                cache_tokens,
                est.coding_plan,
                self.req_span.clone(),
            );
        }
    }
}

impl Drop for StreamLogGuard {
    fn drop(&mut self) {
        // 客户端断连 / 上游无 [DONE] → flush 未触发，此处兜底回写已聚合数据。
        // Drop 内不可 async；flush 内部用 tokio::spawn 落库（Drop 发生在 runtime 任务上下文中）。
        self.flush();
    }
}

/// 从 SSE event JSON 尽力累计 usage（Anthropic / OpenAI 兼容字段）
///
/// 用 fetch_max（只增不减）而非 store（覆盖）：Anthropic 流式语义下 input/cache 在
/// `message_start` 起始即定值，但后续 `message_delta`（及中转站尾部汇总事件）常携带
/// `input_tokens: 0`，store 覆盖会把真实 input 清零。output 在 message_delta 里是累计值，
/// 取流中最大即终值。OpenAI 末尾一次性给全量，从 0 升上去同样安全。
pub(crate) fn accumulate_sse_usage(
    json: &Value,
    acc_in: &std::sync::atomic::AtomicI32,
    acc_out: &std::sync::atomic::AtomicI32,
    acc_cache: &std::sync::atomic::AtomicI32,
) {
    use std::sync::atomic::Ordering::Relaxed;
    // usage 可能在顶层，也可能在 message.usage（Anthropic message_start）
    let usage = json
        .get("usage")
        .or_else(|| json.get("message").and_then(|m| m.get("usage")));
    let usage = match usage {
        Some(u) => u,
        None => return,
    };
    if let Some(i) = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_i64())
    {
        acc_in.fetch_max(i as i32, Relaxed);
    }
    if let Some(o) = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_i64())
    {
        acc_out.fetch_max(o as i32, Relaxed);
    }
    if let Some(c) = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|v| v.as_i64())
        })
        .or_else(|| usage.get("cache_tokens").and_then(|v| v.as_i64()))
    {
        acc_cache.fetch_max(c as i32, Relaxed);
    }
}

/// Extract input/output/cache tokens from non-stream response JSON
/// 流式判定：请求 body 的 stream 字段与上游响应 content-type 取并。
/// 中转站常对未声明 stream 的请求强制以 `text/event-stream` 响应；仅凭请求字段会误判为非流式，
/// 进而用 JSON 解析 SSE 文本拿不到 usage → token/est_cost 全为 0。OR 语义保证既有流式路径不回归。
pub(crate) fn resolve_is_stream(req_stream: bool, upstream_content_type: &str) -> bool {
    req_stream || upstream_content_type.contains("text/event-stream")
}

pub(crate) fn extract_usage(body: &str) -> (i32, i32, i32) {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return (0, 0, 0),
    };
    let usage = match v.get("usage") {
        Some(u) => u,
        None => return (0, 0, 0),
    };
    let input = usage.get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    let output = usage.get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    // Cache tokens: Anthropic (cache_read_input_tokens), OpenAI (prompt_tokens_details.cached_tokens), generic
    let cache = usage.get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("prompt_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .and_then(|v| v.as_i64()))
        .or_else(|| usage.get("cache_tokens").and_then(|v| v.as_i64()))
        .unwrap_or(0) as i32;
    (input, output, cache)
}

/// Replace "model" field in a JSON response body back to the original model name
pub(crate) fn replace_model_in_json(bytes: &[u8], original_model: &str) -> Vec<u8> {
    let mut v: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return bytes.to_vec(),
    };
    if let Some(obj) = v.as_object_mut() {
        obj.insert("model".to_string(), Value::String(original_model.to_string()));
    }
    serde_json::to_vec(&v).unwrap_or_else(|_| bytes.to_vec())
}

#[cfg(test)]
#[path = "test_stream.rs"]
mod test_stream;
