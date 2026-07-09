# 补全 CCSub model_list + endpoints 全部官方信息

## Goal
CCSub（https://www.ccsub.net）实为多供应商聚合 AI API 中转（Anthropic Claude 全系列 + OpenAI GPT/o-系列 + Google Gemini），非 Claude-only。当前 preset 仅 7 个 Claude 模型 + 2 端点（anthropic/openai），desc 误标 "Claude 兼容模型"。改动范围：补 model_list 至 19 个全量模型（移除已被取代的 `claude-opus-4-7`、新增 `claude-sonnet-5` + 8 OpenAI + 4 Google）、新增 gemini 协议端点、补 models.default 三档、改写 desc、source_urls 保留。

## Research References
- [`research/ccsub-models.md`](research/ccsub-models.md) — 平台定位为多供应商聚合；`https://www.ccsub.net/v1/models` 公开免鉴权返回 19 个模型；现有 anthropic/openai 端点验证正确，缺 gemini 端点；id 格式为裸 id（无 `provider/` 前缀）；`claude-opus-4-7` 不在 API 返回，已被 4-8 取代。

## Requirements

### 1. endpoints（default 分支，3 端点）

| # | 操作 | protocol | base_url | client_type | 依据 |
|---|------|----------|----------|-------------|------|
| 1 | 保留 | anthropic | `https://www.ccsub.net` | claude_code | 文档 `ANTHROPIC_BASE_URL=https://www.ccsub.net`（无 `/v1`） |
| 2 | 保留 | openai | `https://www.ccsub.net/v1` | codex_tui | 文档 `OPENAI_BASE_URL=https://www.ccsub.net/v1` |
| 3 | 新增 | gemini | `https://www.ccsub.net` | default | 文档「Gemini CLI 指向 `https://www.ccsub.net`」（同 anthropic，无 `/v1`） |

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://www.ccsub.net", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://www.ccsub.net/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://www.ccsub.net", "client_type": "default"}
  ]
}
```

### 2. model_list.default（19 模型，裸 id）

按供应商分组（保留 / 新增 / 删）：

- **Anthropic（7，保留 6 + 新增 1）**：保留 `claude-opus-4-8` / `claude-opus-4-6` / `claude-opus-4-5` / `claude-sonnet-4-6` / `claude-sonnet-4-5` / `claude-haiku-4-5`；**新增** `claude-sonnet-5`；**删** `claude-opus-4-7`（API 不返回，已被 4-8 取代）
- **OpenAI（8，全新增）**：`gpt-5.4` / `gpt-5` / `gpt-5-mini` / `gpt-4o` / `o3` / `o3-pro` / `o4-mini` / `codex-mini-latest`
- **Google（4，全新增）**：`gemini-3.5-flash` / `gemini-2.5-pro` / `gemini-2.5-flash` / `gemini-2.5-flash-lite`

```json
"model_list": {
  "default": [
    "claude-opus-4-8", "claude-opus-4-6", "claude-opus-4-5",
    "claude-sonnet-5", "claude-sonnet-4-6", "claude-sonnet-4-5",
    "claude-haiku-4-5",
    "gpt-5.4", "gpt-5", "gpt-5-mini", "gpt-4o",
    "o3", "o3-pro", "o4-mini", "codex-mini-latest",
    "gemini-3.5-flash", "gemini-2.5-pro", "gemini-2.5-flash", "gemini-2.5-flash-lite"
  ]
}
```

### 3. models.default（三档，档位名 key → model id string，对齐 `Partial<Record<ModelSlot, string>>`）

主力 = `claude-sonnet-4-6`（文档「日常编码主力」，$3/$15）；重型 = `claude-opus-4-8`（最新旗舰，$5/$25）；轻量 = `claude-haiku-4-5`（$0.8/$4）。

```json
"models": {
  "default": {
    "sonnet": "claude-sonnet-4-6",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

### 4. desc（改写，8 语言）

平台定位由 "Claude 兼容" 改为 "多供应商聚合（Claude/GPT/Gemini）"：

```json
"desc": {
  "en-US": "CCSub multi-vendor AI API relay (Claude / GPT / Gemini)",
  "zh-Hans": "CCSub 多供应商 AI API 中转（Claude / GPT / Gemini）",
  "ar-SA": "بوابة CCSub متعددة الموردين لواجهات AI (Claude / GPT / Gemini)",
  "fr-FR": "Relais API IA multi-fournisseurs CCSub (Claude / GPT / Gemini)",
  "de-DE": "CCSub Multi-Vendor KI-API-Relay (Claude / GPT / Gemini)",
  "ru-RU": "Мультивендорный релей AI API CCSub (Claude / GPT / Gemini)",
  "ja-JP": "CCSub マルチベンダー AI API 中継（Claude / GPT / Gemini）",
  "es-ES": "Relé API IA multi-proveedor de CCSub (Claude / GPT / Gemini)"
}
```

### 5. source_urls（保留）

`docs=https://www.ccsub.net/docs`（200）+ `pricing=https://www.ccsub.net/pricing`（200），均实测有效。

## Acceptance Criteria
- [ ] endpoints.default 含 3 端点（anthropic / openai / gemini）
- [ ] model_list.default 含 19 个 id，全部裸 id（无 `provider/` 前缀）
- [ ] model_list.default 不含 `claude-opus-4-7`，含 `claude-sonnet-5` + 8 OpenAI + 4 Google
- [ ] models.default 恰 3 档位名 key（sonnet / opus / haiku），value 为对应 model id string
- [ ] desc 8 语言全改写，反映多供应商聚合定位
- [ ] source_urls 保留不变
- [ ] 未动 STATIC_MODEL_IDS / 其他协议块 / version / last_updated
- [ ] JSON 合法

## Out of Scope
- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan 分支 / 其他协议块 / id 日期后缀（CCSub API 本就使用 alias 裸 id，无日期后缀问题）
- 生图模型（`gpt-image-1` / `dall-e-3` 首页提及但 `/v1/models` 不返回，可能是独立端点）

## Technical Notes
- 真值源：`protocols.ccsub`（src-tauri/defaults/platform-presets.json）
- 数据来源：`https://www.ccsub.net/v1/models` 公开免鉴权 GET（2026-07-09 实测 200 返 19 模型）+ 文档页 `/docs/install` `/docs/usage`
- id 格式：裸 id（无 `provider/` 前缀），与现有 preset 一致
- 数据强度：强（公开 API 端点实测）
