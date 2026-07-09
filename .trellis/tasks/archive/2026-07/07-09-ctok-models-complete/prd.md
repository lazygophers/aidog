# 补全 ctok model_list+endpoints 全部官方信息

## Goal
CTok.ai 是多供应商 API 网关（Claude + GPT + Gemini，非纯 Claude 代理）。**数据局限**：dashboard 需邀请注册（`registration_enabled: false`），`/v1/models`/`/api/pricing` 需鉴权，无免鉴权模型清单 API，docs.ctok.ai 与 /pricing 均 404 —— 全量模型清单不可得。三协议端点经 curl 验证全部 401 存活（= 路由有效，可加）。本次保守处理：endpoints 补 openai/gemini（已验证存活）、model_list.default 保留现有 7 个 Claude alias（不臆造 GPT/Gemini 模型清单）、models.default 补三档（Claude 系内分）、desc 改写为多供应商、source_urls 修正为有效 homepage。

## Research References
- [`research/ctok-models.md`](research/ctok-models.md) — 三协议端点全部 401 存活；api.ctok.ai 首页 "Claude Supported, GPT Supported, Gemini Supported"；无公开文档/定价/模型清单 API；现有 7 alias 与纯 Claude 代理 preset 一致

## Requirements

### 1. endpoints（default 分支，3 端点，新增 2）
现有仅 anthropic，research curl 验证三协议均 401 存活，新增 openai/gemini：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.ctok.ai", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.ctok.ai/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://api.ctok.ai", "client_type": "default"}
  ]
}
```

> anthropic 401 报错 `{"code":"INVALID_API_KEY"}` / gemini 401 报错 `{"error":{"code":401,"status":"UNAUTHENTICATED"}}`，均证明路由有效。

### 2. model_list.default（保留现有 7 alias，保守）
```json
[
  "claude-opus-4-8",
  "claude-sonnet-4-6",
  "claude-haiku-4-5",
  "claude-opus-4-7",
  "claude-opus-4-6",
  "claude-opus-4-5",
  "claude-sonnet-4-5"
]
```

**保守理由**：CTok 虽确认支持 GPT/Gemini，但具体模型 id 清单不可得（需登录 dashboard 或有效 key 调 `/v1/models`）。research 列出的 GPT/Gemini 候选均为「推测，非官方清单」。按 aidog 约定「禁臆造」，保留现有 7 个 Claude alias（这是 aidog 项目级 alias 约定，跨 18+ 纯 Claude 代理 preset 一致），待有效 key 验证后再扩。

### 3. models.default（三档，Claude 系内分，档位名 key → model id string）
aidog 真值格式 = `Partial<Record<ModelSlot, string>>`，key 是档位名（opus/sonnet/haiku 等），value 是 model id 字符串。

```json
"models": {
  "default": {
    "opus": "claude-opus-4-8",
    "sonnet": "claude-sonnet-4-6",
    "haiku": "claude-haiku-4-5"
  }
}
```

| 档位（key） | 模型（value） | 理由 |
|------|------|------|
| `opus` | `claude-opus-4-8` | 最新 Opus，重型主力 |
| `sonnet` | `claude-sonnet-4-6` | 最新 Sonnet，均衡 |
| `haiku` | `claude-haiku-4-5` | 最新 Haiku，轻量 |

### 4. desc（改写，8 语言）
定位变化：从「Claude 兼容」改为「多供应商网关」。

```json
"desc": {
  "en-US": "CTok.ai API gateway for Claude, GPT and Gemini models",
  "zh-Hans": "CTok.ai API 网关, 支持 Claude / GPT / Gemini 多模型",
  "ar-SA": "بوابة CTok.ai لنماذج Claude و GPT و Gemini",
  "fr-FR": "Passerelle CTok.ai pour les modèles Claude, GPT et Gemini",
  "de-DE": "CTok.ai-API-Gateway für Claude-, GPT- und Gemini-Modelle",
  "ru-RU": "Шлюз CTok.ai для моделей Claude, GPT и Gemini",
  "ja-JP": "Claude / GPT / Gemini 対応 CTok.ai API ゲートウェイ",
  "es-ES": "Gateway CTok.ai para modelos Claude, GPT y Gemini"
}
```

### 5. source_urls（修正，移除 404）
原 docs/pricing 均 404（`docs.ctok.ai` 404、`ctok.ai/pricing` 404），改为有效入口：

```json
"source_urls": {
  "docs": "https://api.ctok.ai/",
  "pricing": "https://api.ctok.ai/"
}
```

> homepage 字段保留 `https://ctok.ai`。

## Acceptance Criteria
- [ ] endpoints.default 含 3 端点（anthropic + 新增 openai/gemini）
- [ ] model_list.default 保留 7 个 Claude alias 不变
- [ ] models.default 三档：opus=claude-opus-4-8 / sonnet=claude-sonnet-4-6 / haiku=claude-haiku-4-5（档位名 key → string）
- [ ] desc 8 语言改为多供应商定位
- [ ] source_urls 移除 404，指向有效入口
- [ ] JSON 合法
- [ ] 验证命令输出：`7 {'opus': 'claude-opus-4-8', 'sonnet': 'claude-sonnet-4-6', 'haiku': 'claude-haiku-4-5'} 3`

## Out of Scope
- GPT/Gemini 具体模型 id（待有效 key 验证后另起 task）/ STATIC_MODEL_IDS / peak_hours / coding_plan / 其他协议块 / id 日期后缀

## Technical Notes
- 真值源：`protocols.ctok`
- 数据来源：curl 端点探测（401 = 存活）+ Jina Reader 抓首页；**局限**：无公开文档/模型清单 API，GPT/Gemini 具体支持模型未确认
- id 格式：裸 id（基于现有 preset 约定）
