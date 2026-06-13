# Research: 转换调用点 + 既有透传/同协议跳过现状

- **Query**: convert_request / parse_sse 在哪被调用？转换强制还是已有「同协议跳过」？passthrough 现状？
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### convert_request 调用点（唯一）

- `proxy.rs:742` `adapter::convert_request(&chat_req, target_protocol_enum, platform_protocol)`
  - 仅此一处调用。出站请求体 = 内部 `ChatRequest` 经 `to_anthropic/to_openai/to_gemini/to_responses/to_completions` 重新序列化。
- 定义 `converter.rs:10-41`。

### parse_sse 调用点（唯一）

- `proxy.rs:998` `adapter::parse_sse(&json, &protocol)`（流式响应解析，`protocol = target_protocol_enum.clone()` at `proxy.rs:882`）
- 配合 `to_client_sse` `proxy.rs:1010`（`converter.rs:72-79`）把内部事件再格式化为**客户端协议** SSE。
- 即响应链路：上游 SSE → `parse_sse`(按 target wire) → 内部 `ChatStreamEvent` → `to_client_sse`(按 source wire) → 客户端。**双向转换**。

### 转换是强制的吗？是否有「同协议跳过」？

**对走 `convert_request` 路径的请求：转换是无条件强制的，没有「入站==出站协议则跳过」的短路**。

- `proxy.rs:740-742` 无任何 `if source_protocol == target_protocol { skip }` 判断。
- 即使端点匹配命中（入站协议 == 端点协议），仍执行 `convert_request`：
  - 内部 `ChatRequest` 是「Anthropic 兼容结构」(`converter.rs:66-67` 注释)，但对 openai/gemini 入站已做过 `from_*` 归一化。
  - 出站再 `to_*`，构成**有损往返**（字段映射、未建模字段丢失、SSE 重格式化）。
- **结论：当前没有「同协议字节级跳过转换」**。端点匹配只决定「用哪个协议出站 + base_url」，不决定「是否跳过转换」。

### 既有透传（passthrough）现状

**唯一字节级透传：`Protocol::ClaudeCode` 平台**。

- 拦截点 `proxy.rs:722-738`：`if matches!(route.platform.platform_type, Protocol::ClaudeCode)` → `handle_passthrough(...)`，**bypass 所有转换，1:1 relay**。
- `handle_passthrough` `proxy.rs:1185-1364`：
  - 不解析 body，不转协议，不动 header（`passthrough_headers` 仅剔 hop-by-hop，保留客户端 OAuth）`proxy.rs:1215,1378`。
  - 目标 URL = `base_url(host 根) + 客户端原始 path+query` `proxy.rs:1202` / `build_passthrough_url` `proxy.rs:1367`。
  - 注意：与普通路径 URL 构造**不同** —— 透传用客户端**原始 path**（如 `/v1/messages`），普通路径用 `convert_request` 返回的 `api_path`。
  - source/target 都标 `claude_code` `proxy.rs:1198-1199`。
  - 流式 `proxy.rs:1321-` 原样 relay SSE bytes，仅尽力累计 usage，不改写 chunk `proxy.rs:1334`。
  - 设计意图（注释 `proxy.rs:1182-1183`）：客户端自带订阅 OAuth，纯透传保认证。
- `Protocol::Mock` `proxy.rs:705-720` 也 bypass，但生成假响应，非透传。

### grep 透传/passthrough 命中清单

- `passthrough` 出现：`proxy.rs:444,522,723,725,1182-1378`（注释 + handle_passthrough + build_passthrough_url + passthrough_headers）。
- `透传`：`proxy.rs:522,664,722,829,1182,1197,1216,1321,1366,1376` + CLAUDE.md「ClaudeCode 透传」「statusline 透传」描述。
- 关键 `proxy.rs:522-523` 注释：「捕获原始请求量（用于 Claude Code 纯透传：未 redact 的真实 header/method/uri）」——`orig_method/orig_uri/orig_headers/bytes` 在 body 消费前 clone 保存，正是为透传准备的。**这套原始量捕获基础设施已存在**，可复用于「优先原协议透传」。

## Caveats / Not Found

- 透传当前**强绑定 `platform_type == ClaudeCode`**，是「平台级开关」而非「协议级条件」。用户要的「入站协议被平台支持就透传」需要把透传从「平台类型判定」泛化为「协议匹配判定」。
- 透传 URL 用客户端原始 path，普通路径用 `convert_request` 生成 path —— 两套 URL 构造规则，泛化透传时这是主要风险点（见 04）。
