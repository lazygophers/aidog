# 补全 APINebula model_list+endpoints 全部官方信息

## Goal
APINebula 是多供应商聚合平台（homepage 描述「unified AI model hub for aggregation & distribution，supports cross-converting LLMs into OpenAI/Claude/Gemini-compatible formats」），非纯 Claude 代理。pricing 页公开（200 OK）列出全量 20 模型（8 Claude + 5 GPT + 6 Gemini + 1 custom）。现有 preset 仅 7 个 Claude alias。desc 描述「Claude 兼容模型」失实。本次改动：修正 desc 8 语言、补全 model_list 至 20 款、补 models.default 三档、endpoints 与 source_urls 保留不变。

## Research References
- [`research/apinebula-models.md`](research/apinebula-models.md) — pricing 页公开列出全量 20 模型 ID；3 协议 endpoint 经 docs 子页核验全部正确；无 /v1/models 公开端点（404）；claude-opus-4-5 未在 pricing 发现（存疑保留）。

## Requirements

### 1. endpoints（default 分支，3 端点，全部保留）
现有 3 端点经 research 文档核验全部正确：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://apinebula.com", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://apinebula.com/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://apinebula.com", "client_type": "default"}
  ]
}
```

### 2. model_list.default（20 模型，裸 id 无 provider/ 前缀）

按 Claude → GPT → Gemini → Custom 分组：

**Claude 系（8 款）：**
- `claude-opus-4-8`（保留 alias）
- `claude-fable-5`（新增，最新顶级）
- `claude-sonnet-5`（新增）
- `claude-opus-4-7`（保留 alias）
- `claude-sonnet-4-6`（保留 alias）
- `claude-opus-4-6`（保留 alias）
- `claude-haiku-4-5`（保留 alias；research 真值含 -20251001，按 alias 约定保持裸 id）
- `claude-sonnet-4-5`（保留 alias；research 真值含 -20250929，按 alias 约定保持裸 id）

> 注：`claude-opus-4-5`（现有 preset 中有）pricing 页未发现，research 标注「可能已下架或未公开计费」。按 research 建议删除。

**GPT 系（5 款，全部新增）：**
- `gpt-5.5`
- `gpt-5.4`
- `gpt-5.4-mini`
- `gpt-5.5-openai-compact`
- `gpt-image-2`（图像模型，按次计费；research 列入，保留以与 pricing 一致）

**Gemini 系（6 款，全部新增）：**
- `gemini-3.1-pro-preview`
- `gemini-3.5-flash`
- `gemini-2.5-pro`
- `gemini-2.5-flash-lite`
- `gemini-3-pro-image-preview`（图像模型，按次计费）
- `gemini-3.1-flash-image-preview`（图像模型，按量计费）

**Custom（1 款，新增）：**
- `codex-auto-review`（Codex 专属代码审查模型）

最终 model_list.default：
```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-fable-5",
    "claude-sonnet-5",
    "claude-opus-4-7",
    "claude-sonnet-4-6",
    "claude-opus-4-6",
    "claude-haiku-4-5",
    "claude-sonnet-4-5",
    "gpt-5.5",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.5-openai-compact",
    "gpt-image-2",
    "gemini-3.1-pro-preview",
    "gemini-3.5-flash",
    "gemini-2.5-pro",
    "gemini-2.5-flash-lite",
    "gemini-3-pro-image-preview",
    "gemini-3.1-flash-image-preview",
    "codex-auto-review"
  ]
}
```

### 3. models.default（三档，档位名 key → model id string，对齐 `Partial<Record<ModelSlot, string>>` + 20 官方 protocol）

```json
"models": {
  "default": {
    "default": "claude-sonnet-4-6",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

三档选型理由：
- 主力 `claude-sonnet-4-6`：平衡型 Claude 默认 → slot `default`
- 重型 `claude-opus-4-8`：Claude 旗舰（research 标注 0.74 折性价比高） → slot `opus`
- 轻量 `claude-haiku-4-5`：成本敏感场景 → slot `haiku`

### 4. desc（改写 8 语言）

平台定位从「Claude 兼容模型」改为「AI 模型聚合平台，支持 OpenAI/Claude/Gemini 协议」：

```json
"desc": {
  "zh-Hans": "APINebula AI 模型聚合平台，支持 OpenAI/Claude/Gemini 协议",
  "en-US": "APINebula unified AI model hub (OpenAI/Claude/Gemini protocols)",
  "ar-SA": "مركز APINebula الموحد لنماذج الذكاء الاصطناعي (بروتوكولات OpenAI/Claude/Gemini)",
  "fr-FR": "Hub AI unifié APINebula (protocoles OpenAI/Claude/Gemini)",
  "de-DE": "APINebula einheitliches AI-Modell-Hub (OpenAI/Claude/Gemini-Protokolle)",
  "ru-RU": "Единый хаб AI-моделей APINebula (протоколы OpenAI/Claude/Gemini)",
  "ja-JP": "APINebula 統合 AI モデルハブ（OpenAI/Claude/Gemini プロトコル）",
  "es-ES": "Hub unificado de modelos AI APINebula (protocolos OpenAI/Claude/Gemini)"
}
```

### 5. source_urls（保留）

现有 source_urls 经 research 验证有效（docs 200 OK / pricing 200 OK）：
```json
"source_urls": {
  "docs": "https://docs.apinebula.com/",
  "pricing": "https://apinebula.com/pricing"
}
```

## Acceptance Criteria
- [ ] endpoints 3（不变）
- [ ] model_list 20（8 Claude + 5 GPT + 6 Gemini + 1 custom）
- [ ] models.default 三档 档位名 key → string（default=claude-sonnet-4-6 / opus=claude-opus-4-8 / haiku=claude-haiku-4-5）
- [ ] desc 8 语言改写为 AI 模型聚合平台
- [ ] source_urls 保留不变
- [ ] JSON 合法
- [ ] 仅改 protocols.apinebula 块

## Out of Scope
- 上下文窗口字段
- STATIC_MODEL_IDS
- peak_hours / coding_plan 分支
- 其他协议块
- id 日期后缀（alias 约定，claude-haiku-4-5 / claude-sonnet-4-5 保持裸 id）
- 图像模型计费模式区分（aidog 无此概念，保留在 model_list 以匹配 pricing 页）

## Technical Notes
- 真值源：protocols.apinebula
- 数据来源：pricing 页（公开 200 OK，全量 20 模型 ID）+ docs/cli/{claude-code,codex,gemini} 子页核验 endpoint；/v1/models 返 404 无公开端点
- 数据强度：**强**（pricing 页公开列出全量 20 模型 + 3 协议 endpoint 文档核验）
- id 格式：裸 id（无 provider/ 前缀）；Claude alias 模型按约定不加日期后缀（research 真值含 -20251001/-20250929，保持 alias 裸 id）；GPT/Gemini/custom 用原始 upstream ID
- claude-opus-4-5 删除依据：pricing 页未发现，research 标注「可能已下架或未公开计费」
