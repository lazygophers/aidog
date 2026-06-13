# Research: 平台协议能力声明机制

- **Query**: 平台「支持哪些协议」如何声明？一个平台能否挂多协议端点？如何查「平台 X 是否支持协议 Y」？
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### Protocol 枚举 (models.rs:4-137)

`Protocol` 枚举共 53 变体，明确分两类（源码注释 `models.rs:6,17`）：

- **AI 请求协议（可作为 endpoint 协议）** `models.rs:6-16`：
  `Anthropic` / `OpenAI` / `OpenAIResponses` / `OpenAICompletions` / `Gemini`
  —— **只有这 5 个是真正的 wire 协议**，能作为 `PlatformEndpoint.protocol` 且被 `convert_request`/`parse_sse` 识别。
- **平台类型（仅作平台主协议，不作 endpoint 协议）** `models.rs:17+`：
  `Mock` / `ClaudeCode` / `Glm` / `Kimi` / `MiniMax` / `Codex` / `DeepSeek` / `OpenRouter` / `NewApi` … 共 48 个。
  这些是「平台身份标签」，主要用途：决定计费/quota/header 模拟/coding-plan path，**不直接决定 wire 格式**。

### convert_request 实际只认 4 个 wire 分支 (converter.rs:10-41)

```rust
match wire_protocol {
    Protocol::Anthropic     => to_anthropic,  // /v1/messages
    Protocol::Gemini        => to_gemini,     // /v1beta/.../streamGenerateContent
    Protocol::OpenAIResponses => to_responses,// /v1/responses
    Protocol::OpenAICompletions => to_completions, // /v1/completions
    _ => to_openai (provider_api_path 永远 "/chat/completions")  // 所有平台类型变体落这里
}
```
- 即：48 个平台类型变体 + `OpenAI` 全部映射到 OpenAI Chat Completions wire 格式。
- `provider_api_path` `converter.rs:44-46` 写死返回 `"/chat/completions"`（参数 `_protocol` 未用）。
- `parse_sse` 同理 `converter.rs:50-57`：只区分 Anthropic / Gemini，其余全用 `parse_openai_sse`。

### Platform.endpoints — 多协议端点声明 (models.rs:245-259, 337-339)

```rust
pub struct PlatformEndpoint {
    pub protocol: Protocol,      // 该端点的 wire 协议
    pub base_url: String,        // 该协议专属 base_url（含版本前缀）
    pub client_type: ClientType, // 模拟客户端类型（过上游校验）
    pub coding_plan: bool,       // 是否 coding plan 端点
}

pub struct Platform {
    pub platform_type: Protocol, // 平台主协议（兜底 wire + 计费身份）
    pub base_url: String,        // 主 base_url
    pub endpoints: Vec<PlatformEndpoint>, // 额外协议端点，#[serde(default)]
    ...
}
```

- **一个平台可挂多协议端点** —— `endpoints: Vec<PlatformEndpoint>` 明确支持。注释 `models.rs:245`「同一平台可支持多种协议，每种协议对应不同的 base_url」。
- 端点容错反序列化 `models.rs:237-243` + `db.rs:1886-1908`：未知 `client_type` 回退 Default，不让整个数组解析失败。
- DB 列 `endpoints` JSON 字符串持久化 `db.rs:128,178,192-193`；新建默认 `unwrap_or_default()`（空 Vec）`db.rs:178`。

### 如何查「平台 X 是否支持协议 Y」

当前**唯一**判定点 `proxy.rs:629-638`：遍历 `platform.endpoints`，比对 `format!("{:?}", ep.protocol).to_lowercase() == source_protocol`。

- **没有独立的 `supports(protocol)` 方法**（grep `supports` 在 gateway 无定义）。
- **重要语义空白**：`platform.platform_type`（主协议）**不被算作一个隐式端点**。例如某平台 `platform_type=OpenAI` 但 `endpoints=[]`，入站 `openai` 时：`matched_ep=None` → 回退分支 `proxy.rs:641` 用 `platform_type=OpenAI` 出站，结果碰巧也对；但判定逻辑上「该平台支持 openai」这一事实只通过回退分支隐式成立，不是显式端点匹配命中。
- 即：「平台支持入站协议」= `endpoints 含该协议` **OR** `platform_type 的 wire 归类 == 入站协议`。后半句当前代码**没有**显式表达——回退分支无脑用 platform_type，不判断它的 wire 归类是否真等于入站协议。

## Caveats / Not Found

- `需要`复核：`platform_type` → wire 协议的归类函数不存在。要实现「优先原协议」精确判定，需要一个 `Protocol -> wire Protocol(5选1)` 的映射（把 48 个平台类型归到 anthropic/openai/gemini/responses/completions 之一）。当前这层映射隐含在 `convert_request` 的 match 默认分支里（除 4 个特例外全是 openai），未抽成可查询函数。
- 前端 endpoints 编辑入口在 `src/pages/Platforms.tsx` / `src/services/api.ts`（grep 命中），本次未深读 UI；若改判定逻辑需确认前端是否允许用户为平台声明任意协议端点。
