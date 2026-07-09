# 补全 nvidia model_list+endpoints 全部官方信息

## Goal

NVIDIA build (integrate.api.nvidia.com) 是 **OpenAI 兼容托管平台**，**121 模型 / 29 供应商**（2026-07-09 `GET /v1/models` 金标准）。仅 OpenAI 协议（不支持 anthropic/gemini，实测 404）。现有 1 openai endpoint base_url 正确。model_list 12 个中 **2 个 id 错误需修正**（`deepseek/deepseek-v3.2` 不存在→`deepseek-ai/deepseek-v4-pro`；`z-ai/glm-5.1` 不存在→`z-ai/glm-5.2`）。需补全主力模型 + 修正错误 id + 三档默认。

## Research References

- [`research/nvidia-models.md`](research/nvidia-models.md) — 121 模型金标准 `/v1/models` + endpoint 实测 + 12 现有核对

## Requirements

### 1. endpoints（default 分支，1 端点正确，不动）

```json
"endpoints": {
  "default": [
    {"protocol": "openai", "base_url": "https://integrate.api.nvidia.com/v1", "client_type": "default"}
  ]
}
```

仅 OpenAI 协议（实测 `/v1/messages` anthropic 返 404，无 gemini `:generateContent`）。

### 2. model_list.default（主力对话/coding/推理/VL，`provider/model-id` 格式）

从 research 121 清单提取主力文本对话 / 推理 / coding / 多模态 VL 模型，排除：纯 embedding(nv-embed/embedqa/embedcode) / rerank(nemoretriever-parse) / safety/guard(nemoguard/content-safety) / reward / translate(riva) / PII(gliner) / 科学计算(ising) / 视频检测(ai-synthetic-video-detector) / 旧非主力(llama2-70b)。

**NVIDIA 自研（主力对话+推理+VL，约 15）**：nemotron-3-ultra-550b-a55b / nemotron-3-super-120b-a12b / nemotron-3-nano-30b-a3b / nemotron-3-nano-omni-30b-a3b-reasoning / nemotron-nano-3-30b-a3b / nvidia-nemotron-nano-9b-v2 / nemotron-mini-4b-instruct / llama-3.1-nemotron-ultra-253b-v1 / llama-3.1-nemotron-70b-instruct / llama-3.1-nemotron-51b-instruct / llama-3.1-nemotron-nano-8b-v1 / llama-3.3-nemotron-super-49b-v1 / llama-3.3-nemotron-super-49b-v1.5 / llama3-chatqa-1.5-70b / mistral-nemo-minitron-8b-8k-instruct / llama-3.1-nemotron-nano-vl-8b-v1 / nemotron-nano-12b-v2-vl / neva-22b / vila / cosmos-reason2-8b

**Meta Llama（对话+VL+coder，9）**：llama-4-maverick-17b-128e-instruct / llama-3.3-70b-instruct / llama-3.1-70b-instruct / llama-3.1-8b-instruct / llama-3.2-90b-vision-instruct / llama-3.2-11b-vision-instruct / llama-3.2-3b-instruct / llama-3.2-1b-instruct / llama-guard-4-12b / codellama-70b

**DeepSeek（2，⚠️ 修正）**：deepseek-ai/deepseek-v4-pro / deepseek-ai/deepseek-v4-flash / deepseek-ai/deepseek-coder-6.7b-instruct
（现有 `deepseek/deepseek-v3.2` 前缀+版本双错，删）

**Qwen（3）**：qwen/qwen3.5-397b-a17b / qwen/qwen3.5-122b-a10b / qwen/qwen3-next-80b-a3b-instruct

**Mistral（对话+coder，约 9）**：mistralai/mistral-large-3-675b-instruct-2512 / mistralai/mistral-medium-3.5-128b / mistralai/mistral-small-4-119b-2603 / mistralai/mistral-large-2-instruct / mistralai/mistral-large / mistralai/mistral-nemotron / mistralai/ministral-14b-instruct-2512 / mistralai/mistral-7b-instruct-v0.3 / mistralai/mixtral-8x22b-v0.1 / mistralai/mixtral-8x7b-instruct-v0.1 / mistralai/codestral-22b-instruct-v0.1 / nv-mistralai/mistral-nemo-12b-instruct

**Google Gemma（对话+coder，8）**：google/gemma-4-31b-it / google/gemma-3-12b-it / google/gemma-3-4b-it / google/gemma-3n-e4b-it / google/gemma-3n-e2b-it / google/gemma-2-2b-it / google/codegemma-7b / google/codegemma-1.1-7b
（排除 diffusiongemma 图像 / deplot 图表 / recurrentgemma 非主力）

**Microsoft Phi（5 全）**：microsoft/phi-4-multimodal-instruct / microsoft/phi-4-mini-instruct / microsoft/phi-3.5-moe-instruct / microsoft/phi-3-vision-128k-instruct / microsoft/kosmos-2

**国内第三方旗舰（6，⚠️ 含 glm 修正）**：z-ai/glm-5.2（现有 `z-ai/glm-5.1` 错，改）/ moonshotai/kimi-k2.6 / minimaxai/minimax-m3 / minimaxai/minimax-m2.7 / stepfun-ai/step-3.7-flash / stepfun-ai/step-3.5-flash / bytedance/seed-oss-36b-instruct

**OpenAI 开源（2）**：openai/gpt-oss-120b / openai/gpt-oss-20b

**其他主流开源（对话，约 9）**：01-ai/yi-large / abacusai/dracarys-llama-3.1-70b-instruct / ai21labs/jamba-1.5-large-instruct / aisingapore/sea-lion-7b-instruct / bigcode/starcoder2-15b / databricks/dbrx-instruct / stockmark/stockmark-2-100b-instruct / upstage/solar-10.7b-instruct / zyphra/zamba2-7b-instruct / sarvamai/sarvam-m / adept/fuyu-8b

合计约 **85-95 模型**。

### 3. models.default（三档默认）

档位名 key → model id string（对齐 `Partial<Record<ModelSlot, string>>`，与 20 官方 protocol 同构）：

```json
"models": {
  "default": {
    "default": "nvidia/llama-3.3-nemotron-super-49b-v1.5",
    "thinking": "nvidia/nemotron-3-ultra-550b-a55b",
    "coder": "deepseek-ai/deepseek-v4-pro"
  }
}
```

三档：NVIDIA 主力通用（49B Nemotron Super v1.5，slot `default`）/ 推理旗舰（Nemotron 3 Ultra 550B，slot `thinking`）/ 第三方旗舰（DeepSeek V4 Pro，slot `coder`，DeepSeek 强编程向，推测归类）。

### 4. desc 改写（8 语言）

现有无 desc 或失实则改写。NVIDIA 是托管多供应商 OpenAI 兼容平台：
- en-US: "NVIDIA build OpenAI-compatible hosted models (Nemotron/Llama/Gemma/Phi/DeepSeek/Qwen/Mistral etc.)"
- zh-Hans: "NVIDIA build OpenAI 兼容托管模型（Nemotron/Llama/Gemma/Phi/DeepSeek/Qwen/Mistral 等）"
- 其余 6 语言同步翻译

## Acceptance Criteria

- [ ] endpoints 1 openai 端点保留
- [ ] `deepseek/deepseek-v3.2` 删除 → `deepseek-ai/deepseek-v4-pro` 等
- [ ] `z-ai/glm-5.1` → `z-ai/glm-5.2`
- [ ] model_list 约 85-95（主力对话/coding/推理/VL）
- [ ] models.default 三档
- [ ] desc 8 语言
- [ ] JSON 合法
- [ ] 仅改 nvidia 块

## Out of Scope

- 上下文窗口字段
- 价格（per-model Credit，走 price_sync.rs）
- IBM/Writer 细分商务模型（非通用主力）
- 纯 embedding/rerank/safety/reward 模型
- NIM 自托管（用户自有 GPU，非云）
- STATIC_MODEL_IDS
- 其他协议块

## Technical Notes

- 真值源：`protocols.nvidia`
- 金标准数据源：`GET https://integrate.api.nvidia.com/v1/models`（121 模型）
- 仅 OpenAI 协议（anthropic/gemini 实测 404）
- id 格式：`provider/model-id` 单斜杠
- 121 id 是 2026-07-09 快照，建议季度复核
