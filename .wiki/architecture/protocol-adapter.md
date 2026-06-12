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

## 转换流程

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
2. **入站/出站解耦** — 入站 openai_chat → 出站 anthropic，自动协议转换
3. **URL 构造规则** — `base_url` 含版本前缀，`provider_api_path()` 只返回路径后缀，最终 URL = 拼接
