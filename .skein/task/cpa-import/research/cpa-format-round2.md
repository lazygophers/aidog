# CLIProxyAPI 二轮调研 — 4 cpa-* 协议上游 + vertex + 余额

> task: cpa-import | 调研日期: 2026-07-13 | 仓库: router-for-me/CLIProxyAPI (main)
> 一轮见 [cpa-format.md](cpa-format.md)。本轮聚焦 4 cpa-* 协议上游默认 + adapter 映射。

## 核心发现 (颠覆 PRD 推测)

**PRD 原写「4 cpa-* 都 OpenAI 兼容,复用 openai adapter」= 错。** 实际 4 协议走各自原生 API:

| cpa-* | 上游 base_url | API 路径 | 协议族 | aidog adapter 候选 |
|---|---|---|---|---|
| cpa-grok (xai) | `https://api.x.ai` | `/responses` (native, 非 /chat/completions) | OpenAI Responses | `openai_responses` (现 aids) |
| cpa-aistudio | `https://generativelanguage.googleapis.com` / `v1beta` | `:streamGenerateContent` / `:generateContent` | Gemini 原生 | `gemini` (= gemini-api-key 同源,仅 auth 不同) |
| cpa-antigravity | `https://cloudcode-pa.googleapis.com` | `/v1internal:streamGenerateContent` / `/v1internal:generateContent` | Gemini-style (Google 内部) | `gemini` (路径前缀 `/v1internal` 需 adapter 支持或 preset base_url 编码) |
| cpa-vertex | `{region}-aiplatform.googleapis.com` (Vertex AI) | generateContent (Vertex 路径含 project/location) | Vertex 原生 | `gemini` (URL 结构不同,待 s2 验证) |

证据:
- antigravity_executor.go: base `https://cloudcode-pa.googleapis.com` + `/v1internal:*` 路径 + OAuth `oauth2.googleapis.com/token` + ClientID `1071006060591-tmhss...apps.googleusercontent.com`。检查 `claude` / `gemini-3-pro` / `gemini-3.1-flash-image` 模型名,无静态清单(动态从请求提取)。
- aistudio (gemini_executor.go): `glEndpoint = "https://generativelanguage.googleapis.com"`, `glAPIVersion = "v1beta"`, `x-goog-api-key` 头。与 `gemini-api-key` 段**同源同 API**,区别仅在 OAuth vs api-key。
- xai_executor.go: base `https://api.x.ai`,OAuth 经 `https://cli-chat-proxy.x.ai`,native `/responses` 端点,模型 `grok-*` / `grok-composer-*` 前缀(无静态清单)。
- vertex_executor.go: 文件名实为 `vertexai_executor.go` 或他名(WebFetch `vertex_executor.go` 404)—— 路径未定位,字段未读全。标 待 s2 补。

## adapter 映射策略 (修正 design)

不复用 openai adapter。改:
- **cpa-grok** → `openai_responses` adapter (aidog 现有,走 `/responses`,与 xAI native `/responses` 同语义。需 s2 验证字段对齐)。
- **cpa-aistudio** → `gemini` adapter (与 gemini-api-key 同 API;OAuth token 当 api_key 填 `x-goog-api-key` 头)。
- **cpa-antigravity** → `gemini` adapter (generateContent 同;但路径 `/v1internal:*` 非标准 —— 取舍:base_url 编码 `/v1internal` 让 gemini adapter 拼接,或 s2 给 antigravity 单独路径处理)。
- **cpa-vertex** → `gemini` adapter 候选 (Vertex generateContent 同族,但 URL 含 `projects/{project}/locations/{location}/publishers/google/models/` 结构,s2 验证 gemini adapter 是否兼容;若不兼容 → cpa-vertex 仅存配置 + 标注「路由暂不支持」)。

## preset 默认 (4 cpa-* 协议填 platform-presets.json)

| 协议 | base_url | 默认 models | 来源 |
|---|---|---|---|
| cpa-grok | `https://api.x.ai/v1` | (待补:grok-4 / grok-code-fast 等;s2 查 xAI 官方模型清单) | xai_executor.go |
| cpa-aistudio | `https://generativelanguage.googleapis.com/v1beta` | 同 gemini 协议现有 model_list | gemini_executor.go |
| cpa-antigravity | `https://cloudcode-pa.googleapis.com` | `claude-*` / `gemini-3-pro` / `gemini-3.1-flash-image` (动态,从请求提取,preset 给常见) | antigravity_executor.go |
| cpa-vertex | (用户必填 region-specific) | (待 s2) | vertex_executor 未定位 |

## 余额查询机制

CLIProxyAPI **无内置余额/配额查询**。grep `balance`/`quota`/`usage`/`dashboard` 在 executor 层无命中(本次 WebFetch 各 executor 未现余额逻辑)。CPA 仅做路由转发,不查上游余额。

→ aidog 预览余额展示复用自身 `gateway::quota::query_quota` (支持 DeepSeek/OpenRouter/GLM/Kimi/MiniMax/NewAPI/SiliconFlow/StepFun/Novita)。**cpa-* 4 协议 + 其他不支持的 provider 预览显「—」**(无余额 API)。

## 待 s2 补

- vertex_executor 文件真名 + VertexCompatKey 字段全表 (research/cpa-format.md 待确认 #3 续)。
- xAI 官方模型清单 (grok-* 具体 id)。
- antigravity 常见模型名 (preset 填充用)。
- openai_responses adapter 是否完全兼容 xAI `/responses` 字段 (s2 读 adapter/codex.rs + openai_responses.rs 验证)。
