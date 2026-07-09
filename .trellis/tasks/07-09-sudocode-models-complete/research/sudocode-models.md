# Research: SudoCode 全量模型与 Endpoints 调研

- **Query**: SudoCode (sudocode.us) 全量模型清单 + endpoints 形态 + 鉴权方式
- **Scope**: 外部 API/文档调研
- **Date**: 2026-07-09

---

## 数据来源表

| URL | 访问日期 | 状态码 | 用途 |
|-----|---------|--------|------|
| https://sudocode.us | 2026-07-09 | 200 | 首页确认平台定位 |
| https://docs.sudocode.us | 2026-07-09 | 200 | 文档（JS 渲染，需浏览器） |
| https://sudocode.us/pricing | 2026-07-09 | 200 | 价格页（JS 渲染） |
| https://sudocode.us/api/models | 2026-07-09 | 200 | **全量模型 API（免鉴权）** |
| https://sudocode.us/v1/models | 2026-07-09 | 400+ | OpenAI 格式端点（需鉴权） |
| https://sudocode.us/v1/chat/completions | 2026-07-09 | 400+ | OpenAI 聊天端点（需鉴权） |
| https://sudocode.us/v1/messages | 2026-07-09 | 400+ | Anthropic 消息端点（需鉴权） |
| https://sudocode.us/v1beta/models/...:generateContent | 2026-07-09 | 400+ | Gemini 端点（需鉴权） |

---

## 平台定位

**SudoCode 是多供应商聚合平台**，非 Claude-only。

文档首页描述：「SudoCode-全球一流AI模型聚合服务」，「一个API key多模型通用」。

支持 7 家供应商：
- Anthropic (Claude)
- OpenAI (GPT)
- Google (Gemini)
- MiniMax
- Moonshot (Kimi)
- 智谱 (GLM)
- DeepSeek

---

## API Endpoints 核验

### 现有 preset 配置（`src-tauri/defaults/platform-presets.json:2661-2712`）

```json
{
  "endpoints": {
    "default": [{
      "protocol": "anthropic",
      "base_url": "https://sudocode.us",
      "client_type": "claude_code"
    }]
  }
}
```

**问题**：仅 1 个 `anthropic` endpoint，**缺失 openai 和 gemini 端点**。

### 支持的端点类型（来自 `/api/models` 响应）

| 端点类型 | 路径 | 方法 | 探测状态 |
|---------|------|------|---------|
| `anthropic` | `/v1/messages` | POST | exists |
| `openai` | `/v1/chat/completions` | POST | exists |
| `gemini` | `/v1beta/models/{model}:generateContent` | POST | exists |
| `image-generation` | `/v1/images/generations` | POST | exists |
| `openai-response` | `/v1/responses` | POST | exists |

**核验方式**：curl 探测各端点，返回 `{"error":{"message":"无效的令牌"}}`（非 404），确认端点存在。

### 建议补全 endpoints

```json
{
  "endpoints": {
    "default": [{
      "protocol": "anthropic",
      "base_url": "https://sudocode.us",
      "client_type": "claude_code"
    }],
    "openai": [{
      "protocol": "openai",
      "base_url": "https://sudocode.us",
      "client_type": "default"
    }],
    "gemini": [{
      "protocol": "gemini",
      "base_url": "https://sudocode.us",
      "client_type": "default"
    }]
  }
}
```

---

## 全量模型清单

按 vendor_id 分组（来自 `/api/models`）：

| 供应商 | vendor_id | 模型名称 | 支持端点类型 |
|-------|-----------|---------|-------------|
| Anthropic | 1 | claude-opus-4-8 | anthropic, openai |
| Anthropic | 1 | claude-sonnet-4-6 | anthropic, openai |
| Anthropic | 1 | claude-haiku-4-5-20251001 | anthropic, openai |
| Anthropic | 1 | claude-fable-5 | anthropic, openai |
| Anthropic | 1 | claude-sonnet-4-5-20250929 | anthropic, openai |
| Anthropic | 1 | claude-sonnet-5 | anthropic, openai |
| Anthropic | 1 | claude-opus-4-6 | anthropic, openai |
| Anthropic | 1 | claude-opus-4-7 | anthropic, openai |
| Anthropic | 1 | claude-opus-4-5-20251101 | anthropic, openai |
| Google | 2 | gemini-3-pro-preview | gemini, openai |
| Google | 2 | gemini-3.5-flash | gemini, openai |
| Google | 2 | gemini-3.1-flash-image-preview | gemini, openai |
| Google | 2 | gemini-3.1-flash-lite | gemini, openai |
| Google | 2 | gemini-3-flash-preview | gemini, openai |
| Google | 2 | gemini-3.1-flash-lite-preview | gemini, openai |
| Google | 2 | gemini-3.1-pro-preview | gemini, openai |
| OpenAI | 3 | gpt-5.4 | openai |
| OpenAI | 3 | gpt-5.5 | openai |
| OpenAI | 3 | gpt-5.3-codex | openai |
| OpenAI | 3 | gpt-5.4-mini | openai |
| OpenAI | 3 | gpt-image-2 | image-generation, openai |
| MiniMax | 4 | MiniMax-M2.7 | openai |
| MiniMax | 4 | MiniMax-M2.5 | openai |
| Moonshot | 5 | kimi-k2.7-code | anthropic, openai |
| Moonshot | 5 | kimi-k2.6 | anthropic, openai |
| 智谱 | 6 | glm-5.1 | openai, anthropic |
| 智谱 | 6 | glm-5.2 | openai, anthropic |
| DeepSeek | 7 | deepseek-v4-pro | openai |
| DeepSeek | 7 | deepseek-v4-flash | openai |

**共计 30 模型**（9 Claude + 6 Gemini + 5 GPT + 2 MiniMax + 2 Kimi + 2 GLM + 2 DeepSeek + 1 生图 + 1 codex）

---

## 现有 7 模型核对表

| 现有 preset 模型 | API 响应中是否存在 | 日期后缀状态 |
|----------------|------------------|-------------|
| claude-opus-4-8 | exists | 无日期后缀 |
| claude-sonnet-4-6 | exists | 无日期后缀 |
| claude-haiku-4-5 | exists | API 返回 `claude-haiku-4-5-20251001` |
| claude-opus-4-7 | exists | 无日期后缀 |
| claude-opus-4-6 | exists | 无日期后缀 |
| claude-opus-4-5 | exists | API 返回 `claude-opus-4-5-20251101` |
| claude-sonnet-4-5 | exists | API 返回 `claude-sonnet-4-5-20250929` |

**结论**：
- 7 个模型全部存在于 API
- 3 个模型在 API 中带日期后缀：`haiku-4-5-20251001`、`opus-4-5-20251101`、`sonnet-4-5-20250929`
- aidog 使用内部 alias（无日期），与 aicodemirror 模式一致

---

## 三档默认推荐

### 建议的 models.default

```json
{
  "models": {
    "default": {
      "default": "claude-sonnet-4-6",
      "opus": "claude-opus-4-8",
      "haiku": "claude-haiku-4-5"
    }
  }
}
```

**理由**：
- `default`: Claude Sonnet 4.6 为中端主力，性价比高（model_ratio=1.5）
- `opus`: Opus 4.8 为当前最高端（model_ratio=2.5）
- `haiku`: Haiku 4.5 为入门级（model_ratio=0.5）

注：API 中 haiku 完整 id 为 `claude-haiku-4-5-20251001`，但 aidog 可用内部 alias `claude-haiku-4-5`。

---

## model_id 格式分析

- **Claude 系列**：使用 aidog 内部 alias（无日期），与 aicodemirror 模式一致
- **其他供应商**：使用原生 model id（如 `gpt-5.4`、`gemini-3-pro-preview`）
- **无前缀**：所有 model id 不带 `provider/` 前缀

---

## desc 核验

当前描述：
- en-US: "SudoCode API for Claude-compatible models"
- zh-Hans: "SudoCode API, Claude 兼容模型"

**问题**：描述低估了平台能力。SudoCode 是多供应商聚合（7 家），非 Claude-only。

**建议改写**：
- en-US: "SudoCode API - aggregated access to Claude, GPT, Gemini, and domestic models"
- zh-Hans: "SudoCode API - 聚合 Claude/GPT/Gemini/国产模型，一键调用"

---

## source_urls 核验

| URL | 状态 |
|-----|------|
| https://docs.sudocode.us/ | 200 OK |
| https://sudocode.us/pricing | 200 OK |
| https://sudocode.us | 200 OK |

**结论**：全部存活，无需修正。

---

## Caveats / Not Found

1. **价格信息**：`/api/pricing` 端点不存在（推测需要鉴权或未公开），无法获取价格结构化数据
2. **文档详情**：docs.sudocode.us 为 JS 渲染页面，curl 无法提取内容，需浏览器访问
3. **Gemini 路径**：API 返回的 gemini 端点为 `/v1beta/models/{model}:generateContent`，需动态替换 model 占位符
4. **group 权限**：模型按 group 分级（dev/pro/ent/special/test），非所有用户可用全部模型

---

## 建议补全 action items

1. **补 endpoints**：加 `openai` 和 `gemini` 两个端点到 preset
2. **扩 model_list**：从 7 个扩到 30 个（全量）
3. **填 models.default**：三档默认推荐
4. **改 desc**：修正为多供应商聚合描述
5. **核 alias 约定**：确认 Claude 系列使用无日期后缀 alias（haiku-4-5 而非 haiku-4-5-20251001）

---

## Cross-reference

- Preset 路径：`src-tauri/defaults/platform-presets.json:2661-2712`
- Protocol 枚举：需在 Rust `src-tauri/src/gateway/models.rs` 和 TS `src/services/api.ts` 确认已有 `sudocode` 变体
- 相关文档：
  - `.wiki/modules/pricing.md` — peak_hours 机制
  - `.trellis/spec/backend/adapter-layer.md` — 协议转换逻辑

---

## 附录：完整 API 响应（略）

完整 `/api/models` 响应已保存，含 30 模型 + vendor 信息 + group 权限 + 端点路径。
