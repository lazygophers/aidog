# perf-oom-cpu-optimize — 审计收敛

静态审计(aidog-perf-audit), 无运行时 profile 基线。结论标 `推测:` 或代码实证。

## OOM 根因(按影响)
1. **StreamAggregator 累积 512MB/流式请求** — `proxy/stream.rs:5` `STREAM_BODY_MAX_BYTES=512*1024*1024`; `record_upstream_body = log_settings.enabled`(默认 true)→ 默认每流式请求累积上游 + 客户端 SSE 双份; N 并发 × 512MB。**极高**
2. **flush join 翻倍峰值** — `join_stream_body`(stream.rs:9-30)`Vec::with_capacity(512MB)` + `String::from_utf8_lossy().into_owned()` = 512MB + 512MB String。**高**
3. **log_snapshots 持完整 body** — `ProxyLogColumns` 5 body String 随快照存 in-flight HashMap(`proxy/mod.rs:144`, `db/proxy_log.rs:79-112`)。**高**
4. **body 三份拷贝** — `handler.rs:200-212` `to_bytes(10MB)` + `String::from_utf8_lossy().to_string()` + `serde_json::from_slice<Value>`。**中**
5. agg_done FIFO 8192 entries ≈ 512KB, 有界, 影响小。**低**

## CPU 根因(按影响)
1. **emit 风暴** — `upsert_log`(proxy/mod.rs:147 注释: 单请求 40+ 次)`write_ok` 分支无条件 emit `proxy-log-updated` + `tray-refresh`(`log.rs:154-156`); tray-refresh listener(`app_setup.rs:391-413`)50ms debounce → 每 50ms 完整重建菜单(build_menu + 多 SQL + set_menu/title/icon 原生)。**高**
2. **get_platform 重复查询** — `log.rs:38-63` upsert_log 内 `est_cost==0 && tokens>0` 每次调 `get_platform` + `calc_est_cost`, per request 40+ 次。**中-高**
3. **log_snapshots 锁竞争 + MB clone** — 单 Mutex 共享, 每次三拿锁 + 5 body String clone。**中-高**
4. **feed_sse 每 chunk 全 split + String 化** — `stream.rs:63-89` `split('\n').map(to_string).collect<Vec<String>>()`。**中**
5. **proxy-log-updated 广播引发多页并发重拉** — Home/Stats/Logs/Groups/Popover 各 listener debounce 后 reload。**中**

## 推测触发场景(需用户核实, 但按静态直接修高置信项)
- 推测: 高 CPU 主因 = 持续密集 LLM 请求(Claude Code / 多 agent 并发)触发 emit 风暴 + DB 单写连接排队 + 全局锁竞争
- 推测: OOM 主因 = 流式长响应累积(长上下文会话), 多并发放大
- 用户确认: 无运行时数据, 直接修高置信项

## 关键证据文件
- `gateway/proxy/stream.rs`(MAX_BYTES, join, feed_sse)
- `gateway/proxy/log.rs`(upsert_log emit, get_platform 重复)
- `gateway/proxy/finish.rs`(StreamAggregator flush:232-306)
- `gateway/proxy/handler.rs`(body 三份拷贝:200-212)
- `gateway/proxy/mod.rs`(log_snapshots Mutex:144, agg_done:154,163)
- `gateway/db/proxy_log.rs`(ProxyLogColumns:79-112, changed_since)
- `src-tauri/src/app_setup.rs`(tray-refresh listener:391-413)
