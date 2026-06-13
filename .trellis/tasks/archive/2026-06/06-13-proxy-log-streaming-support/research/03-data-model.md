# Research: proxy_logs 数据模型 + 写日志函数 + SSE 解析

- **Query**: proxy_log 表结构 / 流式标记 / usage 字段 / 写日志 fn / converter SSE 解析
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### Files Found

| File Path | Description |
|---|---|
| `src-tauri/src/gateway/models.rs:652` | `ProxyLog` struct（27 字段）|
| `src-tauri/src/gateway/db.rs:996` | `PROXY_LOG_COLUMNS` 全列序常量 |
| `src-tauri/src/gateway/db.rs:1034` | `upsert_proxy_log` 写日志 fn |
| `src-tauri/migrations/001_init.sql:68` | `CREATE TABLE proxy_log` DDL |
| `src-tauri/src/gateway/adapter/converter.rs:50` | `parse_sse` / `:60` `parse_incoming_request` / `:72` `to_client_sse` |

### ProxyLog 表结构相关字段（`models.rs:653-707`）

```rust
pub response_body: String,        // 注释明示：流式为 "[stream]"（:674-675）
pub user_response_body: String,   // 注释明示：流式为 "[stream]"（:691-693）
pub input_tokens: i32,            // :696
pub output_tokens: i32,           // :697
pub cache_tokens: i32,            // :698
pub status_code: i32,             // :694
pub est_cost: f64,                // :700-701
```

**无 `is_stream` 字段** —— 表/struct 均无流式标记列。当前靠 `response_body == "[stream]"` 哨兵字符串隐式区分（前端 `src/pages/Logs.tsx:123, 138, 228` 据此判定）。

### 写日志函数签名

```rust
// proxy.rs:305 —— 业务层包装（设置过滤 + est_cost 计算 + emit 事件）
async fn upsert_log(state: &Arc<ProxyState>, log: &ProxyLog, settings: &ProxyLogSettings)

// db.rs:1034 —— DB 层（INSERT OR REPLACE，27 列全写）
pub async fn upsert_proxy_log(db: &Db, log: &ProxyLog) -> Result<(), String>
```

`row_to_proxy_log`（`db.rs:1001`）列序须与 `PROXY_LOG_COLUMNS`（`db.rs:997`）一致 —— **新增列须同步改 3 处：DDL/migration、PROXY_LOG_COLUMNS、row_to_proxy_log、upsert 的 VALUES + params（`db.rs:1039-1041`）**。

`ProxyLogSummary`（`models.rs:729`）不含 body 字段，列表查询不取 body（`db.rs:1049` list / `:1082` filtered）；详情走 `get_proxy_log`（`db.rs:1174`）取全列。

### SSE 解析（converter.rs）

- `parse_sse(data, wire_protocol)`（`converter.rs:50`）：按协议分派 → `parse_anthropic_sse`（`anthropic.rs:100`）/ `parse_openai_sse`（`openai.rs:211`）/ `parse_gemini_sse`。返回 `Option<ChatStreamEvent>`。
- `ChatStreamEvent`（`types.rs:132`）变体：`Start{id,model}` / `Delta{text}` / `ToolDelta{...}` / `Stop{finish_reason}` / `Usage{usage}`。
- **关键**：`parse_anthropic_sse`（`anthropic.rs:100-150`）**不解析 usage** —— `message_start` 只取 id/model（`:105-108`），`message_delta` 只取 stop_reason（`:138-143`），usage 字段被忽略。`parse_openai_sse`（`openai.rs:211-259`）同样不返回 usage。`ChatStreamEvent::Usage` 变体存在但**无解析器产出它**（`to_*_sse` 中 `Usage{..} => None`，`converter.rs:163` / `openai.rs:449`）。
- 故主流式分支的 token 累计**绕过 parse_sse**，在 `proxy.rs:982-996` 直接读裸 JSON 的 `usage`（只读顶层，见 02 缺口 #2）。
- `parse_incoming_request`（`converter.rs:60`）：按 source_protocol 解析入站为 `ChatRequest`；`ChatRequest.stream` 字段即 `is_stream` 来源（`proxy.rs:605`）。

## Caveats / Not Found

- 无任何「流式标记」持久化字段；若修复需可查询区分流式日志，需新增列（见 04）。
- `Usage` 单独 SSE 事件类型（OpenAI `stream_options.include_usage` 末尾 usage-only chunk）当前**无解析路径**进入 ChatStreamEvent，但裸 JSON 累计（`proxy.rs:983`）能捕获顶层 `usage`。
