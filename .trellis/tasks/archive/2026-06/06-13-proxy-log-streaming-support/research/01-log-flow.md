# Research: proxy.rs 完整日志记录链路（流式 vs 非流式）

- **Query**: 代理请求日志对流式请求/响应的兼容缺口 — 日志链路调查
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### 渐进式日志：upsert 阶段（同一 `log.id` = trace_id，`INSERT OR REPLACE` 覆盖）

主处理函数 `handle_proxy_inner`（`src-tauri/src/gateway/proxy.rs:450`）。每阶段即时 `upsert_log`，用 `request_id` 串联：

| 阶段 | 行 | 写入字段 |
|---|---|---|
| #1 请求收到 | `proxy.rs:551` | request_headers / request_body / request_url / model |
| #2 分组解析 | `proxy.rs:581` | group_name / source_protocol |
| #3 路由解析 | `proxy.rs:657` | actual_model / target_protocol / platform_id |
| 上游请求前 | （随 #3 后）`proxy.rs:777-780` | upstream_request_headers / upstream_request_body / upstream_request_url |
| 上游响应头 | `proxy.rs:799-809` | upstream_status_code / upstream_response_headers |
| **完成（非流式）** | `proxy.rs:856` | response_body / status_code / duration_ms / input/output/cache_tokens / user_response_body |
| **完成（流式，返回前）** | `proxy.rs:1032` | response_body=`"[stream]"` / user_response_body=`"[stream]"` / token=0（此刻流未消费）|
| **流式 token 回写（[DONE] 时）** | `proxy.rs:961-963` | 仅 token + status_code + duration_ms，**response_body 仍为 `[stream]`** |

`upsert_log`（`proxy.rs:305`）：受 `ProxyLogSettings` 控制清字段；`est_cost` 在此按 token best-effort 计算（`proxy.rs:323-342`）。

### 流式 vs 非流式分支判定

- 主路：`let is_stream = chat_req.stream.unwrap_or(false);`（`proxy.rs:605`）—— **依据请求体 `stream` 字段**。
- 非流式分支：`if !is_stream { ... }`（`proxy.rs:830-879`）—— `resp.bytes().await` 读全 body → `extract_usage` → 记 `response_body` / `user_response_body` 完整。
- 流式分支：`proxy.rs:881-1044` —— `resp.bytes_stream().map(...)` 边转边发，闭包内逐行 `strip_prefix("data: ")` 解析 SSE。
- 透传（ClaudeCode）分支独立判定：按响应头 `content-type: text/event-stream` 或 `transfer-encoding: chunked`（`proxy.rs:1290-1296`），非流式 `proxy.rs:1301-1319` / 流式 `proxy.rs:1321-1363`。

### 流式闭包关键代码（`proxy.rs:925-1019`）

```rust
let stream = resp.bytes_stream().map(move |chunk_result| {
    let chunk = ...;
    let text = String::from_utf8_lossy(&chunk);
    let mut output = String::new();
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim() == "[DONE]" {
                // 触发 Stop event + 一次性回写最终 token + spawn_estimate（:944-978）
                continue;
            }
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                if let Some(usage) = json.get("usage") { /* acc token :983-996 */ }
                if let Some(event) = adapter::parse_sse(&json, &protocol) {
                    // 转换为客户端 SSE 追加到 output（:998-1013）
                }
            }
        }
    }
    Ok(output)   // ← 仅返回转换后的 SSE，response_body 从未在此累计
});
```

`grep` 命中（stream / sse / chunk / [DONE]）集中在 `proxy.rs:605, 830, 881-1044, 1290-1363`，无遗漏分支。

## Caveats / Not Found

- 流式分支 `[DONE]` 回写在 **stream 闭包内**（同步上下文不可 await），用 `tokio::spawn` fire-and-forget（`proxy.rs:961`）。若客户端提前断连或上游不发 `[DONE]`，回写不触发 → token 停留在返回前 upsert 的 0 值。
- 闭包由 axum 在 req span 外轮询，故显式 clone `req_span` 进闭包保留 trace_id（`proxy.rs:923, 963, 976`）。
