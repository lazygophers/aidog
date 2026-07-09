# Research: OpenRouter 全量模型与端点验证

- **Query**: 研究 OpenRouter 官方信息，补全 platform-presets.json（全量模型清单 + endpoints 验证）
- **Scope**: 外部（OpenRouter 官方 API）
- **Date**: 2026-07-09
- **API Source**: `https://openrouter.ai/api/v1/models`

---

## Findings

### 全量模型清单

**总计**: 344 个模型，来自 57 个不同的 provider

#### Provider 分布统计（前 30 名）:

| Provider | 模型数 |
|----------|--------|
| openai | 62 |
| qwen | 49 |
| google | 29 |
| mistralai | 19 |
| anthropic | 15 |
| z-ai | 12 |
| meta-llama | 12 |
| nvidia | 11 |
| deepseek | 11 |
| minimax | 8 |
| poolside | 6 |
| moonshotai | 6 |
| cohere | 5 |
| openrouter | 5 |
| amazon | 5 |
| perplexity | 5 |
| nousresearch | 5 |
| aion-labs | 4 |
| tencent | 4 |
| ~anthropic | 4 |
| x-ai | 4 |
| arcee-ai | 4 |
| bytedance-seed | 4 |
| thedrummer | 4 |
| sao10k | 4 |
| inclusionai | 3 |
| liquid | 3 |
| nex-agi | 2 |
| stepfun | 2 |
| ibm-granite | 2 |

#### 完整模型清单（按 provider 分组）:

##### ai21 (1)
- ai21/jamba-large-1.7

##### aion-labs (4)
- aion-labs/aion-2.0
- aion-labs/aion-3.0
- aion-labs/aion-3.0-mini
- aion-labs/aion-rp-llama-3.1-8b

##### allenai (1)
- allenai/olmo-3-32b-think

##### amazon (5)
- amazon/nova-2-lite-v1
- amazon/nova-lite-v1
- amazon/nova-micro-v1
- amazon/nova-premier-v1
- amazon/nova-pro-v1

##### anthropic (15)
- anthropic/claude-3-haiku
- anthropic/claude-fable-5
- anthropic/claude-haiku-4.5
- anthropic/claude-opus-4
- anthropic/claude-opus-4.1
- anthropic/claude-opus-4.5
- anthropic/claude-opus-4.6
- anthropic/claude-opus-4.7
- anthropic/claude-opus-4.7-fast
- anthropic/claude-opus-4.8
- anthropic/claude-opus-4.8-fast
- anthropic/claude-sonnet-4
- anthropic/claude-sonnet-4.5
- anthropic/claude-sonnet-4.6
- anthropic/claude-sonnet-5

##### arcee-ai (4)
- arcee-ai/coder-large
- arcee-ai/trinity-large-thinking
- arcee-ai/trinity-mini
- arcee-ai/virtuoso-large

##### baidu (1)
- baidu/ernie-4.5-vl-424b-a47b

##### bytedance (1)
- bytedance/ui-tars-1.5-7b

##### bytedance-seed (4)
- bytedance-seed/seed-1.6
- bytedance-seed/seed-1.6-flash
- bytedance-seed/seed-2.0-lite
- bytedance-seed/seed-2.0-mini

##### cognitivecomputations (1)
- cognitivecomputations/dolphin-mistral-24b-venice-edition:free

##### cohere (5)
- cohere/command-a
- cohere/command-r-08-2024
- cohere/command-r-plus-08-2024
- cohere/command-r7b-12-2024
- cohere/north-mini-code:free

##### deepcogito (1)
- deepcogito/cogito-v2.1-671b

##### deepseek (11)
- deepseek/deepseek-chat
- deepseek/deepseek-chat-v3-0324
- deepseek/deepseek-chat-v3.1
- deepseek/deepseek-r1
- deepseek/deepseek-r1-0528
- deepseek/deepseek-r1-distill-llama-70b
- deepseek/deepseek-v3.1-terminus
- deepseek/deepseek-v3.2
- deepseek/deepseek-v3.2-exp
- deepseek/deepseek-v4-flash
- deepseek/deepseek-v4-pro

##### google (29)
- google/gemini-2.5-flash
- google/gemini-2.5-flash-image
- google/gemini-2.5-flash-lite
- google/gemini-2.5-flash-lite-preview-09-2025
- google/gemini-2.5-pro
- google/gemini-2.5-pro-preview
- google/gemini-2.5-pro-preview-05-06
- google/gemini-3-flash-preview
- google/gemini-3-pro-image
- google/gemini-3-pro-image-preview
- google/gemini-3.1-flash-image
- google/gemini-3.1-flash-image-preview
- google/gemini-3.1-flash-lite
- google/gemini-3.1-flash-lite-image
- google/gemini-3.1-flash-lite-preview
- google/gemini-3.1-pro-preview
- google/gemini-3.1-pro-preview-customtools
- google/gemini-3.5-flash
- google/gemma-2-27b-it
- google/gemma-3-12b-it
- google/gemma-3-27b-it
- google/gemma-3-4b-it
- google/gemma-3n-e4b-it
- google/gemma-4-26b-a4b-it
- google/gemma-4-26b-a4b-it:free
- google/gemma-4-31b-it
- google/gemma-4-31b-it:free
- google/lyria-3-clip-preview
- google/lyria-3-pro-preview

##### gryphe (1)
- gryphe/mythomax-l2-13b

##### ibm-granite (2)
- ibm-granite/granite-4.0-h-micro
- ibm-granite/granite-4.1-8b

##### inception (1)
- inception/mercury-2

##### inclusionai (3)
- inclusionai/ling-2.6-1t
- inclusionai/ling-2.6-flash
- inclusionai/ring-2.6-1t

##### inflection (2)
- inflection/inflection-3-pi
- inflection/inflection-3-productivity

##### kwaipilot (1)
- kwaipilot/kat-coder-pro-v2

##### liquid (3)
- liquid/lfm-2-24b-a2b
- liquid/lfm-2.5-1.2b-instruct:free
- liquid/lfm-2.5-1.2b-thinking:free

##### mancer (1)
- mancer/weaver

##### meta-llama (12)
- meta-llama/llama-3-8b-instruct
- meta-llama/llama-3.1-70b-instruct
- meta-llama/llama-3.1-8b-instruct
- meta-llama/llama-3.2-11b-vision-instruct
- meta-llama/llama-3.2-1b-instruct
- meta-llama/llama-3.2-3b-instruct
- meta-llama/llama-3.2-3b-instruct:free
- meta-llama/llama-3.3-70b-instruct
- meta-llama/llama-3.3-70b-instruct:free
- meta-llama/llama-4-maverick
- meta-llama/llama-4-scout
- meta-llama/llama-guard-4-12b

##### microsoft (2)
- microsoft/phi-4
- microsoft/wizardlm-2-8x22b

##### minimax (8)
- minimax/minimax-01
- minimax/minimax-m1
- minimax/minimax-m2
- minimax/minimax-m2-her
- minimax/minimax-m2.1
- minimax/minimax-m2.5
- minimax/minimax-m2.7
- minimax/minimax-m3

##### mistralai (19)
- mistralai/codestral-2508
- mistralai/devstral-2512
- mistralai/ministral-14b-2512
- mistralai/ministral-3b-2512
- mistralai/ministral-8b-2512
- mistralai/mistral-large
- mistralai/mistral-large-2407
- mistralai/mistral-large-2512
- mistralai/mistral-medium-3
- mistralai/mistral-medium-3-5
- mistralai/mistral-medium-3.1
- mistralai/nemo
- mistralai/mistral-saba
- mistralai/mistral-small-24b-instruct-2501
- mistralai/mistral-small-2603
- mistralai/mistral-small-3.1-24b-instruct
- mistralai/mistral-small-3.2-24b-instruct
- mistralai/mixtral-8x22b-instruct
- mistralai/voxtral-small-24b-2507

##### moonshotai (6)
- moonshotai/kimi-k2
- moonshotai/kimi-k2-0905
- moonshotai/kimi-k2-thinking
- moonshotai/kimi-k2.5
- moonshotai/kimi-k2.6
- moonshotai/kimi-k2.7-code

##### morph (2)
- morph/morph-v3-fast
- morph/morph-v3-large

##### nex-agi (2)
- nex-agi/nex-n2-mini
- nex-agi/nex-n2-pro

##### nousresearch (5)
- nousresearch/hermes-3-llama-3.1-405b
- nousresearch/hermes-3-llama-3.1-405b:free
- nousresearch/hermes-3-llama-3.1-70b
- nousresearch/hermes-4-405b
- nousresearch/hermes-4-70b

##### nvidia (11)
- nvidia/llama-3.3-nemotron-super-49b-v1.5
- nvidia/nemotron-3-nano-30b-a3b
- nvidia/nemotron-3-nano-30b-a3b:free
- nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free
- nvidia/nemotron-3-super-120b-a12b
- nvidia/nemotron-3-super-120b-a12b:free
- nvidia/nemotron-3-ultra-550b-a55b
- nvidia/nemotron-3-ultra-550b-a55b:free
- nvidia/nemotron-3.5-content-safety:free
- nvidia/nemotron-nano-12b-v2-vl:free
- nvidia/nemotron-nano-9b-v2:free

##### openai (62)
- openai/gpt-3.5-turbo
- openai/gpt-3.5-turbo-0613
- openai/gpt-3.5-turbo-16k
- openai/gpt-3.5-turbo-instruct
- openai/gpt-4
- openai/gpt-4-turbo
- openai/gpt-4-turbo-preview
- openai/gpt-4.1
- openai/gpt-4.1-mini
- openai/gpt-4.1-nano
- openai/gpt-4o
- openai/gpt-4o-2024-05-13
- openai/gpt-4o-2024-08-06
- openai/gpt-4o-2024-11-20
- openai/gpt-4o-mini
- openai/gpt-4o-mini-2024-07-18
- openai/gpt-4o-mini-search-preview
- openai/gpt-4o-search-preview
- openai/gpt-5
- openai/gpt-5-chat
- openai/gpt-5-codex
- openai/gpt-5-image
- openai/gpt-5-image-mini
- openai/gpt-5-mini
- openai/gpt-5-nano
- openai/gpt-5-pro
- openai/gpt-5.1
- openai/gpt-5.1-chat
- openai/gpt-5.1-codex
- openai/gpt-5.1-codex-max
- openai/gpt-5.1-codex-mini
- openai/gpt-5.2
- openai/gpt-5.2-chat
- openai/gpt-5.2-codex
- openai/gpt-5.2-pro
- openai/gpt-5.3-chat
- openai/gpt-5.3-codex
- openai/gpt-5.4
- openai/gpt-5.4-image-2
- openai/gpt-5.4-mini
- openai/gpt-5.4-nano
- openai/gpt-5.4-pro
- openai/gpt-5.5
- openai/gpt-5.5-pro
- openai/gpt-audio
- openai/gpt-audio-mini
- openai/gpt-chat-latest
- openai/gpt-oss-120b
- openai/gpt-oss-120b:free
- openai/gpt-oss-20b
- openai/gpt-oss-20b:free
- openai/gpt-oss-safeguard-20b
- openai/o1
- openai/o1-pro
- openai/o3
- openai/o3-deep-research
- openai/o3-mini
- openai/o3-mini-high
- openai/o3-pro
- openai/o4-mini
- openai/o4-mini-deep-research
- openai/o4-mini-high

##### openrouter (5)
- openrouter/auto
- openrouter/bodybuilder
- openrouter/free
- openrouter/fusion
- openrouter/pareto-code

##### perceptron (1)
- perceptron/perceptron-mk1

##### perplexity (5)
- perplexity/sonar
- perplexity/sonar-deep-research
- perplexity/sonar-pro
- perplexity/sonar-pro-search
- perplexity/sonar-reasoning-pro

##### poolside (6)
- poolside/laguna-m.1
- poolside/laguna-m.1:free
- poolside/laguna-xs-2.1
- poolside/laguna-xs-2.1:free
- poolside/laguna-xs.2
- poolside/laguna-xs.2:free

##### qwen (49)
- qwen/qwen-2.5-72b-instruct
- qwen/qwen-2.5-7b-instruct
- qwen/qwen-2.5-coder-32b-instruct
- qwen/qwen-plus
- qwen/qwen-plus-2025-07-28
- qwen/qwen-plus-2025-07-28:thinking
- qwen/qwen2.5-vl-72b-instruct
- qwen/qwen3-14b
- qwen/qwen3-235b-a22b
- qwen/qwen3-235b-a22b-2507
- qwen/qwen3-235b-a22b-thinking-2507
- qwen/qwen3-30b-a3b
- qwen/qwen3-30b-a3b-instruct-2507
- qwen/qwen3-30b-a3b-thinking-2507
- qwen/qwen3-32b
- qwen/qwen3-8b
- qwen/qwen3-coder
- qwen/qwen3-coder-30b-a3b-instruct
- qwen/qwen3-coder-flash
- qwen/qwen3-coder-next
- qwen/qwen3-coder-plus
- qwen/qwen3-coder:free
- qwen/qwen3-max
- qwen/qwen3-max-thinking
- qwen/qwen3-next-80b-a3b-instruct
- qwen/qwen3-next-80b-a3b-instruct:free
- qwen/qwen3-next-80b-a3b-thinking
- qwen/qwen3-vl-235b-a22b-instruct
- qwen/qwen3-vl-235b-a22b-thinking
- qwen/qwen3-vl-30b-a3b-instruct
- qwen/qwen3-vl-30b-a3b-thinking
- qwen/qwen3-vl-32b-instruct
- qwen/qwen3-vl-8b-instruct
- qwen/qwen3-vl-8b-thinking
- qwen/qwen3.5-122b-a10b
- qwen/qwen3.5-27b
- qwen/qwen3.5-35b-a3b
- qwen/qwen3.5-397b-a17b
- qwen/qwen3.5-9b
- qwen/qwen3.5-flash-02-23
- qwen/qwen3.5-plus-02-15
- qwen/qwen3.5-plus-20260420
- qwen/qwen3.6-27b
- qwen/qwen3.6-35b-a3b
- qwen/qwen3.6-flash
- qwen/qwen3.6-max-preview
- qwen/qwen3.6-plus
- qwen/qwen3.7-max
- qwen/qwen3.7-plus

##### rekaai (2)
- rekaai/reka-edge
- rekaai/reka-flash-3

##### relace (2)
- relace/relace-apply-3
- relace/relace-search

##### sakana (1)
- sakana/fugu-ultra

##### sao10k (4)
- sao10k/l3-lunaris-8b
- sao10k/l3.1-70b-hanami-x1
- sao10k/l3.1-euryale-70b
- sao10k/l3.3-euryale-70b

##### stepfun (2)
- stepfun/step-3.5-flash
- stepfun/step-3.7-flash

##### switchpoint (1)
- switchpoint/router

##### tencent (4)
- tencent/hunyuan-a13b-instruct
- tencent/hy3
- tencent/hy3-preview
- tencent/hy3:free

##### thedrummer (4)
- thedrummer/cydonia-24b-v4.1
- thedrummer/rocinante-12b
- thedrummer/skyfall-36b-v2
- thedrummer/unslopnemo-12b

##### undi95 (1)
- undi95/remm-slerp-l2-13b

##### upstage (1)
- upstage/solar-pro-3

##### writer (1)
- writer/palmyra-x5

##### x-ai (4)
- x-ai/grok-4.20
- x-ai/grok-4.20-multi-agent
- x-ai/grok-4.3
- x-ai/grok-build-0.1

##### xiaomi (2)
- xiaomi/mimo-v2.5
- xiaomi/mimo-v2.5-pro

##### z-ai (12)
- z-ai/glm-4.5
- z-ai/glm-4.5-air
- z-ai/glm-4.5v
- z-ai/glm-4.6
- z-ai/glm-4.6v
- z-ai/glm-4.7
- z-ai/glm-4.7-flash
- z-ai/glm-5
- z-ai/glm-5-turbo
- z-ai/glm-5.1
- z-ai/glm-5.2
- z-ai/glm-5v-turbo

##### ~anthropic (4)
- ~anthropic/claude-fable-latest
- ~anthropic/claude-haiku-latest
- ~anthropic/claude-opus-latest
- ~anthropic/claude-sonnet-latest

##### ~google (2)
- ~google/gemini-flash-latest
- ~google/gemini-pro-latest

##### ~moonshotai (1)
- ~moonshotai/kimi-latest

##### ~openai (2)
- ~openai/gpt-latest
- ~openai/gpt-mini-latest

##### ~x-ai (1)
- ~x-ai/grok-latest

---

## Endpoints 核实

### 当前 preset 配置 (src-tauri/defaults/platform-presets.json):

```json
"endpoints": {
  "default": [
    {
      "protocol": "anthropic",
      "base_url": "https://openrouter.ai/api",
      "client_type": "claude_code"
    },
    {
      "protocol": "openai",
      "base_url": "https://openrouter.ai/api/v1",
      "client_type": "codex_tui"
    },
    {
      "protocol": "gemini",
      "base_url": "https://openrouter.ai/api",
      "client_type": "default"
    }
  ]
}
```

### 验证结果:

#### ✅ anthropic 端点: 正确
- **base_url**: `https://openrouter.ai/api` ✓
- **协议路径**: `/v1/messages`
- **Headers**: `x-api-key: $OPENROUTER_API_KEY`
- **格式**: Anthropic Messages API 兼容

#### ✅ openai 端点: 正确
- **base_url**: `https://openrouter.ai/api/v1` ✓
- **协议路径**: `/chat/completions`
- **Headers**: `Authorization: Bearer $OPENROUTER_API_KEY`
- **格式**: 标准 OpenAI Chat Completions API

#### ❌ gemini 端点: **不支持**
- **结论**: OpenRouter **不提供**原生 Gemini 协议端点
- **原因**: OpenRouter 只提供 OpenAI 和 Anthropic 兼容层
- **影响**: Google Gemini 模型只能通过 OpenAI 或 Anthropic 兼容协议访问
- **建议**: **移除** preset 中的 gemini 端点配置

---

## Preset 现状核实

### 当前 model_list.default (15 项):

| # | 模型 ID | 状态 |
|---|---------|------|
| 1 | anthropic/claude-opus-4.8 | ✅ 有效 |
| 2 | anthropic/claude-sonnet-4.6 | ✅ 有效 |
| 3 | anthropic/claude-opus-4.5 | ✅ 有效 |
| 4 | openai/gpt-5.5 | ✅ 有效 |
| 5 | openai/gpt-5.5-pro | ✅ 有效 |
| 6 | openai/gpt-5.3-codex | ✅ 有效 |
| 7 | google/gemini-3.5-flash | ✅ 有效 |
| 8 | google/gemini-3.1-pro-preview | ✅ 有效 |
| 9 | deepseek/deepseek-v4-pro | ✅ 有效 |
| 10 | deepseek/deepseek-v4-flash | ✅ 有效 |
| 11 | qwen/qwen3.7-max | ✅ 有效 |
| 12 | z-ai/glm-5.2 | ✅ 有效 |
| 13 | moonshotai/kimi-k2.7-code | ✅ 有效 |
| 14 | x-ai/grok-4.3 | ✅ 有效 |
| 15 | minimax/minimax-m3 | ✅ 有效 |

**结论**: 全部 15 个 preset 模型均有效 ✅

---

## models.default 建议

### 推荐配置:

```json
"models": {
  "default": {
    "default": "anthropic/claude-sonnet-4.6"
  }
}
```

**理由**:
- `claude-sonnet-4.6` 平衡性能与成本
- 是最受欢迎的 Claude 模型
- 适合通用场景

---

## 时效性与腐化风险

### 全量清单维护成本: **高**

#### 风险分析:
1. **高频更新**: OpenRouter 每日有模型上下架
2. **硬编码腐化**: preset 硬编码 344 个模型会快速过时
3. **验证成本**: 需定期调用 `/api/v1/models` 验证有效性

#### 建议:
- **Preset 保留精选模式**: 维持 15-20 个旗舰模型
- **运行时拉取全量**: 若需全量清单，建议在运行时从 API 动态获取
- **定期验证**: 设置 weekly/monthly 任务验证 preset 模型有效性

---

## 结论摘要

**一句话**: OpenRouter 聚合 57 个 provider 的 344 个模型，支持 OpenAI 和 Anthropic 协议，**不支持** Gemini 原生协议。

**关键数据**:
- 模型总数: 344
- Provider 数: 57
- 有效 preset 模型: 15/15 ✅
- 需移除端点: gemini
- 推荐默认模型: anthropic/claude-sonnet-4.6

---

## External References

- [OpenRouter Docs](https://openrouter.ai/docs)
- [OpenRouter Models](https://openrouter.ai/docs/models)
- [OpenRouter Quick Start](https://openrouter.ai/docs/quick-start)
- API: `GET https://openrouter.ai/api/v1/models`
