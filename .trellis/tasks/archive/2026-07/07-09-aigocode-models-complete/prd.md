# 补全 AIGoCode model_list+endpoints 全部官方信息

## Goal
AIGoCode 是多供应商聚合平台（Claude + GPT + Gemini，4 大类 12 官方模型），非纯 Claude 代理。现有 preset 仅含 7 个 Claude alias（其中 2 个已下架），缺全部 GPT/Gemini 模型。desc 描述为「Claude 兼容模型」失实。本次改动：修正 desc 8 语言、补全 model_list 至 11 款（5 Claude alias + 3 GPT + 3 Gemini，排除 image-2 非对话模型）、补 models.default 三档、endpoints 与 source_urls 保留不变。

## Research References
- [`research/aigocode-models.md`](research/aigocode-models.md) — 官方 /docs/api/models 页确认全量 12 模型 ID；3 endpoint base_url 全部正确无需改；image-2 非对话模型不入路由；pricing 页 SPA 客户端渲染无数值。

## Requirements

### 1. endpoints（default 分支，3 端点，全部保留）
现有 3 端点经 research 核验全部正确，base_url 符合规则（openai 带 /v1，anthropic/gemini 仅 host）：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.aigocode.com", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.aigocode.com/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://api.aigocode.com", "client_type": "default"}
  ]
}
```

- 保留 anthropic endpoint（Claude Code SDK 追加 /v1/messages）
- 保留 openai endpoint（追加 /chat/completions）
- 保留 gemini endpoint（追加 /v1beta/models/...）

### 2. model_list.default（11 模型，裸 id 无 provider/ 前缀）

按 Claude → GPT → Gemini 分组：

**保留 5 款（alias 集，无日期后缀）：**
- `claude-opus-4-8`（官方表确认）
- `claude-opus-4-7`（官方表确认）
- `claude-opus-4-6`（官方表确认）
- `claude-sonnet-4-6`（官方表确认）
- `claude-haiku-4-5`（官方真值含 -20251001 后缀，按 aidog alias 约定保持裸 id）

**删除 2 款（官方 /docs/api/models 未列，已下架）：**
- ~~`claude-opus-4-5`~~（官方表无）
- ~~`claude-sonnet-4-5`~~（官方表无）

**新增 6 款：**
- `gpt-5.5`（OpenAI 旗舰）
- `gpt-5.4`（codex 文档示例模型）
- `gpt-5.4-mini`（成本敏感档）
- `gemini-3.1-pro-preview`（Pro 档）
- `gemini-3.5-flash`（flash 档）
- `gemini-3-flash-preview`（preview 档）

**排除 1 款：** `image-2`（图像生成，非文本对话，aidog 路由不适用）

最终 model_list.default：
```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "gpt-5.5",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gemini-3.1-pro-preview",
    "gemini-3.5-flash",
    "gemini-3-flash-preview"
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
- 主力 `claude-sonnet-4-6`：sonnet 档唯一选择，平衡型默认 → slot `default`
- 重型 `claude-opus-4-8`：Claude 旗舰，复杂推理 → slot `opus`
- 轻量 `claude-haiku-4-5`：成本敏感场景 → slot `haiku`

### 4. desc（改写 8 语言）

平台定位从「Claude 兼容」改为「多供应商聚合（Claude/GPT/Gemini）」：

```json
"desc": {
  "zh-Hans": "AIGoCode 多供应商聚合平台，支持 Claude/GPT/Gemini 协议",
  "en-US": "AIGoCode multi-provider aggregator (Claude/GPT/Gemini protocols)",
  "ar-SA": "منصة AIGoCode متعددة المزودين (بروتوكولات Claude/GPT/Gemini)",
  "fr-FR": "Agrégateur multi-fournisseurs AIGoCode (protocoles Claude/GPT/Gemini)",
  "de-DE": "AIGoCode Multi-Provider-Aggregator (Claude/GPT/Gemini-Protokolle)",
  "ru-RU": "Мультипровайдерная агрегация AIGoCode (протоколы Claude/GPT/Gemini)",
  "ja-JP": "AIGoCode マルチプロバイダ集約（Claude/GPT/Gemini プロトコル）",
  "es-ES": "Agregador multiproveedor AIGoCode (protocolos Claude/GPT/Gemini)"
}
```

### 5. source_urls（保留）

现有 source_urls 经 research 验证有效（docs 200 OK / pricing 200 OK SPA）：
```json
"source_urls": {
  "docs": "https://www.aigocode.com/",
  "pricing": "https://www.aigocode.com/pricing"
}
```

## Acceptance Criteria
- [ ] endpoints 3（anthropic/openai/gemini，base_url 不变）
- [ ] model_list 11（5 Claude alias + 3 GPT + 3 Gemini，排除 image-2）
- [ ] models.default 三档 档位名 key → string（default=claude-sonnet-4-6 / opus=claude-opus-4-8 / haiku=claude-haiku-4-5）
- [ ] desc 8 语言改写为多供应商聚合定位
- [ ] source_urls 保留不变
- [ ] JSON 合法
- [ ] 仅改 protocols.aigocode 块

## Out of Scope
- 上下文窗口字段（pricing SPA 无法抓取）
- STATIC_MODEL_IDS
- peak_hours / coding_plan 分支
- 其他协议块
- id 日期后缀（alias 约定保持裸 id）
- endpoint client_type 改动（现有正确）

## Technical Notes
- 真值源：protocols.aigocode
- 数据来源：官方 /docs/api/models 页（Next.js RSC flight payload 解析）+ 3 份 coding-tools 接入文档 + /docs/getting-started/base-url + /docs/getting-started/quickstart；pricing 页纯 SPA 无数值
- 数据强度：**强**（官方文档页明文全量 12 模型 ID + 3 协议 base_url 交叉验证）
- id 格式：裸 id（无 provider/ 前缀），与兄弟 preset aicodemirror / apinebula / sudocode 等一致
- claude-haiku-4-5 官方真值为 `claude-haiku-4-5-20251001`，按 aidog alias 约定（不增删日期后缀）保持裸 id
