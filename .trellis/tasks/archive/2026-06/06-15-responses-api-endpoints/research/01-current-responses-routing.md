# Research: Responses API 端点当前路由现状（逐端点判定）

- **Query**: aidog 代理对 OpenAI Responses API 4 端点（create/compact/cancel/retrieve）的支持现状
- **Scope**: internal（含官方语义推测，已标注）
- **Date**: 2026-06-15

## 关键事实链（routing pipeline）

入口 `handle_proxy` 为 axum `.fallback`（`src-tauri/src/gateway/proxy.rs:69-72`），所有非 `/api/group-info`、`/api/notify` 的请求都进 `handle_proxy_inner`。

请求处理顺序（`handle_proxy_inner`，proxy.rs:673 起）：

1. 读 header / body / 原始 method/uri（proxy.rs:752-757）。body 读取走 `axum::body::to_bytes`（最大 10MB），空 body 不报错（`bytes` 为空字节）。
2. `resolve_group`（proxy.rs:783）：先按 Bearer token = group_name 精确匹配，再按 path 前缀。Codex 走 `Authorization: Bearer $AIDOG_KEY`（值=分组名，见 codex.rs:51），故按 token 命中。
3. `detect_source_protocol(&path)`（proxy.rs:806 → 定义 2337）。
4. **models 分流**（proxy.rs:814）：`GET && is_models_endpoint` → `handle_models_passthrough`，**return**。
5. 否则 `serde_json::from_slice::<Value>(&bytes)`（proxy.rs:819）→ 失败返回 400（`parse request json error`）。**空 body 在此 EOF 400**（与刚修的 /v1/models 同类）。
6. `parse_incoming_request(source_protocol, &req_value)`（proxy.rs:829）。
7. 路由候选 → 端点匹配（proxy.rs:959）→ same_protocol_passthrough 判定（proxy.rs:982）→ convert_request 或 passthrough（proxy.rs:1074）。

## detect_source_protocol 对 responses 子路径的行为

```
proxy.rs:2337  fn detect_source_protocol(path)
proxy.rs:2339  api_path = path[idx..] where idx = path.find("/v1/")  // strip /proxy 前缀
proxy.rs:2347  if starts_with("/v1/messages") -> anthropic
proxy.rs:2349  else if api_path.starts_with("/v1/responses") -> "openai_responses"   ← 关键
```

`starts_with("/v1/responses")` **吞掉所有子路径**：

| client path（含 /proxy 前缀） | api_path（strip 后） | detect 结果 |
|---|---|---|
| `/proxy/v1/responses` | `/v1/responses` | openai_responses ✓ |
| `/proxy/v1/responses/resp_123` | `/v1/responses/resp_123` | openai_responses（误吞，应单独识别） |
| `/proxy/v1/responses/resp_123/cancel` | `/v1/responses/resp_123/cancel` | openai_responses（误吞） |
| `/proxy/v1/responses/compact` 或 `/proxy/v1/compact` | 视拼法 | openai_responses（误吞）或落 else→anthropic |

注意 strip 逻辑是 `path.find("/v1/")`，若 path 不含 `/v1/`（如假设的 `/proxy/responses` 无版本前缀）→ 进 else 分支 → 若也不含 `/v1beta/` → **返回 "anthropic"**（兜底，proxy.rs:2344）。详见 02 文件「前缀问题」。

## 逐端点判定

### 1. `POST /v1/responses`（create，主请求，有 body）→ **work（条件成立时）**

- detect = openai_responses ✓
- body 非空 → JSON parse OK → `parse_incoming_request("openai_responses", ...)` → `from_responses`（converter.rs:81 / openai_responses.rs:71）。`from_responses` 已容错 string/array input + instructions→system（openai_responses.rs，有 3 个 #[test] 通过）。
- **端点匹配 + 透传判定**（proxy.rs:959-984）：
  - 若路由到的平台有**显式 `openai_responses` 端点** → `matched_ep` 精确命中 → `same_protocol_passthrough = true`（proxy.rs:982）。此时走透传分支（proxy.rs:1074-1082）：用客户端原始 `req_value`（仅 patch `model` 字段），path = `passthrough_api_path(OpenAIResponses, ...)` = **硬编码 `/v1/responses`**（converter.rs:58）。即**不转换、原样转发 create**。这是 Codex→Responses 平台的正确路径（同 memory [[protocol-same-proto-passthrough]] / [[codex-config-subsystem]]）。
  - 若平台**无 responses 端点** → 回退查 openai 端点（proxy.rs:962-967，仅 openai_responses 源有此回退）→ `target_protocol = openai`，`same_protocol_passthrough = false`（proxy.rs:980-984 注释明确）→ 走 `convert_request(chat_req, OpenAI, ...)`（proxy.rs:1084），有损转换为 chat/completions（converter.rs:34-39）。
  - 测试 `same_protocol_passthrough_condition`（proxy.rs:2924-2942）固化了「openai_responses→openai 回退不透传」。
- **结论**：create 在「平台有 responses 端点」时透传正确；在「回退 openai」时有损转换（已知设计）。**create 本身 work**，是 4 端点里唯一正常的。

### 2. `GET /v1/responses/{id}`（retrieve）→ **broken（EOF 400）**

- detect = openai_responses（误吞）。
- 不是 models 端点（`is_models_endpoint` 只认尾段 `/v1/models`|`/models`，proxy.rs:2023-2030），故**不进 models 分流**。
- GET 无 body → `bytes` 空 → proxy.rs:819 `serde_json::from_slice` **EOF error → 400 `parse request json error`**。
- 即使侥幸过 parse，`from_responses` 需要 `body.get("model")`（openai_responses.rs:72，缺则 return None → 400 `failed to parse request`）。
- **结论**：retrieve 必 400，与 /v1/models 修复前同类根因（GET 空 body 落 chat parse）。

### 3. `POST /v1/responses/{id}/cancel`（cancel）→ **broken（body 形态不符 → 400 或语义错误）**

- detect = openai_responses（误吞）。
- cancel 的 body 形态：**推测**（未在 docs 验证，见 04）Codex/OpenAI cancel 通常**无 body 或空 body**。
  - 若空 body → 同 retrieve，proxy.rs:819 EOF 400。
  - 若有非空 JSON 但无 `model` 字段 → `from_responses` return None → 400 `failed to parse request`。
- 即便构造出 ChatRequest，path 会被 `passthrough_api_path` 重写成 `/v1/responses`（丢掉 `/{id}/cancel`）或 convert_request 转 chat → **cancel 语义完全丢失**。
- **结论**：cancel broken（400 或路径/语义错乱）。

### 4. `POST /v1/responses/compact`（compact）→ **broken / 未在 docs 确认端点存在**

- detect：取决于实际 path。若 `/proxy/v1/responses/compact` → 误吞为 openai_responses；若 `/proxy/v1/compact`（不含 responses）→ 落 else→anthropic 兜底（更错）。
- body 含 compact 专属字段、大概率无 `model` → `from_responses` None → 400；或 anthropic 兜底 serde 反序列化为 ChatRequest（字段不符，行为未定）。
- path 重写丢失 `/compact`。
- **结论**：compact broken；**且该端点是否真实存在/是否 Codex 特有尚未在 docs 确认**（见 04）。

## 汇总表

| 端点 | method | detect 结果 | 走哪条路径 | 现状 |
|---|---|---|---|---|
| `/v1/responses` create | POST(body) | openai_responses ✓ | 平台有 responses 端点→透传；否则回退 openai 转换 | **work** |
| `/v1/responses/{id}` retrieve | GET(空) | openai_responses(误吞) | chat parse → EOF | **broken 400** |
| `/v1/responses/{id}/cancel` | POST(空/小body) | openai_responses(误吞) | chat parse → EOF/None；path 丢 /cancel | **broken** |
| `/v1/responses/compact` | POST(body) | openai_responses(误吞)或anthropic兜底 | chat parse → None；path 丢 /compact | **broken** |

## Caveats / Not Found

- `passthrough_api_path` 对 OpenAIResponses 返回**硬编码 `/v1/responses`**（converter.rs:54-62），透传分支不保留客户端原始子路径 → 即使绕过 detect/parse，子端点 path 也会被丢。这是子端点透传的核心障碍（详见 02/03）。
- 子端点的真实存在性 / body 形态 / HTTP 方法**未在官方 docs 验证**（本 agent 无 WebFetch/web 工具）。`需要:` main agent 用 WebFetch 核 openai/codex responses API docs（见 04 待核清单）。
