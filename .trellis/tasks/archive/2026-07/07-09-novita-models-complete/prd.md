# 补全 novita model_list+endpoints 全部官方信息

## Goal
Novita AI 是聚合路由平台（35+ provider，国产为主）。免鉴权 OpenAI 兼容端点 `GET https://api.novita.ai/v3/openai/models` 返回 200，含 **139 个模型**（`provider/model` 前缀格式）。当前 preset 仅 11 精选 + 单 anthropic 端点。本次改动：endpoints 补 openai 兼容端点（`/v3/openai`，验证存活）、model_list.default 扩为全量 139 模型（`provider/` 前缀）、models.default 补三档、desc/source_urls 保留（已准确）。数据强度：强（免鉴权 models API 200）。

## Research References
- [`research/novita-models.md`](research/novita-models.md) — `/v3/openai/models` 返 200 含 139 模型；id 格式 `provider/model`；`/v1/models` 404 不可用；preset 现 11 精选全部存在于全量清单；推荐 glm-5.2 / kimi-k2.7-code / deepseek-v4-flash

## Requirements

### 1. endpoints（default 分支，2 端点，新增 1）
现有 anthropic 保留，新增 openai 兼容（research 验证 `/v3/openai/models` 返 200，`/v1` 404 不可用）：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.novita.ai/anthropic", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.novita.ai/v3/openai", "client_type": "codex_tui"}
  ]
}
```

### 2. model_list.default（139 模型，`provider/model` 前缀，按 provider 分组）
来源：`GET https://api.novita.ai/v3/openai/models`（200，2026-07-09）。

**DeepSeek（21）**
```
deepseek/deepseek_v3, deepseek/deepseek-ocr, deepseek/deepseek-ocr-2, deepseek/deepseek-prover-v2-671b, deepseek/deepseek-r1, deepseek/deepseek-r1-0528, deepseek/deepseek-r1-0528-qwen3-8b, deepseek/deepseek-r1-distill-llama-70b, deepseek/deepseek-r1-distill-qwen-14b, deepseek/deepseek-r1-distill-qwen-32b, deepseek/deepseek-r1-turbo, deepseek/deepseek-r1/community, deepseek/deepseek-v3-0324, deepseek/deepseek-v3-turbo, deepseek/deepseek-v3.1, deepseek/deepseek-v3.1-terminus, deepseek/deepseek-v3.2, deepseek/deepseek-v3.2-exp, deepseek/deepseek-v3/community, deepseek/deepseek-v4-flash, deepseek/deepseek-v4-pro
```

**Qwen（35）**
```
qwen/qwen-2-7b-instruct, qwen/qwen-2-vl-72b-instruct, qwen/qwen-2.5-72b-instruct, qwen/qwen-mt-plus, qwen/qwen2.5-7b-instruct, qwen/qwen2.5-vl-72b-instruct, qwen/qwen3-235b-a22b-fp8, qwen/qwen3-235b-a22b-instruct-2507, qwen/qwen3-235b-a22b-thinking-2507, qwen/qwen3-30b-a3b-fp8, qwen/qwen3-32b-fp8, qwen/qwen3-4b-fp8, qwen/qwen3-8b-fp8, qwen/qwen3-coder-30b-a3b-instruct, qwen/qwen3-coder-480b-a35b-instruct, qwen/qwen3-coder-next, qwen/qwen3-max, qwen/qwen3-next-80b-a3b-instruct, qwen/qwen3-next-80b-a3b-thinking, qwen/qwen3-omni-30b-a3b-instruct, qwen/qwen3-omni-30b-a3b-thinking, qwen/qwen3-vl-235b-a22b-instruct, qwen/qwen3-vl-235b-a22b-thinking, qwen/qwen3-vl-30b-a3b-instruct, qwen/qwen3-vl-30b-a3b-thinking, qwen/qwen3-vl-8b-instruct, qwen/qwen3.5-122b-a10b, qwen/qwen3.5-27b, qwen/qwen3.5-35b-a3b, qwen/qwen3.5-397b-a17b, qwen/qwen3.5-plus, qwen/qwen3.6-27b, qwen/qwen3.6-35b-a3b, qwen/qwen3.6-plus, qwen/qwen3.7-max
```

**zai-org / GLM（14）**
```
zai-org/autoglm-phone-9b-multilingual, zai-org/glm-4.5, zai-org/glm-4.5-air, zai-org/glm-4.5v, zai-org/glm-4.6, zai-org/glm-4.6v, zai-org/glm-4.7, zai-org/glm-4.7-flash, zai-org/glm-4.7-h, zai-org/glm-5, zai-org/glm-5-turbo, zai-org/glm-5.1, zai-org/glm-5.2, zai-org/glm-5v-turbo
```

**MiniMax（8）**
```
minimax/m2-her, minimax/minimax-m2, minimax/minimax-m2.1, minimax/minimax-m2.5, minimax/minimax-m2.5-highspeed, minimax/minimax-m2.7, minimax/minimax-m2.7-highspeed, minimax/minimax-m3
```

**MoonshotAI / Kimi（6）**
```
moonshotai/kimi-k2-0905, moonshotai/kimi-k2-instruct, moonshotai/kimi-k2-thinking, moonshotai/kimi-k2.5, moonshotai/kimi-k2.6, moonshotai/kimi-k2.7-code
```

**Meta Llama（8）**
```
meta-llama/llama-3-70b-instruct, meta-llama/llama-3-8b-instruct, meta-llama/llama-3.1-8b-instruct, meta-llama/llama-3.2-1b-instruct, meta-llama/llama-3.2-3b-instruct, meta-llama/llama-3.3-70b-instruct, meta-llama/llama-4-maverick-17b-128e-instruct-fp8, meta-llama/llama-4-scout-17b-16e-instruct
```

**Baidu ERNIE（7）**
```
baidu/cobuddy, baidu/ernie-4.5-21B-a3b, baidu/ernie-4.5-21B-a3b-thinking, baidu/ernie-4.5-300b-a47b-paddle, baidu/ernie-4.5-vl-28b-a3b, baidu/ernie-4.5-vl-28b-a3b-thinking, baidu/ernie-4.5-vl-424b-a47b
```

**XiaoMi MiMo（4）**
```
xiaomimimo/mimo-v2-flash, xiaomimimo/mimo-v2-pro, xiaomimimo/mimo-v2.5, xiaomimimo/mimo-v2.5-pro
```

**Google Gemma（4）**
```
google/gemma-3-12b-it, google/gemma-3-27b-it, google/gemma-4-26b-a4b-it, google/gemma-4-31b-it
```

**Sao10K（3）**
```
sao10k/l3-70b-euryale-v2.1, sao10k/l3-8b-lunaris, sao10k/l31-70b-euryale-v2.2
```

**InclusionAI（3）**
```
inclusionai/ling-2.6-1t, inclusionai/ling-2.6-flash, inclusionai/ring-2.6-1t
```

**其他（30，22 个小 provider + 散点）**
```
ai_infer_test_1, ai_infer_test_2, ai_infer_test_3, baichuan/baichuan-m2-32b, bunny, dev/glm46, elephant, gryphe/mythomax-l2-13b, gt-4p, kwaipilot/kat-coder-pro, microsoft/wizardlm-2-8x22b, minimaxai/minimax-m1-80k, mistralai/mistral-nemo, nex-agi/nex-n2-pro, nousresearch/hermes-2-pro-llama-3-8b, nousresearch/nous-hermes-llama2-13b, nvidia/nemotron-3-nano-30b-a3b, openai/gpt-oss-120b, openai/gpt-oss-20b, openchat/openchat-7b, paddlepaddle/paddleocr-vl, Sao10K/L3-8B-Stheno-v3.2, stepfun/step-3.7-flash, teknium/openhermes-2.5-mistral-7b, tencent/hy3, thudm/glm-4-32b-0414
```

> 注：research「其他」标题写「30 个，22 个 provider」但实际列出 26 条；以 research 原文为真值转录，最终以验证命令 `len(model_list.default)` 为准。

### 3. models.default（三档，档位名 key → model id string）
aidog 真值格式 = `Partial<Record<ModelSlot, string>>`，key 是档位名（default/coder/fast 等），value 是 model id 字符串。research 原文用档位名 key 的写法**符合** aidog 约定，prd 沿用。

```json
"models": {
  "default": {
    "default": "zai-org/glm-5.2",
    "coder": "moonshotai/kimi-k2.7-code",
    "fast": "deepseek/deepseek-v4-flash"
  }
}
```

| 档位（key） | 模型（value） | 理由 |
|------|------|------|
| `default` | `zai-org/glm-5.2` | 智谱最新主力，性价比高，通用对话兜底 |
| `coder` | `moonshotai/kimi-k2.7-code` | Kimi 编程优化版，coding 档 |
| `fast` | `deepseek/deepseek-v4-flash` | DeepSeek 快速版，轻量响应档 |

### 4. desc（保留，8 语言不动）
现有 "Novita AI API, 大语言与图像模型" 准确，保留。

### 5. source_urls（保留）
- docs: `https://novita.ai/docs`
- pricing: `https://novita.ai/pricing`

## Acceptance Criteria
- [ ] endpoints.default 含 2 端点（anthropic + 新增 openai `/v3/openai`）
- [ ] model_list.default 含 139 个 `provider/model` 前缀 id
- [ ] models.default 三档：default=zai-org/glm-5.2 / coder=moonshotai/kimi-k2.7-code / fast=deepseek/deepseek-v4-flash（档位名 key → string）
- [ ] desc 8 语言保留
- [ ] source_urls 保留
- [ ] JSON 合法
- [ ] 验证命令输出：`139 {'default': 'zai-org/glm-5.2', 'coder': 'moonshotai/kimi-k2.7-code', 'fast': 'deepseek/deepseek-v4-flash'} 2`

## Out of Scope
- STATIC_MODEL_IDS / peak_hours / coding_plan / 其他协议块 / id 日期后缀 / 测试模型（ai_infer_test_*）过滤（research 未明确要求过滤，照录）

## Technical Notes
- 真值源：`protocols.novita`
- 数据来源：`GET https://api.novita.ai/v3/openai/models`（免鉴权 200）+ curl 端点验证；`/v1/models` 404 不可用
- id 格式：`provider/model`（小写前缀，`/` 分隔）
- `models.default` 真值格式 = 档位名 key → model id string（`Partial<Record<ModelSlot, string>>`）；research 原文用档位名 key 符合 aidog 约定，prd 沿用
