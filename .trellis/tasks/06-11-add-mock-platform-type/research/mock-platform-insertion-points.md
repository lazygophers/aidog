# Research: Mock 平台类型实现插入点 + 现有模式

- **Query**: 在 aidog 添加 mock 平台类型（本地生成假响应，按入站协议返对应格式 + SSE + 假 token）的全部实现插入点 + 现有模式
- **Scope**: internal
- **Date**: 2026-06-11

## 结论摘要（先读这段）

- **拦截点**: `proxy.rs:386` `req_builder.send().await` 之前。最干净的插入位置是 `proxy.rs:340` 协议转换之后、`proxy.rs:342` 之前/之间——判断 `route.platform.platform_type == Protocol::Mock` 则跳过 reqwest，直接走本地 mock 生成分支，构造 `Response` 并 `upsert_log` 后返回。
- **响应复用**: 非流式无现成 builder（proxy 当前是上游 JSON **透传**，不回转协议）。流式有现成 builder：`adapter::to_client_sse(event, source_protocol, model)`（converter.rs:72）已能按 anthropic/openai/gemini 输出 SSE，mock 可直接复用，只需自己造 `ChatStreamEvent` 序列（Start→Delta→Stop）。**非流式响应需新增 3 个 builder**（anthropic/openai/gemini 的完整 JSON body）。
- **配置载体**: 推荐用现有 `platform.extra`（TEXT JSON 列，已存在、已读写、无需迁移、前端目前未占用）存 mock 场景配置。理由见下文第 6 项。
- **改动规模估计**: 中等。后端 ~5 文件（models.rs / proxy.rs / 新增 adapter/mock.rs / router.rs 极小或无 / db.rs 无需改），前端 ~2 文件（api.ts + Platforms.tsx，外加 mock 配置编辑 UI）。无 DB schema 变更。

---

## Findings

### 1. Protocol enum 添加点

| 位置 | file:line | 现状 | 插入建议 |
|---|---|---|---|
| Rust enum 定义 | `src-tauri/src/gateway/models.rs:5-129` | `Protocol` enum，每变体带 `#[serde(rename = "...")]`。AI 请求协议（anthropic/openai/.../gemini）在前，平台类型在后 | 新增 `#[serde(rename = "mock")] Mock,`。归类于「平台类型」区（17 行注释之后），因为 mock 是平台主类型而非 endpoint 协议 |
| `default_for_protocol` | `models.rs:216-222` | `match` 仅处理 Anthropic / OpenAI 系，其余落 `_ => ClientType::Default` | **无需改**，`_` 已覆盖。mock 不发真实请求，client_type 无意义 |
| `ClientType` enum | `models.rs:178-208` | 客户端模拟类型 | **无需改**，mock 不模拟客户端 header |
| 前端 Protocol union | `src/services/api.ts:5-20` | TS union，与 Rust enum 一一对应（serde rename 值） | 在「平台类型」段加 `\| "mock"` |
| 前端 PROTOCOLS 数组 | `src/pages/Platforms.tsx:10-76`（`PROTOCOLS: ProtocolOption[]`） | 平台下拉选项：`{ value, label, codingPlan?, keywords? }`。带分组注释 | 加一项 `{ value: "mock", label: "Mock（本地模拟）", keywords: ["mock","测试","调试","假数据"] }` |
| 前端 ENDPOINT_PROTOCOLS | `Platforms.tsx:77`（`ENDPOINT_PROTOCOLS`） | endpoint 协议子集（仅 AI 请求协议） | **不加** mock（mock 不是 endpoint 协议） |
| 前端默认 base_url 预设 | `Platforms.tsx:136` `getDefaultEndpoints(protocol, codingPlan)` | 按 protocol 返回默认 endpoints（base_url 预设） | mock 应返回**空 endpoints**（无真实上游）；UI 需隐藏 base_url/api_key 必填校验（见第 5 项） |
| protocol→label/icon 映射 | 无独立 icon 映射 | label 即 `PROTOCOLS[].label`；无 icon 字段 | 只需补 label |

注：Rust enum 序列化为带引号 JSON 字面量存 `platform_type` 列（`db.rs:102` `serde_json::to_string`），读用 `from_str`（`db.rs:86`）。加变体即自动支持，**db.rs 无需改**。

### 2. proxy.rs 转发链路 + 拦截点

完整流程（函数 `handle_proxy` `proxy.rs:127-553`）：

1. `proxy.rs:131-172` 初始化计时 + request_id + ProxyLog 空壳
2. `proxy.rs:174-198` 捕获请求头 / auth / path / url
3. `proxy.rs:200-222` 读 body → 提取 model → upsert #1
4. `proxy.rs:224-263` `resolve_group` 找分组 → upsert #2，`detect_source_protocol` 定 source_protocol
5. `proxy.rs:265-289` `adapter::parse_incoming_request(source_protocol, ...)` 解析为 `ChatRequest`；`is_stream = chat_req.stream`
6. `proxy.rs:291-335` `select_platform` 路由 → 匹配 endpoint 协议 → 得 `target_protocol_enum / target_base_url / client_type / coding_plan` → upsert #3
7. **`proxy.rs:340-354` 协议转换 + 构建 URL**：`adapter::convert_request(&chat_req, target_protocol_enum, platform_protocol)` → `(req_body, api_path)`；`url = base_url + api_path`
8. `proxy.rs:357-378` 解析超时 + 构建 reqwest client + 上游 header
9. **`proxy.rs:386` `req_builder.send().await` ← 真正发往上游**
10. `proxy.rs:399-455` 非流式：`resp.bytes()` → `extract_usage` → 透传 JSON（含 model 回写 `proxy.rs:439`）
11. `proxy.rs:457-552` 流式：`resp.bytes_stream()` → 逐行解析 `data:` → 累计 usage → `parse_sse` → `to_client_sse` → SSE 输出

**精确拦截插入点**：在 `proxy.rs:340`（协议转换）之后判定 `route.platform.platform_type == Protocol::Mock`：

```text
// 伪代码，插在 proxy.rs:341-342 附近（已拿到 chat_req / is_stream / source_protocol / requested_model / route）
if matches!(route.platform.platform_type, Protocol::Mock) {
    // 1. 解析 mock 配置（route.platform.extra）
    // 2. 模拟延迟 tokio::time::sleep
    // 3. 若配置错误码/429/超时 → 直接造对应 status Response + upsert_log
    // 4. is_stream → 用 to_client_sse 造 SSE body；else → 造非流式 JSON body
    // 5. 填假 token 到 log.input_tokens/output_tokens/cache_tokens
    // 6. upsert_log 后 return Response
}
```

放在 340 之后即可拿到全部上下文（`source_protocol` `proxy.rs:261`、`is_stream` `proxy.rs:287`、`requested_model` `proxy.rs:288`、`route` `proxy.rs:305`），且不构建无用的真实 URL/client。也可更早放在 `proxy.rs:336`（替换模型名）之前，差异极小。

mock 走**自己的 builder**（reqwest client `proxy.rs:363` 和上游 header `proxy.rs:370` 对 mock 无意义，跳过）。

### 3. converter.rs 响应格式 + 复用点

| 能力 | file:line | 现状 | mock 复用 |
|---|---|---|---|
| 流式 SSE 输出 | `adapter/converter.rs:72-79` `to_client_sse(event, source_protocol, model)` | 按 source_protocol 分派：openai 系→`openai::to_openai_sse`；gemini→`gemini::to_gemini_sse`；默认 anthropic→`to_anthropic_sse` | **直接复用**。mock 造 `ChatStreamEvent` 序列即可得三协议 SSE |
| anthropic SSE 分块模板 | `converter.rs:82-165` `to_anthropic_sse` | 已生成 message_start / content_block_delta / message_delta+message_stop | 复用 |
| openai SSE 分块模板 | `adapter/openai.rs:392-451` `to_openai_sse` | chat.completion.chunk delta + `[DONE]` | 复用 |
| gemini SSE 分块模板 | `adapter/gemini.rs:253` `to_gemini_sse` | gemini 流式格式 | 复用 |
| 统一流事件类型 | `adapter/types.rs:130-153` `ChatStreamEvent`（Start/Delta/ToolDelta/Stop/Usage） | tagged enum | mock 构造 `Start{id,model}` → `Delta{text}` (可拆多块) → `Stop{finish_reason}` |
| **非流式 JSON 响应** | **无** | proxy 非流式是上游 JSON **透传**（`proxy.rs:426-454`），converter 无 response→client 的非流式 builder | **需新增**：mock 要按 source_protocol 造完整非流式 body（anthropic `{type:"message",content:[{type:"text",text}],usage:{...}}` / openai `{object:"chat.completion",choices:[{message:{...}}],usage:{...}}` / gemini `{candidates:[...],usageMetadata:{...}}`）。建议新建 `adapter/mock.rs` 放这 3 个 builder + SSE 序列生成 |

请求构造侧参考形状：anthropic `adapter/anthropic.rs:1-37`（AnthropicRequest 结构），openai `adapter/openai.rs:63`（to_openai），gemini `adapter/gemini.rs:73`（to_gemini）——这些是 request 侧，非 response，但可参照各协议 JSON 字段命名。

### 4. proxy_log token 填充

| 路径 | file:line | 现状 |
|---|---|---|
| 非流式 token | `proxy.rs:429` `extract_usage(&resp_str)` → `proxy.rs:434-436` 写 `log.input/output/cache_tokens` | `extract_usage` (`proxy.rs:556-582`) 从响应 `usage` 解析（兼容 anthropic `input_tokens`/openai `prompt_tokens`/cache 多来源） |
| 流式 token | `proxy.rs:459-461` 原子计数器 → `proxy.rs:492-505` 逐块累加 → `proxy.rs:538-540` 写 log | 从 SSE `usage` 字段抓 |
| log 字段定义 | `models.rs:493-495` `input_tokens / output_tokens / cache_tokens: i32` | — |

**mock 填假值**：在拦截分支里直接 `log.input_tokens = <mock>; log.output_tokens = <mock>; log.cache_tokens = <mock>;`（来自 mock 配置或按响应文本长度估算），无需走 `extract_usage`。其余 log 字段（status_code/duration_ms/response_body/user_response_body）按 mock 分支自行填，复用现有 `upsert_log(&state, &log, &log_settings)`（`proxy.rs:70`）。

### 5. 平台选择 / 路由 + base_url/api_key 校验

- 路由逻辑 `router.rs:13-63` `select_platform`：按 model_mappings → group platforms → routing_mode 选平台。**完全不校验 base_url/api_key**，对 mock 透明，`router.rs` **无需改**。
- endpoint 协议匹配在 proxy `proxy.rs:319-326`：按 source_protocol 找 platform.endpoints；mock 平台无 endpoints 时落 `unwrap_or((&route.platform.platform_type, route.platform.base_url, ...))` → `target_protocol_enum = Mock`。因为我们在 `proxy.rs:340` 之后即拦截，target_protocol/base_url 对 mock 无实际作用。
- **校验风险在前端 + DB CRUD**：
  - `db.rs:102-130` `create_platform` 不校验 base_url/api_key 非空（DB 层 `NOT NULL DEFAULT ''`，空串合法），**后端无拦截**。
  - 前端 `Platforms.tsx` 表单可能对 base_url/api_key 做必填校验 + `getDefaultEndpoints` 自动填 endpoints。需让 mock 类型时：base_url/api_key 可空、不强制 endpoints、改为展示 mock 场景配置编辑器。
- proxy 日志会把 mock 平台 api_key 走 `redact_key`（`proxy.rs:939`），mock 不发 header 故无影响。

### 6. mock 配置载体建议（明确推荐）

**推荐：复用现有 `platform.extra`（TEXT JSON）**。

现有 extra 使用情况：
- 定义 `models.rs:250` `pub extra: String`（JSON 字符串），CRUD 已读写（`db.rs:118/130/173/194`），DB 列 `extra TEXT NOT NULL DEFAULT ''`（db-conventions `No NULL`）。
- **前端目前完全未使用** extra（`grep extra src/pages/Platforms.tsx` = 0 命中；api.ts 仅类型层 round-trip）。即 extra 是一个已存在但闲置的扩展位。

理由：
1. **零 DB 迁移**：列已存在，符合 db-conventions「破坏式变更才需迁移脚本」，加字段反而违背。
2. **语义契合**：extra 本就是「JSON 额外配置」(models.rs:249 注释)，mock 多场景配置正是平台级额外配置。
3. **CRUD 已通**：create/update 已透传 extra，无需动 db.rs / api.ts 的命令签名。
4. **隔离性**：mock 配置只对 mock 平台有意义，放专用列会让其余 50+ 平台多一个永远为空的列。

建议 extra JSON 结构（供 design 阶段细化）：

```json
{
  "mock": {
    "status_code": 200,
    "delay_ms": 0,
    "stream": true,
    "response_text": "Hello from mock",
    "finish_reason": "end_turn",
    "input_tokens": 100,
    "output_tokens": 50,
    "cache_tokens": 0,
    "error_mode": "none",      // none | http_error | rate_limit_429 | timeout
    "chunk_count": 5            // 流式时把 response_text 切成 N 块
  }
}
```

备选（不推荐）：新增专用列 → 需迁移 + 改 8 处 db.rs/models.rs/api.ts；endpoints 复用 → 语义不符（endpoints 是 base_url 列表）。

### 7. 流式 SSE 现状 + 复用点

- proxy 当前流式（`proxy.rs:457-552`）：**透传并转换上游 SSE**——`resp.bytes_stream()` → 逐行 strip `data: ` → `[DONE]` 转 Stop → 解析 usage → `adapter::parse_sse(json, target_protocol)` 得统一 `ChatStreamEvent` → `adapter::to_client_sse(event, client_protocol, model)` 转回客户端格式。响应头 `text/event-stream` + `no-cache` + `keep-alive`（`proxy.rs:543-551`）。
- **mock 需自己生成 SSE 流**（无上游可透传）。复用点：
  - `adapter::to_client_sse`（`converter.rs:72`）：把自造的 `ChatStreamEvent` 转成 anthropic/openai/gemini SSE 字符串——**核心复用**。
  - SSE body 构造：用 `axum::body::Body::from_stream`（参考 `proxy.rs:473-530`），或对固定内容用 `futures::stream::iter` 造静态分块流；延迟可在 stream 中 `tokio::time::sleep` 模拟逐块吐字。
  - 响应头三件套直接照抄 `proxy.rs:543-551`。
- mock 事件序列：`Start{id,model}` → 多个 `Delta{text}`（按 chunk_count 切分 response_text）→ `Stop{finish_reason}`。usage 假值在流结束写入 `log.*_tokens`（不依赖上游 usage 解析）。

## 涉及文件清单（供 subtask 拆分）

后端（Rust）：
1. `src-tauri/src/gateway/models.rs:5-129` — Protocol enum 加 `Mock` 变体（+ serde rename "mock"）
2. `src-tauri/src/gateway/adapter/mod.rs:1-16` — 注册 `pub mod mock;` + re-export
3. `src-tauri/src/gateway/adapter/mock.rs`（**新建**）— 3 协议非流式 JSON builder + SSE `ChatStreamEvent` 序列生成 + mock 配置 struct（反序列化 extra）
4. `src-tauri/src/gateway/proxy.rs:340` 附近 — mock 拦截分支（非流式 + 流式 + 错误/延迟/超时模拟 + 假 token + upsert_log）
5. `src-tauri/src/gateway/router.rs` — **无需改**（已确认不校验 base_url/api_key）
6. `src-tauri/src/gateway/db.rs` — **无需改**（extra 列已存在，enum 自动序列化）

前端（TS/React）：
7. `src/services/api.ts:5-20` — Protocol union 加 `"mock"`
8. `src/pages/Platforms.tsx:10` PROTOCOLS — 加 mock 选项；`getDefaultEndpoints`（:136）mock 返空；表单适配（base_url/api_key 可空 + mock 场景配置编辑器写入 `platform.extra`）

## Caveats / Not Found

- **非流式响应 builder 不存在**：proxy 非流式是上游 JSON 透传（`proxy.rs:426-454`），不存在「ChatResponse → 各协议非流式 JSON」的现成函数。mock 需新写。已确认（converter.rs 仅有 request 转换 + SSE 转换，无非流式 response 转换）。
- **前端 extra 编辑 UI 不存在**：`Platforms.tsx` 无 extra 字段渲染（grep 0 命中），mock 配置编辑器需从零做。
- **mock 平台的 model 槽位 / endpoints UI**：现有 `getDefaultEndpoints`/PlatformModels 表单对 mock 无意义，需在前端按 `platform_type === "mock"` 条件隐藏/替换，具体 UI 形态留给 design。
- **超时模拟语义**：`error_mode: "timeout"` 应在 mock 分支用 `tokio::time::sleep` 超过客户端超时还是直接返 504/挂起，需 design 阶段定义（建议 sleep 一段超长时间或直接返超时状态码，避免真正 hang 住连接）。
- 未深入 gemini.rs / openai_responses.rs / openai_completions.rs 的非流式 response 字段细节（仅确认 SSE builder 存在）。若 mock 要支持 openai_responses/openai_completions 入站协议的非流式格式，需 design 阶段补充各自 JSON shape；当前需求只列了 anthropic/openai/gemini 三种。
