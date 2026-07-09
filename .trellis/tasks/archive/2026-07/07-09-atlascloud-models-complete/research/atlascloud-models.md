# Research: AtlasCloud (atlascloud 协议)

- **Query**: 补全 AtlasCloud 聚合路由平台的 platform-presets.json 配置（模型清单 / 端点 / 默认模型）
- **Scope**: external（官方 API 端点 + 文档）
- **Date**: 2026-07-09

## 官方支持模型清单

来源：`curl -s https://api.atlascloud.ai/v1/models`（OpenAI 兼容端点）

**总计：114 个模型**

### Provider 分布统计

| Provider | 模型数 | 说明 |
|----------|--------|------|
| openai | 30 | GPT-4/5 系列、o1/o3 系列、image-2 |
| anthropic | 20 | Claude Opus/Sonnet/Haiku 系列（含 coding 后缀） |
| google | 13 | Gemini 2.0/2.5/3 系列（含 flash/pro/image/lite） |
| qwen | 11 | Qwen3.5/3.6/3.7 系列 |
| bytedance | 10 | Doubao Seed 1.6/1.8/2.0/2.1 系列 |
| deepseek-ai | 7 | DeepSeek V3.1/V3.2/V4 系列 |
| zai-org | 7 | GLM-4/5 系列 |
| moonshotai | 3 | Kimi K2.5/K2.6/K2.7-code |
| minimaxai | 3 | MiniMax M2.5/M2.7/M3 |
| kwaipilot | 3 | KAT Coder Air/Pro 系列 |
| xai | 2 | Grok 4.3 / build-0.1 |
| xiaomi | 2 | Mimo v2.5 / v2.5-pro |
| Qwen | 1 | Qwen3-235B-A22B-Instruct-2507（大小写特殊） |
| meituan-longcat | 1 | longcat-2.0 |
| tencent | 1 | hy3 |

### 完整模型清单（按 Provider 分组）

```
[anthropic] 20 个
  anthropic/claude-haiku-4.5-20251001
  anthropic/claude-haiku-4.5-20251001-coding
  anthropic/claude-opus-4-20250514
  anthropic/claude-opus-4-20250514-coding
  anthropic/claude-opus-4.1-20250805
  anthropic/claude-opus-4.1-20250805-coding
  anthropic/claude-opus-4.5-20251101
  anthropic/claude-opus-4.5-20251101-coding
  anthropic/claude-opus-4.6
  anthropic/claude-opus-4.6-coding
  anthropic/claude-opus-4.7
  anthropic/claude-opus-4.7-coding
  anthropic/claude-opus-4.8
  anthropic/claude-opus-4.8-coding
  anthropic/claude-sonnet-4-20250514
  anthropic/claude-sonnet-4-20250514-coding
  anthropic/claude-sonnet-4.5-20250929
  anthropic/claude-sonnet-4.5-20250929-coding
  anthropic/claude-sonnet-4.6
  anthropic/claude-sonnet-4.6-coding

[bytedance] 10 个
  bytedance/doubao-seed-1.6-251015
  bytedance/doubao-seed-1.6-flash-250828
  bytedance/doubao-seed-1.8-251228
  bytedance/doubao-seed-2.0-code-preview-260215
  bytedance/doubao-seed-2.0-lite-260428
  bytedance/doubao-seed-2.0-mini-260428
  bytedance/doubao-seed-2.0-pro-260215
  bytedance/doubao-seed-2.1-pro-260628
  bytedance/doubao-seed-2.1-turbo-260628
  bytedance/doubao-seed-evolving

[deepseek-ai] 7 个
  deepseek-ai/DeepSeek-V3.1
  deepseek-ai/DeepSeek-V3.1-Terminus
  deepseek-ai/DeepSeek-V3.2-Exp
  deepseek-ai/deepseek-ocr
  deepseek-ai/deepseek-v3.2
  deepseek-ai/deepseek-v4-flash
  deepseek-ai/deepseek-v4-pro

[google] 13 个
  google/gemini-2.0-flash
  google/gemini-2.0-flash-lite
  google/gemini-2.5-flash
  google/gemini-2.5-flash-image
  google/gemini-2.5-flash-lite
  google/gemini-2.5-pro
  google/gemini-3-flash-preview
  google/gemini-3-pro-image-preview
  google/gemini-3.1-flash-image
  google/gemini-3.1-flash-image-preview
  google/gemini-3.1-flash-lite
  google/gemini-3.1-pro-preview
  google/gemini-3.5-flash

[kwaipilot] 3 个
  kwaipilot/kat-coder-air-v2.5
  kwaipilot/kat-coder-pro-v2
  kwaipilot/kat-coder-pro-v2.5

[meituan-longcat] 1 个
  meituan-longcat/longcat-2.0

[minimaxai] 3 个
  minimaxai/minimax-m2.5
  minimaxai/minimax-m2.7
  minimaxai/minimax-m3

[moonshotai] 3 个
  moonshotai/kimi-k2.5
  moonshotai/kimi-k2.6
  moonshotai/kimi-k2.7-code

[openai] 30 个
  openai/gpt-4.1
  openai/gpt-4.1-mini
  openai/gpt-4.1-nano
  openai/gpt-4o
  openai/gpt-4o-mini
  openai/gpt-5
  openai/gpt-5-chat
  openai/gpt-5-codex
  openai/gpt-5-mini
  openai/gpt-5-nano
  openai/gpt-5-pro
  openai/gpt-5.1
  openai/gpt-5.1-chat
  openai/gpt-5.1-codex
  openai/gpt-5.1-codex-max
  openai/gpt-5.1-codex-mini
  openai/gpt-5.2
  openai/gpt-5.2-chat
  openai/gpt-5.2-codex
  openai/gpt-5.3-codex
  openai/gpt-5.4
  openai/gpt-5.4-mini
  openai/gpt-5.4-nano
  openai/gpt-5.5
  openai/gpt-image-2
  openai/o1
  openai/o3
  openai/o3-mini
  openai/o3-pro
  openai/o4-mini

[Qwen] 1 个（注意：Q 大写，是唯一例外）
  Qwen/Qwen3-235B-A22B-Instruct-2507

[qwen] 11 个（q 小写）
  qwen/qwen3-vl-235b-a22b-thinking
  qwen/qwen3.5-122b-a10b
  qwen/qwen3.5-27b
  qwen/qwen3.5-35b-a3b
  qwen/qwen3.5-397b-a17b
  qwen/qwen3.5-flash
  qwen/qwen3.5-plus
  qwen/qwen3.6-35b-a3b
  qwen/qwen3.6-plus
  qwen/qwen3.7-max
  qwen/qwen3.7-plus

[tencent] 1 个
  tencent/hy3

[xai] 2 个
  xai/grok-4.3
  xai/grok-build-0.1

[xiaomi] 2 个
  xiaomi/mimo-v2.5
  xiaomi/mimo-v2.5-pro

[zai-org] 7 个
  zai-org/GLM-4.6
  zai-org/glm-4.7
  zai-org/glm-5
  zai-org/glm-5-turbo
  zai-org/glm-5.1
  zai-org/glm-5.2
  zai-org/glm-5v-turbo
```

## 模型 ID 命名格式

官方 API 接受的格式：`provider/model-name`

- **provider**：全小写（唯一例外 `Qwen/` 是大写 Q）
- **model-name**：保留各 provider 原始命名习惯（大小写混合）

**规则**：直接使用 `/v1/models` 返回的 `id` 字段，不做任何转换。

## preset 现状 11 项核实

| 现有 preset | 官方存在 | 匹配状态 |
|------------|---------|---------|
| `deepseek-ai/DeepSeek-V3.2-Exp` | ✓ | 匹配 |
| `deepseek-ai/DeepSeek-V3.1-Terminus` | ✓ | 匹配 |
| `deepseek-ai/DeepSeek-V3-0324` | ✗ | **不存在**（可能是旧命名） |
| `zai-org/GLM-4.6` | ✓ | 匹配 |
| `Qwen/Qwen3-235B-A22B-Instruct-2507` | ✓ | 匹配（Q 大写是官方规范） |
| `Qwen/Qwen3-Coder` | ✗ | **不存在** |
| `Qwen/Qwen3-Next-80B-A3B-Instruct` | ✗ | **不存在** |
| `Qwen/Qwen3-VL-235B-A22B-Instruct` | ✗ | **不存在** |
| `moonshotai/Kimi-K2-Thinking` | ✗ | **大小写错误**（官方小写 kimi-k2.5） |
| `moonshotai/Kimi-K2-Instruct-0905` | ✗ | **不存在** |
| `MiniMaxAI/MiniMax-M2` | ✗ | **大小写错误**（官方 minimaxai/minimax-m2.5） |

**结论**：
- 仅 4 项准确匹配（36%）
- 3 项大小写错误
- 4 项完全不存在（下架或从未存在）

## endpoints 核实

### 当前配置
```json
"endpoints": {
  "default": [
    {
      "protocol": "anthropic",
      "base_url": "https://api.atlascloud.ai",
      "client_type": "claude_code"
    }
  ]
}
```

### 核实结果

| 端点类型 | 路径 | 状态 | HTTP 响应 |
|---------|------|------|----------|
| Anthropic | `/v1/messages` | ✓ 存在 | 401 Unauthorized（需认证） |
| OpenAI | `/v1/chat/completions` | ✓ 存在 | 401 Unauthorized（需认证） |
| Models | `/v1/models` | ✓ 存在 | 200 OK（公开） |

### 建议

AtlasCloud **同时支持** Anthropic 和 OpenAI 兼容协议：

**方案 A（仅 Anthropic）**：保持现状
- 适用场景：Claude Code / claude-code 客户端
- 限制：OpenAI 格式客户端无法使用

**方案 B（双端点）**：补充 OpenAI 端点
```json
"endpoints": {
  "default": [
    {
      "protocol": "anthropic",
      "base_url": "https://api.atlascloud.ai",
      "client_type": "claude_code"
    },
    {
      "protocol": "openai",
      "base_url": "https://api.atlascloud.ai/v1",
      "client_type": "codex_tui"
    }
  ]
}
```

**Gemini 原生端点**：未检测到 Gemini 原生协议端点。

## models.default 建议

基于官方 114 模型的能力分析：

### 推荐配置

```json
"models": {
  "default": {
    "default": "openai/gpt-5.5",           // 主力通用对话
    "coder": "openai/gpt-5.3-codex",      // 编程专用
    "fast": "anthropic/claude-haiku-4.5-20251001",  // 快速响应
    "reasoning": "openai/o3",             // 推理专用
    "chinese": "qwen/qwen3.7-max"         // 中文优化
  }
}
```

### 选择依据

| 用途 | 模型 | 理由 |
|------|------|------|
| default | `openai/gpt-5.5` | OpenAI 最新主力，通用能力强 |
| coder | `openai/gpt-5.3-codex` | 专门标注 codex 后缀 |
| fast | `anthropic/claude-haiku-4.5-20251001` | Haiku 系列以快著称 |
| reasoning | `openai/o3` | OpenAI 推理系列 |
| chinese | `qwen/qwen3.7-max` | 通义千问中文场景优化 |

## 认证方式

- **方式**：API Key（Bearer Token）
- **Header**：`Authorization: Bearer <api_key>`
- **来源**：AtlasCloud 控制台

## 结论摘要

AtlasCloud 是聚合路由平台，官方支持 **114 个模型**，来自 **15 家 provider**。当前 preset 仅 11 项精选，准确率 36%（4/11），存在大小写错误和已下架模型。

**端点**：同时支持 Anthropic（`/v1/messages`）和 OpenAI（`/v1/chat/completions`）协议，当前仅配置 Anthropic 端点，建议补充 OpenAI 端点以支持更多客户端。

**模型 ID 格式**：`provider/model-name`，provider 全小写（唯一例外 `Qwen/`），直接使用 `/v1/models` 返回的 `id` 字段。

## 信息源

- **模型清单**：`curl -s https://api.atlascloud.ai/v1/models`（实时）
- **端点测试**：HTTP HEAD 请求（/v1/messages、/v1/chat/completions）
- **文档**：https://docs.atlascloud.ai/
- **计费**：https://atlascloud.ai/pricing
