# Research: 中转协议端点补全（Batch 4 - M-Z 中转）

- **Query**: 查字母 M-Z 范围内未被 batch1/2 覆盖的中转平台官网，补全 preset 数据
- **Scope**: 外部搜索 + 现有 JSON 验证
- **Date**: 2026-07-08

---

## 研究目标

补全 M-Z 字母范围内、未被 batch1（原厂）和 batch2（聚合平台）覆盖的中转协议的端点/模型/价格信息。

---

## 摘要

| 协议 | 官网状态 | 支持协议 | 端点完整性 | 模型列表 | 备注 |
|------|----------|----------|------------|----------|------|
| micu | ✅ | anthropic | ✅ 完整 | ✅ 预设存在 | 米醋工作室 |
| pateway | ✅ | anthropic/openai | ✅ 完整 | ✅ 预设存在 | 透明定价 |
| pipellm | ✅ | openai/anthropic/gemini | ⚠️ 企业级 | ⚠️ 空 | 企业控制平面 |
| relaxycode | ✅ | anthropic/openai/gemini | ✅ 完整 | ✅ 预设存在 | 企业共享池 |
| runapi | ✅ | anthropic | ✅ 完整 | ✅ 预设存在 | 高质量中转 |
| siliconflow_en | ✅ | anthropic | ✅ 完整 | ⚠️ 空 | 硅基流动国际版 |
| sssaicode | ❌ 404 | anthropic | ✅ 预设存在 | ✅ 预设存在 | 服务不可访问 |
| stepfun_en | ✅ | anthropic | ✅ 完整 | ✅ 预设存在 | 阶跃星辰国际版 |
| sudocode | ✅ | anthropic | ✅ 完整 | ✅ 预设存在 | 降智检测 |
| lemondata | ✅ | openai/anthropic/gemini | ⚠️ 需确认 | ⚠️ 需确认 | TokenLab 聚合 |
| ccsub | ✅ | anthropic/openai | ✅ 完整 | ✅ 预设存在 | Claude 平替 |
| apikeyfun | ✅ | openai/anthropic/gemini | ✅ 完整 | ✅ 预设存在 | 通用网关 |
| apinebula | ✅ | anthropic/openai/gemini | ✅ 完整 | ✅ 预设存在 | 中转平台 |
| claudeapi | ✅ | anthropic | ✅ 完整 | ✅ 预设存在 | 第三方 Claude |
| claudecn | ✅ | openai/anthropic/gemini | ✅ 完整 | ✅ 预设存在 | 国内加速 |
| compshare | ✅ | openai/anthropic/gemini | ✅ 完整 | ⚠️ 空 | 优云智算 |
| compshare_coding | ❌ 404 | anthropic | ✅ 预设存在 | ✅ 预设存在 | 编程套餐不可用 |
| crazyrouter | ✅ | anthropic/openai/gemini | ✅ 完整 | ✅ 预设存在 | 300+ 模型 |
| opencode | ✅ | N/A | N/A | N/A | 非中转（开源工具） |
| opencode_zen | ✅ | N/A | N/A | N/A | 非中转（精选服务） |
| cherryin | ❌ DNS 失败 | anthropic | ✅ 预设存在 | ✅ 预设存在 | **需要: 用户** |
| ctok | ❌ DNS 失败 | anthropic | ✅ 预设存在 | ✅ 预设存在 | **需要: 用户** |

---

## 详细发现

### M-Z 中转平台（本批主要目标）

---

### 1. micu (米醋工作室)

**官网**: https://www.micuapi.ai / https://docs.micuapi.ai/

**支持协议**:
- ✅ Anthropic Messages API (Claude Code)
- ✅ OpenAI Compatible (Codex CLI)

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://www.micuapi.ai", "client_type": "claude_code"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://docs.micuapi.ai/

**备注**: 米醋工作室出品，专注 Claude Code / Codex CLI / OpenClaw 配置。提供 CC Switch 统一配置工具。

---

### 2. pateway (PatewayAI)

**官网**: https://pateway.ai

**支持协议**:
- ✅ Anthropic Messages API
- ✅ OpenAI Compatible (Base URL: `https://api.pateway.ai/v1`)

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.pateway.ai", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 兼容端点
```json
{"protocol": "openai", "base_url": "https://api.pateway.ai/v1", "client_type": "codex_tui"}
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**价格示例** (来源官网):
- Claude Sonnet 5: $0.3 / $1.5 per 1M tokens
- GPT 5.5: $0.25 / $1.5 per 1M tokens
- Claude Opus 4.8: $0.75 / $3.75 per 1M tokens
- GLM 5.2: $1.177 / $4.118 per 1M tokens
- DeepSeek V4 Pro: $0.441 / $0.882 per 1M tokens

**来源**: https://pateway.ai/#/models

**备注**: 官方品质，透明定价。注册送 $3 额度，支持支付宝跨境支付。

---

### 3. pipellm (PipeLLM)

**官网**: https://pipellm.com / https://docs.pipellm.ai/

**支持协议**:
- ✅ OpenAI Compatible (`/openai/*`)
- ✅ Anthropic Compatible (`/anthropic/*`)
- ✅ Gemini Compatible (`/gemini/*`)

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.pipellm.ai", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 和 Gemini 兼容端点
```json
{"protocol": "openai", "base_url": "https://api.pipellm.ai/openai", "client_type": "codex_tui"},
{"protocol": "gemini", "base_url": "https://api.pipellm.ai/gemini", "client_type": "default"}
```

**model_list.default**: **空数组** ⚠️ 企业级平台，模型列表取决于客户配置

**来源**: https://pipellm.com/

**备注**: PipeLLM 是企业 AI 控制平面，提供策略路由、运行时控制、托管工具和审计审查，面向生产级 AI 系统。

---

### 4. relaxycode (RelaxyCode)

**官网**: https://www.relaxycode.com/

**支持协议**:
- ✅ Anthropic (Claude Code)
- ✅ OpenAI (Codex)
- ✅ Gemini CLI

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://www.relaxycode.com", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 和 Gemini 兼容端点
```json
{"protocol": "openai", "base_url": "https://www.relaxycode.com/v1", "client_type": "codex_tui"},
{"protocol": "gemini", "base_url": "https://www.relaxycode.com", "client_type": "default"}
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://www.relaxycode.com/

**备注**: 企业级 AI 编码平台，支持多用户共享 API 配额，智能负载均衡，成本降低 60%。

---

### 5. runapi (RunAPI)

**官网**: https://runapi.co / https://runapi.co/docs

**支持协议**:
- ✅ Anthropic Messages API

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://runapi.co", "client_type": "claude_code"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://runapi.co/docs

**备注**: 高质量 API 中转平台。

---

### 6. siliconflow_en (SiliconFlow 国际版)

**官网**: https://siliconflow.com / https://docs.siliconflow.com/

**支持协议**:
- ✅ Anthropic Messages API

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.siliconflow.com", "client_type": "claude_code"}
]
```

**model_list.default**: **空数组** ⚠️ 需补全（与国内版不同）

**来源**: https://docs.siliconflow.com/

**备注**: SiliconFlow 国际版（`.com`），托管 DeepSeek-R1、DeepSeek-V3、Qwen3-Coder、GLM-4.6V、Kimi-K2 等开源模型。与国内版（`.cn`）模型列表不同。

---

### 7. sssaicode (SSSAiCode)

**官网**: https://sssaicode.com / https://node-hk.sssaicodeapi.com/

**状态**: ❌ **404 - 服务不可访问**

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://node-hk.sssaicodeapi.com/api", "client_type": "claude_code"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**备注**: **需要: 用户** — 节点返回 404，服务可能已下线或迁移。

---

### 8. stepfun_en (StepFun 国际版)

**官网**: https://stepfun.ai / https://api.stepfun.ai/

**支持协议**:
- ✅ Anthropic Messages API (推测，与国内版一致)

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.stepfun.ai/step_plan", "client_type": "claude_code"}
]
```

**model_list.default**: **空数组** ⚠️ 需补全（与国内版 `step-3.7-flash` 等可能不同）

**来源**: https://stepfun.ai/

**备注**: StepFun 国际版（`.ai`），与国内版（`.com`）是不同端点。**原厂平台**，不属于中转。

---

### 9. sudocode (SudoCode)

**官网**: https://sudocode.us/

**支持协议**:
- ✅ Anthropic Messages API (推测)

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://sudocode.us", "client_type": "claude_code"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://sudocode.us/

**备注**: 中转平台，特色是"渠道验真不降智"——通过保密且定期轮换的题库，对渠道方模型效果进行周期性检测，持续识别质量异常、能力衰减。

---

### 10. lemondata (LemonData / TokenLab)

**官网**: https://lemondata.cc/ / https://docs.lemondata.cc/ / https://docs.tokenlab.sh/

**支持协议**:
- ✅ OpenAI Compatible
- ✅ Anthropic Native
- ✅ Gemini Native

**endpoints.default[]** (JSON 现有 - 仅 anthropic):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.lemondata.cc", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 和 Gemini 兼容端点（base_url 需确认）
```json
{"protocol": "openai", "base_url": "https://api.lemondata.cc/v1", "client_type": "codex_tui"},
{"protocol": "gemini", "base_url": "https://api.lemondata.cc", "client_type": "default"}
```

**model_list.default**: **空数组** ⚠️ 需补全

**来源**: https://docs.lemondata.cc/ + https://lemondata.cc/zh/models

**备注**: LemonData 品牌已更名为 **TokenLab**。聚合平台，支持数百个 AI 模型，包括 GPT、Claude、Gemini、图片、视频模型。官网显示为 TokenLab (lemondata.cc)。

---

## 补充 A-L 未覆盖协议

---

### 11. ccsub (CCSub)

**官网**: https://www.ccsub.net/

**支持协议**:
- ✅ Anthropic Messages API
- ✅ OpenAI Compatible

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://www.ccsub.net", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 兼容端点
```json
{"protocol": "openai", "base_url": "https://www.ccsub.net/v1", "client_type": "codex_tui"}
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**价格**: 1 RMB = 1 USD，官方 1.4 折

**来源**: https://www.ccsub.net/

**备注**: CC = Claude Code · Sub = Substitute。专为中国用户打造，免梯子、免注册海外账号。内置一键 Key 测试功能。

---

### 12. apikeyfun (APIKEY.FUN)

**官网**: https://apikey.fun/

**支持协议**:
- ✅ Anthropic Messages API
- ✅ OpenAI Compatible
- ✅ Gemini Native

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.apikey.fun", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 和 Gemini 兼容端点（base_url 需确认）
```json
{"protocol": "openai", "base_url": "https://api.apikey.fun/v1", "client_type": "codex_tui"},
{"protocol": "gemini", "base_url": "https://api.apikey.fun", "client_type": "default"}
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**价格**: 1 RMB = 1 USD，按量付费，永不过期

**来源**: https://apikey.fun/

**备注**: 通用 AI 网关，支持 Claude Code、Codex、Gemini CLI。

---

### 13. apinebula (APINebula)

**官网**: https://apinebula.com/ / https://docs.apinebula.com/

**支持协议**:
- ✅ Anthropic Messages API
- ✅ OpenAI Compatible
- ✅ Gemini Native

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://apinebula.com", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 和 Gemini 兼容端点（base_url 需确认）
```json
{"protocol": "openai", "base_url": "https://apinebula.com/v1", "client_type": "codex_tui"},
{"protocol": "gemini", "base_url": "https://apinebula.com", "client_type": "default"}
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://docs.apinebula.com/

**备注**: 中转平台，提供 Claude Code、Codex、Gemini CLI 配置教程。

---

### 14. claudeapi (ClaudeAPI)

**官网**: https://claudeapi.com / https://docs.claudeapi.com/

**支持协议**:
- ✅ Anthropic Messages API

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://gw.claudeapi.com", "client_type": "claude_code"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**价格**: 比官方便宜 20%

**来源**: https://docs.claudeapi.com/

**备注**: 第三方 Claude API，兼容官方 API 和 AWS Bedrock。99.8% 可用性，<200ms 平均延迟。

---

### 15. claudecn (ClaudeCN)

**官网**: https://claudecn.top / https://claudecn.ai / https://claudecn.top/document

**支持协议**:
- ✅ OpenAI Compatible
- ✅ Anthropic Compatible

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://claudecn.top", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 兼容端点
```json
{"protocol": "openai", "base_url": "https://claudecn.ai/v1", "client_type": "codex_tui"}
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**来源**: https://claudecn.top/document

**备注**: Claude API 中转站，国内 AI 加速平台。一个 endpoint，100+ 主流大模型。稳定运行 570+ 天。

---

### 16. compshare (Compshare / UCloud 优云)

**官网**: https://www.compshare.cn/ / https://www.compshare.cn/docs/modelverse/models/quick-start

**支持协议**:
- ✅ OpenAI Compatible
- ✅ Anthropic Compatible
- ✅ Gemini Compatible

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://api.modelverse.cn", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 和 Gemini 兼容端点
```json
{"protocol": "openai", "base_url": "https://api.modelverse.cn/v1", "client_type": "codex_tui"},
{"protocol": "gemini", "base_url": "https://api.modelverse.cn", "client_type": "default"}
```

**model_list.default**: **空数组** ⚠️ 需补全

**来源**: https://www.compshare.cn/docs/modelverse/models/quick-start

**备注**: UCloud 优云智算，按量 API 调用。支持 DeepSeek-R1、GPT-5、Qwen3-Coder 等模型。备用域名 `https://api.umodelverse.ai`。

---

### 17. compshare_coding (Compshare Coding Plan)

**官网**: https://cp.compshare.cn/

**状态**: ❌ **404 - 编程套餐端点不可用**

**endpoints.default[]** (JSON 现有):
```json
[
  {"protocol": "anthropic", "base_url": "https://cp.compshare.cn", "client_type": "claude_code"}
]
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**备注**: **需要: 用户** — `cp.compshare.cn` 返回 404，编程套餐端点可能已下线或迁移。

---

### 18. crazyrouter (CrazyRouter)

**官网**: https://www.crazyrouter.com/ / https://cn.crazyrouter.com/

**支持协议**:
- ✅ Anthropic Messages API
- ✅ OpenAI Compatible
- ✅ Gemini Native

**endpoints.default[]** (与 JSON 一致):
```json
[
  {"protocol": "anthropic", "base_url": "https://cn.crazyrouter.com", "client_type": "claude_code"}
]
```

**需要新增**: OpenAI 和 Gemini 兼容端点（base_url 需确认）
```json
{"protocol": "openai", "base_url": "https://cn.crazyrouter.com/v1", "client_type": "codex_tui"},
{"protocol": "gemini", "base_url": "https://cn.crazyrouter.com", "client_type": "default"}
```

**model_list.default** (JSON 现有):
```
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

**价格示例** (来源官网):
- qwen3-vl-flash: $0.0350 / $0.2800 per 1M tokens (-30%)
- gemini-2.5-flash-lite: $0.0550 / $0.2200 per 1M tokens (-45%)
- deepseek-v4-flash: $0.1260 / $0.2520 per 1M tokens (-10%)

**来源**: https://www.crazyrouter.com/

**备注**: 一个 key 调用 300+ 模型。浏览模型价格并直接跳转到定价视图。

---

## 非中转服务（需从 preset 移除或重新分类）

---

### 19. opencode (OpenCode)

**官网**: https://opencode.ai / https://opencode.ai/docs/

**性质**: **开源 AI 编码 Agent**，非中转平台

**描述**: OpenCode 是开源 AI 编码代理，提供终端界面、桌面应用和 IDE 扩展。支持 GitHub Copilot、ChatGPT Plus/Pro 登录，或通过 Models.dev 接入 75+ LLM 提供商。

**备注**: **不应作为 relay 协议使用**。这是用户端工具，不是 API 中转服务。`opencode_zen` 是 OpenCode 团队测试和验证的精选模型列表服务，同样不是中转平台。

---

## 需要用户确认的平台

| 协议 | 问题 |
|------|------|
| sssaicode | 节点返回 404，服务可能已下线 |
| compshare_coding | `cp.compshare.cn` 返回 404 |
| cherryin | DNS 解析失败 |
| ctok | DNS 解析失败 |
| lemondata | 品牌已更名为 TokenLab，需确认端点 |

---

## 价格信息汇总

| 平台 | 价格页面 | 备注 |
|------|----------|------|
| PatewayAI | https://pateway.ai/#/models | 透明定价，官方 1.4 折 |
| CCSub | https://www.ccsub.net/pricing | 1 RMB = 1 USD |
| APIKEY.FUN | https://apikey.fun/purchase | 1 RMB = 1 USD |
| CrazyRouter | https://www.crazyrouter.com/ | 300+ 模型，直接浏览价格 |
| ClaudeAPI | https://docs.claudeapi.com/#pricing | 比官方便宜 20% |
| SudoCode | https://sudocode.us/ | 不降智检测 |
| PipeLLM | https://docs.pipellm.ai/ | 企业级，价格取决于配置 |
| TokenLab (LemonData) | https://lemondata.cc/zh/models | 数百个模型，便宜 30-70% |

---

## 协议支持矩阵

| 平台 | Anthropic | OpenAI | Gemini |
|------|-----------|--------|--------|
| micu | ✅ | ⚠️ (推测) | ❌ |
| pateway | ✅ | ✅ | ❌ |
| pipellm | ✅ | ✅ | ✅ |
| relaxycode | ✅ | ✅ | ✅ |
| runapi | ✅ | ⚠️ (推测) | ❌ |
| siliconflow_en | ✅ | ❌ | ❌ |
| sudocode | ✅ | ⚠️ (推测) | ❌ |
| lemondata | ✅ | ✅ | ✅ |
| ccsub | ✅ | ✅ | ❌ |
| apikeyfun | ✅ | ✅ | ✅ |
| apinebula | ✅ | ✅ | ✅ |
| claudeapi | ✅ | ❌ | ❌ |
| claudecn | ✅ | ✅ | ❌ |
| compshare | ✅ | ✅ | ✅ |
| crazyrouter | ✅ | ✅ | ✅ |

---

## 来源汇总

| 平台 | 文档 URL |
|------|----------|
| micu | https://docs.micuapi.ai/ |
| pateway | https://pateway.ai/ |
| pipellm | https://docs.pipellm.ai/ |
| relaxycode | https://www.relaxycode.com/ |
| runapi | https://runapi.co/docs |
| siliconflow_en | https://docs.siliconflow.com/ |
| stepfun_en | https://stepfun.ai/ |
| sudocode | https://sudocode.us/ |
| lemondata | https://docs.lemondata.cc/ |
| ccsub | https://www.ccsub.net/ |
| apikeyfun | https://apikey.fun/ |
| apinebula | https://docs.apinebula.com/ |
| claudeapi | https://docs.claudeapi.com/ |
| claudecn | https://claudecn.top/document |
| compshare | https://www.compshare.cn/docs/modelverse/models/quick-start |
| crazyrouter | https://www.crazyrouter.com/ |
| opencode | https://opencode.ai/docs/ |

---

## 下一步建议

1. **补全 OpenAI 兼容端点** — pateway, ccsub, apikeyfun, apinebula, claudecn, compshare, crazyrouter 等平台需要新增 `openai` 协议端点
2. **补全 Gemini 兼容端点** — pipellm, relaxycode, lemondata, apikeyfun, apinebula, compshare, crazyrouter 等平台需要新增 `gemini` 协议端点
3. **LemonData 更名为 TokenLab** — preset 中的 `homepage` 和 `desc` 需要更新
4. **企业级平台 model_list** — pipellm 等企业级平台的 `model_list` 依赖客户配置，保持空数组
5. **服务不可用确认** — sssaicode、compshare_coding、cherryin、ctok 需要用户确认服务状态
6. **移除非中转协议** — opencode 和 opencode_zen 不是中转平台，需要从 preset 移除或重新分类

---

## Caveats / Not Found

1. **sssaicode** — 节点返回 404，服务可能已下线或迁移
2. **compshare_coding** — `cp.compshare.cn` 返回 404，编程套餐端点可能已下线
3. **cherryin / ctok** — DNS 解析失败，无法访问官网
4. **lemondata** — 品牌已更名为 TokenLab，需确认端点是否变更
5. **opencode / opencode_zen** — 非中转平台，不应作为 relay 协议使用
