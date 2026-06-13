# 协议适配器体系

## 核心入口

`adapter/converter.rs` — 统一协议转换入口：
- `convert_request()` — 入站请求 → 出站请求
- `parse_sse()` — SSE 流解析
- `parse_incoming_request()` — 入站协议检测

## 适配器文件

| 文件 | 协议 | 说明 |
|------|------|------|
| `openai_completions.rs` | openai_completions | OpenAI Text Completions（旧版） |
| `openai_responses.rs` | openai_responses | OpenAI Responses API（Codex 使用） |
| `gemini.rs` | gemini | Google Gemini 原生协议 |
| `codex.rs` | — | Codex TOML 配置（CodexSettings） |
| `minimax.rs` | minimax | MiniMax 原生协议 |
| `glm.rs` | glm | 智谱 GLM 原生协议 |
| `types.rs` | — | 适配器共享类型定义 |

## Protocol 枚举

`models.rs` 中定义 `Protocol` 枚举，包含 53 个变体，覆盖：

### OpenAI 系列
- `OpenAIChat` — Chat Completions（最通用）
- `OpenAICompletions` — Text Completions
- `OpenAIResponses` — Responses API

### Anthropic
- `Anthropic` — Claude Messages API

### Google
- `Gemini` — Generate Content API

### 国内平台
- `DeepSeek`, `Qwen`, `GLM`, `Kimi`, `MiniMax` 等

### 协议族
每个平台可以有多个协议变体，例如 DeepSeek 可用 `openai_chat` 兼容协议。

## 出站协议决策：端点匹配优先

出站 wire 协议**不是**无条件取平台主协议，而是先按入站协议在 `platform.endpoints` 里找匹配端点
（`proxy.rs` `matched_ep`）：

1. **精确匹配** — `endpoints` 中存在 `protocol == 入站协议` 的端点 → 用该端点的协议 / `base_url` / `client_type` / `coding_plan`。
2. **跨协议回退** — 入站 `openai_responses`（Codex）若无 Responses 端点，回退到 `openai` 端点（出站经 `to_openai` 真转换）。
3. **无匹配回退** — 无任何匹配端点 → 回退平台 `platform_type` + `ClientType::Default`，`convert_request` 转平台主协议。

## 同协议透传优先（跳过有损格式转换）

当**精确匹配**命中（端点协议 == 入站协议）时走**逻辑透传**：跳过 `convert_request`（请求）与
`parse_sse → to_client_sse`（响应）的 `from_*→to_*` 有损往返，直接用客户端原始请求体 / 上游原始 SSE 转发。

透传**不等于** ClaudeCode 那种纯字节透传，以下旁路改写仍全部保留：

1. **model remap** — 仅 patch 请求体 `model` 字段为路由目标模型（不重序列化 messages/tools）。
2. **鉴权改写** — 注入平台 `api_key` + `client_type` 模拟 header（非原样保留客户端鉴权）。
3. **URL 构造** — `passthrough_api_path()` 产出 wire path（与 `convert_request` 一致），`base_url`（端点）+ path，遵守版本前缀约束。
4. **coding_plan 注入** — 端点 `coding_plan=true` 时仍注入平台特有字段。
5. **usage 提取** — 响应侧仍 `accumulate_sse_usage` 提取 token，est_cost / 统计不丢。

跨协议回退（如 `openai_responses→openai`）与无匹配回退**不透传**，仍走 `convert_request` 真转换。

## 转换流程（非透传 / 跨协议回退路径）

```
入站请求 (openai_chat)
  → parse_incoming_request() 检测协议
  → 提取模型名、消息、参数
  → 内部统一格式
  → 目标平台协议 (anthropic)
  → convert_request() 生成出站请求
  → 发送到上游
```

## 关键设计决策

1. **入站检测基于路径** — 不同路径映射到不同协议，同一代理地址服务多协议客户端
2. **出站端点匹配优先** — 先按入站协议匹配平台端点；精确同协议 → 透传，跨协议/无匹配 → `convert_request` 转换
3. **同协议透传跳过有损往返** — 仅当平台显式声明同协议端点；保留 model remap / 鉴权 / URL / coding_plan / usage 五项旁路改写
4. **URL 构造规则** — `base_url` 含版本前缀，`provider_api_path()` 只返回路径后缀，最终 URL = 拼接
