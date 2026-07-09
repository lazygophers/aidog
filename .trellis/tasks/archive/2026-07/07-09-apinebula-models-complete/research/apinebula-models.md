# Research: APINebula 全量模型清单

- **Query**: APINebula 全量模型清单 + endpoints 形态 + 鉴权方式
- **Scope**: external（外部官方文档调研）
- **Date**: 2026-07-09

## 数据来源表

| URL | 访问日期 | 状态码 | 用途 |
|---|---|---|---|
| https://apinebula.com | 2026-07-09 | 200 | 平台定位（元描述） |
| https://docs.apinebula.com | 2026-07-09 | 200 | 文档入口 |
| https://apinebula.com/pricing | 2026-07-09 | 200 | **核心来源：全量模型清单** |
| https://docs.apinebula.com/docs/quickstart | 2026-07-09 | 200 | endpoint/base_url |
| https://docs.apinebula.com/docs/cli/claude-code | 2026-07-09 | 200 | Anthropic 协议 endpoint |
| https://docs.apinebula.com/docs/cli/codex | 2026-07-09 | 200 | OpenAI 协议 endpoint |
| https://docs.apinebula.com/docs/cli/gemini | 2026-07-09 | 200 | Gemini 协议 endpoint |
| https://apinebula.com/v1/models | 2026-07-09 | 404 | 公开模型端点不存在 |

## 平台定位

**APINebula 是多供应商聚合平台**（非纯 Claude 代理）。

Homepage 元描述：
> "A unified AI model hub for aggregation & distribution. It supports cross-converting various LLMs into OpenAI-compatible, Claude-compatible, or Gemini-compatible formats."

支持三家上游协议格式，聚合分发 GPT / Claude / Gemini 全家桶。

**结论**: `desc` 中的 "Claude 兼容模型" 描述失实，低估了平台能力。应改为类似 "AI 模型聚合平台，支持 OpenAI/Claude/Gemini 协议"。

---

## API Endpoints 核验表

| 协议 | 现有 preset base_url | 文档验证 | 探测响应 | 结论 |
|---|---|---|---|---|
| anthropic | `https://apinebula.com` | docs/cli/claude-code: `ANTHROPIC_BASE_URL=https://apinebula.com` | - | **正确** |
| openai | `https://apinebula.com/v1` | docs/cli/codex: `base_url = "https://apinebula.com/v1"`（保留 /v1） | - | **正确** |
| gemini | `https://apinebula.com` | docs/cli/gemini: `GOOGLE_GEMINI_BASE_URL=https://apinebula.com` | - | **正确** |

**无需修改 endpoints 配置**。

---

## 全量模型清单（按供应商分组）

### Claude 系列（8 模型）

| 模型 ID | 对 aidog preset | 备注 |
|---|---|---|
| claude-opus-4-8 | 已有 | aidog alias，无需日期后缀 |
| claude-opus-4-7 | 已有 | aidog alias |
| claude-opus-4-6 | 已有 | aidog alias |
| claude-sonnet-4-6 | 已有 | aidog alias |
| claude-sonnet-5 | **缺失** | aidog alias 风格，建议补 |
| claude-sonnet-4-5-20250929 | **部分缺失** | preset 有 `claude-sonnet-4-5`，缺日期后缀 |
| claude-haiku-4-5-20251001 | **部分缺失** | preset 有 `claude-haiku-4-5`，缺日期后缀 |
| claude-fable-5 | **缺失** | 最新 Claude 顶级模型 |

### GPT 系列（5 模型）

| 模型 ID | 对 aidog preset | 备注 |
|---|---|---|
| gpt-5.5 | **缺失** | OpenAI 最新旗舰 |
| gpt-5.4 | **缺失** | |
| gpt-5.4-mini | **缺失** | |
| gpt-5.5-openai-compact | **缺失** | 变体 |
| gpt-image-2 | **缺失** | 图像模型（按次计费） |

### Gemini 系列（6 模型）

| 模型 ID | 对 aidog preset | 备注 |
|---|---|---|
| gemini-3.1-pro-preview | **缺失** | |
| gemini-3.5-flash | **缺失** | |
| gemini-2.5-pro | **缺失** | |
| gemini-2.5-flash-lite | **缺失** | |
| gemini-3-pro-image-preview | **缺失** | 图像（按次） |
| gemini-3.1-flash-image-preview | **缺失** | 图像（按量） |

### 自定义模型（1 模型）

| 模型 ID | 对 aidog preset | 备注 |
|---|---|---|
| codex-auto-review | **缺失** | Codex 专属，代码审查模型 |

---

## 现有 preset 7 模型核对表

| preset 中的 ID | pricing 页是否支持 | 备注 |
|---|---|---|
| claude-opus-4-8 | 已支持 | 正确 |
| claude-sonnet-4-6 | 已支持 | 正确 |
| claude-haiku-4-5 | **部分匹配** | pricing 中为 `claude-haiku-4-5-20251001`，preset 缺日期后缀 |
| claude-opus-4-7 | 已支持 | 正确 |
| claude-opus-4-6 | 已支持 | 正确 |
| claude-opus-4-5 | **未在 pricing 发现** | 可能已下架或未列 pricing |
| claude-sonnet-4-5 | **部分匹配** | pricing 中为 `claude-sonnet-4-5-20250929`，preset 缺日期后缀 |

---

## Model ID 格式分析

APINebula 使用 **混合格式**：

1. **aidog alias 风格**（无日期后缀）：`claude-opus-4-8`、`claude-sonnet-4-6`、`claude-haiku-4-5`
2. **带日期后缀**（官方格式）：`claude-sonnet-4-5-20250929`、`claude-haiku-4-5-20251001`
3. **原始 upstream ID**：`gpt-5.5`、`gemini-2.5-pro`、`codex-auto-review`

**关键发现**：Claude 系列同时存在 alias 和带日期版本，两者在 pricing 页并列存在。

**策略建议**：
- 保持 aidog alias 约定（`claude-opus-4-8` 等）用于主干
- 补充带日期版本作为 `model_list` 扩展
- GPT/Gemini 用原始 upstream ID

---

## 三档默认推荐（JSON 片段）

```json
{
  "models": {
    "default": {
      "default": "claude-opus-4-8",
      "opus": "claude-opus-4-8",
      "sonnet": "claude-sonnet-5",
      "haiku": "claude-haiku-4-5-20251001",
      "gpt": "gpt-5.5",
      "gemini": "gemini-2.5-pro"
    }
  }
}
```

**理由**：
- `default`: Claude Opus 4.8 是最顶级通用模型（0.74 折性价比高）
- `opus`: 同上
- `sonnet`: Claude Sonnet 5（新发布，2.5 折）
- `haiku`: Haiku 4.5 带日期后缀（2.5 折，preset 当前缺后缀需修正）
- `gpt`: GPT-5.5（OpenAI 旗舰，0.52 折）
- `gemini`: Gemini 2.5 Pro（2.21 折）

---

## 建议补全 model_list

```json
{
  "model_list": {
    "default": [
      "claude-opus-4-8",
      "claude-fable-5",
      "claude-sonnet-5",
      "claude-opus-4-7",
      "claude-sonnet-4-6",
      "claude-opus-4-6",
      "claude-haiku-4-5-20251001",
      "claude-sonnet-4-5-20250929",
      "gpt-5.5",
      "gpt-5.4",
      "gpt-5.4-mini",
      "gpt-5.5-openai-compact",
      "gpt-image-2",
      "gemini-3.1-pro-preview",
      "gemini-3.5-flash",
      "gemini-2.5-pro",
      "gemini-2.5-flash-lite",
      "gemini-3-pro-image-preview",
      "gemini-3.1-flash-image-preview",
      "codex-auto-review"
    ]
  }
}
```

**变更说明**：
- 补 14 模型（fable-5 / sonnet-5 / 全家 GPT/Gemini/codex-auto-review）
- 修正 2 模型（haiku/sonnet-4-5 加日期后缀）
- 保留 4 模型（opus-4-8/4-7/4-6 / sonnet-4-6 alias）
- 移除 1 模型（opus-4-5，pricing 未列）

---

## Caveats / Not Found

1. **无公开 `/v1/models` 端点**：探测返回 404，无法通过免鉴权端点获取模型列表，必须依赖 pricing 页或控制台。
2. **claude-opus-4-5 存疑**：preset 中有此 ID，但 pricing 页未发现，可能已下架或未公开计费。
3. **模型广场需登录**：docs 提到「模型广场」功能，但需要账号登录，无法通过公开 API 获取实时模型清单。
4. **Token 分组依赖**：创建 API Key 时需选择分组（Codex / Claude Code / Gemini），分组与模型可用性绑定，preset 未体现此概念。
5. **图像模型计费模式**：`gpt-image-2`、`gemini-*-image-preview` 为「按次计费」，非 token 计费，aidog 无此区分。

---

## Cross-reference

- **预设文件路径**: `src-tauri/defaults/platform-presets.json`
- **apinebula 配置行号**: 2599-2660
- **TS 类型定义**: `src/services/api.ts`（Protocol 枚举需同步）
- **前端展示**: `src/pages/Platforms.tsx`（下拉选项）
- **同类参考**: `aicodemirror`（纯 Claude 代理，7 alias）、`pateway`（多供应商聚合，漏 8 模型）

---

## 鉴权方式

根据 docs/quickstart：

- **格式**: `Bearer YOUR_API_KEY`
- **Header**: `Authorization: Bearer <token>`
- **Base URL 依协议**:
  - Anthropic: `https://apinebula.com`
  - OpenAI: `https://apinebula.com/v1`
  - Gemini: `https://apinebula.com`

**无特殊鉴权头**，标准 Bearer token。

---

## 总结

1. **平台定位**: 多供应商聚合（GPT + Claude + Gemini），非纯 Claude 代理，`desc` 需改写。
2. **endpoints**: 现有 3 个完全正确，无需修改。
3. **模型总数**: pricing 页列出 **20 模型**（8 Claude + 5 GPT + 6 Gemini + 1 custom），preset 当前仅 7 Claude，缺 13。
4. **source_urls**: 两个 URL 均返回 200，有效。
5. **model_list**: 需大幅扩充 + 修正日期后缀。
6. **models.default**: 建议补充三档默认推荐（见上）。
