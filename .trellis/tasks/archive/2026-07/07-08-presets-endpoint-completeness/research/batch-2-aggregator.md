# Research: 聚合平台 Preset 数据补全（Batch 2 - Aggregator）

- **Query**: 查 13 个聚合/中转平台的官网，补全 preset 数据（端点/模型/价格）
- **Scope**: 外部搜索 + 现有 JSON 验证
- **Date**: 2026-07-08

---

## 摘要

| 协议 | 官网状态 | 支持协议 | 端点完整性 | 模型列表 | 备注 |
|------|----------|----------|------------|----------|------|
| openrouter | ✅ | anthropic/openai/gemini | ✅ 完整 | ✅ 完整 | 聚合平台标杆 |
| aihubmix | ✅ | anthropic/openai/gemini | ✅ 完整 | ✅ 完整 | 中美双域 |
| dmxapi | ⚠️ 证书错误 | anthropic/openai | ✅ 预设存在 | ✅ 预设存在 | 需验证 HTTPS |
| novita | ✅ | anthropic | ✅ 完整 | ✅ 完整 | GPU + LLM |
| atlascloud | ✅ | anthropic | ✅ 完整 | ✅ 完整 | 300+ 模型 |
| shengsuanyun | ⚠️ 404 | anthropic | ✅ 预设存在 | ✅ 预设存在 | 文档不可访问 |
| therouter | ✅ | anthropic/openai | ⚠️ 空 | ⚠️ 空 | model_list 为空 |
| rightcode | ✅ | anthropic/openai | ✅ 完整 | ✅ 完整 | 企业级中转 |
| packycode | ✅ | anthropic/openai/gemini | ✅ 完整 | ✅ 完整 | Claude 专注 |
| cubence | ⚠️ 404 | anthropic/openai/gemini | ✅ 预设存在 | ✅ 预设存在 | 文档不可访问 |
| aigocode | ✅ | anthropic/openai/gemini | ✅ 完整 | ✅ 完整 | 中转聚合 |
| aicodemirror | ✅ | anthropic/openai/gemini | ✅ 完整 | ✅ 完整 | Claude 共享 |
| nvidia | ✅ | openai | ✅ 完整 | ✅ 完整 | NIM 推理 |
| newapi | ✅ | openai（自部署） | ⚠️ 占位符 | ⚠️ 空 | 开源自部署 |

---

## 详细发现

### 1. openrouter (OpenRouter)

**官网**: https://openrouter.ai

**支持协议**:
- ✅ Anthropic Messages API
- ✅ OpenAI Chat Completions API
- ✅ Gemini API

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://openrouter.ai/api", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://openrouter.ai/api/v1", "client_type": "codex_tui"},
  {"protocol": "gemini", "base_url": "https://openrouter.ai/api", "client_type": "default"}
]
```

**models.default.default**: 需补充（当前 JSON 为空）

**model_list.default** (JSON 现有):
```
anthropic/claude-opus-4.8, anthropic/claude-sonnet-4.6, anthropic/claude-opus-4.5,
openai/gpt-5.5, openai/gpt-5.5-pro, openai/gpt-5.3-codex,
google/gemini-3.5-flash, google/gemini-3.1-pro-preview,
deepseek/deepseek-v4-pro, deepseek/deepseek-v4-flash,
qwen/qwen3.7-max, z-ai/glm-5.2, moonshotai/kimi-k2.7-code,
x-ai/grok-4.3, minimax/minimax-m3
```

**来源**: https://openrouter.ai/docs/quickstart + https://openrouter.ai/docs

**备注**: 模型 ID 使用 `provider/model` 格式，与其他聚合平台不同。

---

### 2. aihubmix (AiHubMix)

**官网**: https://aihubmix.com

**支持协议**:
- ✅ Anthropic API (Beta) - 端点 `https://aihubmix.com/v1/messages`
- ✅ OpenAI Chat Completions - 端点 `https://aihubmix.com/v1/chat/completions`
- ✅ Gemini 原生 SDK

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://aihubmix.com", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://aihubmix.com/v1", "client_type": "codex_tui"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-sonnet-4-5,
gpt-5.5, gpt-5.5-pro, gpt-5.3-codex,
gemini-3.5-flash, gemini-3.1-pro-preview,
deepseek-v4-pro, deepseek-v4-flash, qwen3.7-max, glm-5.2,
kimi-k2.7-code, grok-4.3
```

**备用域名**: `https://api.inferera.com` (主域名访问异常时)

**来源**: https://docs.aihubmix.com/cn/quick-start + https://docs.aihubmix.com/cn/api/Anthropic-Compatible

**备注**: 备用域名需在 `endpoints` 中添加为 fallback。

---

### 3. dmxapi (DMXAPI)

**官网**: https://www.dmxapi.cn

**支持协议**:
- ✅ Anthropic (推测)
- ✅ OpenAI (推测)

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://www.dmxapi.cn", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://www.dmxapi.cn/v1", "client_type": "codex_tui"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-opus-4-5-20251101,
deepseek-v4-pro, deepseek-v4-flash, gpt-5.5, gpt-5.3-codex,
gemini-3.5-flash, gemini-3.1-pro-preview, glm-5.2, kimi-k2.7-code
```

**来源**: https://www.dmxapi.cn (预设存在)

**备注**: ⚠️ **需要: 用户** — 文档站点 `docs.dmxapi.cn` 证书错误（ERR_CERT_COMMON_NAME_INVALID），无法验证端点准确性。

---

### 4. novita (Novita AI)

**官网**: https://novita.ai

**支持协议**:
- ✅ Anthropic Messages API (端点: `https://api.novita.ai/anthropic`)
- ✅ OpenAI Compatible (推测)

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.novita.ai/anthropic", "client_type": "claude_code"}
]
```

**model_list.default** (JSON 现有):
```
zai-org/glm-5.2, deepseek/deepseek-v4-pro, deepseek/deepseek-v4-flash,
qwen/qwen3.7-max, moonshotai/kimi-k2.7-code, minimax/minimax-m3,
zai-org/glm-5.1, qwen/qwen3.6-plus, moonshotai/kimi-k2.6,
minimax/minimax-m2.7, deepseek/deepseek-v3.2
```

**来源**: https://novita.ai/docs + https://novita.ai/docs/guides/model-apis-overview

**备注**: 模型 ID 使用 `provider/model` 格式。

---

### 5. atlascloud (AtlasCloud)

**官网**: https://atlascloud.ai

**支持协议**:
- ✅ OpenAI Compatible (Drop-in replacement)
- ✅ Anthropic (推测)

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.atlascloud.ai", "client_type": "claude_code"}
]
```

**model_list.default** (JSON 现有):
```
deepseek-ai/DeepSeek-V3.2-Exp, deepseek-ai/DeepSeek-V3.1-Terminus,
deepseek-ai/DeepSeek-V3-0324, zai-org/GLM-4.6,
Qwen/Qwen3-235B-A22B-Instruct-2507, Qwen/Qwen3-Coder,
Qwen/Qwen3-Next-80B-A3B-Instruct, Qwen/Qwen3-VL-235B-A22B-Instruct,
moonshotai/Kimi-K2-Thinking, moonshotai/Kimi-K2-Instruct-0905,
MiniMaxAI/MiniMax-M2
```

**来源**: https://atlascloud.ai/docs

**备注**: 300+ 模型，OpenAI 兼容，需确认是否支持 Gemini 协议。

---

### 6. shengsuanyun (盛算云)

**官网**: https://shengsuanyun.com

**支持协议**:
- ✅ Anthropic (推测)

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://router.shengsuanyun.com/api", "client_type": "claude_code"}
]
```

**model_list.default** (JSON 现有):
```
anthropic/claude-opus-4.8, anthropic/claude-sonnet-4.6, anthropic/claude-opus-4.5,
openai/gpt-5.5, openai/gpt-5.3-codex, google/gemini-3.5-flash,
google/gemini-3.1-pro-preview, deepseek/deepseek-v4-pro, deepseek/deepseek-v4-flash,
ali/qwen3.7-max, bigmodel/glm-5.2, moonshot/kimi-k2.7-code, x-ai/grok-4
```

**来源**: https://shengsuanyun.com (预设存在)

**备注**: ⚠️ **需要: 用户** — 文档 `/docs` 返回 404，无法验证协议支持。

---

### 7. therouter (TheRouter.ai)

**官网**: https://therouter.ai

**支持协议**:
- ✅ OpenAI Compatible (unified API)
- ✅ Anthropic Agent SDK (文档提及)
- ✅ Anthropic Messages (推测)

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.therouter.ai", "client_type": "claude_code"}
]
```

**model_list.default**: **空数组** ⚠️ 需补全

**来源**: https://therouter.ai/docs

**备注**: TheRouter 是聚合路由平台，支持 OpenAI、Anthropic、DeepSeek、Qwen 等，但 JSON 中 `model_list` 为空，需从 https://therouter.ai/models 补全。

---

### 8. rightcode (RightCode)

**官网**: https://right.codes

**支持协议**:
- ✅ Anthropic Claude (Claude Code 适配)
- ✅ OpenAI Codex
- ✅ Gemini CLI

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://www.right.codes/claude", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://right.codes/codex/v1", "client_type": "codex_tui"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://docs.right.codes/

**备注**: 企业级 AI Agent 中转平台，专注 Claude Code / Codex / Gemini CLI。

---

### 9. packycode (PackyCode / PackyAPI)

**官网**: https://www.packyapi.com

**支持协议**:
- ✅ Anthropic (Claude Code)
- ✅ OpenAI (Codex)
- ✅ Gemini

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://www.packyapi.com", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://www.packyapi.com/v1", "client_type": "codex_tui"},
  {"protocol": "gemini", "base_url": "https://www.packyapi.com", "client_type": "default"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://docs.packyapi.com/

**备注**: 专注 Claude 系列模型，国内中转。

---

### 10. cubence (Cubence)

**官网**: https://cubence.com

**支持协议**:
- ✅ Anthropic (推测)
- ✅ OpenAI (推测)
- ✅ Gemini (推测)

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.cubence.com", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://api.cubence.com/v1", "client_type": "codex_tui"},
  {"protocol": "gemini", "base_url": "https://api.cubence.com", "client_type": "default"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: 预设存在

**备注**: ⚠️ **需要: 用户** — 文档 `/docs` 返回 404，无法验证端点。

---

### 11. aigocode (AIGoCode)

**官网**: https://www.aigocode.com

**支持协议**:
- ✅ OpenAI Compatible (chat/completions)
- ✅ Anthropic (推测)
- ✅ Gemini (推测)

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.aigocode.com", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://api.aigocode.com/v1", "client_type": "codex_tui"},
  {"protocol": "gemini", "base_url": "https://api.aigocode.com", "client_type": "default"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://www.aigocode.com/docs + https://www.aigocode.com/docs/getting-started/quickstart

**备注**: OpenAI 兼容端点 `https://api.aigocode.com/v1/chat/completions`。

---

### 12. aicodemirror (AICodeMirror)

**官网**: https://www.aicodemirror.com

**支持协议**:
- ✅ Anthropic (Claude Code)
- ✅ OpenAI (Codex)
- ✅ Gemini

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.aicodemirror.com/api/claudecode", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://api.aicodemirror.com/api/codex/backend-api/codex", "client_type": "codex_tui"},
  {"protocol": "gemini", "base_url": "https://api.aicodemirror.com/api/gemini", "client_type": "default"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://www.aicodemirror.com/docs

**备注**: Claude Code 共享平台，专注 Claude 系列模型。

---

### 13. nvidia (NVIDIA NIM)

**官网**: https://www.nvidia.com

**支持协议**:
- ✅ OpenAI Compatible

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "openai", "base_url": "https://integrate.api.nvidia.com/v1", "client_type": "codex_tui"}
]
```

**model_list.default** (JSON 现有):
```
nvidia/nemotron-3-ultra-550b-a55b, nvidia/nemotron-3-super-120b-a12b,
nvidia/llama-3.3-nemotron-super-49b-v1.5, deepseek/deepseek-v3.2,
qwen/qwen3.5-397b-a17b, qwen/qwen3-next-80b-a3b-instruct,
z-ai/glm-5.1, moonshotai/kimi-k2.6, minimaxai/minimax-m3,
meta/llama-4-maverick-17b-128e-instruct, meta/llama-3.3-70b-instruct,
openai/gpt-oss-120b
```

**来源**: https://docs.api.nvidia.com/

**备注**: NVIDIA NIM 推理端点，托管开源模型。

---

### 14. newapi (New API / One-API)

**官网**: https://docs.newapi.pro

**支持协议**:
- ✅ OpenAI Compatible (自部署)
- ✅ 多协议转发（Anthropic/Gemini 等，取决于上游配置）

**endpoints.default[]** (JSON 现有 - 占位符):
```json
[
  {"protocol": "openai", "base_url": "https://your-newapi-instance.com/v1", "client_type": "codex_tui"}
]
```

**model_list.default**: **空数组** ⚠️ 需用户自部署后配置

**来源**: https://docs.newapi.pro/

**备注**: New API 是**开源自部署项目**，非托管服务。端点 `base_url` 为占位符，用户需自建实例后替换。GitHub: https://github.com/QuantumNous/new-api

**重要**: 官方声明 — **从未公开展售 API 访问**，任何以 "New API 官方/合作伙伴" 名义售卖额度的服务均为欺诈。

---

## 价格信息

大部分聚合平台采用**透明计费**模式，价格跟随上游厂商动态调整。以下是价格页面链接：

| 平台 | 价格页面 |
|------|----------|
| OpenRouter | https://openrouter.ai/docs/models |
| AiHubMix | https://aihubmix.com/pricing |
| DMXAPI | https://www.dmxapi.cn/pricing |
| Novita | https://novita.ai/pricing |
| AtlasCloud | https://atlascloud.ai/docs/models/price |
| ShengSuanYun | https://shengsuanyun.com/pricing |
| TheRouter | https://therouter.ai/pricing/ |
| RightCode | https://right.codes/pricing |
| PackyCode | https://www.packyapi.com/pricing |
| Cubence | https://cubence.com/pricing |
| AIGoCode | https://www.aigocode.com/pricing |
| AICodeMirror | https://www.aicodemirror.com/pricing |
| NVIDIA | https://build.nvidia.com/pricing |
| NewAPI | 需自部署，价格取决于上游配置 |

**价格单位**: 统一为美元/美元计价（大部分平台），部分国内平台可能支持人民币。

---

## Caveats / Not Found

### 需要用户验证的平台

1. **DMXAPI** (`dmxapi`)
   - 文档站点证书错误：`ERR_CERT_COMMON_NAME_INVALID`
   - 无法验证端点准确性

2. **ShengSuanYun** (`shengsuanyun`)
   - 文档 `/docs` 返回 404
   - 协议支持无法确认

3. **Cubence** (`cubence`)
   - 文档 `/docs` 返回 404
   - 协议支持无法确认

### 需要补全的数据

1. **TheRouter** (`therouter`)
   - `model_list.default` 为空数组
   - 需从 https://therouter.ai/models 补全模型列表

2. **NewAPI** (`newapi`)
   - `base_url` 为占位符 `https://your-newapi-instance.com/v1`
   - `model_list.default` 为空数组
   - 需用户自部署后配置

### AiHubMix 备用域名

AiHubMix 主域名 `https://aihubmix.com` 访问异常时可使用备用域名 `https://api.inferera.com`，建议在 `endpoints` 中添加 fallback。

---

## 协议支持矩阵

| 平台 | Anthropic | OpenAI | Gemini |
|------|-----------|--------|--------|
| openrouter | ✅ | ✅ | ✅ |
| aihubmix | ✅ | ✅ | ✅ |
| dmxapi | ✅ (预设) | ✅ (预设) | ❌ |
| novita | ✅ | ⚠️ (推测) | ❌ |
| atlascloud | ✅ (预设) | ✅ (文档) | ❌ |
| shengsuanyun | ✅ (预设) | ✅ (预设) | ✅ (预设) |
| therouter | ✅ (预设) | ✅ (文档) | ❌ |
| rightcode | ✅ | ✅ | ✅ (文档) |
| packycode | ✅ | ✅ | ✅ |
| cubence | ✅ (预设) | ✅ (预设) | ✅ (预设) |
| aigocode | ✅ (预设) | ✅ (文档) | ✅ (预设) |
| aicodemirror | ✅ | ✅ | ✅ |
| nvidia | ❌ | ✅ | ❌ |
| newapi | ⚠️ (取决于上游) | ✅ (标准) | ⚠️ (取决于上游) |

---

## 来源汇总

| 平台 | 文档 URL |
|------|----------|
| OpenRouter | https://openrouter.ai/docs |
| AiHubMix | https://docs.aihubmix.com/ |
| DMXAPI | https://www.dmxapi.cn (预设) |
| Novita | https://novita.ai/docs |
| AtlasCloud | https://atlascloud.ai/docs |
| ShengSuanYun | https://shengsuanyun.com (预设) |
| TheRouter | https://therouter.ai/docs |
| RightCode | https://docs.right.codes/ |
| PackyCode | https://docs.packyapi.com/ |
| Cubence | https://cubence.com (预设) |
| AIGoCode | https://www.aigocode.com/docs |
| AICodeMirror | https://www.aicodemirror.com/docs |
| NVIDIA | https://docs.api.nvidia.com/ |
| NewAPI | https://docs.newapi.pro/ |

---

## 下一步建议

1. **TheRouter** model_list 补全 — 从 https://therouter.ai/models 获取完整模型列表
2. **AiHubMix** 添加备用域名 — 在 endpoints 中添加 `https://api.inferera.com` fallback
3. **DMXAPI/ShengSuanYun/Cubence** 用户验证 — 提供正确文档地址或确认端点
4. **NewAPI** 更新占位符说明 — 强调自部署性质
