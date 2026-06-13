# Research: 流式日志具体缺口（缺什么）

- **Query**: 流式响应 body / token / est_cost 在日志中是否完整
- **Scope**: internal
- **Date**: 2026-06-13

## Findings：流式 vs 非流式逐项对比

| 项 | 非流式（`proxy.rs:830-879`） | 流式（`proxy.rs:881-1044`） | 缺口 |
|---|---|---|---|
| `response_body`（上游响应体）| 完整 JSON（`:835`）| 硬编码 `"[stream]"`（`:1025`），**SSE chunk 全部丢弃，不聚合** | **缺：流式响应内容完全未记录** |
| `user_response_body`（返回用户）| 完整（含 model 替换）（`:848`）| 硬编码 `"[stream]"`（`:1026`）| **缺：同上** |
| `input/output/cache_tokens` | `extract_usage` 同步取（`:833,838-840`）| 闭包内累计，`[DONE]` 时异步回写（`:947-949, 961`）| 部分：依赖上游发 usage chunk + `[DONE]`；客户端断连/无 `[DONE]` → token=0 |
| `est_cost` | `upsert_log` 内按 token 算（`:323-342`）| 同机制，但仅在 `[DONE]` 回写的 upsert 内才有非 0 token → est_cost 才非 0 | 部分：token 缺失时 est_cost=0 |
| `status_code` | 200（`:836`）| `[DONE]` 回写设 200（`:950`），返回前 upsert 也设 200（`:1024）| OK |
| `duration_ms` | 准确（`:837`）| 返回前 upsert 仅到「开始流」时刻；`[DONE]` 回写才是真实流结束时长（`:951`）| 部分：依赖回写触发 |

### 核心缺口

1. **流式响应 body 被丢弃**：`response_body` / `user_response_body` 永远是 `"[stream]"`（`proxy.rs:1025-1026`）。闭包 `output` 仅用于转发给客户端（`Body::from_stream`，`:1021`），**从未累计回 log**。前端因此显示「(流式响应，内容未记录)」（`src/pages/Logs.tsx:229, 234`）。
2. **token 依赖两个前提**：① 上游在 SSE 中发 `usage`；② 上游发 `data: [DONE]` 触发回写。Anthropic 的 usage 在 `message_start.message.usage`(input) + `message_delta.usage`(output)，但 `proxy.rs:982-996` 的累计**只读顶层 `json.get("usage")`**，未读 `message.usage`（对比透传分支 `accumulate_sse_usage` `proxy.rs:1404-1406` 有 `.or_else(message.usage)` 兜底）→ **主转换分支对 Anthropic 流式 input_tokens 可能漏读**。
3. **客户端断连 / 上游无 `[DONE]`**：`[DONE]` 回写（`proxy.rs:944` `est_fired` 守卫）不触发 → token 停在返回前 upsert 的 0，est_cost=0，duration 偏短。

### 透传（ClaudeCode）分支同样缺 body

`proxy.rs:1353-1354`：流式透传也写 `response_body = "[stream]"`，token 用 `accumulate_sse_usage`（含 `message.usage` 兜底，比主分支健壮），但 **无 `[DONE]` 回写机制** —— token 在返回前 upsert（`:1356-1358`）一次性写，此刻流尚未消费 → **透传流式 token 几乎总是 0**（更严重）。

## Caveats / Not Found

- 「缺什么」结论：**流式响应体内容（response_body / user_response_body）100% 缺失**（设计如此）；流式 token/est_cost/duration **条件性缺失**（依赖 usage chunk + [DONE] + 不断连）；透传流式 token 基本缺失（无回写）；主分支 Anthropic input_tokens 可能漏读（usage 路径不全）。
