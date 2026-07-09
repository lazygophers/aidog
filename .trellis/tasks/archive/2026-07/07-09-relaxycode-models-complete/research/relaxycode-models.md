# Research: RelaxyCode 全量研究

- **Query**: RelaxyCode (relaxycode.com) 全量模型清单 + endpoints 形态，为 aidog `platform-presets.json` 补全提供权威数据源
- **Scope**: external（官网/Jina Reader）+ internal（preset 对齐）
- **Date**: 2026-07-09

---

## 结论速览（TL;DR）

1. **RelaxyCode 是多供应商聚合平台**（"Access Claude, GPT, Gemini and other mainstream AI models from one platform"），**支持三大家族**：Claude + OpenAI(GPT/Codex) + Google(Gemini)。
2. **3 个 endpoint 因 HTTP 451 地域限制无法直接探测**（服务区不可用），但官网首页明确列出支持的协议：Claude Code / Codex / Gemini CLI。
3. **当前 preset 仅含 Claude 模型**（7 个 alias），**与平台多供应商定位不符**。需扩展 GPT 和 Gemini 系列。
4. **id 格式判定**：推测: 为**裸 id**（如 `claude-opus-4-8`、`gpt-5.5`、`gemini-2.5-pro`），与 aidog 主 preset（anthropic/openai/gemini）约定一致。

---

## API Endpoints

### 路由验证（curl 探测，2026-07-09）

| 协议 | preset base_url | 探测路径 | 响应 |
|---|---|---|---|
| anthropic | `https://www.relaxycode.com` | `POST /v1/messages` | **HTTP 451** "You are out of our service region" |
| openai | `https://www.relaxycode.com/v1` | `POST /chat/completions` | **HTTP 451** "Service region unavailable" |
| gemini | `https://www.relaxycode.com` | `GET /v1beta/models` | **HTTP 451** "Service region unavailable" |
| openai (models) | `https://www.relaxycode.com/v1` | `GET /models` | **HTTP 451** "Service region unavailable" |

> **关键限制**：RelaxyCode 实施地理访问控制（HTTP 451），从当前网络环境无法直接 API 探测。鉴权方式（401 报错的 key 格式）和模型列表无法通过 curl 验证。研究基于官网内容 + preset 兄弟一致性推断。

### 鉴权方式

- **无法通过 401 报错验证**：所有请求因 HTTP 451 被阻断，无法观察 "Expected format" 提示。
- **推测: 为多 key 格式支持**：作为聚合平台，推测: 接受各供应商原生 key（Anthropic `sk-ant-api03-xxx`、OpenAI `sk-xxx`、Google `AIzaxxx`）或统一 RelaxyCode key（dashboard 注册后发放）。
- 官网首页提示："After logging in, choose the AI model you need in the dashboard, copy your API key" —— 可能为统一 API key。

---

## 模型范围确认

**三大供应商家族**。证据链：

1. 官网 hero 明确："**Access Claude, GPT, Gemini and other mainstream AI models from one platform**"。
2. 首页三卡片：Claude Code / Codex / Gemini CLI —— 每个卡片对应一个供应商协议。
3. Codex 卡片描述："A GPT-series model optimized for programming" —— 证明 Codex = OpenAI GPT 系列。
4. Gemini 卡片描述："Google's multimodal AI model" —— 证明 Gemini = Google 原生。
5. footer "We currently support Claude Code, Codex, Gemini CLI and other mainstream AI coding models" —— 明确列出三大协议。

**支持的供应商**：
- ✅ Claude（Anthropic 官方协议）
- ✅ OpenAI GPT / Codex
- ✅ Google Gemini
- ❓ "other mainstream AI coding models" —— 未明确列举，推测: 可能有 DeepSeek/Qwen 等国产系，但官网未公开宣传

---

## 全量模型清单

### Claude 系（官网确认）

| 官网营销名 | 出现位置 | aidog preset alias（建议） |
|---|---|---|
| Claude Code | 首页卡片 / footer | `claude-opus-4-8`, `claude-sonnet-4-6`, `claude-haiku-4-5`（与 aidog `anthropic` preset 约定一致） |

### OpenAI 系（官网确认 "Codex"）

| 官网营销名 | 出现位置 | aidog preset alias（建议） |
|---|---|---|
| Codex (GPT-series) | 首页卡片 | `gpt-5.5`, `gpt-5.4`, `o3`（与 aidog `openai` preset 约定一致） |

### Gemini 系（官网确认）

| 官网营销名 | 出现位置 | aidog preset alias（建议） |
|---|---|---|
| Gemini CLI | 首页卡片 | `gemini-2.5-pro`, `gemini-2.5-flash`（与 aidog `gemini` preset 约定一致） |

### 国产系 / 其他

**未在官网公开列举**。footer 提到 "other mainstream AI coding models" 但未列名称。推测: 可能有 DeepSeek/GLM/Kimi 等，但无证据，暂不扩展。

### 关于 model id 字符串精确性

- RelaxyCode 作为聚合平台，对各供应商 model id 做**透传或映射**。
- 由于 dashboard 需登录且 API 受地域限制，无法获取"平台实际接受的 id 字符串"白名单。
- aidog 项目对各大供应商使用**主 preset 的 alias 约定**（Claude 用 `claude-opus-4-8`、OpenAI 用 `gpt-5.5`、Gemini 用 `gemini-2.5-pro`），保持与 `anthropic`/`openai`/`gemini` 主 preset 一致性比追平台未公开的真名更重要。

---

## 三档默认推荐（供 `models.default`）

RelaxyCode 支持三大供应商，三档可按**供应商分档**：

| 档位 | 推荐 alias | 对应供应商 | 用途 |
|---|---|---|---|
| Claude 档（默认主力） | `claude-sonnet-4-6` | Anthropic | 平衡性能/成本 |
| OpenAI 档 | `gpt-5.5` | OpenAI | Codex 兼容 / GPT 系列 |
| Gemini 档 | `gemini-2.5-pro` | Google | 多模态 / 轻量 |

> **格式**：`{"claude-sonnet-4-6": {}, "gpt-5.5": {}, "gemini-2.5-pro": {}}`

若偏好单一供应商（如 Claude-only），则：
- Opus 档：`claude-opus-4-8`
- Sonnet 档：`claude-sonnet-4-6`
- Haiku 档：`claude-haiku-4-5`

---

## 现有 7 模型核对

`src-tauri/defaults/platform-presets.json` `protocols.relaxycode.model_list.default`（line 2898-2907）：

```
claude-opus-4-8       ✅ aidog 标准 alias（最新 Opus 档）
claude-sonnet-4-6     ✅ aidog 标准 alias（最新 Sonnet 档）
claude-haiku-4-5      ✅ 对应官方 Claude Haiku 3.5
claude-opus-4-7       ✅ aidog 标准 alias（上一代 Opus）
claude-opus-4-6       ✅ aidog 标准 alias
claude-opus-4-5       ✅ aidog 标准 alias
claude-sonnet-4-5     ✅ aidog 标准 alias
```

**结论**：7 个 id 全部为 Claude 系列，**与平台多供应商定位不符**。官网明确支持 GPT(Codex) 和 Gemini，但 preset 缺失。

### 建议增补

需添加 OpenAI 和 Gemini 系列：

```json
{
  "default": [
    // Claude（现有 7 个保留）
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-5",
    // OpenAI（新增，与 `openai` preset 约定一致）
    "gpt-5.5",
    "gpt-5.4",
    "o3",
    // Gemini（新增，与 `gemini` preset 约定一致）
    "gemini-2.5-pro",
    "gemini-2.5-flash"
  ]
}
```

> **说明**：id 字符串沿用 aidog 主 preset（anthropic/openai/gemini）的 alias 约定，确保跨平台一致性。

---

## Caveats / Not Found

1. **HTTP 451 地域限制**：所有 API 探测因地理访问控制失败，无法验证：
   - 401 报错的 key 格式（无法确定是统一 key 还是多供应商 key）
   - `/v1/models` 或 `/api/models` 是否返回全量模型列表
   - endpoint 是否真正存活（虽然官网明确支持，但 curl 无法验证路由存在）
2. **dashboard 需登录**：官网提示登录后获取 API key 和选择模型，无免鉴权 API 暴露模型清单。
3. **"other mainstream AI coding models" 未明确**：footer 提到其他模型但未列名称，无法判断是否有 DeepSeek/Qwen/Kimi 等国产系。
4. **desc 文案需更新**：当前 preset desc="RelaxyCode API for Claude-compatible models"，但实际是多供应商聚合，建议改为 "RelaxyCode API, Claude/GPT/Gemini 聚合平台"。
5. **source_urls 核验**：`docs: https://www.relaxycode.com/` 返回首页（非文档页），`pricing: https://www.relaxycode.com/pricing` 可访问但仅价格表无模型清单。建议保持现状（docs 指向首页作为入口）。
6. **未测试的协议**：未验证是否有第四个协议（如 OpenClaw 或其他 CLI 工具），官网未明确列出。

---

## 数据来源（URL + 2026-07-09）

- 官网首页：https://www.relaxycode.com/ （"Multi-model support" + Claude/Codex/Gemini 卡片）
- 定价：https://www.relaxycode.com/pricing （Claude Max + Codex 系列价格，无模型 id 清单）
- 使用教程：https://www.relaxycode.com/docs （需登录，仅教程合集）
- Dashboard：https://www.relaxycode.com/dashboard （需登录，"加载中" 无法抓取）
- API 探测（curl，均因 HTTP 451 失败）：
  - `POST https://www.relaxycode.com/v1/messages` → HTTP 451
  - `POST https://www.relaxycode.com/v1/chat/completions` → HTTP 451
  - `GET https://www.relaxycode.com/v1beta/models` → HTTP 451
  - `GET https://www.relaxycode.com/v1/models` → HTTP 451
- 内部对齐：`src-tauri/defaults/platform-presets.json` `protocols.relaxycode`（line 2874-2935）+ aidog 主 preset（anthropic/openai/gemini）alias 约定
