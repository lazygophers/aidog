# 补全 runapi model_list+endpoints 全部官方信息

## Goal
RunAPI（runapi.co）自我定位"国内 OpenRouter 替代"，支持 OpenAI / Claude / Gemini / DeepSeek / Grok 等 150+ 模型，统一 API 接入。当前 preset 仅 1 个 anthropic endpoint + 7 个 Claude alias + 空 `models.default` + desc"Claude 兼容模型"严重低估定位。

改动范围：`protocols.runapi` 单块（endpoints 补 openai+gemini / model_list 扩 / models.default 填三档 / desc 改写）。source_urls 保留。

## Research References
- [`research/runapi-models.md`](research/runapi-models.md) — 公开 `/api/pricing` 返回 204 模型（免鉴权）；三类端点（anthropic/openai/gemini）全存活 401；裸 id 格式（部分老 Claude 带日期后缀）；建议补 openai/gemini endpoints + 扩 Claude 模型 + 改 desc。

## Requirements
### 1. endpoints（default 分支，3 端点：1 保留 + 2 新增）
现有 anthropic 保留；按 research `/v1/chat/completions` + `/v1beta/models` 端点存活，补 openai（带 `/v1`）+ gemini（仅 host）：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://runapi.co", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://runapi.co/v1", "client_type": "default"},
    {"protocol": "gemini", "base_url": "https://runapi.co", "client_type": "default"}
  ]
}
```

> openai base_url 含 `/v1`（符合全局 URL 约定 + research `/v1/chat/completions` 路径核验）；gemini 仅 host（research `/v1beta/models` 由 provider_api_path 处理）。

### 2. model_list.default（10 模型，裸 id，保留现有 alias 格式 + 新增 3）
现有 7 个 alias 保留不动（不增删日期后缀，符合全局规则）。新增 research `/api/pricing` 确认的 3 个最新 Claude 模型：

```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-5",
    "claude-sonnet-5",
    "claude-fable-5",
    "claude-sonnet-4-6-thinking"
  ]
}
```

> 新增依据：`claude-sonnet-5`（最新 Sonnet，research 表列第 3 行）、`claude-fable-5`（最新 Fable 系列，research 表列第 1 行）、`claude-sonnet-4-6-thinking`（research 表列 thinking 变体）。GPT/Gemini/国产系列 id 虽 research 列出，但本 preset 仍以 Claude 系为主（与其他聚合平台 preset 一致，避免主 preset 过宽），不扩展非 Claude 条目。

### 3. models.default（三档，Claude 系内分档，档位名 key → model id 字符串）
`models.default` 是 `Partial<Record<ModelSlot, string>>`，key = 档位名（default/opus/haiku 等），value = model id 字符串：

```json
"models": {
  "default": {
    "default": "claude-sonnet-5",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

> default 档（主力兜底）= `claude-sonnet-5`（research 推荐通用性价比），opus 档（重型）= `claude-opus-4-8`（复杂推理），haiku 档（轻量）= `claude-haiku-4-5`（快速响应，用短 alias 与 model_list 一致）。

### 4. desc（8 语言改写）
现"Claude-compatible"改写为多供应商聚合定位：

- en-US: "RunAPI proxy - aggregated access to 150+ models (Claude / GPT / Gemini / DeepSeek / Grok)"
- zh-Hans: "RunAPI 中转 - 聚合 150+ 模型（Claude / GPT / Gemini / DeepSeek / Grok 等）"
- ar-SA: "وكيل RunAPI - وصول مجمع إلى أكثر من 150 نموذجًا (Claude / GPT / Gemini / DeepSeek / Grok)"
- fr-FR: "Proxy RunAPI - accès agrégé à 150+ modèles (Claude / GPT / Gemini / DeepSeek / Grok)"
- de-DE: "RunAPI-Proxy - aggregierter Zugriff auf 150+ Modelle (Claude / GPT / Gemini / DeepSeek / Grok)"
- ru-RU: "Прокси RunAPI — агрегированный доступ к 150+ моделям (Claude / GPT / Gemini / DeepSeek / Grok)"
- ja-JP: "RunAPI 中継 - 150+ モデル統合（Claude / GPT / Gemini / DeepSeek / Grok 等）"
- es-ES: "Proxy RunAPI - acceso agregado a 150+ modelos (Claude / GPT / Gemini / DeepSeek / Grok)"

### 5. source_urls（保留）
research 确认 docs/pricing 两 URL 存活：

```json
"source_urls": {
  "docs": "https://runapi.co/docs",
  "pricing": "https://runapi.co/pricing"
}
```

## Acceptance Criteria
- [ ] endpoints default 分支 = 3 端点（anthropic 保留 + openai 带 `/v1` + gemini 仅 host）
- [ ] model_list.default = 10 模型（保留原 7 + 新增 claude-sonnet-5 / claude-fable-5 / claude-sonnet-4-6-thinking）
- [ ] models.default 三档 = `claude-sonnet-5` / `claude-opus-4-8` / `claude-haiku-4-5`，档位名 key（default/opus/haiku）→ model id 字符串
- [ ] desc 8 语言全改写为 150+ 模型聚合定位
- [ ] source_urls 保留
- [ ] JSON 合法，protocols 其他块未动

## Out of Scope
- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan / 其他协议块 / id 日期后缀
- 扩展 GPT/Gemini/国产系列条目到 model_list（保持 Claude 系为主，与其他聚合平台 preset 一致）
- 改 endpoint client_type / name / homepage / logo_url

## Technical Notes
- 真值源：`src-tauri/defaults/platform-presets.json` 的 `protocols.runapi` 块
- 数据来源：公开 API `https://runapi.co/api/pricing`（免鉴权，204 模型，2026-07-09）+ curl 端点存活探测（401）+ 官网 meta/schema.org
- id 格式：裸 id（无 `provider/` 前缀）；现有 7 alias 保留无日期后缀格式，新增 3 个均为 research 表裸 id（`claude-sonnet-5` / `claude-fable-5` / `claude-sonnet-4-6-thinking`，均无日期后缀）
- URL 构造：`base_url + provider_api_path`，openai base_url 含 `/v1`
