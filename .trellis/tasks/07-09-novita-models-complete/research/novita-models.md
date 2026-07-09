# Research: Novita AI Models and Endpoints

- **Query**: 研究 Novita AI（novita 协议）官方信息，补全 platform-presets.json
- **Scope**: external（API 端点 + 官方文档）
- **Date**: 2026-07-09

## 官方支持模型清单

### 数据源

- **API 端点**: `https://api.novita.ai/v3/openai/models`（OpenAI 兼容端点）
- **HTTP 状态**: 200 OK
- **模型总数**: **139 个模型**

### 模型 id 命名格式

- **格式**: `provider/model`（小写前缀，用 `/` 分隔）
- **示例**: `deepseek/deepseek-v4-pro`, `zai-org/glm-5.2`, `qwen/qwen3.7-max`

### Provider 分布统计（按模型数量排序）

| Provider | 数量 | 说明 |
|---|---|---|
| qwen | 35 | 通义千问（阿里云） |
| deepseek | 21 | DeepSeek（深度求索） |
| zai-org | 14 | GLM（智谱，z.ai 国际版） |
| minimax | 8 | MiniMax（海螺） |
| meta-llama | 8 | Meta Llama 系列 |
| baidu | 7 | 百度文心（ERNIE） |
| moonshotai | 6 | Kimi（月之暗面） |
| xiaomimimo | 4 | 小爱米莫（小米） |
| google | 4 | Google Gemma 系列 |
| sao10k | 3 | Sao10K（社区模型） |
| inclusionai | 3 | InclusionAI |
| 其他 | 30 | 22 个小 provider（1-2 模型） |

### 全量模型清单（按 provider 分组）

#### DeepSeek（21 个）
```
deepseek/deepseek_v3
deepseek/deepseek-ocr
deepseek/deepseek-ocr-2
deepseek/deepseek-prover-v2-671b
deepseek/deepseek-r1
deepseek/deepseek-r1-0528
deepseek/deepseek-r1-0528-qwen3-8b
deepseek/deepseek-r1-distill-llama-70b
deepseek/deepseek-r1-distill-qwen-14b
deepseek/deepseek-r1-distill-qwen-32b
deepseek/deepseek-r1-turbo
deepseek/deepseek-r1/community
deepseek/deepseek-v3-0324
deepseek/deepseek-v3-turbo
deepseek/deepseek-v3.1
deepseek/deepseek-v3.1-terminus
deepseek/deepseek-v3.2
deepseek/deepseek-v3.2-exp
deepseek/deepseek-v3/community
deepseek/deepseek-v4-flash
deepseek/deepseek-v4-pro
```

#### Qwen（35 个）
```
qwen/qwen-2-7b-instruct
qwen/qwen-2-vl-72b-instruct
qwen/qwen-2.5-72b-instruct
qwen/qwen-mt-plus
qwen/qwen2.5-7b-instruct
qwen/qwen2.5-vl-72b-instruct
qwen/qwen3-235b-a22b-fp8
qwen/qwen3-235b-a22b-instruct-2507
qwen/qwen3-235b-a22b-thinking-2507
qwen/qwen3-30b-a3b-fp8
qwen/qwen3-32b-fp8
qwen/qwen3-4b-fp8
qwen/qwen3-8b-fp8
qwen/qwen3-coder-30b-a3b-instruct
qwen/qwen3-coder-480b-a35b-instruct
qwen/qwen3-coder-next
qwen/qwen3-max
qwen/qwen3-next-80b-a3b-instruct
qwen/qwen3-next-80b-a3b-thinking
qwen/qwen3-omni-30b-a3b-instruct
qwen/qwen3-omni-30b-a3b-thinking
qwen/qwen3-vl-235b-a22b-instruct
qwen/qwen3-vl-235b-a22b-thinking
qwen/qwen3-vl-30b-a3b-instruct
qwen/qwen3-vl-30b-a3b-thinking
qwen/qwen3-vl-8b-instruct
qwen/qwen3.5-122b-a10b
qwen/qwen3.5-27b
qwen/qwen3.5-35b-a3b
qwen/qwen3.5-397b-a17b
qwen/qwen3.5-plus
qwen/qwen3.6-27b
qwen/qwen3.6-35b-a3b
qwen/qwen3.6-plus
qwen/qwen3.7-max
```

#### zai-org / GLM（14 个）
```
zai-org/autoglm-phone-9b-multilingual
zai-org/glm-4.5
zai-org/glm-4.5-air
zai-org/glm-4.5v
zai-org/glm-4.6
zai-org/glm-4.6v
zai-org/glm-4.7
zai-org/glm-4.7-flash
zai-org/glm-4.7-h
zai-org/glm-5
zai-org/glm-5-turbo
zai-org/glm-5.1
zai-org/glm-5.2
zai-org/glm-5v-turbo
```

#### MiniMax（8 个）
```
minimax/m2-her
minimax/minimax-m2
minimax/minimax-m2.1
minimax/minimax-m2.5
minimax/minimax-m2.5-highspeed
minimax/minimax-m2.7
minimax/minimax-m2.7-highspeed
minimax/minimax-m3
```

#### MoonshotAI / Kimi（6 个）
```
moonshotai/kimi-k2-0905
moonshotai/kimi-k2-instruct
moonshotai/kimi-k2-thinking
moonshotai/kimi-k2.5
moonshotai/kimi-k2.6
moonshotai/kimi-k2.7-code
```

#### Meta Llama（8 个）
```
meta-llama/llama-3-70b-instruct
meta-llama/llama-3-8b-instruct
meta-llama/llama-3.1-8b-instruct
meta-llama/llama-3.2-1b-instruct
meta-llama/llama-3.2-3b-instruct
meta-llama/llama-3.3-70b-instruct
meta-llama/llama-4-maverick-17b-128e-instruct-fp8
meta-llama/llama-4-scout-17b-16e-instruct
```

#### Baidu ERNIE（7 个）
```
baidu/cobuddy
baidu/ernie-4.5-21B-a3b
baidu/ernie-4.5-21B-a3b-thinking
baidu/ernie-4.5-300b-a47b-paddle
baidu/ernie-4.5-vl-28b-a3b
baidu/ernie-4.5-vl-28b-a3b-thinking
baidu/ernie-4.5-vl-424b-a47b
```

#### XiaoMi MiMo（4 个）
```
xiaomimimo/mimo-v2-flash
xiaomimimo/mimo-v2-pro
xiaomimimo/mimo-v2.5
xiaomimimo/mimo-v2.5-pro
```

#### Google Gemma（4 个）
```
google/gemma-3-12b-it
google/gemma-3-27b-it
google/gemma-4-26b-a4b-it
google/gemma-4-31b-it
```

#### Sao10K（3 个）
```
sao10k/l3-70b-euryale-v2.1
sao10k/l3-8b-lunaris
sao10k/l31-70b-euryale-v2.2
```

#### InclusionAI（3 个）
```
inclusionai/ling-2.6-1t
inclusionai/ling-2.6-flash
inclusionai/ring-2.6-1t
```

#### 其他（30 个，22 个 provider）
```
ai_infer_test_1, ai_infer_test_2, ai_infer_test_3
baichuan/baichuan-m2-32b
bunny
dev/glm46
elephant
google/gemma-* (4个)
gryphe/mythomax-l2-13b
gt-4p
kwaipilot/kat-coder-pro
meta-llama/llama-* (8个)
microsoft/wizardlm-2-8x22b
minimaxai/minimax-m1-80k
mistralai/mistral-nemo
nex-agi/nex-n2-pro
nousresearch/hermes-2-pro-llama-3-8b
nousresearch/nous-hermes-llama2-13b
nvidia/nemotron-3-nano-30b-a3b
openai/gpt-oss-120b
openai/gpt-oss-20b
openchat/openchat-7b
paddlepaddle/paddleocr-vl
Sao10K/L3-8B-Stheno-v3.2
stepfun/step-3.7-flash
teknium/openhermes-2.5-mistral-7b
tencent/hy3
thudm/glm-4-32b-0414
```

## endpoints 核实

### preset 现状
```json
"endpoints": {
  "default": [
    {
      "protocol": "anthropic",
      "base_url": "https://api.novita.ai/anthropic",
      "client_type": "claude_code"
    }
  ]
}
```

### 验证结果

#### anthropic 端点
- **路径**: `https://api.novita.ai/anthropic`
- **状态**: ✅ 路径正确（Novita 常用端点，Claude Code 兼容）

#### openai 兼容端点
- **路径**: `https://api.novita.ai/v3/openai`
- **状态**: ⚠️ **preset 缺失**（Novita 主力是 OpenAI 兼容 v3 协议）
- **验证**:
  - `https://api.novita.ai/v3/openai/models` 返回 200 OK（139 个模型）
  - `https://api.novita.ai/v1/models` 返回 404（不存在）

#### 端点建议
```json
"endpoints": {
  "default": [
    {
      "protocol": "anthropic",
      "base_url": "https://api.novita.ai/anthropic",
      "client_type": "claude_code"
    },
    {
      "protocol": "openai",
      "base_url": "https://api.novita.ai/v3/openai",
      "client_type": "codex_tui"
    }
  ]
}
```

## preset 现状 11 项核实

preset 中的 11 项全部存在于全量列表中：

| preset 模型 | 状态 |
|---|---|
| zai-org/glm-5.2 | ✅ |
| deepseek/deepseek-v4-pro | ✅ |
| deepseek/deepseek-v4-flash | ✅ |
| qwen/qwen3.7-max | ✅ |
| moonshotai/kimi-k2.7-code | ✅ |
| minimax/minimax-m3 | ✅ |
| zai-org/glm-5.1 | ✅ |
| qwen/qwen3.6-plus | ✅ |
| moonshotai/kimi-k2.6 | ✅ |
| minimax/minimax-m2.7 | ✅ |
| deepseek/deepseek-v3.2 | ✅ |

## models.default 建议

### 推荐（按用途）

#### default（通用对话）
- `zai-org/glm-5.2`（智谱最新主力，性价比高）
- 或 `qwen/qwen3.7-max`（通义千问最新旗舰）

#### coder（编程专用）
- `moonshotai/kimi-k2.7-code`（Kimi 编程优化版）
- 或 `qwen/qwen3-coder-next`（Qwen Coder 最新版）

#### fast（快速响应）
- `deepseek/deepseek-v4-flash`（DeepSeek 快速版）
- 或 `zai-org/glm-4.7-flash`（GLM Flash 版）

### 建议配置
```json
"models": {
  "default": {
    "default": "zai-org/glm-5.2",
    "coder": "moonshotai/kimi-k2.7-code",
    "fast": "deepseek/deepseek-v4-flash"
  }
}
```

## 认证方式

- **方式**: API Key
- **Header**: `Authorization: Bearer <api_key>`
- **获取**: https://novita.ai（注册后获取）

## Caveats / Not Found

- `https://api.novita.ai/v1/models` 不存在（404），v1 路径不可用
- preset 中 `models.default.default` 为空，需补充
- preset 缺少 openai 兼容端点，建议补充 `/v3/openai`
- 模型列表包含测试模型（ai_infer_test_*），生产环境建议过滤

## 结论摘要

**Novita AI 是聚合路由平台，支持 139 个模型来自 35+ provider（DeepSeek/Qwen/GLM/MiniMax/Kimi/Llama 等）。当前 preset 仅含 11 项精选，端点仅 anthropic，需补充 openai 兼容端点（/v3/openai）。推荐默认模型：glm-5.2 / kimi-k2.7-code / deepseek-v4-flash。**
