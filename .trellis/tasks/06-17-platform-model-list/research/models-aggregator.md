# Research: 聚合平台组「常用代表模型」候选集

- **Query**: 为 aidog 聚合平台组（12 平台）核常用代表模型 API id 子集，供内置候选下拉冷启动兜底
- **Scope**: external（平台公开模型列表 / pricing 端点 + 文档）
- **Date**: 2026-06-17（核查日期）

## 总则

- **fetchModels 为主源**：以下列表仅作冷启动兜底候选。模型名月级腐化，运行时必靠平台 `/v1/models` fetchModels 拉取真实可用集。
- 每平台「真实可用 API model id」来自下文 `来源 URL` 列出的该平台**公开模型列表 / pricing JSON 端点**实拉（核查日 2026-06-17），无标注者为推测。
- 旗舰在前，截取 5-15 个热门代表，不求全。聚合平台普遍代理数百模型（OpenRouter 337 / aihubmix 339 / dmxapi 756），此处只挑各家旗舰（claude-opus/sonnet、gpt-5.x、gemini-3.x、deepseek-v4、qwen3.x、glm-5.x、kimi-k2.x、grok-4.x、minimax）。
- **命名风格差异关键**：OpenAI 风格聚合（therouter/shengsuanyun/novita/openrouter）用 `vendor/model` slug；Anthropic 直转聚合（aihubmix/dmxapi）用 `claude-opus-4-8`（**连字符非点号**）；HuggingFace 风格（modelscope/atlascloud/siliconflow）用 `Vendor/Model-Name`。同一平台常同时暴露多套别名，下方取该平台**主流规范 id**。

---

## openrouter

- **来源 URL**: `https://openrouter.ai/api/v1/models`（公开无鉴权，实拉 337 模型）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **代表 API id**（OpenAI 风格 `vendor/model` slug）：

```
anthropic/claude-opus-4.8
anthropic/claude-sonnet-4.6
anthropic/claude-opus-4.5
openai/gpt-5.5
openai/gpt-5.5-pro
openai/gpt-5.3-codex
google/gemini-3.5-flash
google/gemini-3.1-pro-preview
deepseek/deepseek-v4-pro
deepseek/deepseek-v4-flash
qwen/qwen3.7-max
z-ai/glm-5.2
moonshotai/kimi-k2.7-code
x-ai/grok-4.3
minimax/minimax-m3
```

> 注：OpenRouter 同时提供 `~anthropic/claude-sonnet-latest` 等浮动 latest 别名与 `:free` / `:fast` 变体，候选下拉建议用上方固定版本。

---

## siliconflow（SiliconFlow 国内版 api.siliconflow.cn）

- **来源 URL**: `https://api.siliconflow.cn/v1/models`（**需鉴权**，返回 "Invalid token"）；docs `https://docs.siliconflow.cn/cn/userguide/capabilities/text-generation`（**严重缓存腐化**，仅返 2024 年 DeepSeek-V2.5/V3、Qwen2.5 旧模型，不可信）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **状态**: **公开端点无法验证当前列表**。下方为按 SiliconFlow 一贯 HuggingFace 命名风格（`Vendor/Model-Name`，与已验证的 ModelScope/AtlasCloud 同款）推测的旗舰候选，**务必靠 fetchModels 兜底核实**：

```
推测: deepseek-ai/DeepSeek-V3.2
推测: deepseek-ai/DeepSeek-V3.1
推测: Qwen/Qwen3-235B-A22B-Instruct-2507
推测: Qwen/Qwen3-Coder-480B-A35B-Instruct
推测: Qwen/Qwen3-Next-80B-A3B-Instruct
推测: zai-org/GLM-4.6
推测: moonshotai/Kimi-K2-Instruct-0905
推测: MiniMaxAI/MiniMax-M2
```

> 注：SiliconFlow 仅托管开源中系模型（无 claude/gpt 闭源）。具体版本以 fetchModels 实拉为准。

---

## siliconflow_en（SiliconFlow 国际版 api.siliconflow.com）

- **来源 URL**: `https://api.siliconflow.com/v1/models`（**需鉴权**，"Invalid token"）；docs `https://docs.siliconflow.com/en/...`（同样缓存腐化）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **状态**: **未找到可验证公开列表，靠 fetchModels**。国际版模型集与国内版基本同款（HuggingFace 风格 id），参照 `siliconflow` 节推测候选，实际以 fetchModels 为准。

---

## aihubmix

- **来源 URL**: `https://aihubmix.com/v1/models`（公开无鉴权，实拉 339 模型）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **代表 API id**（Anthropic/OpenAI 原厂直名，claude 用**连字符** `claude-opus-4-8`）：

```
claude-opus-4-8
claude-sonnet-4-6
claude-sonnet-4-5
gpt-5.5
gpt-5.5-pro
gpt-5.3-codex
gemini-3.5-flash
gemini-3.1-pro-preview
deepseek-v4-pro
deepseek-v4-flash
qwen3.7-max
glm-5.2
kimi-k2.7-code
grok-4.3
```

> 注：aihubmix 另有 `claude-opus-4-8-think` / `coding-glm-5.2` / `gpt-5.5-free` 等变体。候选下拉取上方主名。

---

## dmxapi

- **来源 URL**: `https://www.dmxapi.cn/api/pricing`（公开 JSON，`data.model_info[].model_name`，实拉 756 模型）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **代表 API id**（原厂直名，claude 用连字符）：

```
claude-opus-4-8
claude-sonnet-4-6
claude-opus-4-5-20251101
deepseek-v4-pro
deepseek-v4-flash
gpt-5.5
gpt-5.3-codex
gemini-3.5-flash
gemini-3.1-pro-preview
glm-5.2
kimi-k2.7-code
```

> 注：dmxapi 大量带后缀变体（`-ssvip` / `-cc` / `-thinking` / `-guan`），候选下拉取无后缀主名。

---

## modelscope（魔搭）

- **来源 URL**: `https://api-inference.modelscope.cn/v1/models`（公开无鉴权，实拉 60 模型）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **代表 API id**（HuggingFace 风格 `Vendor/Model-Name`，仅开源中系模型）：

```
deepseek-ai/DeepSeek-V4-Pro
deepseek-ai/DeepSeek-V4-Flash
deepseek-ai/DeepSeek-V3.2
Qwen/Qwen3.5-397B-A17B
Qwen/Qwen3.5-122B-A10B
Qwen/Qwen3-Coder-30B-A3B-Instruct
ZhipuAI/GLM-5.2
ZhipuAI/GLM-5.1
ZhipuAI/GLM-5
moonshotai/Kimi-K2.5
MiniMax/MiniMax-M3
MiniMax/MiniMax-M2.7
```

---

## shengsuanyun（盛算云）

- **来源 URL**: `https://router.shengsuanyun.com/api/v1/models`（公开无鉴权，`data[].api_name`，实拉 160 模型）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **代表 API id**（OpenAI 风格 `vendor/model` slug，注意 qwen 厂前缀为 `ali/`、glm 为 `bigmodel/`、kimi 为 `moonshot/`）：

```
anthropic/claude-opus-4.8
anthropic/claude-sonnet-4.6
anthropic/claude-opus-4.5
openai/gpt-5.5
openai/gpt-5.3-codex
google/gemini-3.5-flash
google/gemini-3.1-pro-preview
deepseek/deepseek-v4-pro
deepseek/deepseek-v4-flash
ali/qwen3.7-max
bigmodel/glm-5.2
moonshot/kimi-k2.7-code
x-ai/grok-4
```

---

## atlascloud

- **来源 URL**: `https://api.atlascloud.ai/v1/models`（公开无鉴权，`data[].id`，实拉 130 模型）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **代表 API id**（HuggingFace 风格；该平台目录偏开源，**当前最高到 GLM-4.6 / DeepSeek-V3.2，无 V4/GLM-5**）：

```
deepseek-ai/DeepSeek-V3.2-Exp
deepseek-ai/DeepSeek-V3.1-Terminus
deepseek-ai/DeepSeek-V3-0324
zai-org/GLM-4.6
Qwen/Qwen3-235B-A22B-Instruct-2507
Qwen/Qwen3-Coder
Qwen/Qwen3-Next-80B-A3B-Instruct
Qwen/Qwen3-VL-235B-A22B-Instruct
moonshotai/Kimi-K2-Thinking
moonshotai/Kimi-K2-Instruct-0905
MiniMaxAI/MiniMax-M2
```

---

## novita（Novita AI）

- **来源 URL**: `https://api.novita.ai/v3/openai/models`（公开无鉴权，`data[].id`，实拉 139 模型）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **代表 API id**（OpenAI 风格 slug，glm 厂前缀为 `zai-org/`）：

```
zai-org/glm-5.2
deepseek/deepseek-v4-pro
deepseek/deepseek-v4-flash
qwen/qwen3.7-max
moonshotai/kimi-k2.7-code
minimax/minimax-m3
zai-org/glm-5.1
qwen/qwen3.6-plus
moonshotai/kimi-k2.6
minimax/minimax-m2.7
deepseek/deepseek-v3.2
```

> 注：Novita 仅托管开源模型（无 claude/gpt 闭源）。base_url 走 `/anthropic` 子路径做协议转换，但模型 id 仍是 OpenAI 风格 slug。

---

## therouter（TheRouter）

- **核查日**: 2026-06-17 | fetchModels 为主源
- **状态**: **未找到可验证公开模型列表，靠 fetchModels**。已探测端点全部不可用：
  - `https://api.therouter.ai/v1/models` → 需鉴权（"Missing or invalid Authorization header"）
  - `https://api.therouter.ai/v1/pricing` / `/api/pricing` / `/api/v1/models` → 404
  - `https://therouter.ai/models` 页面 → 纯客户端渲染（Next.js），无静态 model id
  - `https://dashboard.therouter.ai/` → Cloudflare 拦截
- **推测**: TheRouter 为 OpenRouter 兼容聚合，采 `vendor/model` slug 风格（如 `anthropic/claude-opus-4.8`、`openai/gpt-5.5`、`deepseek/deepseek-v4-pro`）。**不内置硬编码候选，完全依赖运行时 fetchModels**。

---

## cherryin（CherryIN）

- **来源 URL**: `https://open.cherryin.net/api/pricing`（公开 JSON，model id 见 `cache_ratio` / `model_ratio` keys，实拉 256 模型）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **代表 API id**（该平台**同时暴露多套别名**：`anthropic/claude-opus-4.8` 点号风格、`agent/...` 代理风格、裸名 `deepseek-chat` 等。下方取 OpenAI/Anthropic 规范 slug）：

```
anthropic/claude-opus-4.8
anthropic/claude-sonnet-4.6
anthropic/claude-opus-4.5
openai/gpt-5.5
openai/gpt-5.3-codex
google/gemini-3.5-flash
google/gemini-3-pro-preview
deepseek/deepseek-v4-pro
deepseek/deepseek-v4-flash
deepseek/deepseek-v3.2
agent/glm-5.2
moonshotai/kimi-k2.7-code
grok-4
```

> 注：CherryIN 闭源模型（claude/gpt/gemini/grok）与开源（deepseek/glm/qwen/kimi）混合代理。同一模型常有 `agent/xxx`（代理优惠通道）与 `vendor/xxx` 两种 id，候选下拉取 `vendor/` 规范名，运行时以 fetchModels 实拉为准。

---

## nvidia（Nvidia NIM / integrate.api.nvidia.com）

- **来源 URL**: `https://integrate.api.nvidia.com/v1/models`（公开无鉴权，`data[].id`，实拉 121 模型）
- **核查日**: 2026-06-17 | fetchModels 为主源
- **代表 API id**（NIM 风格 `vendor/model`，旗舰为 nemotron 系 + 主流开源）：

```
nvidia/nemotron-3-ultra-550b-a55b
nvidia/nemotron-3-super-120b-a12b
nvidia/llama-3.3-nemotron-super-49b-v1.5
deepseek/deepseek-v3.2
qwen/qwen3.5-397b-a17b
qwen/qwen3-next-80b-a3b-instruct
z-ai/glm-5.1
moonshotai/kimi-k2.6
minimaxai/minimax-m3
meta/llama-4-maverick-17b-128e-instruct
meta/llama-3.3-70b-instruct
openai/gpt-oss-120b
```

> 注：Nvidia NIM 仅托管开源 / 自家 nemotron 模型（无 claude/gpt 闭源 chat）。`openai/gpt-oss-120b` 是开源权重非 GPT-5。

---

## Caveats / Not Found

- **9/12 平台**给出基于公开端点实拉的代表列表（核查日 2026-06-17，真实可用 id）：openrouter, aihubmix, dmxapi, modelscope, shengsuanyun, atlascloud, novita, cherryin, nvidia。
- **3/12 平台**公开端点不可访问，列表为推测或缺失，靠 fetchModels：
  - `siliconflow`（需鉴权 + 文档缓存腐化）→ 推测 HuggingFace 风格候选
  - `siliconflow_en`（同上）→ 未找到，靠 fetchModels
  - `therouter`（全端点 404 / 需鉴权 / SPA）→ 未找到，靠 fetchModels
- **模型版本快速腐化**：当前（2026-06）旗舰已到 claude-opus-4.8 / gpt-5.5 / gemini-3.5 / deepseek-v4 / glm-5.2 / qwen3.7 / kimi-k2.7。内置候选须随版本翻新，**最终必走 fetchModels 兜底**，内置列表仅冷启动占位。
- **命名风格三派**务必区分（影响 id 拼写）：OpenAI slug（点号）/ Anthropic 直名（连字符）/ HuggingFace（`Vendor/Name`）。同平台多别名时取规范主名。
