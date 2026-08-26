use aidog_stats::DbInitTables;
use super::*;

    #[test]
    fn accumulate_sse_usage_anthropic_and_openai() {
        use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
        let i = AtomicI32::new(0);
        let o = AtomicI32::new(0);
        let c = AtomicI32::new(0);

        // Anthropic message_start: usage 嵌在 message
        let anth: Value = serde_json::json!({
            "type": "message_start",
            "message": { "usage": { "input_tokens": 10, "cache_read_input_tokens": 3 } }
        });
        accumulate_sse_usage(&anth, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 10);
        assert_eq!(c.load(Relaxed), 3);

        // OpenAI 顶层 usage（新 atomics，避免与上面 max 语义相互干扰）
        let oi = AtomicI32::new(0);
        let oo = AtomicI32::new(0);
        let oc = AtomicI32::new(0);
        let oai: Value = serde_json::json!({
            "usage": { "prompt_tokens": 20, "completion_tokens": 7 }
        });
        accumulate_sse_usage(&oai, &oi, &oo, &oc);
        assert_eq!(oi.load(Relaxed), 20);
        assert_eq!(oo.load(Relaxed), 7);
    }

    // ── 回归：Anthropic 流式 message_start 的 input/cache 不被尾部 message_delta(input:0) 覆盖 ──
    // 根因：中转站/relay 的 message_delta 常带 input_tokens:0，store 覆盖会把真实 input 清零。
    // 期望：fetch_max 语义下 input=356、cache=50880 保留，output 取 delta 累计终值 29。
    #[test]
    fn accumulate_sse_usage_anthropic_stream_input_not_clobbered() {
        use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
        let i = AtomicI32::new(0);
        let o = AtomicI32::new(0);
        let c = AtomicI32::new(0);

        // 1) message_start：input/cache 起始即定值
        let start: Value = serde_json::json!({
            "type": "message_start",
            "message": { "usage": {
                "input_tokens": 356,
                "cache_read_input_tokens": 50880,
                "output_tokens": 1
            }}
        });
        accumulate_sse_usage(&start, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 356);
        assert_eq!(c.load(Relaxed), 50880);

        // 2) message_delta（中途）：output 累计上升，input 被中转站带成 0
        let delta1: Value = serde_json::json!({
            "type": "message_delta",
            "usage": { "input_tokens": 0, "output_tokens": 15 }
        });
        accumulate_sse_usage(&delta1, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 356, "input 不可被 message_delta 的 0 清零");
        assert_eq!(o.load(Relaxed), 15);

        // 3) message_delta（终值）：output 累计终值 29，input 仍 0
        let delta2: Value = serde_json::json!({
            "type": "message_delta",
            "usage": { "input_tokens": 0, "output_tokens": 29 }
        });
        accumulate_sse_usage(&delta2, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 356, "input 终态保留");
        assert_eq!(c.load(Relaxed), 50880, "cache 终态保留");
        assert_eq!(o.load(Relaxed), 29, "output 取累计终值");
    }

    // ── 回归：尾部 message_delta(usage) 行被切到两个网络 chunk 仍能解析 usage ──
    // 根因：逐 chunk `.lines()` 解析时，被切断的 `data:` 行喂给 serde 解析失败被静默丢弃，
    // usage(input/output) 永久丢失 → token=0 / est_cost=0（response_body 完整落库但 token 全 0）。
    // 期望：feed_sse_usage 跨 chunk 重组残行后，input=723 / output=2922 / cache=84480 正确累计。
    #[test]
    fn feed_sse_usage_reassembles_split_chunk_boundary() {
        use std::sync::atomic::Ordering::Relaxed;
        let agg = StreamAggregator::new();
        // 真实复现：长流尾部 message_delta usage 行在某字节处被切成两块。
        let full = "event: content_block_stop\ndata: {\"type\": \"content_block_stop\", \"index\": 3}\n\nevent: message_delta\ndata: {\"type\": \"message_delta\", \"delta\": {\"stop_reason\": \"tool_use\"}, \"usage\": {\"input_tokens\": 723, \"output_tokens\": 2922, \"cache_read_input_tokens\": 84480}}\n\nevent: message_stop\ndata: {\"type\": \"message_stop\"}\n\n";
        // 在 message_delta 的 data: 行中间切断（模拟 TCP chunk 边界）。
        let split_at = full.find("\"output_tokens\"").unwrap();
        let (head, tail) = full.split_at(split_at);
        agg.feed_sse_usage(head);
        // 第一块结束时 message_delta 的 data 行不完整，尚不能解析出 output。
        assert_eq!(agg.tokens_out.load(Relaxed), 0, "残行未完成前不应误解析");
        agg.feed_sse_usage(tail);
        assert_eq!(agg.tokens_in.load(Relaxed), 723, "跨 chunk 重组后 input 正确");
        assert_eq!(agg.tokens_out.load(Relaxed), 2922, "跨 chunk 重组后 output 正确");
        assert_eq!(agg.tokens_cache.load(Relaxed), 84480, "跨 chunk 重组后 cache 正确");
    }

    // ── 回归：OpenAI 流式末尾一次性 usage 不因 fetch_max 回退 ──
    // 中途 chunk 无 usage（None → 不触发），末尾一次性给全量，从 0 升上去。
    #[test]
    fn accumulate_sse_usage_openai_stream_final_usage() {
        use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
        let i = AtomicI32::new(0);
        let o = AtomicI32::new(0);
        let c = AtomicI32::new(0);

        // 中途 chunk：无 usage 字段
        let mid: Value = serde_json::json!({
            "choices": [{ "delta": { "content": "hi" } }]
        });
        accumulate_sse_usage(&mid, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 0);
        assert_eq!(o.load(Relaxed), 0);

        // 末尾 chunk：一次性全量 usage（含 cached_tokens）
        let last: Value = serde_json::json!({
            "usage": {
                "prompt_tokens": 1024,
                "completion_tokens": 200,
                "prompt_tokens_details": { "cached_tokens": 512 }
            }
        });
        accumulate_sse_usage(&last, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 1024);
        assert_eq!(o.load(Relaxed), 200);
        assert_eq!(c.load(Relaxed), 512);
    }

    // ── Responses API 子端点识别：精确放行 create，拦所有子端点 ──
    #[test]
    fn gzip_decompressed_anthropic_usage_extracts_tokens() {
        use flate2::read::GzDecoder;
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::{Read, Write};

        // anthropic 非流式响应体（含 usage.input_tokens / output_tokens / cache_read_input_tokens）
        let json = r#"{
            "id": "msg_01abc",
            "type": "message",
            "role": "assistant",
            "model": "glm-5.1",
            "content": [{"type": "text", "text": "hello"}],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 1234,
                "output_tokens": 567,
                "cache_read_input_tokens": 89
            }
        }"#;

        // 模拟上游：gzip 压缩明文 JSON（等价上游回 content-encoding: gzip）
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(json.as_bytes()).unwrap();
        let gzipped = encoder.finish().unwrap();
        // 压缩字节非 UTF-8 可读 → 直接喂 extract_usage 解析失败返回 (0,0,0)（复现旧 bug）
        let lossy = String::from_utf8_lossy(&gzipped);
        assert_eq!(
            extract_usage(&lossy),
            (0, 0, 0),
            "压缩字节当文本解析应失败（复现旧 bug）"
        );

        // 模拟 reqwest 启用 feature 后的解压结果：解压回明文
        let mut decoder = GzDecoder::new(&gzipped[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();

        // 解压后 JSON → extract_usage → token > 0（修复后语义）
        let (input, output, cache) = extract_usage(&decompressed);
        assert_eq!(input, 1234);
        assert_eq!(output, 567);
        assert_eq!(cache, 89);
        assert!(input > 0 && output > 0, "解压后 token 必须 > 0");
    }

    // ── StreamLogGuard flush / 终态回写 response_body 回归 ──
    //   根因：anthropic→anthropic 透传流不发 `[DONE]`（仅 message_stop 收尾），
    //   旧 flush_if_done 只认 [DONE] → 这类流仅靠 Drop 兜底，Drop 内 tokio::spawn
    //   在连接 abort 时序下偶发丢写，response_body 永久停在 `[stream]` 占位。

    use std::sync::atomic::AtomicBool;

    /// 构造一个最小可用、初始化好表的临时文件 DB（避免 :memory: 全局缓存跨 test 串味）。
    async fn flush_test_db() -> (Arc<aidog_db::Db>, std::path::PathBuf) {
        // ponytail: proxy_log 拆库后用 :memory:（主+proxy_log 共享同一物理连接，proxy_log 表可见）。
        // 旧实现用文件库，但本测试关注流式 flush 逻辑而非文件 I/O，:memory: 足够。
        let db = aidog_db::Db::new(":memory:")
            .await
            .expect("open memory db");
        db.init_tables().await.expect("init tables");
        (Arc::new(db), std::path::PathBuf::new())
    }

    fn flush_test_state(db: Arc<aidog_db::Db>) -> Arc<ProxyState> {
        let (log_tx, log_rx) = tokio::sync::mpsc::channel(1024);
        let state = Arc::new(ProxyState {
            db,
            app: None,
            middleware: Arc::new(MiddlewareEngine::new()),
            scheduler: Arc::new(super::super::scheduling::SchedulerState::new()),
            sticky: Arc::new(super::super::scheduling::StickyTable::new()),
            log_snapshots: dashmap::DashMap::new(),
            agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
            listen_addr: std::sync::OnceLock::new(),
            settings_cache: Arc::new(tokio::sync::RwLock::new(Default::default())),
            log_tx,
        });
        spawn_log_writer(state.clone(), log_rx);
        state
    }

    fn placeholder_stream_log(id: &str) -> ProxyLog {
        let ts = aidog_db::now();
        ProxyLog {
            id: id.to_string(),
            group_key: "gk_test".to_string(),
            model: "claude".to_string(),
            actual_model: "glm-5".to_string(),
            source_protocol: "anthropic".to_string(),
            target_protocol: "anthropic".to_string(),
            platform_id: 0,
            request_headers: String::new(),
            request_body: String::new(),
            upstream_request_headers: String::new(),
            upstream_request_body: String::new(),
            response_body: String::new(),
            request_url: String::new(),
            upstream_request_url: String::new(),
            upstream_response_headers: String::new(),
            upstream_status_code: 200,
            user_response_headers: String::new(),
            user_response_body: String::new(),
            status_code: 200,
            duration_ms: 0,
            input_tokens: 0,
            output_tokens: 0,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: true,
            attempts: Vec::new(),
            retry_count: 0,
            blocked_by: String::new(),
            blocked_reason: String::new(),
            created_at: ts,
            updated_at: ts,
            deleted_at: 0,
            cli_proxy_provider_id: None,
            done: false,
            field_trace: String::new(),
        }
    }

    /// 建一个 StreamLogGuard，settings 开启上游记录（log_upstream_request=true），
    /// 以隔离验证 flush 机制本身（占位 → 聚合内容回写），不被 upsert_log 的 strip_upstream
    /// 二次过滤清空 response_body —— strip 行为另由 db::test_proxy_log 专门覆盖。
    /// upstream_chunks 预先 push 进 agg.upstream_body（模拟流式逐 chunk 累积）。
    fn make_guard(
        state: &Arc<ProxyState>,
        log: ProxyLog,
        upstream_chunks: &[&str],
        out_tokens: i32,
    ) -> StreamLogGuard {
        let agg = Arc::new(StreamAggregator::new());
        {
            let mut up = agg.upstream_body.lock().unwrap();
            for c in upstream_chunks {
                up.push(Bytes::from(c.to_string()));
            }
        }
        if out_tokens > 0 {
            agg.tokens_out
                .store(out_tokens, std::sync::atomic::Ordering::Relaxed);
        }
        StreamLogGuard {
            agg,
            est_fired: Arc::new(AtomicBool::new(false)),
            log,
            state: state.clone(),
            settings: ProxyLogSettings {
                log_upstream_request: true, // 开启上游记录，使 flush 回写的 response_body 透过 strip 二次闸
                ..ProxyLogSettings::default()
            },
            start: std::time::Instant::now(),
            record_upstream_body: true, // = log_settings.enabled
            record_client_body: false,  // log_user_request=false
            req_span: tracing::Span::current(),
            est: None,
        }
    }

    async fn read_response_body(db: &aidog_db::Db, id: &str) -> String {
        aidog_logs::get_proxy_log(db, id)
            .await
            .expect("get log")
            .expect("row exists")
            .response_body
    }

    /// 等待 flush 内 tokio::spawn 的落库任务完成（短轮询，最多 ~2s）。
    async fn await_flush_write(db: &aidog_db::Db, id: &str) -> String {
        // 票 06：占位哨兵已废，改等 done 置位（flush 终态回写完成标志）。
        for _ in 0..200 {
            let l = aidog_logs::get_proxy_log(db, id).await.unwrap().unwrap();
            if l.done {
                return l.response_body;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        read_response_body(db, id).await
    }

    // 1) 正常 [DONE] 收尾（OpenAI 风格）：flush 把聚合上游内容写回 response_body。
    #[tokio::test]
    async fn flush_done_writes_aggregated_body() {
        let (db, path) = flush_test_db().await;
        let state = flush_test_state(db.clone());
        let id = "flush_done_0001";
        let log = placeholder_stream_log(id);
        aidog_logs::insert_proxy_log_columns(
            &state.db,
            aidog_logs::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .insert(id.to_string(), aidog_logs::ProxyLogColumns::from_log(&log, false, false));

        let chunks = [
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: [DONE]\n\n",
        ];
        let guard = make_guard(&state, log, &chunks, 7);
        // 模拟闭包逐 chunk：末 chunk 命中 [DONE] → flush_if_done 触发 flush。
        guard.flush_if_done(chunks[1]);
        let body = await_flush_write(&state.db, id).await;
        assert_ne!(body, "", "[DONE] 收尾后 response_body 不应为空");
        assert!(body.contains("hi"), "应写回聚合上游内容: {body}");

        drop(guard);
        let _ = std::fs::remove_file(path);
    }

    // 2) Anthropic message_stop 收尾（不发 [DONE]）：旧 bug 核心场景。
    #[tokio::test]
    async fn flush_message_stop_writes_aggregated_body() {
        let (db, path) = flush_test_db().await;
        let state = flush_test_state(db.clone());
        let id = "flush_mstop_0001";
        let log = placeholder_stream_log(id);
        aidog_logs::insert_proxy_log_columns(
            &state.db,
            aidog_logs::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .insert(id.to_string(), aidog_logs::ProxyLogColumns::from_log(&log, false, false));

        // 典型 anthropic 透传尾块：message_delta + message_stop，无 [DONE]
        let tail = "event: message_delta\ndata: {\"type\":\"message_delta\"}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let chunks = ["event: message_start\ndata: {\"type\":\"message_start\"}\n\n", tail];
        let guard = make_guard(&state, log, &chunks, 11);
        // 旧实现 flush_if_done 只认 [DONE] → 此处不触发，response_body 卡占位（bug）。
        // 修复后认 message_stop → 触发 flush 确定性回写。
        guard.flush_if_done(tail);
        let body = await_flush_write(&state.db, id).await;
        assert_ne!(body, "", "message_stop 收尾后 response_body 不应为空（聚合内容未写回）");
        assert!(body.contains("message_stop"), "应写回聚合上游内容: {body}");

        drop(guard);
        let _ = std::fs::remove_file(path);
    }

    // 3) 客户端断连 / 上游无终止符：Drop 兜底仍写 response_body（已聚合内容）。
    #[tokio::test]
    async fn flush_drop_writes_partial_body() {
        let (db, path) = flush_test_db().await;
        let state = flush_test_state(db.clone());
        let id = "flush_drop_0001";
        let log = placeholder_stream_log(id);
        aidog_logs::insert_proxy_log_columns(
            &state.db,
            aidog_logs::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .insert(id.to_string(), aidog_logs::ProxyLogColumns::from_log(&log, false, false));

        // 仅有部分内容，无 [DONE]/message_stop（模拟中途断裂 / 客户端断连）。
        let chunks = ["event: message_start\ndata: {\"type\":\"message_start\"}\n\n", "data: {\"delta\":{\"text\":\"partial\"}}\n\n"];
        let guard = make_guard(&state, log, &chunks, 3);
        // 不调用 flush_if_done（无终止符）；直接 Drop 触发兜底 flush。
        drop(guard);
        let body = await_flush_write(&state.db, id).await;
        assert_ne!(body, "", "Drop 兜底后 response_body 不应为空（部分内容未写回）");
        assert!(body.contains("partial"), "Drop 应写回已聚合的部分内容: {body}");

        let _ = std::fs::remove_file(path);
    }

    // 4) 空流（上游回 200 头后秒断 / 仅心跳，零内容）：finalize 成空串，绝不留 [stream]。
    #[tokio::test]
    async fn flush_empty_stream_finalizes_to_empty_not_placeholder() {
        let (db, path) = flush_test_db().await;
        let state = flush_test_state(db.clone());
        let id = "flush_empty_0001";
        let log = placeholder_stream_log(id);
        aidog_logs::insert_proxy_log_columns(
            &state.db,
            aidog_logs::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .insert(id.to_string(), aidog_logs::ProxyLogColumns::from_log(&log, false, false));

        let guard = make_guard(&state, log, &[], 0); // 零 upstream chunk
        drop(guard); // Drop 兜底 flush
        // 空流：join_stream_body([]) == "" → 票 06 后等待 done 置位（body 初始即空串，
        // 不能再用占位差值判定 flush 是否发生）。
        for _ in 0..200 {
            let l = aidog_logs::get_proxy_log(&state.db, id).await.unwrap().unwrap();
            if l.done {
                assert_eq!(l.response_body, "", "空流 finalize 应为空串");
                let _ = std::fs::remove_file(&path);
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let _ = std::fs::remove_file(path);
        panic!("空流 flush 未置 done（finalize 未执行）");
    }

    // ── 回归复现（红，待 s2 修复）：finish.rs:279 `String::from_utf8_lossy(&chunk)` 对
    // 原始网络 chunk 独立解码 —— 多字节字符（中文/emoji）跨 chunk 边界被切断时，
    // 各自产生 U+FFFD 替换字符，原始内容不可逆丢失（压红线 2：token 计数 feed_sse_usage
    // 与转换分支下发客户端的字节，finish.rs:279/:295，均读自这份 lossy 文本）。
    // 修法（design.md）：跨 chunk 保留不完整字节序列，拼接后再解码一次，而非每 chunk 独立 lossy。
    //
    // 复现手法：把一整条合法 SSE data 行的原始字节在某个多字节字符中间切成两个网络 chunk，
    // 喂给生产函数 `Utf8ChunkReassembler::feed`（finish.rs:279 现用的同一重组器），
    // 拼接后的文本再喂给生产函数 `adapter::parse_upstream_sse`（finish.rs:295 同一调用）。
    // 断言直接锁定「不应出现 U+FFFD 且内容应等于原文」——s1 时代逐 chunk 独立 lossy 解码必红，
    // s2 把跨 chunk 字节拼接后再解码一次已转绿。
    //
    // 已知范围边界（记录，非本用例断言项）：上述复现刻意让被切字符所在整行仍在两个 chunk
    // 内各自重组为语法合法的 JSON（不影响 parse_upstream_sse 的可解析性），因此断言能精确落在
    // 「内容被 U+FFFD 替换」而非「事件被整体丢弃」。若网络 chunk 边界恰好把 SSE `data:` 行本身
    // 切成两段（而非仅切多字节字符），`parse_upstream_sse` 当前逐 chunk 调用、不做跨 chunk 行
    // 重组，会整体丢弃该行——这是另一个更严重的独立问题（内容整段消失，不止于替换字符），
    // 超出本 task 范围（design.md 仅界定 UTF-8 字节边界），已记入 needs 供 main 判断是否补建 task。
    fn split_mid_multibyte_char(line: &str, byte_offset_in_char: usize, ch: char) -> (Bytes, Bytes) {
        let bytes = line.as_bytes();
        let char_start = line.find(ch).expect("目标字符必须出现在待切分行中");
        let split_at = char_start + byte_offset_in_char;
        (
            Bytes::copy_from_slice(&bytes[..split_at]),
            Bytes::copy_from_slice(&bytes[split_at..]),
        )
    }

    /// 中文场景：三字节字符「好」（E5 A5 BD）在第 2 字节处被切成两个网络 chunk。
    #[test]
    fn utf8_char_split_across_network_chunk_corrupts_chinese_content() {
        let line = r#"data: {"choices":[{"index":0,"delta":{"content":"你好，世界"}}]}"#;
        let (chunk1, chunk2) = split_mid_multibyte_char(line, 2, '好');

        // finish.rs:279 现用的确切操作：跨 chunk 字节层重组器，逐 chunk feed。
        let mut utf8_buf = Utf8ChunkReassembler::new();
        let text1 = utf8_buf.feed(&chunk1);
        let text2 = utf8_buf.feed(&chunk2);
        let reassembled = format!("{text1}{text2}");
        let events = adapter::parse_upstream_sse(&reassembled, &Protocol::OpenAI);
        let delta_text = events.iter().find_map(|e| match e {
            ChatStreamEvent::Delta { text } => Some(text.clone()),
            _ => None,
        });

        // 期望（修复后应满足，今天必红）：跨 chunk 切断多字节字符不应产生 U+FFFD 替换字符，
        // 解析出的内容应与原始未损坏文本一致。
        assert!(
            !delta_text.as_deref().unwrap_or("").contains('\u{FFFD}'),
            "跨 chunk 切断「好」不应在解析出的内容中留下 U+FFFD 替换字符，实际: {delta_text:?}"
        );
        assert_eq!(
            delta_text.as_deref(),
            Some("你好，世界"),
            "解析出的内容应与原始未损坏文本一致"
        );
    }

    // ── s3-push-cap 回归：push_upstream/push_client 达 STREAM_BODY_MAX_BYTES 后停止累积 ──
    // 根因：旧实现直接 `up.push(chunk.clone())` 无上界，单流超大响应下 Vec<Bytes> 无界增长
    // （OOM 风险）。修复后达上限即跳过 push，vec 总字节数不再继续增长。
    #[test]
    fn push_upstream_stops_growing_past_cap() {
        let agg = StreamAggregator::new();
        // 单 chunk 4MB，喂 8 次（共 32MB）远超 16MB 上限。
        let chunk = Bytes::from(vec![b'x'; 4 * 1024 * 1024]);
        for _ in 0..8 {
            agg.push_upstream(&chunk);
        }
        let total: usize = agg.upstream_body.lock().unwrap().iter().map(|c| c.len()).sum();
        assert!(
            total <= 16 * 1024 * 1024 + 4 * 1024 * 1024,
            "累积应在上限附近停止增长（至多一个 chunk 的越界余量），实际 total={total}"
        );
        // 继续喂更多 chunk，total 不应再增长（已封顶）。
        let total_before = total;
        for _ in 0..8 {
            agg.push_upstream(&chunk);
        }
        let total_after: usize = agg.upstream_body.lock().unwrap().iter().map(|c| c.len()).sum();
        assert_eq!(total_after, total_before, "达上限后再 push 不应继续增长");
    }

    #[test]
    fn push_client_stops_growing_past_cap() {
        let agg = StreamAggregator::new();
        let chunk = Bytes::from(vec![b'y'; 4 * 1024 * 1024]);
        for _ in 0..10 {
            agg.push_client(&chunk);
        }
        let total: usize = agg.client_body.lock().unwrap().iter().map(|c| c.len()).sum();
        assert!(
            total <= 16 * 1024 * 1024 + 4 * 1024 * 1024,
            "client_body 累积也应在上限附近封顶，实际 total={total}"
        );
    }

    // ── s3-push-cap 回归：sse_line_buf remainder 超 SSE_LINE_BUF_MAX_BYTES 应丢弃而非无界增长 ──
    #[test]
    fn feed_sse_usage_remainder_capped_when_no_newline_ever_arrives() {
        let agg = StreamAggregator::new();
        // 持续喂无换行文本，模拟恶意/异常上游永不发完整行。
        let junk = "x".repeat(64 * 1024); // 64KB / 次
        for _ in 0..20 {
            // 20*64KB = 1.25MB > 1MB 上限
            agg.feed_sse_usage(&junk);
        }
        let remainder_len = agg.sse_line_buf.lock().unwrap().len();
        assert!(
            remainder_len < 1024 * 1024,
            "remainder 超上限后应被丢弃重置，不应无界增长，实际 len={remainder_len}"
        );
    }

    /// emoji 场景：四字节字符「😀」（F0 9F 98 80）在第 2 字节处被切成两个网络 chunk。
    #[test]
    fn utf8_char_split_across_network_chunk_corrupts_emoji_content() {
        let line = r#"data: {"choices":[{"index":0,"delta":{"content":"hi 😀 there"}}]}"#;
        let (chunk1, chunk2) = split_mid_multibyte_char(line, 2, '😀');

        let mut utf8_buf = Utf8ChunkReassembler::new();
        let text1 = utf8_buf.feed(&chunk1);
        let text2 = utf8_buf.feed(&chunk2);
        let reassembled = format!("{text1}{text2}");
        let events = adapter::parse_upstream_sse(&reassembled, &Protocol::OpenAI);
        let delta_text = events.iter().find_map(|e| match e {
            ChatStreamEvent::Delta { text } => Some(text.clone()),
            _ => None,
        });

        assert!(
            !delta_text.as_deref().unwrap_or("").contains('\u{FFFD}'),
            "跨 chunk 切断 emoji 不应在解析出的内容中留下 U+FFFD 替换字符，实际: {delta_text:?}"
        );
        assert_eq!(
            delta_text.as_deref(),
            Some("hi 😀 there"),
            "解析出的内容应与原始未损坏文本一致"
        );
    }

    // ── 红：内容路径逐 chunk 独立调用 parse_upstream_sse，无跨 chunk 行重组（design.md 定性）──
    // 根因：finish.rs 转换分支 utf8_buf.feed(&chunk) 只补 UTF-8 字节层（s2-utf8-fix 已修），
    // 拿到的文本仍逐 chunk 独立喂给 `adapter::parse_upstream_sse`（无状态、按行分帧）。
    // 一条 SSE `data:` 事件行若被网络 chunk 边界切成两半：前半没有结束换行、后半没有 `data:`
    // 前缀，两边都不构成合法帧——**整行内容被双双丢弃**，客户端静默少内容，无任何错误信号。
    // usage 侧早有同型重组（`feed_sse_usage` + `sse_line_buf`），本 task 要把这套 idiom
    // 补给内容路径；本 subtask 只写复现用例、不修（修在 s2-reassemble）。
    //
    // 复现手法照抄 `feed_sse_usage_reassembles_split_chunk_boundary`：取一条真实 SSE 行，
    // 按字节位切两半分别喂给生产函数链，断言指向具体内容缺失（切分后解析 None vs 不切分解析
    // Some(text)，逐字节对照），而非笼统 assert_ne。

    /// 复刻 finish.rs 转换分支现状：逐 chunk 先过 `Utf8ChunkReassembler`（字节层）、再过
    /// `SseLineReassembler`（行层，s2-reassemble 生产代码），再对重组出的完整行文本调用
    /// `adapter::parse_upstream_sse`——与 finish.rs 实际调用链完全一致（同一批生产函数，
    /// 非重新实现一遍逻辑）。s1 时代（无行层重组）此函数必红，s2 接上行层重组后转绿。
    fn naive_per_chunk_parse(chunks: &[&[u8]], wire: &Protocol) -> Vec<ChatStreamEvent> {
        let mut utf8_buf = Utf8ChunkReassembler::new();
        let mut line_buf = SseLineReassembler::new();
        let mut events = Vec::new();
        for chunk in chunks {
            let text = utf8_buf.feed(chunk);
            let line_ready = line_buf.feed(&text);
            events.extend(adapter::parse_upstream_sse(&line_ready, wire));
        }
        events
    }

    fn delta_text_of(events: &[ChatStreamEvent]) -> Option<String> {
        events.iter().find_map(|e| match e {
            ChatStreamEvent::Delta { text } => Some(text.clone()),
            _ => None,
        })
    }

    /// 切法一：切在 `data: ` 前缀中间（"da" | "ta: {...}"）。
    #[test]
    fn content_lost_when_sse_line_split_mid_data_prefix() {
        let line = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hello world\"}}]}\n\n";
        let split_at = 2; // 落在 "data: " 前缀（6 字节）中间，非边界
        let (chunk1, chunk2) = line.as_bytes().split_at(split_at);

        let reference = adapter::parse_upstream_sse(line, &Protocol::OpenAI);
        let reference_text = delta_text_of(&reference);
        assert_eq!(
            reference_text.as_deref(),
            Some("hello world"),
            "参照（不切分）解析必须先拿到完整内容，否则用例构造有误"
        );

        let naive = naive_per_chunk_parse(&[chunk1, chunk2], &Protocol::OpenAI);
        let naive_text = delta_text_of(&naive);
        // 期望（修复后应满足，今天必红）：跨 chunk 切断 data: 前缀不应丢内容，逐 chunk 解析结果
        // 应与不切分的参照解析逐字节一致。今天的实现（无行层重组）两半都不成帧、整行丢失，
        // naive_text 会是 None，与 reference_text=Some("hello world") 不等，断言失败（红）。
        assert_eq!(
            naive_text, reference_text,
            "切在 data: 前缀中间：内容不应丢失，应与不切分参照逐字节一致；实际 {naive_text:?}，\
             今天必红（前半无结束换行、后半无 data: 前缀，双双不成帧被静默丢弃）"
        );
    }

    /// 切法二：切在 JSON body 中间（`"content":"hel` | `lo world"}}]}`）。
    #[test]
    fn content_lost_when_sse_line_split_mid_json_body() {
        let line = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hello world\"}}]}\n\n";
        let split_at = line.find("hel").expect("待切分行必须包含目标子串") + 3;
        let (chunk1, chunk2) = line.as_bytes().split_at(split_at);

        let reference = adapter::parse_upstream_sse(line, &Protocol::OpenAI);
        let reference_text = delta_text_of(&reference);
        assert_eq!(
            reference_text.as_deref(),
            Some("hello world"),
            "参照（不切分）解析必须先拿到完整内容，否则用例构造有误"
        );

        let naive = naive_per_chunk_parse(&[chunk1, chunk2], &Protocol::OpenAI);
        let naive_text = delta_text_of(&naive);
        // 期望（修复后应满足，今天必红）：跨 chunk 切断 JSON body 不应丢内容，逐 chunk 解析结果
        // 应与不切分的参照解析逐字节一致。今天的实现两半各自解析失败被静默丢弃，naive_text 会是
        // None，与 reference_text=Some("hello world") 不等，断言失败（红）。
        assert_eq!(
            naive_text, reference_text,
            "切在 JSON body 中间：内容不应丢失，应与不切分参照逐字节一致；实际 {naive_text:?}，\
             今天必红（半截 JSON 各自解析失败被静默丢弃）"
        );
    }

    // ── s3-bound：内容路径 SseLineReassembler 自身上界（与 usage 侧 feed_sse_usage 同 SSE_LINE_BUF_MAX_BYTES 口径）──
    // 持续无换行喂入，模拟异常/恶意上游永不发完整行：buf 不应无界增长，应在超上限后丢弃重置（不 panic）。
    #[test]
    fn sse_line_reassembler_buf_capped_when_no_newline_ever_arrives() {
        let mut line_buf = SseLineReassembler::new();
        let junk = "x".repeat(64 * 1024); // 64KB / 次
        for _ in 0..20 {
            // 20*64KB = 1.25MB > 1MB 上限（SSE_LINE_BUF_MAX_BYTES），全程不应 panic。
            let ready = line_buf.feed(&junk);
            assert!(ready.is_empty(), "无换行时不应有完整行可下发");
        }
        // test_stream 是 stream.rs 的子模块（#[path] 内嵌），可直接访问私有字段 buf，
        // 直接断言内部状态有界（不应无界增长到 20*64KB=1.25MB）。
        assert!(
            line_buf.buf.len() < 1024 * 1024,
            "buf 超上限后应被丢弃重置，不应无界增长，实际 len={}",
            line_buf.buf.len()
        );
    }

    // ── 红线 1 钉子：完整行必须随本次 feed 立即下发，禁攒批 ──
    // 只留尾巴（残行）不完整才等下一个 chunk；已完整的行不能因为同一 chunk 里还带着残行
    // 就被一并压住不吐——那等于把行重组做成了「攒够再吐」，首 token 时延随缓冲深度退化。
    #[test]
    fn sse_line_reassembler_delivers_complete_line_immediately_without_batching() {
        let complete_line =
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hello world\"}}]}\n";
        let partial_next_line = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"par"; // 无结束换行，故意残
        let chunk = format!("{complete_line}{partial_next_line}");

        let mut line_buf = SseLineReassembler::new();
        let ready = line_buf.feed(&chunk);

        // 完整行必须已经在本次 feed 的返回值里，可直接解析出内容——不依赖下一次 feed。
        let events = adapter::parse_upstream_sse(&ready, &Protocol::OpenAI);
        let delta_text = delta_text_of(&events);
        assert_eq!(
            delta_text.as_deref(),
            Some("hello world"),
            "完整行应随本次 feed 立即可下发，不应攒批等待残行凑齐"
        );
        // 残行不应混入本次可下发文本——否则等于把不完整帧提前下发。
        assert!(
            !ready.contains("par"),
            "残行不应被提前下发，应留在内部 buf 里等下次 feed 拼接: {ready:?}"
        );
    }

/// 下发 model 对齐钉子：SSE 透传行内 model 改写 + JSON 已对齐早退。
#[test]
fn replace_model_aligns_to_requested() {
    // SSE：完整行（含跨帧重复出现）内 model 值全部改写为客户端请求名
    let sse = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"model\":\"glm-5\",\"role\":\"assistant\"}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\"}}\n\n",
    );
    let out = replace_model_in_sse_text(sse, "claude-sonnet-4-5");
    assert_eq!(out.matches("\"model\":\"claude-sonnet-4-5\"").count(), 1);
    assert!(!out.contains("glm-5"));
    // 无 model 字段的行原样保留
    assert!(out.contains("text_delta"));

    // JSON：model 不一致时改写；一致时原字节返回（免重序列化）
    let bytes = br#"{"id":"x","model":"upstream-name","content":[]}"#;
    let out = replace_model_in_json(bytes, "requested-name");
    assert!(String::from_utf8(out).unwrap().contains("\"model\":\"requested-name\""));
    let aligned = br#"{"id":"x","model":"requested-name"}"#;
    assert_eq!(replace_model_in_json(aligned, "requested-name"), aligned.to_vec());
}

// ── 流终态判定：flush 的 status_code 来源（修复前恒 200，断流被谎报成功）──────
#[test]
fn end_status_upstream_error_is_502() {
    let agg = StreamAggregator::new();
    agg.mark_upstream_err();
    assert_eq!(agg.end_status_code(), 502);
}

#[test]
fn end_status_exhausted_is_200() {
    // 上游流自然读完但无 [DONE]/message_stop（如 Gemini streamGenerateContent）→ 仍算成功。
    let agg = StreamAggregator::new();
    agg.mark_exhausted();
    assert_eq!(agg.end_status_code(), 200);
}

#[test]
fn end_status_neither_is_499() {
    // 流没读完也没报错就被 Drop = 客户端提前断连。
    let agg = StreamAggregator::new();
    assert_eq!(agg.end_status_code(), 499);
}

#[test]
fn end_status_upstream_error_wins_over_exhausted() {
    // 报错后流也会随即耗尽（Err 之后 poll 返 None）；错误优先，禁被 exhausted 覆盖成 200。
    let agg = StreamAggregator::new();
    agg.mark_upstream_err();
    agg.mark_exhausted();
    assert_eq!(agg.end_status_code(), 502);
}
