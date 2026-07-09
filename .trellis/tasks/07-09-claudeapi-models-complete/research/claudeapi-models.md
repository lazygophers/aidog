# Research: ClaudeAPI 全量模型清单与 Endpoints 核验

- **Query**: claudeapi.com 全站调研 + docs + pricing，查清平台定位、endpoints、全量模型清单、鉴权方式
- **Scope**: 外部调研（claudeapi.com / docs.claudeapi.com / apito.ai 博客）
- **Date**: 2026-07-09

## 数据来源表

| URL | 访问日期 | 状态码 | 备注 |
|-----|---------|-------|------|
| https://claudeapi.com | 2026-07-09 | 200 | 首页，含 API 示例 |
| https://claudeapi.com/en/ | 2026-07-09 | 200 | 英文首页 |
| https://docs.claudeapi.com | 2026-07-09 | 301 | 重定向到 www.claudeapi.com |
| https://docs.claudeapi.com/ | 2026-07-09 | 200 | 文档首页（中文/English） |
| https://claudeapi.com/pricing | 2026-07-09 | **404** | **需修正** |
| https://apito.ai/en/blog/getting-started/claude-api-model-id-list/ | 2026-07-09 | 200 | 完整模型 ID 列表 |
| https://apito.ai/en/blog/pricing/claude-api-pricing-guide/ | 2026-07-09 | 200 | 定价指南 |
| https://apito.ai/en/blog/news/claude-fable-5-api-guide/ | 2026-07-09 | 200 | Fable 5 模型指南 |
| https://apito.ai/en/blog/news/claude-sonnet-5-api-guide/ | 2026-07-09 | 200 | Sonnet 5 模型指南 |
| https://console.claudeapi.com/ | 2026-07-09 | 200 | 控制台首页 |

## 平台定位

**ClaudeAPI 是纯 Claude 代理平台，非多供应商聚合。**

首页明确声明：
> "We're not another 'all-in-one' AI gateway. Everything we build and optimize is centered on one frontier AI model family"

- **定位**：Anthropic Claude API 第三方代理
- **优势**：比官方便宜 20%（80% of Anthropic published prices）
- **上游**：官方 API + AWS Bedrock
- **计费**：按 token 付费，USD，无订阅/最低消费

## API Endpoints 核验

### 现有配置（preset line 2713-2764）

```json
"endpoints": {
  "default": [
    {
      "protocol": "anthropic",
      "base_url": "https://gw.claudeapi.com",
      "client_type": "claude_code"
    }
  ]
}
```

### 核验结果

| 检查项 | 结果 | 说明 |
|-------|------|------|
| 仅 1 个 anthropic endpoint | **正确** | `gw.claudeapi.com` 确认为唯一 endpoint |
| 是否缺 openai endpoint | **否** | 博客确认支持 OpenAI 兼容模式（见下） |
| 是否缺 gemini endpoint | **否** | 平台仅支持 Claude 系列 |

### 协议兼容性（重要发现）

博客文章显示 **同时支持 OpenAI 兼容模式**：

| 模式 | Base URL | 说明 |
|-----|----------|------|
| Anthropic-style | `https://gw.claudeapi.com` | 标准 Anthropic SDK |
| OpenAI-compatible | `https://gw.claudeapi.com/v1` | OpenAI SDK/工具兼容 |

**Endpoint 探测结果**：
- `POST /v1/models` → 返回 `Invalid token`（端点存在，需有效 token）
- `POST /v1/chat/completions` → 返回 `Invalid token`（端点存在，需有效 token）

**结论**：现有 1 个 anthropic endpoint 配置**正确**，但实际支持双协议模式。aidog 若需支持 OpenAI 工具接入，可考虑新增 openai protocol endpoint（base_url 同 `gw.claudeapi.com`，但协议格式不同）。

## 鉴权方式

| 参数 | 值 |
|-----|---|
| Header | `x-api-key: $API_KEY` |
| API Key 格式 | `sk-...`（在 console.claudeapi.com 创建） |
| SDK 示例 | `anthropic.Anthropic(api_key="your-api-key", base_url="https://gw.claudeapi.com")` |

## 全量模型清单

### 当前 preset 配置（7 个）

```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-5"
  ]
}
```

### 实际完整模型清单（来源：官方博客）

#### Opus 4 Family（4 个）
| Model ID | Context | ClaudeAPI Price | Status |
|----------|---------|-----------------|--------|
| `claude-opus-4-8` | 1M | $4 / $20 per MTok | Available |
| `claude-opus-4-7` | 1M | $4 / $20 per MTok | Available |
| `claude-opus-4-6` | 1M | $4 / $20 per MTok | Available |
| `claude-opus-4-5-20251101` | 200K | $4 / $20 per MTok | Available |

#### Sonnet 4 Family（2 个）
| Model ID | Context | ClaudeAPI Price | Status |
|----------|---------|-----------------|--------|
| `claude-sonnet-4-6` | 1M | $2.4 / $12 per MTok | Available |
| `claude-sonnet-4-5-20250929` | 200K | $2.4 / $12 per MTok | Available |

#### Haiku 4 Family（1 个）
| Model ID | Context | ClaudeAPI Price | Status |
|----------|---------|-----------------|--------|
| `claude-haiku-4-5-20251001` | 200K | $0.8 / $4 per MTok | Available |

#### **新增 2026 年模型（2 个）**

| Model ID | 发布日期 | 说明 |
|----------|---------|------|
| `claude-fable-5` | 2026-06-09 | 最强广泛可用模型，复杂推理/长时 agent/代码工程 |
| `claude-sonnet-5` | 2026-06-30 | Sonnet 5 新版，长上下文，推荐用于复杂工作 |

**总计：10 个模型**（Opus 4 + Sonnet 2 + Haiku 1 + Fable 5 + Sonnet 5）

### Model ID 格式

- **标准格式**：`claude-{family}-{major}-{minor}`（如 `claude-opus-4-8`）
- **日期后缀格式**：`claude-{family}-{major}-{minor}-{YYYYMMDD}`（如 `claude-opus-4-5-20251101`）
- **特殊规则**：Haiku **必须**带完整日期后缀，不能简写

**示例**：
```
Correct: claude-haiku-4-5-20251001
Wrong:   claude-haiku-4-5
```

### 现有 7 模型核对

| preset 模型 | 实际正确 ID | 状态 |
|------------|-------------|------|
| `claude-opus-4-8` | ✓ 正确 | 保留 |
| `claude-sonnet-4-6` | ✓ 正确 | 保留 |
| `claude-haiku-4-5` | **错误** | 需改为 `claude-haiku-4-5-20251001` |
| `claude-opus-4-7` | ✓ 正确 | 保留 |
| `claude-opus-4-6` | ✓ 正确 | 保留 |
| `claude-opus-4-5` | **需日期后缀** | 需改为 `claude-opus-4-5-20251101` |
| `claude-sonnet-4-5` | **需日期后缀** | 需改为 `claude-sonnet-4-5-20250929` |

### 缺失模型（需补充）

- `claude-fable-5`（2026-06-09 新增）
- `claude-sonnet-5`（2026-06-30 新增）

## 三档默认推荐

根据博客官方推荐和定价梯度：

```json
"models": {
  "default": {
    "default": "claude-opus-4-8",
    "opus": "claude-opus-4-8",
    "fable": "claude-fable-5",
    "sonnet": "claude-sonnet-5",
    "haiku": "claude-haiku-4-5-20251001"
  }
}
```

**理由**：
- **default / opus**: `claude-opus-4-8` 是 Opus 4 系列最新最强模型（博客推荐："Use `claude-opus-4-8` for new projects"）
- **fable**: `claude-fable-5` 是 2026 年最强广泛可用模型
- **sonnet**: `claude-sonnet-5` 是最新 Sonnet 5 系列
- **haiku**: `claude-haiku-4-5-20251001` 必须完整日期后缀

## desc 是否失实

当前：
```
en-US: "ClaudeAPI proxy for Claude-compatible models"
zh-Hans: "ClaudeAPI 中转, Claude 兼容模型"
```

**判定：需改写**

现有描述 "Claude 兼容模型" 可能被误解为支持其他厂商的 Claude 兼容模型。实际平台是 **纯 Anthropic Claude 代理**，建议改为：

```
en-US: "Anthropic Claude API proxy, 20% cheaper than official"
zh-Hans: "Anthropic Claude API 中转，官价 8 折"
```

或更保守：

```
en-US: "Third-party Anthropic Claude API proxy"
zh-Hans: "Anthropic Claude API 第三方中转"
```

## source_urls 核验

当前配置（line 2758-2760）：
```json
"source_urls": {
  "docs": "https://docs.claudeapi.com/",
  "pricing": "https://claudeapi.com/pricing"
}
```

### 核验结果

| URL | 状态 | 建议修正 |
|-----|------|---------|
| `https://docs.claudeapi.com/` | **301 重定向** | 保留（会自动跳转）或改为 `https://claudeapi.com/en/` |
| `https://claudeapi.com/pricing` | **404** | **必须修正** |

### 修正建议

```json
"source_urls": {
  "docs": "https://apito.ai/en/blog/getting-started/claude-api-model-id-list/",
  "pricing": "https://apito.ai/en/blog/pricing/claude-api-pricing-guide/"
}
```

理由：
- `docs.claudeapi.com` 已重定向到主站，文档内容托管在 `apito.ai` 博客系统
- 模型列表和定价指南均在 `apito.ai` 博客，是权威文档

## 建议补全

### model_list 补全

```json
"model_list": {
  "default": [
    "claude-fable-5",
    "claude-sonnet-5",
    "claude-opus-4-8",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5-20251101",
    "claude-sonnet-4-6",
    "claude-sonnet-4-5-20250929",
    "claude-haiku-4-5-20251001"
  ]
}
```

### models.default 补全

```json
"models": {
  "default": {
    "default": "claude-opus-4-8",
    "opus": "claude-opus-4-8",
    "fable": "claude-fable-5",
    "sonnet": "claude-sonnet-5",
    "haiku": "claude-haiku-4-5-20251001"
  }
}
```

### desc 改写

```json
"desc": {
  "en-US": "Third-party Anthropic Claude API proxy, 20% cheaper than official",
  "zh-Hans": "Anthropic Claude API 第三方中转，官价 8 折",
  "ar-SA": "وكيل Claude API التابع لجهة خارجية، أرخص بنسبة 20% من الرسمي",
  "fr-FR": "Proxy API Anthropic Claude tiers, 20% moins cher que l'officiel",
  "de-DE": "Anthropic Claude API Proxy von Drittanbietern, 20% günstiger als offiziell",
  "ru-RU": "Сторонний прокси Anthropic Claude API, на 20% дешевле официального",
  "ja-JP": "Anthropic Claude API サードパーティ中継、公式価格の20%割引",
  "es-ES": "Proxy API Anthropic Claude de terceros, 20% más barato que el oficial"
}
```

### source_urls 修正

```json
"source_urls": {
  "docs": "https://apito.ai/en/blog/getting-started/claude-api-model-id-list/",
  "pricing": "https://apito.ai/en/blog/pricing/claude-api-pricing-guide/"
}
```

## Caveats / Not Found

1. **模型可用性**：博客强调 "Model availability can change"，生产环境需通过 console 或 `/v1/models` 端点确认实际可用模型
2. **日期后缀规则**：仅部分模型需要日期后缀（Opus 4.5 / Sonnet 4.5 / Haiku 4.5），新模型（Opus 4.6+ / Sonnet 4.6+ / Fable 5 / Sonnet 5）不需要
3. **Fable 5 / Sonnet 5 访问限制**：博客提到这些新模型可能有账户级别访问限制，需在 console 确认
4. **OpenAI 兼容模式**：虽然支持 `/v1` 前缀的 OpenAI 格式，但 aidog 当前 preset 无 openai protocol endpoint，需确认是否需要新增

## Cross-reference

- **preset 文件**: `src-tauri/defaults/platform-presets.json`
- **claudeapi 协议**: line 2713-2764
- **当前配置**: 1 endpoint (anthropic) / 7 models / 空 models.default / 失实 desc / 404 pricing URL

## 关键发现总结

1. **平台定位**: Claude-only 代理，非多供应商聚合
2. **Endpoints**: 现有 1 个 anthropic endpoint **正确**，但实际支持 OpenAI 兼容模式（`/v1` 前缀）
3. **desc 需改写**: "Claude 兼容模型" 误导，改为 "Anthropic Claude API 第三方中转"
4. **模型总数**: 10 个（当前 preset 缺 `claude-fable-5` / `claude-sonnet-5` + 部分日期后缀错误）
5. **source_urls 需修正**: pricing URL 404，改为 apito.ai 博客地址
