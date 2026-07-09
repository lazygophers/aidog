# 补全 ClaudeCN model_list + endpoints 全部官方信息

## Goal
ClaudeCN（claudecn.top / claudecn.ai）是多供应商聚合平台（首页明确 "全面支持 Claude、GPT、Gemini"，文档宣称 "100+ 主流大模型"）。当前 preset 7 个 Claude 模型 + 2 端点（anthropic `claudecn.top` + openai `claudecn.ai/v1`），desc 误标 "Claude 兼容模型"，anthropic 端点 `claudecn.top` 实测 SSL 握手失败。

**数据局限**：`/v1/models` 需鉴权（401），`/models` 页面 JS 动态加载无预渲染数据，价格页是营销内容 — 无法获取官方全量模型清单。GPT/Gemini 具体模型 id 未公开。

**保守策略**（research 弱）：保留现有 7 个 Claude alias 不臆造 GPT/Gemini id；仅补 models.default 三档、改写 desc、修正 anthropic 端点 host。

## Research References
- [`research/claudecn-models.md`](research/claudecn-models.md) — 平台为多供应商聚合（非纯 Claude 代理）；`.top` 与 `.ai` 是同站双域名，`.ai` 是主 API 域名（文档示例用 `ANTHROPIC_BASE_URL=https://claudecn.ai`）；`https://claudecn.top/v1/messages` 实测 SSL 握手失败（状态码 35）；openai 端点 `https://claudecn.ai/v1/chat/completions` 验证有效（401 鉴权错）；无法获取全量模型清单。

## Requirements

### 1. endpoints（default 分支，2 端点，修正 anthropic host）

| # | 操作 | protocol | base_url | client_type | 依据 |
|---|------|----------|----------|-------------|------|
| 1 | **修正** | anthropic | `https://claudecn.top` → `https://claudecn.ai` | claude_code | `.top` SSL 握手失败（curl 35）；官方文档示例用 `claudecn.ai` |
| 2 | 保留 | openai | `https://claudecn.ai/v1` | codex_tui | 实测 401（端点存活，需有效密钥） |

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://claudecn.ai", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://claudecn.ai/v1", "client_type": "codex_tui"}
  ]
}
```

> 不新增 gemini 端点：官方文档提到支持 Gemini CLI 但未给出具体端点，未明确前不臆造。

### 2. model_list.default（7 模型，裸 id，全保留）

**数据局限**：无公开模型清单 API，**保守保留现有 7 个 aidog alias**，不臆造 GPT/Gemini id（research 推测的 `claude-fable-5` / `claude-sonnet-5` / `claude-mythos-5` 未验证，不加入）：

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

### 3. models.default（三档，档位名 key → model id string，对齐 `Partial<Record<ModelSlot, string>>`）

主力 = `claude-sonnet-4-6`；重型 = `claude-opus-4-8`；轻量 = `claude-haiku-4-5`。

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

"Claude 兼容模型" 改为 "多模型聚合平台（Claude/GPT/Gemini）"：

```json
"desc": {
  "en-US": "ClaudeCN multi-model gateway (Claude / GPT / Gemini)",
  "zh-Hans": "ClaudeCN 多模型聚合中转（Claude / GPT / Gemini）",
  "ar-SA": "بوابة ClaudeCN متعددة النماذج (Claude / GPT / Gemini)",
  "fr-FR": "Passerelle multi-modèles ClaudeCN (Claude / GPT / Gemini)",
  "de-DE": "ClaudeCN Multi-Modell-Gateway (Claude / GPT / Gemini)",
  "ru-RU": "Мультимодельный шлюз ClaudeCN (Claude / GPT / Gemini)",
  "ja-JP": "ClaudeCN マルチモデルゲートウェイ（Claude / GPT / Gemini）",
  "es-ES": "Pasarela multimodelo ClaudeCN (Claude / GPT / Gemini)"
}
```

### 5. source_urls（保留）

`docs=https://claudecn.top/document`（200）+ `pricing=https://claudecn.top/price`（200），均实测有效（虽为营销内容）。保留。

## Acceptance Criteria
- [ ] endpoints.default = 2 端点；anthropic base_url 由 `claudecn.top` 改为 `claudecn.ai`
- [ ] model_list.default 保留原 7 个 Claude aidog alias 不变（不臆造 GPT/Gemini）
- [ ] models.default 恰 3 档位名 key（sonnet / opus / haiku），value 为对应 model id string
- [ ] desc 8 语言全改写，反映多模型聚合定位
- [ ] source_urls 保留
- [ ] 未动 STATIC_MODEL_IDS / 其他协议块 / version / last_updated
- [ ] JSON 合法

## Out of Scope
- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan 分支 / 其他协议块 / id 日期后缀
- 新增 GPT / Gemini 模型 id（数据局限，需有效 token 调 `/v1/models` 验证后再补）
- 新增 gemini 协议端点（官方未明确）

## Technical Notes
- 真值源：`protocols.claudecn`（src-tauri/defaults/platform-presets.json）
- 数据来源：`https://claudecn.ai/v1/chat/completions` 实测 401（端点存活）；`https://claudecn.top/v1/messages` SSL 握手失败；首页 meta + 文档页营销宣称
- id 格式：裸 id（无 `provider/` 前缀）
- **数据强度：弱**（无公开模型清单，GPT/Gemini 模型 id 未公开）— prd 已标注「数据局限」，采用保守策略
