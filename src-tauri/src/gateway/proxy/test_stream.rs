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
    async fn flush_test_db() -> (Arc<super::super::db::Db>, std::path::PathBuf) {
        use std::sync::atomic::AtomicU64;
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let mut path = std::env::temp_dir();
        let uniq = format!(
            "aidog_flush_test_{}_{}_{}.db",
            std::process::id(),
            super::super::db::now(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        path.push(uniq);
        let db = super::super::db::Db::new(path.to_str().unwrap())
            .await
            .expect("open temp db");
        db.init_tables().await.expect("init tables");
        (Arc::new(db), path)
    }

    fn flush_test_state(db: Arc<super::super::db::Db>) -> Arc<ProxyState> {
        Arc::new(ProxyState {
            db,
            app: None,
            middleware: Arc::new(MiddlewareEngine::new()),
            scheduler: Arc::new(super::super::scheduling::SchedulerState::new()),
            sticky: Arc::new(super::super::scheduling::StickyTable::new()),
            log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
            agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
        })
    }

    fn placeholder_stream_log(id: &str) -> ProxyLog {
        let ts = super::super::db::now();
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
            response_body: "[stream]".to_string(),
            request_url: String::new(),
            upstream_request_url: String::new(),
            upstream_response_headers: String::new(),
            upstream_status_code: 200,
            user_response_headers: String::new(),
            user_response_body: "[stream]".to_string(),
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
        }
    }

    /// 建一个 StreamLogGuard，settings = 默认（enabled=true, log_user_request=false）。
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
            settings: ProxyLogSettings::default(),
            start: std::time::Instant::now(),
            record_upstream_body: true, // = log_settings.enabled
            record_client_body: false,  // log_user_request=false
            req_span: tracing::Span::current(),
            est: None,
        }
    }

    async fn read_response_body(db: &super::super::db::Db, id: &str) -> String {
        super::super::db::get_proxy_log(db, id)
            .await
            .expect("get log")
            .expect("row exists")
            .response_body
    }

    /// 等待 flush 内 tokio::spawn 的落库任务完成（短轮询，最多 ~2s）。
    async fn await_flush_write(db: &super::super::db::Db, id: &str) -> String {
        for _ in 0..200 {
            let body = read_response_body(db, id).await;
            if body != "[stream]" {
                return body;
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
        super::super::db::insert_proxy_log_columns(
            &state.db,
            super::super::db::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .lock()
            .unwrap()
            .insert(id.to_string(), super::super::db::ProxyLogColumns::from_log(&log, false, false));

        let chunks = [
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: [DONE]\n\n",
        ];
        let guard = make_guard(&state, log, &chunks, 7);
        // 模拟闭包逐 chunk：末 chunk 命中 [DONE] → flush_if_done 触发 flush。
        guard.flush_if_done(chunks[1]);
        let body = await_flush_write(&state.db, id).await;
        assert_ne!(body, "[stream]", "[DONE] 收尾后 response_body 不应停在占位");
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
        super::super::db::insert_proxy_log_columns(
            &state.db,
            super::super::db::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .lock()
            .unwrap()
            .insert(id.to_string(), super::super::db::ProxyLogColumns::from_log(&log, false, false));

        // 典型 anthropic 透传尾块：message_delta + message_stop，无 [DONE]
        let tail = "event: message_delta\ndata: {\"type\":\"message_delta\"}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let chunks = ["event: message_start\ndata: {\"type\":\"message_start\"}\n\n", tail];
        let guard = make_guard(&state, log, &chunks, 11);
        // 旧实现 flush_if_done 只认 [DONE] → 此处不触发，response_body 卡占位（bug）。
        // 修复后认 message_stop → 触发 flush 确定性回写。
        guard.flush_if_done(tail);
        let body = await_flush_write(&state.db, id).await;
        assert_ne!(body, "[stream]", "message_stop 收尾后 response_body 不应停在占位（核心 bug）");
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
        super::super::db::insert_proxy_log_columns(
            &state.db,
            super::super::db::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .lock()
            .unwrap()
            .insert(id.to_string(), super::super::db::ProxyLogColumns::from_log(&log, false, false));

        // 仅有部分内容，无 [DONE]/message_stop（模拟中途断裂 / 客户端断连）。
        let chunks = ["event: message_start\ndata: {\"type\":\"message_start\"}\n\n", "data: {\"delta\":{\"text\":\"partial\"}}\n\n"];
        let guard = make_guard(&state, log, &chunks, 3);
        // 不调用 flush_if_done（无终止符）；直接 Drop 触发兜底 flush。
        drop(guard);
        let body = await_flush_write(&state.db, id).await;
        assert_ne!(body, "[stream]", "Drop 兜底后 response_body 不应停在占位");
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
        super::super::db::insert_proxy_log_columns(
            &state.db,
            super::super::db::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .lock()
            .unwrap()
            .insert(id.to_string(), super::super::db::ProxyLogColumns::from_log(&log, false, false));

        let guard = make_guard(&state, log, &[], 0); // 零 upstream chunk
        drop(guard); // Drop 兜底 flush
        // 空流：join_stream_body([]) == "" → response_body 应被改写成空串而非占位。
        for _ in 0..200 {
            let body = read_response_body(&state.db, id).await;
            if body != "[stream]" {
                assert_eq!(body, "", "空流 finalize 应为空串");
                let _ = std::fs::remove_file(&path);
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let _ = std::fs::remove_file(path);
        panic!("空流 response_body 仍停在 [stream] 占位（finalize 未执行）");
    }
