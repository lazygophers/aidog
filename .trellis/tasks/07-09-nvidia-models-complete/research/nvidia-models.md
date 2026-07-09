# Research: NVIDIA build 全量托管模型 + endpoints

- **Query**: 查清 NVIDIA build/integrate API 全量托管模型 + endpoint 形态，为 `platform-presets.json` 补全提供权威数据
- **Scope**: external
- **Date**: 2026-07-09

## 关键结论（TL;DR）

- **base_url `https://integrate.api.nvidia.com/v1` 正确**，OpenAI 兼容，chat 路径 `/chat/completions`（验证：无 auth POST 返 401）
- **不支持 anthropic/gemini 协议**：`/v1/messages`（Anthropic 风格）返 404
- **鉴权**：标准 `Authorization: Bearer <NVIDIA_API_KEY>`（build.nvidia.com 控制台生成）；`/v1/models` 列模型不需有效 token（用于探测），其他接口 401
- **全量 121 个模型**（直接命中官方 OpenAI 兼容 `/v1/models` 端点，gold source），29 个供应商
- **现有 12 个 preset 模型核对**：9 ✅ + 3 ⚠️（见末节，需修正）
- **NIM（NVIDIA Inference Microservices）**：是 NVIDIA 自托管容器产品（cloud.ngc.nvidia.com 部署到用户自有 GPU），与 integrate.api.nvidia.com 云托管 API 不同；aidog preset 不需要单独 NIM endpoint

## API Endpoints

### 主端点（云托管，aidog preset 用此）

| 项 | 值 | 来源 |
|---|---|---|
| base_url | `https://integrate.api.nvidia.com/v1` | build.nvidia.com 模型页「Try API」+ curl 示例 |
| 协议 | OpenAI Chat Completions 兼容 | 实测响应 `{object:list,data:[...]}` 标准格式 |
| chat 路径 | `/chat/completions` | POST 实测无 token 返 401 |
| models 列表路径 | `/models` | GET 不需有效 token |
| embeddings 路径 | `/embeddings` | 标准 OpenAI 格式（embedding 模型调用） |
| 鉴权 | `Authorization: Bearer <key>` | NVIDIA 控制台生成 NGC API key |
| Anthropic 兼容 | ❌ 不支持（`/v1/messages` 实测 404） | 直接探测 |
| Gemini 兼容 | ❌ 不支持（无 `:generateContent` 路径） | NVIDIA 仅暴露 OpenAI 协议 |

curl 调用范式（来自 build.nvidia.com 模型详情页）：
```
curl https://integrate.api.nvidia.com/v1/chat/completions \
  -H "Authorization: Bearer $NVIDIA_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"meta/llama-3.3-70b-instruct","messages":[{"role":"user","content":"hi"}]}'
```

### NIM 自托管（不是云）

NIM = 容器化部署到用户自有 GPU（本地或云厂商 GPU 实例），base_url 由部署者决定（如 `http://localhost:8000/v1`）。不在 aidog 默认 preset 范围内，用户如自部署可在平台 form 里改 base_url。

## 全量模型清单（121 个，来源：`GET /v1/models` 2026-07-09）

> 模型 id 即 API 调用字符串，精确可用。`owned_by` 为 NVIDIA 平台归档分类（与 id 前缀一致）。括号内为类型标注：💬 通用对话 / 🧠 推理旗舰 / ⚡ 轻量 / 👁 多模态 VL / 💻 代码 / 🔢 embedding / 🛡 安全 / 🎁 reward / 🌐 翻译 / 🔍 rerank-parse。

### NVIDIA 自研（45）

#### Nemotron 旗舰 / 推理
- `nvidia/nemotron-3-ultra-550b-a55b` 🧠 旗舰
- `nvidia/nemotron-3-super-120b-a12b` 🧠
- `nvidia/nemotron-3-nano-30b-a3b` ⚡
- `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning` 🧠
- `nvidia/nemotron-nano-3-30b-a3b` ⚡
- `nvidia/nvidia-nemotron-nano-9b-v2` ⚡
- `nvidia/nemotron-mini-4b-instruct` ⚡
- `nvidia/nemotron-4-340b-instruct` 🧠 老旗舰

#### Llama-Nemotron 系（NVIDIA 后训练 Llama）
- `nvidia/llama-3.1-nemotron-ultra-253b-v1` 🧠
- `nvidia/llama-3.1-nemotron-70b-instruct` 💬
- `nvidia/llama-3.1-nemotron-51b-instruct` 💬
- `nvidia/llama-3.1-nemotron-nano-8b-v1` ⚡
- `nvidia/llama-3.3-nemotron-super-49b-v1` 💬
- `nvidia/llama-3.3-nemotron-super-49b-v1.5` 💬（preset 已含 ✅）
- `nvidia/llama3-chatqa-1.5-70b` 💬
- `nvidia/mistral-nemo-minitron-8b-8k-instruct` 💬

#### Nemotron 多模态 / VL
- `nvidia/llama-3.1-nemotron-nano-vl-8b-v1` 👁
- `nvidia/nemotron-nano-12b-v2-vl` 👁
- `nvidia/neva-22b` 👁
- `nvidia/vila` 👁
- `nvidia/cosmos-reason2-8b` 👁 推理
- `nvidia/ai-synthetic-video-detector` 👁（视频合成检测）

#### Embedding / Retrieval / Rerank
- `nvidia/nv-embed-v1` 🔢 通用旗舰
- `nvidia/nv-embedqa-e5-v5` 🔢
- `nvidia/nv-embedqa-mistral-7b-v2` 🔢
- `nvidia/nv-embedcode-7b-v1` 🔢 代码 embedding
- `nvidia/llama-3.2-nv-embedqa-1b-v1` 🔢
- `nvidia/llama-3.2-nemoretriever-1b-vlm-embed-v1` 🔢 多模态检索
- `nvidia/llama-nemotron-embed-1b-v2` 🔢
- `nvidia/llama-nemotron-embed-vl-1b-v2` 🔢
- `nvidia/embed-qa-4` 🔢
- `nvidia/nvclip` 🔢 多模态
- `nvidia/nemoretriever-parse` 🔍 文档解析
- `nvidia/nemotron-parse` 🔍

#### 安全 / Guard / Reward
- `nvidia/llama-3.1-nemoguard-8b-content-safety` 🛡
- `nvidia/llama-3.1-nemoguard-8b-topic-control` 🛡
- `nvidia/llama-3.1-nemotron-safety-guard-8b-v3` 🛡
- `nvidia/nemotron-3-content-safety` 🛡
- `nvidia/nemotron-3.5-content-safety` 🛡
- `nvidia/nemotron-content-safety-reasoning-4b` 🛡
- `nvidia/nemotron-4-340b-reward` 🎁

#### 工具 / 其他
- `nvidia/riva-translate-4b-instruct` 🌐 翻译
- `nvidia/riva-translate-4b-instruct-v1.1` 🌐
- `nvidia/gliner-pii` PII 识别
- `nvidia/ising-calibration-1-35b-a3b` 科学计算

### Meta Llama 系（11）
- `meta/llama-4-maverick-17b-128e-instruct` 🧠（preset 已含 ✅）
- `meta/llama-3.3-70b-instruct` 💬（preset 已含 ✅）
- `meta/llama-3.1-70b-instruct` 💬
- `meta/llama-3.1-8b-instruct` ⚡
- `meta/llama-3.2-90b-vision-instruct` 👁
- `meta/llama-3.2-11b-vision-instruct` 👁
- `meta/llama-3.2-3b-instruct` ⚡
- `meta/llama-3.2-1b-instruct` ⚡
- `meta/llama-guard-4-12b` 🛡
- `meta/codellama-70b` 💻
- `meta/llama2-70b` 旧

### DeepSeek 系（3）
- `deepseek-ai/deepseek-v4-pro` 🧠 旗舰
- `deepseek-ai/deepseek-v4-flash` ⚡
- `deepseek-ai/deepseek-coder-6.7b-instruct` 💻
- ⚠️ preset 写的 `deepseek/deepseek-v3.2` **不存在**（前缀应为 `deepseek-ai/`，且当前最新是 v4 系，无 v3.2 版本）

### Qwen 系（3）
- `qwen/qwen3.5-397b-a17b` 🧠 旗舰（preset 已含 ✅）
- `qwen/qwen3.5-122b-a10b` 🧠
- `qwen/qwen3-next-80b-a3b-instruct` ⚡（preset 已含 ✅）

### Mistral 系（11 + nv-mistralai 1）
- `mistralai/mistral-large-3-675b-instruct-2512` 🧠 旗舰
- `mistralai/mistral-medium-3.5-128b` 🧠
- `mistralai/mistral-small-4-119b-2603` 💬
- `mistralai/mistral-large-2-instruct` 🧠
- `mistralai/mistral-large` 🧠
- `mistralai/mistral-nemotron` 💬（NVIDIA 联合后训练）
- `mistralai/ministral-14b-instruct-2512` 💬
- `mistralai/mistral-7b-instruct-v0.3` ⚡
- `mistralai/mixtral-8x22b-v0.1` 🧠
- `mistralai/mixtral-8x7b-instruct-v0.1` 🧠
- `mistralai/codestral-22b-instruct-v0.1` 💻
- `nv-mistralai/mistral-nemo-12b-instruct` 💬

### Google Gemma 系（12）
- `google/gemma-4-31b-it` 🧠 最新大版
- `google/gemma-3-12b-it` 💬
- `google/gemma-3-4b-it` ⚡
- `google/gemma-3n-e4b-it` ⚡
- `google/gemma-3n-e2b-it` ⚡
- `google/diffusiongemma-26b-a4b-it` 🧠
- `google/gemma-2-2b-it` ⚡
- `google/gemma-2b` ⚡
- `google/codegemma-7b` 💻
- `google/codegemma-1.1-7b` 💻
- `google/recurrentgemma-2b` ⚡
- `google/deplot` 👁（图表理解）

### Microsoft Phi 系（5）
- `microsoft/phi-4-multimodal-instruct` 👁💻
- `microsoft/phi-4-mini-instruct` ⚡
- `microsoft/phi-3.5-moe-instruct` 💬
- `microsoft/phi-3-vision-128k-instruct` 👁
- `microsoft/kosmos-2` 👁

### 国内第三方旗舰
- `z-ai/glm-5.2` 🧠（⚠️ preset 写 `z-ai/glm-5.1` 不存在，当前最新 5.2）
- `moonshotai/kimi-k2.6` 🧠（preset 已含 ✅）
- `minimaxai/minimax-m3` 🧠（preset 已含 ✅）
- `minimaxai/minimax-m2.7` ⚡
- `stepfun-ai/step-3.7-flash` ⚡
- `stepfun-ai/step-3.5-flash` ⚡
- `bytedance/seed-oss-36b-instruct` 💬

### OpenAI 开源系（2）
- `openai/gpt-oss-120b` 🧠（preset 已含 ✅）
- `openai/gpt-oss-20b` ⚡

### 其他主流开源（11）
- `01-ai/yi-large` 🧠
- `abacusai/dracarys-llama-3.1-70b-instruct` 💬
- `ai21labs/jamba-1.5-large-instruct` 🧠
- `aisingapore/sea-lion-7b-instruct` ⚡
- `bigcode/starcoder2-15b` 💻
- `databricks/dbrx-instruct` 🧠
- `stockmark/stockmark-2-100b-instruct` 🧠
- `upstage/solar-10.7b-instruct` 💬
- `zyphra/zamba2-7b-instruct` ⚡
- `sarvamai/sarvam-m` 💬（印度语系）
- `adept/fuyu-8b` 👁

### Embedding / Rerank（其他供应商）
- `baai/bge-m3` 🔢
- `snowflake/arctic-embed-l` 🔢

### IBM（4）/ Writer（4）
- `ibm/granite-3-8b-instruct` 等（具体 4 个 ibm/* 模型， Granite 系商务模型）
- `writer/palmyra-*` 等（4 个 writer/* 系企业写作模型）

> 完整 IBM / Writer 4 个 id 见 `GET /v1/models` 原始响应；属商务细分模型，非通用对话主力，preset 不强制收录。

## 三档默认推荐（供 `models.default`）

> 建议按「主力通用 + 推理 + 轻量」三档，每档选 1 个最稳旗舰，避免 preset default 膨胀。最终选型由 main 决定。

| 档 | 推荐 id | 理由 |
|---|---|---|
| 主力通用（默认） | `nvidia/llama-3.3-nemotron-super-49b-v1.5` | NVIDIA 自研后训练 + 中等规模（49B）+ 速度快 + 已在 preset ✅；NVIDIA build 平台首推 |
| 推理旗舰 | `nvidia/nemotron-3-ultra-550b-a55b` | Nemotron 3 Ultra 550B，NVIDIA 当前最强推理模型，已在 preset ✅ |
| 第三方旗舰 | `deepseek-ai/deepseek-v4-pro` | DeepSeek V4 Pro，推理强、社区热度高；备选 `z-ai/glm-5.2` / `qwen/qwen3.5-397b-a17b` |

## 现有 12 个 preset 模型核对

| preset 当前 id | 状态 | 备注 |
|---|---|---|
| `nvidia/nemotron-3-ultra-550b-a55b` | ✅ 存在 | 保留 |
| `nvidia/nemotron-3-super-120b-a12b` | ✅ 存在 | 保留 |
| `nvidia/llama-3.3-nemotron-super-49b-v1.5` | ✅ 存在 | 保留 |
| `deepseek/deepseek-v3.2` | ⚠️ **不存在** | 前缀错（应 `deepseek-ai/`）+ 版本错（无 v3.2，当前 v4）→ 改 `deepseek-ai/deepseek-v4-pro` |
| `qwen/qwen3.5-397b-a17b` | ✅ 存在 | 保留 |
| `qwen/qwen3-next-80b-a3b-instruct` | ✅ 存在 | 保留 |
| `z-ai/glm-5.1` | ⚠️ **不存在** | 当前是 `z-ai/glm-5.2`（无 5.1）→ 升级 |
| `moonshotai/kimi-k2.6` | ✅ 存在 | 保留 |
| `minimaxai/minimax-m3` | ✅ 存在 | 保留 |
| `meta/llama-4-maverick-17b-128e-instruct` | ✅ 存在 | 保留 |
| `meta/llama-3.3-70b-instruct` | ✅ 存在 | 保留 |
| `openai/gpt-oss-120b` | ✅ 存在 | 保留 |

**核对结论**：12 个中 9 个精确命中 NVIDIA 目录；3 个 id 字符串与官方目录不一致（`deepseek/deepseek-v3.2` / `z-ai/glm-5.1` 需修正，`nvidia/llama-3.3-nemotron-super-49b-v1.5` 命中）。补全 model_list 时应按本文件「全量模型清单」筛选主力模型扩充。

## 数据来源

| URL | 用途 | 访问日期 |
|---|---|---|
| `GET https://integrate.api.nvidia.com/v1/models` | **全量 121 模型权威清单**（OpenAI 兼容端点） | 2026-07-09 |
| `POST https://integrate.api.nvidia.com/v1/chat/completions`（无 auth） | 验证 chat 路径 + 鉴权要求（401） | 2026-07-09 |
| `POST https://integrate.api.nvidia.com/v1/messages`（Anthropic 风格） | 验证不支持 Anthropic（404） | 2026-07-09 |
| https://build.nvidia.com/models | 模型目录（客户端渲染，HTML 不含静态列表，靠 /v1/models 取真值） | 2026-07-09 |
| https://build.nvidia.com/pricing | 价格页（per-model Credit 计费） | 2026-07-09 |
| https://docs.api.nvidia.com/ | NGC / NGC API key 文档 | 2026-07-09 |

## Caveats / 不确定项

- `/v1/models` 端点 `created` 字段对所有模型都是同一 Unix 时间戳（`735790403`），表明这是目录同步时间非模型发布时间；不影响 id 准确性。
- 121 个 id 是 2026-07-09 快照，NVIDIA 平台定期新增/下线（如下次 v5 模型发布）；补 preset 后建议每季度复核一次。
- IBM / Writer 各 4 个 id 未在主表展开（细分商务用途），如需收录运行 `curl https://integrate.api.nvidia.com/v1/models | jq '.data[].id' | grep -E '^(ibm|writer)/'` 取最新。
- 上下文窗口未逐个标注（build.nvidia.com 模型详情页才显示，无批量 API）；如需 ctx 长度需逐模型 fetch `https://build.nvidia.com/<id>`。
- 价格信息未列入本文件（task 未要求；如需补可在 build.nvidia.com/pricing 取每模型 Credit 数）。
