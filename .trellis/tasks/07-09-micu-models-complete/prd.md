# 补全 micu model_list+endpoints 全部官方信息

## Goal
Micu（米醋工作室，micuapi.ai）是多供应商聚合平台（Claude + GPT-5.x + Gemini + Grok + 国产），new-api 系（401 报错 `new_api_error`）。**数据局限**：模型广场 `https://www.micuapi.ai/pricing` 动态渲染需 dashboard 登录，文档仅列 13 个令牌分组与家族名（GLM-5.x/DeepSeek-V4/Kimi-K2.x/Qwen3.x/MiniMax-M2.7·M3），具体子版本 id 不可得。四端点（anthropic/openai/models/gemini）curl 全部 401 存活。本次保守处理：endpoints 补 openai/gemini（已验证存活）、model_list.default 保留现有 7 个 Claude alias（不臆造 Gemini/Grok/国产 id）、models.default 补三档（Claude 系内分）、desc 改写为多供应商聚合、source_urls 保留。

## Research References
- [`research/micu-models.md`](research/micu-models.md) — 4 端点全部 401 存活；new-api 系；13 令牌分组速查覆盖 5 大家族；Gemini「40+ 个」、国产仅家族名，具体 id 需模型广场登录

## Requirements

### 1. endpoints（default 分支，3 端点，新增 2）
现有仅 anthropic，research curl 验证 openai/gemini 同样 401 存活，新增：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://www.micuapi.ai", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://www.micuapi.ai/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://www.micuapi.ai", "client_type": "default"}
  ]
}
```

> Codex CLI 的 OpenAI 协议 base_url 需带 `/v1`；Claude Code 用根域。401 报错格式：`{"error":{"type":"new_api_error","message":"Invalid token"}}`。

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

**保守理由**：Micu 虽确认支持 GPT/Gemini/Grok/国产，但文档仅列分组名与家族概览（如「Gemini 2.5/3/3.1 全系，40+ 个」），具体 id 需模型广场登录查看。research 列出的 GPT-5.x 候选（gpt-5.5/gpt-5.4 等）虽来自官方文档令牌分组速查，但完整清单不可得，按 aidog 约定「禁臆造」保留现有 7 个 Claude alias，待有效 key 调 `/v1/models` 后另起 task 扩。

### 3. models.default（三档，Claude 系内分，档位名 key → model id string）
aidog 真值格式 = `Partial<Record<ModelSlot, string>>`，key 是档位名（sonnet/opus/haiku 等），value 是 model id 字符串。

```json
"models": {
  "default": {
    "sonnet": "claude-sonnet-4-6",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

| 档位（key） | 模型（value） | 理由 |
|------|------|------|
| `sonnet` | `claude-sonnet-4-6` | 最新 Sonnet，均衡主力 |
| `opus` | `claude-opus-4-8` | 最新 Opus，重型 |
| `haiku` | `claude-haiku-4-5` | 最新 Haiku，轻量 |

### 4. desc（改写，8 语言）
定位变化：从「Claude 兼容」改为「多供应商聚合」。

```json
"desc": {
  "en-US": "Micu aggregator for Claude, GPT, Gemini, Grok and CN models",
  "zh-Hans": "Micu 聚合平台, 支持 Claude / GPT / Gemini / Grok / 国产模型",
  "ar-SA": "مجمع Micu لنماذج Claude و GPT و Gemini و Grok والنماذج الصينية",
  "fr-FR": "Agrégateur Micu pour Claude, GPT, Gemini, Grok et modèles CN",
  "de-DE": "Micu-Aggregator für Claude, GPT, Gemini, Grok und CN-Modelle",
  "ru-RU": "Агрегатор Micu для Claude, GPT, Gemini, Grok и китайских моделей",
  "ja-JP": "Claude / GPT / Gemini / Grok / 中国系モデル対応 Micu アグリゲータ",
  "es-ES": "Agregador Micu para Claude, GPT, Gemini, Grok y modelos CN"
}
```

### 5. source_urls（保留）
- docs: `https://docs.micuapi.ai/`
- pricing: `https://www.micuapi.ai/pricing`

> 两 URL 均可达（pricing 为模型广场 SPA，需登录看完整清单）。

## Acceptance Criteria
- [ ] endpoints.default 含 3 端点（anthropic + 新增 openai/gemini）
- [ ] model_list.default 保留 7 个 Claude alias 不变
- [ ] models.default 三档：sonnet=claude-sonnet-4-6 / opus=claude-opus-4-8 / haiku=claude-haiku-4-5（档位名 key → string）
- [ ] desc 8 语言改为多供应商聚合定位
- [ ] source_urls 保留
- [ ] JSON 合法
- [ ] 验证命令输出：`7 {'sonnet': 'claude-sonnet-4-6', 'opus': 'claude-opus-4-8', 'haiku': 'claude-haiku-4-5'} 3`

## Out of Scope
- GPT-5.x / Gemini / Grok / 国产 具体模型 id（待有效 key 调 `/v1/models` 后另起 task）/ STATIC_MODEL_IDS / peak_hours / coding_plan / 其他协议块 / id 日期后缀

## Technical Notes
- 真值源：`protocols.micu`
- 数据来源：curl 端点探测（4 端点全 401 存活）+ 官方文档「令牌分组速查」（13 分组覆盖 5 家族）；**局限**：模型广场需登录，Gemini/国产仅家族名，具体 id 不可得
- id 格式：裸 id（无 provider 前缀，基于现有 preset 约定）
