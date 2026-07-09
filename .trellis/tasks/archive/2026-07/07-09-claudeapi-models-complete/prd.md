# 补全 ClaudeAPI model_list + endpoints 全部官方信息

## Goal
ClaudeAPI（claudeapi.com / gw.claudeapi.com）是纯 Anthropic Claude 第三方代理（非多供应商聚合），官价 8 折。当前 preset 仅 7 个 Claude 模型，缺 `claude-fable-5`（2026-06-09 发布）和 `claude-sonnet-5`（2026-06-30 发布）；desc 误标 "Claude 兼容模型"（实际是 Claude-only 代理）；`source_urls.pricing=https://claudeapi.com/pricing` 实测 404、`docs` 301 重定向。改动范围：补 model_list 至 9 个（仅新增，日期后缀改动 OOS）、补 models.default 三档、改写 desc、修正 source_urls、endpoints 保留。

## Research References
- [`research/claudeapi-models.md`](research/claudeapi-models.md) — 平台为纯 Claude 代理（首页明确 "not another all-in-one AI gateway"）；官方博客 apito.ai 列出 10 个模型 id；现有 anthropic endpoint `gw.claudeapi.com` 验证正确；platform 同时支持 OpenAI 兼容模式（`gw.claudeapi.com/v1`）但本任务 OOS 不新增；pricing URL 404。

## Requirements

### 1. endpoints（default 分支，1 端点，保留）

| # | 操作 | protocol | base_url | client_type | 依据 |
|---|------|----------|----------|-------------|------|
| 1 | 保留 | anthropic | `https://gw.claudeapi.com` | claude_code | 文档 + SDK 示例 `base_url="https://gw.claudeapi.com"` |

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://gw.claudeapi.com", "client_type": "claude_code"}
  ]
}
```

> OpenAI 兼容模式（`gw.claudeapi.com/v1`）虽然存在，但 aidog 当前 preset 无 openai endpoint，新增属功能扩展，标记 OOS。

### 2. model_list.default（9 模型，裸 id）

- **保留 7**：`claude-opus-4-8` / `claude-sonnet-4-6` / `claude-haiku-4-5` / `claude-opus-4-7` / `claude-opus-4-6` / `claude-opus-4-5` / `claude-sonnet-4-5`
- **新增 2**：`claude-fable-5`（2026-06-09） / `claude-sonnet-5`（2026-06-30）
- **id 日期后缀改动 OOS**：research 指出 `claude-haiku-4-5` / `claude-opus-4-5` / `claude-sonnet-4-5` 在 ClaudeAPI 实际为带日期后缀版本，但 aidog alias 约定禁增删日期后缀，保留现状

```json
"model_list": {
  "default": [
    "claude-fable-5",
    "claude-sonnet-5",
    "claude-opus-4-8",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-6",
    "claude-sonnet-4-5",
    "claude-haiku-4-5"
  ]
}
```

### 3. models.default（三档，档位名 key → model id string，对齐 `Partial<Record<ModelSlot, string>>`）

主力 = `claude-sonnet-5`（最新 Sonnet，长上下文，推荐复杂工作）；重型 = `claude-opus-4-8`（Opus 4 系最新最强，博客推荐新项目首选）；轻量 = `claude-haiku-4-5`（最便宜，$0.8/$4）。

```json
"models": {
  "default": {
    "sonnet": "claude-sonnet-5",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

### 4. desc（改写，8 语言）

"Claude 兼容模型" 易误解为多供应商 Claude 兼容层，实际是纯 Claude 代理 + 官价 8 折：

```json
"desc": {
  "en-US": "Anthropic Claude API third-party proxy, 20% cheaper than official",
  "zh-Hans": "Anthropic Claude API 第三方中转，官价 8 折",
  "ar-SA": "وكيل طرف ثالث لـ Anthropic Claude API، أرخص بنسبة 20% من الرسمي",
  "fr-FR": "Proxy tiers Anthropic Claude API, 20% moins cher que l'officiel",
  "de-DE": "Drittanbieter-Proxy für Anthropic Claude API, 20% günstiger als offiziell",
  "ru-RU": "Сторонний прокси Anthropic Claude API, на 20% дешевле официального",
  "ja-JP": "Anthropic Claude API サードパーティ中継、公式より 20% 安い",
  "es-ES": "Proxy de terceros de Anthropic Claude API, 20% más barato que el oficial"
}
```

### 5. source_urls（修正）

| 字段 | 现状 | 修正为 | 依据 |
|------|------|--------|------|
| docs | `https://docs.claudeapi.com/` (301 重定向) | `https://apito.ai/en/blog/getting-started/claude-api-model-id-list/` | 实际文档托管在 apito.ai 博客 |
| pricing | `https://claudeapi.com/pricing` (**404**) | `https://apito.ai/en/blog/pricing/claude-api-pricing-guide/` | 实际定价指南在 apito.ai 博客 |

```json
"source_urls": {
  "docs": "https://apito.ai/en/blog/getting-started/claude-api-model-id-list/",
  "pricing": "https://apito.ai/en/blog/pricing/claude-api-pricing-guide/"
}
```

## Acceptance Criteria
- [ ] endpoints.default 保留 1 端点不变
- [ ] model_list.default 含 9 个 id，全裸 id；新增 `claude-fable-5` + `claude-sonnet-5`
- [ ] model_list.default 中 `claude-haiku-4-5` / `claude-opus-4-5` / `claude-sonnet-4-5` 保留 aidog alias 形式（不增删日期后缀）
- [ ] models.default 恰 3 档位名 key（sonnet / opus / haiku），value 为对应 model id string
- [ ] desc 8 语言全改写，反映 "Claude 第三方代理 + 8 折" 定位
- [ ] source_urls.docs 改为 apito.ai 模型列表博客
- [ ] source_urls.pricing 改为 apito.ai 定价指南博客（原 404）
- [ ] 未动 STATIC_MODEL_IDS / 其他协议块 / version / last_updated
- [ ] JSON 合法

## Out of Scope
- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan 分支 / 其他协议块
- **id 日期后缀改动**（aidog alias 约定保留 `claude-haiku-4-5` / `claude-opus-4-5` / `claude-sonnet-4-5` 现状）
- 新增 openai 协议端点（虽然 ClaudeAPI 支持 OpenAI 兼容模式，但本任务只补全现有协议信息）

## Technical Notes
- 真值源：`protocols.claudeapi`（src-tauri/defaults/platform-presets.json）
- 数据来源：apito.ai 官方博客（模型 id 列表 + 定价指南 + Fable 5 / Sonnet 5 指南）；端点 `gw.claudeapi.com` 探测返 "Invalid token"（端点存在）
- id 格式：裸 id（`claude-{family}-{major}-{minor}`），与现有 preset 一致
- 数据强度：强（官方博客 + 端点探测）
