use super::*;

/// 聚合 SSE body 的上限（字节）。OOM 止血：512MB → 16MB（N 并发 × 上限 = 物理内存预算）。
/// 超限截断 + 标记，禁 panic / OOM。完整上游响应正文落库不依赖此上限（DB schema 列仍在）。
/// 同时也是 push 点（累积期）的上界：`StreamAggregator::push_upstream`/`push_client` 一旦累计
/// 达此值即停止继续 push（O(1) 原子计数器判断，禁每 chunk 全量扫 vec —— 红线 1），
/// 使流式累积期本身有界，而非仅在 flush 时事后截断。
const STREAM_BODY_MAX_BYTES: usize = 16 * 1024 * 1024;

/// SSE 单行重组缓冲 / UTF-8 字节重组 pending 的上界（字节）。**唯一真值源** —— 独立 task
/// `sse-chunk-line-reassembly` 的内容侧行重组缓冲引用此口径，禁另立一份常量。
/// 正常 SSE `data:` 行远小于此值（几十字节到几 KB）；remainder 持续无换行增长到 1MB 视为
/// 异常/恶意上游（永不发完整行），行为：**丢弃整段 remainder**（不截断保留半截，半截 JSON
/// 也解析不出 usage，保留无意义）并 warn，下条数据从空 buf 重新开始拼接——仅影响 usage 提取，
/// 不影响 relay 给客户端的原始字节。
const SSE_LINE_BUF_MAX_BYTES: usize = 1024 * 1024;

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

/// 跨网络 chunk 的 UTF-8 字节重组器：网络 chunk 边界可能切断一个多字节字符，逐 chunk 独立
/// `from_utf8_lossy` 会把被切断的半个字符替换为 U+FFFD（finish.rs:279 历史缺陷，红线 2）。
/// 本重组器把上一 chunk 末尾不完整的字节序列留到下一次 `feed` 再拼接解码一次，
/// 只影响 finish 路径的解析/累积文本，不碰 passthrough 分支原样 relay 的 chunk 字节。
/// 真正非法字节（并非跨边界切断，而是本身损坏）仍走 lossy 兜底——损坏不升级为断流（不用 `from_utf8` 报错式）。
pub(crate) struct Utf8ChunkReassembler {
    pending: Vec<u8>,
}

impl Utf8ChunkReassembler {
    pub(crate) fn new() -> Self {
        Self { pending: Vec::new() }
    }

    /// 喂入一个网络 chunk，返回本次可解码出的文本（已按需拼接上次残留字节）。
    pub(crate) fn feed(&mut self, chunk: &[u8]) -> String {
        // 无残留字节的常态路径：直接借 chunk 尝试整体解码，避免每 chunk 都拷贝一次
        // （转发热路径最小 diff，符合 memory `high-freq-path-min-diff`）。
        if self.pending.is_empty() {
            match std::str::from_utf8(chunk) {
                Ok(s) => return s.to_string(),
                Err(e) if e.error_len().is_none() => {
                    // 尾部是不完整多字节序列（被 chunk 边界切断）：留到下次 feed 拼接。
                    let valid_up_to = e.valid_up_to();
                    self.pending.extend_from_slice(&chunk[valid_up_to..]);
                    return String::from_utf8_lossy(&chunk[..valid_up_to]).into_owned();
                }
                Err(_) => return String::from_utf8_lossy(chunk).into_owned(),
            }
        }

        self.pending.extend_from_slice(chunk);
        // 防御性上界（同 SSE_LINE_BUF_MAX_BYTES 口径）：正常增量 pending 至多 3 字节（一个被
        // 切断的多字节字符尾部，UTF-8 规范上限）；此分支理论不可达，仅防未来重构回归或恶意
        // 上游异常输入导致 pending 无界增长——命中即当作损坏数据 lossy 兜底并清空，不再等待。
        if self.pending.len() > SSE_LINE_BUF_MAX_BYTES {
            let out = String::from_utf8_lossy(&self.pending).into_owned();
            self.pending.clear();
            return out;
        }
        match std::str::from_utf8(&self.pending) {
            Ok(s) => {
                let out = s.to_string();
                self.pending.clear();
                out
            }
            Err(e) if e.error_len().is_none() => {
                let valid_up_to = e.valid_up_to();
                let out = String::from_utf8_lossy(&self.pending[..valid_up_to]).into_owned();
                self.pending.drain(..valid_up_to);
                out
            }
            Err(_) => {
                // 非跨边界的真正非法字节：lossy 兜底，不阻断转发。
                let out = String::from_utf8_lossy(&self.pending).into_owned();
                self.pending.clear();
                out
            }
        }
    }
}

/// 内容路径的跨 chunk SSE 行重组器（task `sse-chunk-line-reassembly`）。`parse_upstream_sse`
/// 按 `data:` 分帧、对单个 chunk 无状态：一条 SSE 行若被网络 chunk 边界切成两半，前半无结束
/// 换行、后半无 `data:` 前缀，两边都不构成合法帧，**整行被双双静默丢弃**——同一循环里
/// `StreamAggregator::feed_sse_usage` 的 `sse_line_buf` 早已用「尾行缓冲」修过这个问题（见其
/// 文档注释），本结构把同一 idiom 接给内容路径，而非另起炉灶（design.md 否决表）。
/// 完整行立即返回供本 chunk 内下发，不攒批——攒批会让首 token 时延随缓冲深度退化。
/// 上界复用 `SSE_LINE_BUF_MAX_BYTES`（唯一真值源，与 `feed_sse_usage` remainder 同口径）：
/// 持续无换行增长到上限视为异常/恶意上游，丢弃整段残留并 warn（不静默丢——静默丢正是本 bug
/// 长期难被发现的根因，design.md 现状节）。
pub(crate) struct SseLineReassembler {
    buf: String,
}

impl SseLineReassembler {
    pub(crate) fn new() -> Self {
        Self { buf: String::new() }
    }

    /// 喂入一个 chunk 已重组好字节层的文本，返回本次可立即下发解析的完整行文本
    /// （直接可喂 `adapter::parse_upstream_sse`）；不完整的尾行留在内部 buf 里等下次 feed 拼接。
    pub(crate) fn feed(&mut self, text: &str) -> String {
        self.buf.push_str(text);
        let split_pos = if self.buf.ends_with('\n') {
            self.buf.len()
        } else {
            self.buf.rfind('\n').map(|p| p + 1).unwrap_or(0)
        };
        let remainder = self.buf.split_off(split_pos);
        let ready = std::mem::replace(&mut self.buf, remainder);
        if self.buf.len() > SSE_LINE_BUF_MAX_BYTES {
            tracing::warn!(
                len = self.buf.len(),
                "sse content line buf remainder exceeded cap, dropping (malformed/oversized SSE line)"
            );
            self.buf.clear();
        }
        ready
    }
}

impl Drop for SseLineReassembler {
    /// 流末（上游断流 / 客户端断连）时 buf 仍有半行残留：半行本就不是合法帧，解析它只会
    /// 失败。选择「记 warn 再丢」而非静默丢弃——静默丢正是这个 bug 长期难被发现的根因
    /// （design.md）。不 panic：流中途结束是正常时序，不是程序错误。
    fn drop(&mut self) {
        if !self.buf.is_empty() {
            tracing::warn!(
                len = self.buf.len(),
                "SSE stream ended with incomplete trailing line in content path, discarding (not a valid frame)"
            );
        }
    }
}

/// 流式日志聚合状态：旁路累积 token + 上游响应原文 + 转换后下发客户端的 SSE。
/// 闭包内对其加锁是同步短临界区（push），**禁持锁跨 await**。
pub(crate) struct StreamAggregator {
    pub(crate) upstream_body: std::sync::Mutex<Vec<Bytes>>,
    pub(crate) client_body: std::sync::Mutex<Vec<Bytes>>,
    // push 点累积字节计数（红线 1：O(1) 原子读判断是否已达 STREAM_BODY_MAX_BYTES，
    // 达上限后 push_upstream/push_client 跳过 push，禁每 chunk 扫 vec 求 len）。
    upstream_body_bytes: std::sync::atomic::AtomicUsize,
    client_body_bytes: std::sync::atomic::AtomicUsize,
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
            upstream_body_bytes: std::sync::atomic::AtomicUsize::new(0),
            client_body_bytes: std::sync::atomic::AtomicUsize::new(0),
            tokens_in: std::sync::atomic::AtomicI32::new(0),
            tokens_out: std::sync::atomic::AtomicI32::new(0),
            tokens_cache: std::sync::atomic::AtomicI32::new(0),
            sse_line_buf: std::sync::Mutex::new(String::new()),
        }
    }

    /// 有界 push 内部实现：O(1) 原子读判断是否已达上限，未达才加锁 push + 累加计数
    /// （红线 1：禁每 chunk 全量扫 vec 求 len）。达上限后静默跳过，vec 不再增长
    /// ——累积期本身有界，而非仅 flush 时事后截断（对照 cap_nonstream_body 语义对称）。
    fn push_capped(
        mutex: &std::sync::Mutex<Vec<Bytes>>,
        counter: &std::sync::atomic::AtomicUsize,
        chunk: &Bytes,
    ) {
        use std::sync::atomic::Ordering::Relaxed;
        if counter.load(Relaxed) >= STREAM_BODY_MAX_BYTES {
            return;
        }
        if let Ok(mut v) = mutex.lock() {
            v.push(chunk.clone());
        }
        counter.fetch_add(chunk.len(), Relaxed);
    }

    /// 四个 push 点之二：上游响应原文旁路累积（finish.rs/passthrough.rs 共用）。
    pub(crate) fn push_upstream(&self, chunk: &Bytes) {
        Self::push_capped(&self.upstream_body, &self.upstream_body_bytes, chunk);
    }

    /// 四个 push 点之二：下发客户端的 SSE 旁路累积（finish.rs/passthrough.rs 共用）。
    pub(crate) fn push_client(&self, chunk: &Bytes) {
        Self::push_capped(&self.client_body, &self.client_body_bytes, chunk);
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
        // remainder 上界（SSE_LINE_BUF_MAX_BYTES 唯一真值源，见常量注释）：持续无换行增长到
        // 上限视为异常/恶意上游，丢弃整段 remainder 而非无界累积——半截数据本就解析不出 usage。
        if remainder.len() > SSE_LINE_BUF_MAX_BYTES {
            tracing::warn!(
                len = remainder.len(),
                "sse_line_buf remainder exceeded cap, dropping (malformed/oversized SSE line)"
            );
            *buf = String::new();
        } else {
            *buf = remainder;
        }
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
            if let Some(ev) = line.strip_prefix("event: ")
                && ev.trim() == "message_stop" {
                    self.flush();
                    return;
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
        if self.record_client_body
            && let Ok(chunks) = self.agg.client_body.lock() {
                final_log.user_response_body = join_stream_body(&chunks);
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
            // upsert_log 现为异步队列 enqueue（终态阻塞 send 保证不丢），实际落库 + 快照移除
            // 已移入 writer 串行序列内部（process_upsert 终态分支），此处禁再显式
            // remove_log_snapshot：enqueue 几乎瞬时返回，会抢在真正落库前执行，
            // 导致下次 upsert 误判 prev=None 走 INSERT，主键冲突（见 log.rs 需求 5）。
            upsert_log(&upsert_state, &final_log, &upsert_settings).await;
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
