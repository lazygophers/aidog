# 补全 pateway model_list+endpoints 全部官方信息

## Goal

PatewayAI (pateway.ai) 是 **多供应商聚合平台**（非 claude-only），5 家 / 11 模型：Anthropic Claude（5）+ OpenAI Codex（2）+ DeepSeek（2）+ Qwen（2）+ GLM（2）。当前 preset 仅 7 claude（**漏 8 个国产/Codex 模型 + 残留 2 下架 claude-4-5**）。2 endpoint 全正确（anthropic 承载 Claude+国产，openai 仅 Codex Responses API）。无 gemini 协议。`source_urls` 两个 URL 均 404 需修正。需补全 model_list + 删下架 + 修 source_urls + 三档默认。

## Research References

- [`research/pateway-models.md`](research/pateway-models.md) — 5 家 11 模型 + 2 endpoint 核验 + 7 现有核对（2 下架）+ source_urls 修正

## Requirements

### 1. endpoints（default 分支，2 端点全正确，不动）

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.pateway.ai", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.pateway.ai/v1", "client_type": "codex_tui"}
  ]
}
```

- anthropic 端点同时承载 Claude + DeepSeek/Qwen/GLM（走 Anthropic Messages 协议）
- openai 端点仅 Codex（Responses API `/v1/responses`，非 Chat Completions）
- 无 gemini 协议（全站 grep 零命中）

### 2. model_list.default（5 家 13 模型，裸 id）

**Claude（5，删 2 下架 + 保留 5）**：
- claude-opus-4-8 / claude-opus-4-7 / claude-opus-4-6 / claude-sonnet-4-6 / claude-haiku-4-5
- 🔴 **删** `claude-opus-4-5`（官方文档零命中，已下架）
- 🔴 **删** `claude-sonnet-4-5`（同上，已下架）

**OpenAI Codex（2，新）**：gpt-5.5 / gpt-5.3-codex

**DeepSeek（2，新）**：deepseek-v4-pro / deepseek-v4-flash

**Qwen（2，新）**：qwen3.7-max / qwen3.6-plus

**GLM（2，新）**：glm-5.1 / glm-5

合计 **13 模型**。全部裸 id（无 `provider/` 前缀）。

### 3. models.default（三档默认）

```json
"models": {
  "default": {
    "claude-sonnet-4-6": {},
    "gpt-5.5": {},
    "deepseek-v4-pro": {}
  }
}
```

三档：Claude 主力（sonnet-4-6）/ OpenAI 旗舰（gpt-5.5）/ 国产（deepseek-v4-pro，性价比高）。

### 4. desc 改写（8 语言，失实修正）

现有 desc 若仅 Claude 兼容则失实。Pateway 多供应商聚合：
- en-US: "PatewayAI multi-vendor API aggregator (Claude/GPT-Codex/DeepSeek/Qwen/GLM)"
- zh-Hans: "PatewayAI 多供应商 API 聚合（Claude/GPT-Codex/DeepSeek/Qwen/GLM）"
- 其余 6 语言同步翻译

### 5. source_urls 修正（两个 URL 均 404）

```json
"source_urls": {
  "docs": "https://pateway.ai/docs/",
  "pricing": "https://pateway.ai/docs/pricing.html"
}
```

- `docs.pateway.ai` 子域弃用 → 改 `pateway.ai/docs/`
- `pateway.ai/pricing` 不存在 → 改 `pateway.ai/docs/pricing.html`

## Acceptance Criteria

- [ ] endpoints 2 保留
- [ ] 删 claude-opus-4-5 + claude-sonnet-4-5
- [ ] model_list 13（5 claude + 2 codex + 2 deepseek + 2 qwen + 2 glm）
- [ ] models.default 三档
- [ ] desc 8 语言
- [ ] source_urls 两个 URL 修正
- [ ] JSON 合法
- [ ] 仅改 pateway 块

## Out of Scope

- 上下文窗口字段（pricing 仅给档位分界，未显式 max context）
- gemini 协议（不支持）
- 日期化别名（pricing 统一裸 id，沿用）
- `/v1/models` 动态全量（需用户 Key，静态三源交叉已覆盖；建议用户真实 Key 终检）
- STATIC_MODEL_IDS
- 其他协议块

## Technical Notes

- 真值源：`protocols.pateway`
- 数据来源：pricing.html + integration.html + api-reference.html 三方静态交叉
- 非 new-api/one-api 系，无免鉴权 pricing API
- id 格式：裸 id，与 packycode/cherryin 一致
- anthropic 端点承载多供应商（国产走 Anthropic Messages 协议）
