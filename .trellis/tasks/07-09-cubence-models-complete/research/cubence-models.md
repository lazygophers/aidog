# Cubence 全量研究

- **Query**: Cubence 平台全量模型清单 + endpoints 形态
- **Scope**: external（官方文档 docs.cubence.com）
- **Date**: 2026-07-09

## 数据来源（URL + 访问日期 2026-07-09）

| 页面 | URL | 状态 |
|---|---|---|
| Endpoints 指南 | https://docs.cubence.com/en/docs/guides/endpoints | 200 |
| Pricing 指南 | https://docs.cubence.com/en/docs/guides/pricing | 200 |
| Quick Start | https://docs.cubence.com/en/docs/quick-start | 200 |
| Claude Code Setup | https://docs.cubence.com/en/docs/setup/claude-code | 200 |
| Codex Setup | https://docs.cubence.com/en/docs/setup/codex | 200 |
| Gemini CLI Setup | https://docs.cubence.com/en/docs/setup/gemini-cli | 200 |
| gpt-image-2 (OpenAI) | https://docs.cubence.com/en/docs/image-models/gpt-image-2 | 200 |
| OpenClaw 集成 | https://docs.cubence.com/en/docs/advanced/openclaw | 200 |
| Introduction (zh) | https://docs.cubence.com/zh-CN/docs | 200 |

> 注：cubence.com 主站为纯 Next.js 客户端渲染，HTML 内不含模型清单；官方未提供 sitemap；docs 站点无独立「全部模型」页。下方模型清单来自各 setup / FAQ / 集成页的官方配置示例与明文提及。

---

## API Endpoints

官方 Endpoints 页明列 4 个等价 base_url（同后端、不同线路）：

| 线路 | BASE_URL | 说明 |
|---|---|---|
| CDN Optimized | `https://api.cubence.com` | **官方默认推荐** |
| DMIT Optimized | `https://api-dmit.cubence.com` | setup / image 文档示例用 |
| BandwagonHost | `https://api-bwg.cubence.com` | 备用 |
| CF CDN | `https://api-cf.cubence.com` | 备用 |

**四者完全等价**，仅链路不同；官方建议默认用 `api.cubence.com`，不稳定时切换。

三协议路径形态（来自官方 setup 页）：

| 协议 | BASE_URL | 路径形态 | wire_api | 鉴权 | 默认模型（官方配置示例） |
|---|---|---|---|---|---|
| **anthropic**（Claude Code） | `https://api.cubence.com`（无 `/v1`） | Anthropic Messages | `anthropic-messages` | `Authorization: Bearer <key>` 或 `x-api-key`（Claude Code 用 `ANTHROPIC_AUTH_TOKEN` + `ANTHROPIC_BASE_URL`） | claude 系，默认 1M 上下文 |
| **openai**（Codex） | `https://api.cubence.com/v1`（**必带 `/v1`**） | OpenAI Responses | `openai-responses`（Codex） / 标准 chat completions | `Authorization: Bearer <key>` | **gpt-5.5**（config.toml 默认） |
| **gemini**（Gemini CLI） | `https://api.cubence.com`（无 `/v1`） | Gemini API | gemini 原生 | `GEMINI_API_KEY` + `GOOGLE_GEMINI_BASE_URL` | **gemini-3-pro-preview** |
| **image**（OpenAI 图像） | `https://api-dmit.cubence.com/v1` | `/v1/images/generations`、`/v1/images/edits` | OpenAI images | `Authorization: Bearer <key>` | **gpt-image-2** |

**结论：当前 preset 3 endpoint 完全正确**（anthropic→无 /v1、openai→带 /v1、gemini→无 /v1）。base_url 用 `api.cubence.com` 与官方默认一致。

OpenClaw 文档原文佐证：
> "Two base URLs are needed later: Claude: https://api-dmit.cubence.com ; Codex: https://api-dmit.cubence.com/v1 (must include /v1)"
> "Codex baseUrl must end with /v1 ; Claude does not."

---

## 模型范围确认

**官方定位：Claude + GPT + Gemini 三大 AI 一站式代理**（非单纯 Claude 兼容）。

- Introduction 原文：「One-stop support for three major AIs: Claude, GPT, Gemini」
- Quick Start：「Cubence supports multiple mainstream AI CLI tools. Claude Code (Anthropic's official) / Codex (OpenAI's official) / Gemini CLI (Google's official)」
- Codex Setup：「Through the Cubence platform, you can use powerful models like GPT-5」
- Gemini CLI Setup：「Through the Cubence platform, you can use Gemini series models」

**不提供** DeepSeek / Qwen / GLM / Kimi / MiniMax / Grok 等国产或第三方模型 —— 文档全站无任何提及。Cubence 是「Anthropic + OpenAI + Google」三巨头的镜像代理，定位与现有 desc「Claude 兼容」相比**范围更广**（含 OpenAI、Google）。

**共享分组（share group）机制**：API Key 必须在控制台为「Anthropic / OpenAI / Gemini」每个服务类型分别指派 share group（如 `gpt-image-2(0.1 per call)`）；未指派的请求 400 错误。share group 分普通组与 Max 组（OpenClaw 文档：「Only Cubence non-Max share groups are supported」—— Max 组为含最新/高端模型的高档分组）。Max 是访问档位，不是 model id。

---

## 全量模型清单

> 官方文档**无独立「Models」页**，下列 id 均从官方配置示例 / 明文提及抽取。标 ✅ = 文档中明确出现；标 ⚠️ = 文档未明示但属现有 preset 列出、推测可用（见核对章节）。

### Claude 系（Anthropic，anthropic 协议）

| model id | 上下文 | 来源 | 状态 |
|---|---|---|---|
| `claude-opus-4-7` | 200000（OpenClaw 配置示例 contextWindow=200000, maxTokens=32000） | OpenClaw 文档明文 | ✅ |
| `claude-opus-4-8` | — | 现有 preset | ⚠️ 文档未明示 |
| `claude-sonnet-4-5` | — | 现有 preset | ⚠️ 文档未明示 |
| `claude-sonnet-4-6` | — | 现有 preset | ⚠️ 文档未明示 |
| `claude-haiku-4-5` | — | 现有 preset | ⚠️ 文档未明示 |
| `claude-opus-4-6` | — | 现有 preset | ⚠️ 文档未明示 |
| `claude-opus-4-5` | — | 现有 preset | ⚠️ 文档未明示 |

文档原文仅明确出现 `claude-opus-4-7` 一个 id（OpenClaw 集成示例）。Claude Code setup 页未指定具体 id，仅说「Claude defaults to a 1M-context model, which burns tokens fast ... add CLAUDE_CODE_AUTO_COMPACT_WINDOW: 200000」—— 暗示默认档接入 Claude 1M 长上下文模型，但未列具体 id 清单。

**推测:** Cubence 作为 Claude Code 官方代理，应跟随 Anthropic 全系 claude-4.x 版本，现有 7 个 id 可能均可用，但官方文档未逐一列出。需要: 用户在控制台 API Key 管理页查看「Anthropic share group」下拉框实际可选项，才能 100% 确认完整 claude 模型清单。

### OpenAI 系（openai 协议）

| model id | 用途 | 来源 | 状态 |
|---|---|---|---|
| `gpt-5.5` | Codex 默认（config.toml `model = "gpt-5.5"`, reasoning_effort=high） | Codex Setup + OpenClaw 明文 | ✅ |
| `gpt-5` | 文本（「powerful models like GPT-5」） | Codex Setup 描述 | ✅（文档泛指，具体子版本未列） |
| `gpt-image-2` | 图像生成（text-to-image + image-to-image） | gpt-image-2 专页 | ✅ |

OpenClaw 配置示例：gpt-5.5 的 cost `{input:0, output:0, cacheRead:0, cacheWrite:0}`、contextWindow=200000、maxTokens=32000 —— 注：cost=0 是示例占位，非真实计价。

**推测:** Codex setup 用 `wire_api = "responses"`，意味着 Cubence 接入的是 OpenAI 新 Responses API（gpt-5.x 系列），传统 chat completions 模型（gpt-4o 等）是否可用文档未提。

### Google 系（gemini 协议）

| model id | 来源 | 状态 |
|---|---|---|
| `gemini-3-pro-preview` | Gemini CLI Setup 配置示例 `GEMINI_MODEL=gemini-3-pro-preview` | ✅ |

Gemini FAQ 列出问题「Which Gemini models are supported?」「What to do if model doesn't exist error appears?」—— 暗示支持多款 gemini 模型，但 FAQ 页（与所有 FAQ 页一样）只列问题标题、未展开答案正文。**推测:** 至少 gemini-3-pro-preview 可用，其他版本（flash 等）需控制台查询。

### 国产 / 其他系

**无。** 全站文档无 deepseek / qwen / glm / kimi / minimax / grok / o1 / o3 等任何提及。Cubence 定位为三大海外 AI 代理，不含国产模型。

---

## 三档默认推荐（供 models.default）

基于官方文档配置示例的默认取向：

```json
{
  "default": {
    "anthropic": "claude-sonnet-4-6",
    "openai": "gpt-5.5",
    "gemini": "gemini-3-pro-preview"
  }
}
```

理由：
- **anthropic → claude-sonnet-4-6**：官方文档未指明默认 claude id；按 aidog preset 现有 sonnet 档（sonnet-4-6 高于 sonnet-4-5）作为 sonnet 默认。**推测:** 实际默认可能是 claude-sonnet-4-5 或更新版本，需控制台核实。
- **openai → gpt-5.5**：Codex setup 官方 config.toml 直接默认 `model = "gpt-5.5"`，权威明确。
- **gemini → gemini-3-pro-preview**：Gemini CLI setup 官方 .env 直接默认 `GEMINI_MODEL=gemini-3-pro-preview`，权威明确。

---

## 现有 7 模型核对

| model id | 官方文档 | 结论 |
|---|---|---|
| `claude-opus-4-8` | 未明示 | ⚠️ 无法从文档确认；推测: 可能是新版本，待控制台核实 |
| `claude-sonnet-4-6` | 未明示 | ⚠️ 同上 |
| `claude-haiku-4-5` | 未明示 | ⚠️ 同上 |
| `claude-opus-4-7` | **OpenClaw 明文出现**（contextWindow=200000） | ✅ 确认存在 |
| `claude-opus-4-6` | 未明示 | ⚠️ 无法从文档确认 |
| `claude-opus-4-5` | 未明示 | ⚠️ 无法从文档确认 |
| `claude-sonnet-4-5` | 未明示 | ⚠️ 无法从文档确认 |

**7 个里仅 `claude-opus-4-7` 一个被官方文档直接证实**。其余 6 个文档未提，但考虑到 Cubence 跟随 Anthropic 官方版本节奏、且现有 preset 来源可信，**推测**它们均真实存在，只是文档未罗列。

---

## 建议补全 model_list（仅供 main 参考）

基于「Claude + GPT + Gemini 三协议齐全」的官方定位，建议 model_list.default 至少补：

```
# Claude（保留现有 7 个）
claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5

# OpenAI（官方明确）
gpt-5.5, gpt-5

# Google（官方明确）
gemini-3-pro-preview

# 图像（可选；走 openai 协议 /v1/images/generations）
gpt-image-2
```

注意 id 格式：Cubence 全用**无 `provider/` 前缀的裸 id**（如 `gpt-5.5` 非 `openai/gpt-5.5`），与现有 7 个 claude id 风格一致。

---

## Caveats / Not Found

1. **官方无独立 Models 页** —— 文档以 setup/集成教程形式分散提及，无法获得「全量」清单的权威来源。本研究的 model id 来自 setup 默认值 + OpenClaw 示例 + 现有 preset，**可能不完整**。
2. **FAQ 页只列问题标题**，正文需展开（JS 渲染），curl 抓不到答案；FAQ 提到的「Which models are supported?」「Which Gemini models are supported?」答案未获取。需要: 用户用浏览器打开 FAQ 页查看，或控制台 share group 下拉框核实。
3. **desc 建议更新**：现有 desc「Claude 兼容模型」低估了范围；应改为「Claude / GPT / Gemini 三协议代理」之类。
4. **gpt-image-2 计费特殊**：0.1/call，需在控制台为 OpenAI 服务类型单独指派 `gpt-image-2(0.1 per call)` share group，否则 400。
5. **Max share group**：高档分组存在，OpenClaw 不支持；具体内容未文档化。
6. **订阅套餐已停售**（Cube ¥238 / Prism ¥468 / Tesseract ¥738），仅 pay-as-you-go（¥1.0 = $1，永不过期）+ 推荐返现 10%。
7. **base_url 备用线路** 4 个均等价，preset 用 `api.cubence.com`（官方推荐）正确；如需做线路切换可在 endpoint 层加 `api-dmit` 等但非必要。

---

## Cross-reference

- 现有 preset: `src-tauri/defaults/platform-presets.json:2123` (`cubence` 条目)
- 项目 CLAUDE.md「platform-presets.json」段
