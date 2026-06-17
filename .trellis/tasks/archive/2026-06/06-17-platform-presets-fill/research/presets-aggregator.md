# Research: 聚合平台组 base_url + 默认模型核实

- **Query**: 逐平台核实 12 个聚合/路由平台官方 base_url，校正过时值；默认模型仅官方明确推荐时填，否则 N/A
- **Scope**: mixed（内部读现有预设 + 外部 WebFetch 官方文档）
- **Date**: 2026-06-17
- **现有预设来源**: `src/pages/Platforms.tsx` `getDefaultEndpoints`(:150-365) + `getDefaultModels`(:371-395)

## 约束复述

- base_url 含版本前缀；最终 URL = base_url + provider_api_path（anthropic 协议追加 `/v1/messages`，openai 协议追加 `/chat/completions`），**禁额外拼接**。
- 聚合平台多模型：默认模型除非官方有「推荐默认/旗舰」明确指向，否则标「N/A（多模型，留空）」。

## 核查方法说明

- 多数 SPA 文档（Mintlify / Vitepress / Next.js）裸 HTML 无内容，经 `r.jina.ai` 渲染或抓官方 quickstart 页提取 base_url。
- 部分平台（DMXAPI / CherryIN）配置以截图呈现，文本中无 base_url，标注「未在文本找到，保持原值（域名一致）」。

## Findings

| 平台key | 现有 base_url(协议) | 核实后 base_url | 变更? | 默认模型建议 | 来源URL | 核查日期 |
|---|---|---|---|---|---|---|
| openrouter | anthropic: `https://openrouter.ai/api`<br>openai: `https://openrouter.ai/api/v1`<br>gemini: `https://openrouter.ai/api` | openai 确认 `https://openrouter.ai/api/v1`(→`/chat/completions`)；anthropic 端点为 `/api/v1/messages`，现有 `https://openrouter.ai/api` + `/v1/messages` 路径 = `https://openrouter.ai/api/v1/messages` 一致 | N | N/A（多模型，留空） | https://openrouter.ai/docs/api-reference/overview | 2026-06-17 |
| siliconflow | anthropic: `https://api.siliconflow.cn` | 官方 Claude Code 文档 `ANTHROPIC_BASE_URL=https://api.siliconflow.cn/`，app 追加 `/v1/messages` 一致 | N | N/A（多模型，留空） | https://docs.siliconflow.cn/cn/usercases/use-siliconcloud-in-ClaudeCode | 2026-06-17 |
| siliconflow_en | anthropic: `https://api.siliconflow.com` | 官方 EN 文档 base `https://api.siliconflow.com/`，一致 | N | N/A（多模型，留空） | https://docs.siliconflow.com/en/usercases/use-siliconcloud-in-ClaudeCode | 2026-06-17 |
| aihubmix | anthropic: `https://aihubmix.com`<br>openai: `https://aihubmix.com/v1` | Claude Code 文档示例端点 `https://aihubmix.com/v1/messages` → anthropic base `https://aihubmix.com` 一致；openai `/v1` 一致 | N | N/A（多模型，留空） | https://docs.aihubmix.com/en/api/Claude-Code | 2026-06-17 |
| dmxapi | anthropic: `https://www.dmxapi.cn`<br>openai: `https://www.dmxapi.cn/v1` | 官方域 `www.dmxapi.cn` 确认；Claude Code 配置页以截图呈现，base_url 不在文本中。域名一致，保持原值 | N（未在文本核到精确路径，域名一致） | N/A（多模型，留空） | https://doc.dmxapi.cn/claude-code-new.html | 2026-06-17 |
| modelscope | anthropic: `https://api-inference.modelscope.cn` | API-Inference 文档确认 host `api-inference.modelscope.cn`，OpenAI base `…/v1`；anthropic 现有 host root + `/v1/messages` 一致 | N | N/A（多模型，留空） | https://modelscope.cn/docs/model-service/API-Inference/intro | 2026-06-17 |
| shengsuanyun | anthropic: `https://router.shengsuanyun.com/api` | 官方首页示例 `router.shengsuanyun.com/api/v1/chat/completions` → openai base `…/api/v1`；anthropic 现有 `…/api` + `/v1/messages` = `…/api/v1/messages` 一致 | N | N/A（多模型，留空） | https://www.shengsuanyun.com | 2026-06-17 |
| atlascloud | anthropic: `https://api.atlascloud.ai` | LLM 文档示例 `api.atlascloud.ai/v1/chat/completions` → openai base `https://api.atlascloud.ai/v1`。**注意**：官方文档自述「OpenAI-Compatible」，**未见独立 Anthropic/Claude 端点说明**；现有 anthropic 预设 base `https://api.atlascloud.ai` + `/v1/messages` 是否被上游支持未在官方文档核实到 | N（host 一致；但 anthropic 端点存在性未官方确认） | N/A（多模型，留空） | https://www.atlascloud.ai/docs/models/llm ; https://docs.atlascloud.ai/introduction | 2026-06-17 |
| novita | anthropic: `https://api.novita.ai/anthropic` | Claude Code 文档确认 `ANTHROPIC_BASE_URL=https://api.novita.ai/anthropic`，一致 | N | N/A（多模型，留空） | https://novita.ai/docs/guides/claude-code | 2026-06-17 |
| therouter | anthropic: `https://api.therouter.ai` | 官方 quickstart 示例 `api.therouter.ai/v1/chat/completions` → openai base `https://api.therouter.ai/v1`；anthropic 现有 host root + `/v1/messages` 一致（new-api 风格网关） | N | N/A（多模型，留空） | https://www.therouter.ai/docs/quickstart | 2026-06-17 |
| cherryin | anthropic: `https://open.cherryin.net` | 官网/文档为 SPA，文本中未提取到 base_url；DNS/TLS 可达。域名一致，保持原值 | N（未在文本核到，保持原值） | N/A（多模型，留空） | https://open.cherryin.net （SPA 未渲染出 base） | 2026-06-17 |
| nvidia | openai: `https://integrate.api.nvidia.com/v1` | build.nvidia.com 示例 `integrate.api.nvidia.com/v1/chat/completions`；`/v1/models` 实测 200。base `https://integrate.api.nvidia.com/v1` 确认 | N | N/A（多模型，留空。NVIDIA NIM 聚合 300+ 开源模型，无单一官方旗舰默认） | https://build.nvidia.com/ ; https://docs.api.nvidia.com/nim/reference/llm-apis | 2026-06-17 |

## 结论汇总

- **base_url 变更建议：0 个**。12 平台现有 base_url 全部与官方一致（含版本前缀语义正确，符合「base + provider_api_path 追加、禁额外拼接」）。
- **官方明确默认/旗舰模型：0 个**。全部为聚合/路由平台，多模型并存，官方均未指向单一默认模型 → 统一标 N/A（多模型，留空），符合任务约束（不强填）。

## Caveats / 待复核

1. **atlascloud anthropic 端点**：官方文档仅文本确认 OpenAI-compatible（`/v1/chat/completions`），**未找到** Claude/Anthropic 端点（`/v1/messages`）的官方说明。现有 anthropic 预设的端点存在性需另行向 AtlasCloud 文档/支持确认；base host 本身无误。
2. **dmxapi / cherryin**：base_url 未能从文档文本提取（DMXAPI 用截图、CherryIN 为未渲染 SPA），仅依据域名一致判定「保持原值」。如需 100% 确认精确路径前缀，建议人工登录控制台查「接口地址」。
3. **siliconflow base 末尾斜杠**：官方文档写作 `https://api.siliconflow.cn/`（带尾斜杠），现有预设无尾斜杠 `https://api.siliconflow.cn`。app 追加 `/v1/messages` 后结果一致，无需改（避免双斜杠反而正确）。
4. 所有「openai base 含 `/v1`、anthropic base 为 host root」的差异是协议路径语义差，非过时，无需统一。
