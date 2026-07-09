# 补全 relaxycode model_list+endpoints 全部官方信息

## Goal
RelaxyCode（relaxycode.com）定位为多供应商聚合平台（Claude / Codex(GPT) / Gemini CLI 三卡片），当前 preset 仅含 7 个 Claude alias、`models.default` 为空、desc 写"Claude 兼容模型"低估定位。

**数据局限**：平台对探测 IP 返回 HTTP 451（地域封锁），无法实测 `/v1/models` 白名单与具体 model id 字符串，model 清单基于官网卡片 + aidog 主 preset alias 约定推断。本任务保守处理：不臆造 GPT/Gemini 具体 id，仅填 `models.default` 三档 + 修正 desc 反映多供应商定位。

改动范围：`protocols.relaxycode` 单块（endpoints / model_list / models.default / desc / source_urls）。

## Research References
- [`research/relaxycode-models.md`](research/relaxycode-models.md) — 多供应商聚合（Claude+Codex+Gemini 三卡片确认）；HTTP 451 地域封锁致 API 探测全失败；无免鉴权模型清单 API；建议 desc 改写、source_urls 保留。

## Requirements
### 1. endpoints（default 分支，3 端点，全保留）
现有 3 端点（anthropic + openai 带 `/v1` + gemini）与官网三协议卡片吻合，无需变更：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://www.relaxycode.com", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://www.relaxycode.com/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://www.relaxycode.com", "client_type": "default"}
  ]
}
```

> openai base_url 含 `/v1`（符合全局 URL 约定）；anthropic/gemini 仅 host。保留 codex_tui client_type（与 Codex CLI 卡片对应）。

### 2. model_list.default（7 模型，裸 id，保守保留）
数据弱不扩展 GPT/Gemini 具体条目（HTTP 451 无法验证 id 字符串白名单）。保留现有 7 个 Claude alias 不动：

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

> 后续若获得有效鉴权 + 可达网络，可按 OpenAI(gpt-5.x) / Gemini(2.5/3.x) 扩展，本任务不臆造。

### 3. models.default（三档，Claude 系内分档，档位名 key → model id 字符串）
平台虽多供应商，但仅 Claude 系 id 经 aidog 主 preset alias 约定验证，三档在 Claude 系内分。`models.default` 是 `Partial<Record<ModelSlot, string>>`，key = 档位名（default/opus/haiku 等），value = model id 字符串：

```json
"models": {
  "default": {
    "default": "claude-sonnet-4-6",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

> default 档（主力兜底）= `claude-sonnet-4-6`（主力均衡），opus 档（重型）= `claude-opus-4-8`（最新旗舰），haiku 档（轻量）= `claude-haiku-4-5`（快速响应）。

### 4. desc（8 语言改写）
现 desc"Claude-compatible"低估平台多供应商定位，改写反映 Claude / GPT / Gemini 聚合：

- en-US: "RelaxyCode API - aggregated access to Claude, GPT, Gemini"
- zh-Hans: "RelaxyCode API - 聚合 Claude / GPT / Gemini 多模型"
- ar-SA: "واجهة RelaxyCode - وصول مجمع إلى Claude و GPT و Gemini"
- fr-FR: "API RelaxyCode - accès agrégé à Claude, GPT, Gemini"
- de-DE: "RelaxyCode-API - aggregierter Zugriff auf Claude, GPT, Gemini"
- ru-RU: "API RelaxyCode — агрегированный доступ к Claude, GPT, Gemini"
- ja-JP: "RelaxyCode API - Claude / GPT / Gemini 統合アクセス"
- es-ES: "API de RelaxyCode - acceso agregado a Claude, GPT y Gemini"

### 5. source_urls（保留）
research 确认 docs/pricing 两 URL 存活（docs 指向首页作入口，pricing 为价格 SPA 无模型清单）。保留现状：

```json
"source_urls": {
  "docs": "https://www.relaxycode.com/",
  "pricing": "https://www.relaxycode.com/pricing"
}
```

## Acceptance Criteria
- [ ] endpoints 3 端点保留不变（openai 带 `/v1`，anthropic/gemini 仅 host）
- [ ] model_list.default 保留 7 个 Claude alias，不增不减
- [ ] models.default 三档 = `claude-opus-4-8` / `claude-sonnet-4-6` / `claude-haiku-4-5`，档位名 key（default/opus/haiku）→ model id 字符串
- [ ] desc 8 语言全改写为多供应商聚合定位
- [ ] source_urls 保留
- [ ] JSON 合法，protocols 其他块未动

## Out of Scope
- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan / 其他协议块 / id 日期后缀
- 臆造 GPT/Gemini 具体 model id（待 HTTP 451 解锁后再补全）
- 主页 / logo_url / name / client_type 字段

## Technical Notes
- 真值源：`src-tauri/defaults/platform-presets.json` 的 `protocols.relaxycode` 块（line 2874-2935 区间）
- 数据来源：官网首页 + pricing 页 Jina Reader 抓取（2026-07-09）；API 探测全 HTTP 451 失败；dashboard 需登录无法获取模型清单
- 数据局限：HTTP 451 地域封锁 → model id 字符串白名单未验证，故 model_list 不扩展，仅基于 aidog 主 preset alias 约定保留现有 7 个
- id 格式：裸 id（`claude-opus-4-8` 等，无 `provider/` 前缀，无日期后缀）
