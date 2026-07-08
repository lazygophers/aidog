# Research: shengsuanyun (盛算云)

- **Query**: 研究盛算云官方信息，补全 platform-presets.json
- **Scope**: external (官方 API 端点 + 文档)
- **Date**: 2026-07-09

## Findings

### 官方支持模型清单（总计 172 个模型）

数据源：`curl -s https://router.shengsuanyun.com/api/v1/models`

#### Provider 分布统计

| Provider | 数量 | 说明 |
|----------|------|------|
| openai | 32 | GPT-4/5/o/o3/o4 系列 + codex 变体 |
| ali | 26 | Qwen 3/3.5/3.6/3.7 系列 + VL/Coder |
| bigmodel | 19 | GLM 4/4.5/4.6/4.7/5/Z1 系列 |
| anthropic | 17 | Claude Opus/Sonnet/Fable/Haiku 全系 |
| bytedance | 13 | Doubao Seed 1.6/2.0/2.1 系列 |
| google | 10 | Gemini 2.5/3/3.1/3.5 + Gemma |
| deepseek | 9 | V3/V4 系列 + R1/OCR |
| minimax | 8 | M1/M2/M2.1/M2.5/M2.7/M3 系列 |
| moonshot | 6 | Kimi K2/K2.5/K2.6/K2.7-code 系列 |
| xiaomi | 5 | MiMo V2/V2.5 系列 |
| tencent | 5 | Hunyuan T1/Turbo + HY3 系列 |
| baidu | 5 | Ernie 4.0/4.5 Turbo 系列 |
| x-ai | 4 | Grok 3/4/4.1-fast 系列 |
| intern | 4 | Intern S1/S2/VL3.5 系列 |
| streamlake | 3 | KAT Coder Air/Exp/Pro 系列 |
| stepfun | 2 | Step 3.5 Flash/2603 系列 |
| meta | 2 | Llama 3.3/4-scout |
| xai | 1 | Grok Code Fast |
| longcat | 1 | Longcat 2.0 |

#### 全量模型列表（按 Provider 分组）

**Anthropic (17):**
- anthropic/claude-3.5-haiku
- anthropic/claude-3.7-sonnet:thinking
- anthropic/claude-fable-5
- anthropic/claude-haiku-4.5
- anthropic/claude-haiku-4.5:thinking
- anthropic/claude-opus-4
- anthropic/claude-opus-4.1
- anthropic/claude-opus-4.5
- anthropic/claude-opus-4.6
- anthropic/claude-opus-4.7
- anthropic/claude-opus-4.8
- anthropic/claude-sonnet-4
- anthropic/claude-sonnet-4:thinking
- anthropic/claude-sonnet-4.5
- anthropic/claude-sonnet-4.5:thinking
- anthropic/claude-sonnet-4.6
- anthropic/claude-sonnet-5

**OpenAI (32):**
- openai/codex-mini-latest
- openai/gpt-4.1
- openai/gpt-4.1-mini
- openai/gpt-4.1-nano
- openai/gpt-4o-2024-11-20
- openai/gpt-4o-mini
- openai/gpt-5
- openai/gpt-5-codex
- openai/gpt-5-mini
- openai/gpt-5-nano
- openai/gpt-5.1
- openai/gpt-5.1-codex
- openai/gpt-5.1-codex-max
- openai/gpt-5.1-codex-mini
- openai/gpt-5.2
- openai/gpt-5.2-codex
- openai/gpt-5.3-chat
- openai/gpt-5.3-codex
- openai/gpt-5.4
- openai/gpt-5.4-mini
- openai/gpt-5.4-nano
- openai/gpt-5.4-pro
- openai/gpt-5.5
- openai/gpt-oss-120b
- openai/gpt-oss-20b
- openai/o1
- openai/o3
- openai/o3-deep-research
- openai/o3-mini
- openai/o3-mini-high
- openai/o4-mini
- openai/o4-mini-high

**Google (10):**
- google/gemini-2.5-flash
- google/gemini-2.5-flash-lite
- google/gemini-2.5-flash-live
- google/gemini-2.5-pro
- google/gemini-3-flash
- google/gemini-3.1-flash-lite
- google/gemini-3.1-flash-lite-preview
- google/gemini-3.1-pro-preview
- google/gemini-3.5-flash
- google/gemma-4-31b-it

**DeepSeek (9):**
- deepseek/deepseek-ocr
- deepseek/deepseek-r1-0528
- deepseek/deepseek-v3
- deepseek/deepseek-v3.1
- deepseek/deepseek-v3.1-think
- deepseek/deepseek-v3.2
- deepseek/deepseek-v3.2-think
- deepseek/deepseek-v4-flash
- deepseek/deepseek-v4-pro

**Ali / Qwen (26):**
- ali/qvq-72b
- ali/qwen-plus-latest
- ali/qwen-plus-latest:thinking
- ali/qwen-turbo-latest
- ali/qwen-turbo-latest:thinking
- ali/qwen-vl-ocr
- ali/qwen-vl-plus
- ali/qwen3-235b-a22b
- ali/qwen3-235b-a22b-instruct-2507
- ali/qwen3-235b-a22b-thinking-2507
- ali/qwen3-coder-480b-a35b-instruct
- ali/qwen3-coder-plus
- ali/qwen3-max
- ali/qwen3-max-2026-01-23
- ali/qwen3-max-preview
- ali/qwen3-next-80b-a3b-Instruct
- ali/qwen3-next-80b-a3b-thinking
- ali/qwen3-omni-flash
- ali/qwen3-vl-plus
- ali/qwen3.5-397b-a17b
- ali/qwen3.5-flash
- ali/qwen3.5-plus
- ali/qwen3.6-max-preview
- ali/qwen3.6-plus
- ali/qwen3.7-max
- ali/qwen3.7-plus

**Bigmodel / GLM (19):**
- bigmodel/glm-4-plus
- bigmodel/glm-4.5
- bigmodel/glm-4.5-air
- bigmodel/glm-4.5-air:thinking
- bigmodel/glm-4.5-airx
- bigmodel/glm-4.5-airx:thinking
- bigmodel/glm-4.5-x
- bigmodel/glm-4.5-x:thinking
- bigmodel/glm-4.5:thinking
- bigmodel/glm-4.6
- bigmodel/glm-4.6:thinking
- bigmodel/glm-4.7
- bigmodel/glm-5
- bigmodel/glm-5-turbo
- bigmodel/glm-5.1
- bigmodel/glm-5.2
- bigmodel/glm-5v-turbo
- bigmodel/glm-z1-air
- bigmodel/glm-z1-airx

**Moonshot / Kimi (6):**
- moonshot/kimi-k2
- moonshot/kimi-k2.5
- moonshot/kimi-k2.6
- moonshot/kimi-k2.7-code
- moonshot/kimi-latest
- moonshot/kimi-thinking-preview

**MiniMax (8):**
- minimax/minimax-m1
- minimax/minimax-m2
- minimax/minimax-m2.1
- minimax/minimax-m2.1-lightning
- minimax/minimax-m2.5
- minimax/minimax-m2.7
- minimax/minimax-m2.7-highspeed
- minimax/minimax-m3

**Bytedance / Doubao (13):**
- bytedance/doubao-pro-256k
- bytedance/doubao-seed-1.6
- bytedance/doubao-seed-1.6-flash
- bytedance/doubao-seed-1.6:thinking
- bytedance/doubao-seed-1.8
- bytedance/doubao-seed-2-0-code
- bytedance/doubao-seed-2-0-lite
- bytedance/doubao-seed-2-0-mini
- bytedance/doubao-seed-2-0-pro
- bytedance/doubao-seed-2-1-pro
- bytedance/doubao-seed-2-1-turbo
- bytedance/doubao-seed-character
- bytedance/doubao-seed-evolving

**x-ai / Grok (4):**
- x-ai/grok-3
- x-ai/grok-4
- x-ai/grok-4-fast
- x-ai/grok-4.1-fast

**Xiaomi / MiMo (5):**
- xiaomi/mimo-v2-flash
- xiaomi/mimo-v2-omni
- xiaomi/mimo-v2-pro
- xiaomi/mimo-v2.5
- xiaomi/mimo-v2.5-pro

**Tencent / Hunyuan (5):**
- tencent/hunyuan-t1-vision
- tencent/hunyuan-turbo-vision
- tencent/hunyuan-turbos-vision
- tencent/hy3
- tencent/hy3-preview

**Baidu / Ernie (5):**
- baidu/ernie-4.0-turbo-128k
- baidu/ernie-4.5-turbo-128k
- baidu/ernie-4.5-turbo-32k
- baidu/ernie-4.5-turbo-preview
- baidu/ernie-4.5-turbo-vl-preview

**Intern (4):**
- intern/intern-s1
- intern/intern-s1-pro
- intern/intern-s2-preview
- intern/internvl3.5

**Streamlake (3):**
- streamlake/kat-coder-air-v1
- streamlake/kat-coder-exp-72b-1010
- streamlake/kat-coder-pro-v1

**StepFun (2):**
- stepfun/step-3.5-flash
- stepfun/step-3.5-flash-2603

**Meta (2):**
- meta/llama-3.3-70b-instruct
- meta/llama-4-scout

**Xai (1):**
- xai/grok-code-fast-1

**Longcat (1):**
- longcat/longcat-2.0

---

### 模型 id 命名格式

**格式**: `provider/model-name` 前缀格式

**来源验证**: API 返回的 `api_name` 字段直接使用此格式（如 `anthropic/claude-opus-4.8`, `deepseek/deepseek-v4-pro`）。

**规则**:
- Provider 前缀小写（anthropic, openai, google, deepseek, ali, bigmodel, moonshot, minimax, bytedance, x-ai, xiaomi, tencent, baidu, intern, streamlake, stepfun, meta, xai, longcat）
- 模型名称保留原始大小写和版本号
- 特殊后缀用冒号分隔（`:thinking`, `:latest`）

**注意**: 某些 provider 在 preset 中可能用不同名称映射（如 `x-ai` vs `xai`），但 API 返回的是真实值。

---

### Preset 现状 13 项核实

**当前 preset 的 model_list.default (13 项):**

| 模型 | 状态 | 说明 |
|------|------|------|
| anthropic/claude-opus-4.8 | ✅ 有效 | 最新 Opus |
| anthropic/claude-sonnet-4.6 | ✅ 有效 | 主力 Sonnet |
| anthropic/claude-opus-4.5 | ✅ 有效 | 较旧 Opus |
| openai/gpt-5.5 | ✅ 有效 | 最新 GPT |
| openai/gpt-5.3-codex | ⚠️ 缺失 | API 中无此名，应为 `openai/gpt-5.3-codex`（需核实） |
| google/gemini-3.5-flash | ✅ 有效 | 主力 Gemini |
| google/gemini-3.1-pro-preview | ✅ 有效 | Gemini 3.1 Pro |
| deepseek/deepseek-v4-pro | ✅ 有效 | DeepSeek V4 Pro |
| deepseek/deepseek-v4-flash | ✅ 有效 | DeepSeek V4 Flash |
| ali/qwen3.7-max | ✅ 有效 | Qwen 3.7 Max |
| bigmodel/glm-5.2 | ✅ 有效 | GLM 5.2 |
| moonshot/kimi-k2.7-code | ✅ 有效 | Kimi Code |
| x-ai/grok-4 | ✅ 有效 | Grok 4 |

**缺失主力模型**（建议补充）:
- `openai/gpt-5.4` / `gpt-5.4-pro`
- `openai/gpt-5.2-codex`
- `openai/o3-mini`
- `anthropic/claude-sonnet-5`
- `google/gemini-2.5-pro`
- `ali/qwen3-coder-plus`
- `minimax/minimax-m3`
- `stepfun/step-3.5-flash`

**需核实的项**:
- `openai/gpt-5.3-codex` — API 中未找到，可能已下架或改名

---

### Endpoints 核实

**当前 preset (仅 anthropic 端点):**
```json
"endpoints": {
  "default": [{
    "protocol": "anthropic",
    "base_url": "https://router.shengsuanyun.com/api",
    "client_type": "claude_code"
  }]
}
```

**API 端点支持情况**（从 `support_apis` 字段）:

| Provider | 支持的 API 端点 |
|----------|----------------|
| anthropic 模型 | `/v1/chat/completions`, `/v1/messages` |
| openai 模型 | `/v1/chat/completions`, `/v1/messages`, `/v1/responses` |
| google 模型 | `/v1/chat/completions`, `/v1/messages`, `/v1/models/*`, `/v1beta/models/*`, `/ws/gemini-live` |
| deepseek 模型 | `/v1/chat/completions`, `/v1/completions`, `/v1/messages` |

**结论**:
1. **Anthropic 端点正确**：`https://router.shengsuanyun.com/api` + `/v1/messages`
2. **建议补充 openai 端点**：`https://router.shengsuanyun.com/api` + `/v1/chat/completions`（OpenAI 兼容）
3. **可选补充 gemini 端点**：`https://router.shengsuanyun.com/api` + `/v1beta/models/*`（原生 Gemini）

**注意**: 所有端点共享同一 `base_url`（`https://router.shengsuanyun.com/api`），仅协议路径不同。

---

### Models.default 建议

基于 API 返回的 172 个模型，推荐按场景分类的默认模型：

**default（通用对话）**:
- 推荐: `anthropic/claude-sonnet-4.6`（平衡性能与成本）
- 备选: `openai/gpt-5.5`（最强推理）

**coder（编程）**:
- 推荐: `openai/gpt-5.3-codex` 或 `ali/qwen3-coder-plus`
- 备选: `openai/gpt-5.2-codex`

**fast（快速响应）**:
- 推荐: `google/gemini-3.5-flash`（性价比最高）
- 备选: `deepseek/deepseek-v4-flash`

**建议实现**:
```json
"models": {
  "default": {
    "default": "anthropic/claude-sonnet-4.6",
    "coder": "ali/qwen3-coder-plus",
    "fast": "google/gemini-3.5-flash"
  }
}
```

---

### 认证方式

**推测**: 基于 OpenAI 兼容 API，应为 `Authorization: Bearer <api_key>` 格式。

**需要确认**:
- API key 获取方式（控制台生成）
- 是否区分不同 provider 的 key
- 是否有其他认证头（如 `X-API-Key`）

---

## 结论摘要

盛算云是聚合路由平台，官方支持 **172 个模型**，来自 **19 家 provider**。模型 id 使用 `provider/model-name` 前缀格式，所有模型共享 `https://router.shengsuanyun.com/api` 基础 URL。

**关键发现**:
1. 模型总数: **172**
2. Provider 覆盖: OpenAI(32) + Qwen(26) + GLM(19) + Anthropic(17) + 等
3. 模型 id 命名: **`provider/model-name` 前缀格式**（API 验证）
4. Endpoint: 需补充 **openai 兼容端点**（当前仅有 anthropic）
5. models.default: 当前为空，建议补充 default/coder/fast

**PRD 用一句话**: 盛算云聚合 19 家 provider 共 172 个模型，使用 provider 前缀命名格式，需补充 openai 端点与默认模型配置。

---

## Caveats / Not Found

- `openai/gpt-5.3-codex` — preset 中存在但 API 返回的 172 模型中未找到，需核实是否已下架或改名
- 认证方式 — 未经官方文档证实，推测为 Bearer token
- Gemini 原生端点 — 虽有 `/v1beta/models/*` 支持，但未在 preset 中配置
- `/v1/completions` 路径 — deepseek 模型支持，但 preset 中未配置对应的 completion 端点
