# PRD: 修复 anthropic 入站工具缺 input_schema 致 400

> task: `06-17-anthropic-tool-optional-input-schema`
> 类型: 单一交付 bugfix（不拆 subtask）

## 1. 问题陈述

### 现象
Claude Code 客户端（`claude-cli/2.1.177`）发起入站 anthropic 请求 `POST /proxy/v1/messages?beta=true`，aidog 代理 **1ms 即返回 client 400**，请求未路由到任何上游（proxy_log `platform_id=0`）。

错误响应体：
```
failed to parse request for protocol (anthropic): missing field `input_schema`
```

### 根因
入站 anthropic 请求经 `parse_incoming_request` 直接反序列化为中性模型 `ChatRequest`：

- `src-tauri/src/gateway/adapter/converter.rs:86` — anthropic 分支 `serde_json::from_value(body.clone())`，serde 错误直接冒泡为 parse 失败。
- `src-tauri/src/gateway/adapter/types.rs:170-175` — 中性模型 `Tool` 结构体，字段 `pub input_schema: serde_json::Value` 为**必填**（无 `#[serde(default)]`）。

客户端发送了**不带 `input_schema`** 的工具定义（典型来源：Anthropic 服务端工具如 `web_search` / `bash` / `code_execution`，或精简工具声明）→ serde 报 `missing field input_schema` → 整个请求体反序列化失败 → 400 秒拒，无法降级、无法路由。

### 证据 / 触点
| 角色 | file:line | 说明 |
| --- | --- | --- |
| parse 入口 | `converter.rs:86` | anthropic 分支 `from_value`，serde error 即 parse 失败 |
| 必填字段（根因） | `types.rs:174` | `pub input_schema: serde_json::Value`，无默认 |
| 出站 anthropic 同型必填 | `anthropic.rs:36` | `AnthropicTool.input_schema: Value`（出站序列化结构） |
| 出站转换 — anthropic | `anthropic.rs:71` | `input_schema: t.input_schema.clone()` |
| 出站转换 — openai | `openai.rs:182` | `parameters: t.input_schema.clone()` |
| 出站转换 — gemini | `gemini.rs:139` | `parameters: t.input_schema.clone()` |

### 设计哲学违反
项目「容错解析」原则（记忆 `anthropic-parse-tolerant-unknown-block`）：`ContentBlock` 对未知类型降级为 `Unknown(Value)` 而非报错。`Tool.input_schema` 必填违反同一原则——单个工具字段缺失不应导致整请求 400。

## 2. 目标与非目标

### 目标
- 入站 anthropic 请求中 `tools[]` 缺 `input_schema` 时**仍能解析成功**，请求正常路由到上游。
- 出站序列化（anthropic / openai / gemini）**不发出破坏上游的字段**（如 `"input_schema": null`）。

### 非目标
- 不改动 `tools` 之外的解析逻辑。
- 不引入工具语义校验 / 改写（不补全工具 schema 内容、不推断字段）。
- 不改动 openai / gemini **入站** 工具解析路径（本 bug 仅入站 anthropic 触发；但 `Tool` 为共享中性模型，改动会被三处出站复用，需一并验证不回归）。

## 3. 验收标准（可机器验证）

1. `cd src-tauri && cargo build` 通过。
2. `cd src-tauri && cargo clippy` 无 warning（遵循 `warnings-are-issues`）。
3. `cd src-tauri && cargo test` 全绿，且**新增** `#[test]`：入站 anthropic 请求 `tools[]` 缺 `input_schema` 时 `parse_incoming_request("anthropic", &body)` 返回 `Ok`（放 `converter.rs` tests 模块，风格参考现有 `converter.rs:240/264/284`）。
4. 缺 `input_schema` 的入站 anthropic 请求**不再返回 client 400**——能反序列化成 `ChatRequest` 并进入路由（由验收点 3 的 test 间接覆盖；如需端到端，可用 `aidog-request-inspect` 核对一条真实请求的 `platform_id != 0`）。
5. 纯文本 / 带正常 `input_schema` 工具的既有请求**回归不受影响**（既有 test `anthropic_parse_plain_text_unchanged` 等仍全绿）。

## 4. 影响面与风险

### 需检查的触点文件
- `src-tauri/src/gateway/adapter/types.rs` — `Tool` 结构体（改动主体）。
- `src-tauri/src/gateway/adapter/anthropic.rs:36,71` — 出站 anthropic：`AnthropicTool.input_schema` 序列化行为，确认空对象不会破坏上游。
- `src-tauri/src/gateway/adapter/openai.rs:182` — 出站 openai：`function.parameters` 取值。
- `src-tauri/src/gateway/adapter/gemini.rs:139` — 出站 gemini：`functionDeclarations.parameters` 取值。

### 风险
- **出站发出 `null` 破坏上游**：若 `input_schema` 改为 `Option<Value>` 且序列化时透传 `None→null`，Anthropic / OpenAI / Gemini 上游可能拒绝。必须保证缺失时落地为**空对象**或合法默认，而非 `null`。
- **三处出站 `.clone()` 类型变更连带**：若把 `Tool.input_schema` 从 `Value` 改为 `Option<Value>`，三处出站 clone 需同步适配（解包默认值），否则编译失败——这是预期的连带改动，非回归。
- 中性模型为跨协议共享，改默认值会影响 openai/gemini 入站构造的 `Tool`，需确认其原本均提供了 `input_schema`（不会因新默认行为产生语义漂移）。

## 5. 实现提示（方向，不写完整代码）

- `Tool.input_schema` 加 `#[serde(default)]`，缺失时默认值为**空对象 `{}`**（`serde_json::json!({})`）而非 `Value::Null`。
  - 方案 A：保持 `input_schema: serde_json::Value` + `#[serde(default = "<返回空对象的函数>")]`，最小连带（三处出站 clone 无需改类型）。**推荐优先评估此方案**——避免 risk 中的出站类型连带。
  - 方案 B：改为 `Option<Value>` + 出站解包 `.unwrap_or_else(|| json!({}))`，连带改三处出站，但语义更显式。
- 出站序列化的「最稳妥默认值」（空对象 `{}` vs `{"type":"object"}` vs `{"type":"object","properties":{}}`）由实现 agent 核实——Anthropic 工具 `input_schema` 通常要求 JSON Schema 对象，需确认上游对空对象的接受度（建议查 Anthropic 官方 tool use 文档，禁凭记忆）。
- 新增 test 模板参考 `converter.rs:278-287` 的 `anthropic_parse_plain_text_unchanged`：构造含 `tools: [{ "name": "...", /* 无 input_schema */ }]` 的 body，断言 `parse_incoming_request("anthropic", &body)` 为 `Ok`，并可进一步断言解析出的 `Tool.input_schema` 为空对象。
