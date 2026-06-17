# PRD: count_tokens 端点协议支持（修复错转发到 /messages 致上游 500）

## 1. 问题描述

claude-cli（claude-code）会在发送实际对话前，调用 anthropic 原生端点
`POST /v1/messages/count_tokens` 预估 token 数（input header 含
`anthropic-beta: ...,token-counting-2024-11-01`）。

经 aidog 代理后，该请求被**错误地当成普通 `/v1/messages` 转发到上游**，导致上游
（GLM `open.bigmodel.cn/api/anthropic`）按 messages 处理一个 count_tokens 形态请求而崩溃。

### 真实失败请求证据（inspect）

- request_id `fe5d849ab2b44283ac66608b681677d0`，2026-06-16 18:04:40
- group=glm-coding-plan-auto，model claude-opus-4-8 → glm-5.1，platform id=2（GLM）
- protocol anthropic → anthropic，stream=否，status client=500 / upstream=500，179ms
- **client URL: `/proxy/v1/messages/count_tokens?beta=true`**
- **upstream URL: `https://open.bigmodel.cn/api/anthropic/v1/messages`** ← 丢了 `count_tokens` 尾段
- 上游响应 body：`{"type":"error","error":{"type":"api_error","code":"500","message":"[500]['>' not supported between instances of 'int' and 'NoneType']..."}}`（GLM 端 Python 报错，因 messages handler 收到 count_tokens 形态体）

## 2. 根因（带 file:line 证据）

整条链路有两个串联缺陷，最终 = path 尾段 `count_tokens` 被吞。

### 根因 A — 入站协议探测把 count_tokens 归为普通 anthropic messages
`src-tauri/src/gateway/proxy.rs:2572`
```rust
if api_path.starts_with("/v1/messages") {
    "anthropic".to_string()
}
```
`/v1/messages/count_tokens` 前缀匹配 `/v1/messages` → 判定为 `anthropic`。请求体含
`model` + `messages` 字段，`parse_incoming_request`（converter.rs:78）解析成功、路由成功
（claude-opus-4-8 → glm-5.1），因此不报 4xx，链路一路走到上游构造。

### 根因 B（直接根因）— 出站 path 由协议**写死**，不保留 count_tokens 尾段
同协议透传分支 `src-tauri/src/gateway/proxy.rs:1132`：
```rust
let path = adapter::passthrough_api_path(target_protocol_enum, &actual_model, platform_protocol);
```
`passthrough_api_path` 对 Anthropic 硬编码返回 `/v1/messages`
（`src-tauri/src/gateway/adapter/converter.rs:56`）：
```rust
Protocol::Anthropic => "/v1/messages".to_string(),
```
转换分支同理 `convert_request`（converter.rs:15）也写死 `/v1/messages`。

最终 URL 构造 `src-tauri/src/gateway/proxy.rs:1148-1149`：
```rust
let base_url = target_base_url.trim_end_matches('/');
let url = format!("{}{}", base_url, api_path);   // api_path = "/v1/messages"
```
→ 客户端原始 path 的 `/count_tokens` 尾段从未被读取，被 `/v1/messages` 覆盖。

### 为何 inbound/upstream body 都为空
推测: count_tokens 请求体（36784 字节，见 inbound header `content-length`）实际存在，但
本条 log 的 `request_body` / `upstream_request_body` 显示空——与 ProxyLogSettings body
开关或本条早期 upsert 落库时序有关，非本 bug 必要路径。token 计数 in=0/out=0 是上游 500
未返回 usage 的正常结果。（此项不影响根因判定，标注待实现期复核。）

### 对比：已有但未覆盖此场景的端点分流
proxy.rs 已对两类「非 chat 语义」子端点做了**前置分流**（绕过 parse + 写死 path）：
- `is_models_endpoint`（proxy.rs:2088） → `handle_models_passthrough`
- `is_responses_subendpoint`（proxy.rs:2131） → `handle_responses_subendpoint`（proxy.rs:2151）

`/v1/messages/count_tokens` 属于同一类「需保留原始 path 尾段、原样透传 body」的子端点，
但**没有对应的分流分支**，于是 fallthrough 到普通 messages 转换路径。这是缺口本质。

## 3. 上游能力调研（决定方案）

- **anthropic 原生**：提供 `POST /v1/messages/count_tokens`，返回 `{"input_tokens": N}`。
- **GLM anthropic 兼容端点**（`open.bigmodel.cn/api/anthropic`）：当前请求打到
  `/v1/messages` 报 Python 500——说明它**没有把 count_tokens 当独立端点处理**。
  其 `/api/anthropic/v1/messages/count_tokens` 是否存在 = `需要: 实测确认`（见失败处理）。
- **OpenAI 兼容平台**（chat/completions 系）：无 count_tokens 端点。
- 结论：能力**因平台而异**，不能假设所有上游都有该端点 → 方案需带「本地估算兜底」。

## 4. 修复方案（含取舍）

参照既有 `is_responses_subendpoint` / `is_models_endpoint` 模式，新增 count_tokens 前置分流。
分流后**按上游能力分层**：

### 方案 X（推荐）：分流 + 透传优先 + 本地估算兜底
在 `handle_proxy_inner` 的 parse 之前（proxy.rs:830-848 同区）插入：
```
if is_count_tokens_endpoint(&path) {
    return handle_count_tokens(...).await;
}
```
`handle_count_tokens` 逻辑：
1. 路由选平台（复用现有 group→platform 选择，拿 anthropic 端点 base_url + 凭证）。
2. **构造上游 URL 保留尾段**：`base_url + /v1/messages/count_tokens`
   （遵 url-construction-rule：base_url 已含版本前缀则只拼 endpoint 后缀，禁额外拼 /v1；
    实现期按平台 base_url 形态对齐，镜像 handle_responses_subendpoint 的 `strip_prefix("/v1")` 处理）。
3. **透传 body**（含 model remap：把 claude-opus-4-8 换成 glm-5.1），原样 POST。
4. 上游返回 2xx → 直接回客户端（anthropic count_tokens 响应 schema 客户端能识别）。
5. 上游 4xx/5xx（平台不支持该端点）→ **本地估算兜底**：用现有 estimate.rs / tokenizer
   近似算 `input_tokens`，返回 `{"input_tokens": N}` 给客户端，避免 claude-cli 流程被 500 阻断。

取舍：
- **纯透传**（不兜底）：实现最简，但平台不支持时仍 500（GLM 当前正是此情况）→ 否决，没真正修好。
- **纯本地估算**（永不透传）：永远不 500，但 token 数与上游计费口径可能偏差，且放弃了
  支持该端点的平台（anthropic 官方）的精确值。
- **透传优先 + 估算兜底**（方案 X）：支持的平台拿精确值，不支持的平台退化为可用估算，
  claude-cli 永不被阻断 → 推荐。代价是多一层 fallback 分支。

### 关键决策点（已拍板 2026-06-16）
- ✅ **采用方案 X：透传优先 + 本地估算兜底**。用户接受「本地近似 input_tokens」与上游计费可能小幅偏差，
  以保证 claude-cli 预估流程永不被上游 500/404 阻断。平台不支持该端点时**返回本地估算值而非错误**。

## 5. 验收标准（可测）

1. `POST /proxy/v1/messages/count_tokens`（带 anthropic count_tokens body）经 aidog：
   - 上游 URL = `<base_url>/v1/messages/count_tokens`（**含尾段**，inspect 可验）。
   - 不再出现 `'>' not supported between int and NoneType` 类上游 500。
2. 上游支持该端点（如 anthropic 官方平台）→ 客户端收到上游真实 `{"input_tokens": N}`，status 200。
3. 上游不支持（如当前 GLM）→ 客户端收到本地估算 `{"input_tokens": N}`，status 200（方案 X）
   或明确错误（若用户选纯透传）。
4. proxy_log 落库：source/target protocol 标 anthropic（或新增 count_tokens 标记），
   upstream_request_url 含尾段，status 正确。
5. `cargo build` 无 error；`cargo clippy` 无 warning；`cargo test` 全绿。
6. 既有 `/v1/messages`（普通对话）、`/v1/models`、`/v1/responses/*` 分流行为不回归。

## 6. 影响面

- 仅 `src-tauri/src/gateway/proxy.rs`（新增 1 个判定函数 + 1 个 handler + 1 处分流调用）。
- 可能触及 `src-tauri/src/gateway/estimate.rs`（复用本地 token 估算做兜底，方案 X）。
- **不改** converter.rs 的 `passthrough_api_path` / `convert_request`（普通 messages 路径不动），
  采用前置分流隔离，零回归面，与 responses 子端点同款隔离策略。
- 前端无改动。i18n 无新文案（除非兜底错误信息需多语言，实现期评估）。
