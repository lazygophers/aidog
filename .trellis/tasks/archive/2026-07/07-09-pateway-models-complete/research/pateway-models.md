# PatewayAI 全量研究

- **Query**: 查清 PatewayAI (pateway.ai) 全量模型清单 + endpoints 形态，为 aidog `platform-presets.json` 补全
- **Scope**: external
- **Date**: 2026-07-09
- **结论一句话**: Pateway 是**多供应商聚合站**（非 claude-only），当前支持 **5 家 / 11 模型**：Anthropic Claude（5）+ OpenAI Codex（2）+ DeepSeek（2）+ Qwen（2）+ GLM（2）。现有 preset 漏了 4 个国产系列与 gpt-5.5；并残留 2 个已下架的 claude-4-5 系列条目。

---

## API Endpoints

官方 `api-reference.html` + `integration.html` 一致结论：

| 协议 | base_url | path | 鉴权头 | 适用模型 |
|---|---|---|---|---|
| Anthropic Messages | `https://api.pateway.ai` | `/v1/messages`（Claude Code 用，SDK 用 `…/v1`） | `x-api-key: <KEY>` | Claude 全系 + **DeepSeek / Qwen / GLM 全系** |
| OpenAI Responses | `https://api.pateway.ai/v1` | `/v1/responses` | `Authorization: Bearer <KEY>` | 仅 Codex 系列（gpt-5.5 / gpt-5.3-codex） |
| 查询模型列表 | `https://api.pateway.ai` | `GET /v1/models` | 同上，二选一 | 全量（按协议分组返回） |

关键引用：
- Claude Code 配置：`"ANTHROPIC_BASE_URL": "https://api.pateway.ai"`（**不带 `/v1`**，由 SDK 自拼 `/v1/messages`）—integration.html
- Anthropic SDK：`base_url="https://api.pateway.ai/v1"`—integration.html（SDK 内部不再追加 `/v1`）
- Codex CLI：`base_url = "https://api.pateway.ai/v1"`, `wire_api = "responses"`—integration.html
- `GET /v1/models` 无 Key 返 `401 Missing API Key`（实测 2026-07-09），需登录控制台取 Key 才能拉动态列表

### 与现有 preset 对照（2 endpoint 全部正确，无需改）
```json
"endpoints": {
  "default": [
    {"protocol":"anthropic","base_url":"https://api.pateway.ai","client_type":"claude_code"},
    {"protocol":"openai","base_url":"https://api.pateway.ai/v1","client_type":"codex_tui"}
  ]
}
```
- anthropic 端点同时承载 Claude + 国产模型（DeepSeek/Qwen/GLM 走 Anthropic Messages 格式，integration.html 明示）
- **不支持 gemini 协议**（全站无 google/gemini 字样，grep 确认）
- **不支持 OpenAI Chat Completions**：openai 协议只走 `/v1/responses`（Responses API），Codex 系列专用

### 工具兼容矩阵（integration.html 官方声明）
- Claude Code：支持 Claude + DeepSeek/Qwen/GLM，**不支持 GPT**
- Codex CLI：**只支持 Codex 系列**，不支持 Claude/DeepSeek/Qwen/GLM

---

## 模型范围确认

**多供应商，非 claude-only**。完整覆盖（pricing.html 表格 + integration.html 国产模型配置 + api-reference.html `/v1/models` 示例 三处交叉核对）：

| 供应商 | 模型数 | 模型 id | 上下文窗口 |
|---|---|---|---|
| Anthropic | 5 | claude-opus-4-8 / claude-opus-4-7 / claude-opus-4-6 / claude-sonnet-4-6 / claude-haiku-4-5 | 官方未在 pricing 页标注（推测: 200K） |
| OpenAI (Codex) | 2 | gpt-5.5 / gpt-5.3-codex | gpt-5.5 长上下文档位 ≥272K；gpt-5.3-codex 单一档 |
| DeepSeek | 2 | deepseek-v4-pro / deepseek-v4-flash | pro 配置中出现 `deepseek-v4-pro[1m]` 形态（1M context 标记） |
| Qwen (通义千问) | 2 | qwen3.7-max / qwen3.6-plus | qwen 长/短档分界 256K |
| GLM (智谱) | 2 | glm-5.1 / glm-5 | 长/短档分界 32K |

### 模型 id 格式
- 全部**裸 id**（非 `provider/model` 形式）
- claude-haiku-4-5 在 `/v1/models` 示例中以**日期化别名** `claude-haiku-4-5-20251001` 出现；pricing 页用裸 `claude-haiku-4-5`。两种形态推测: 都被接受（官方 SDK 示例统一用裸 id）
- deepseek-v4-pro 的 `[1m]` 后缀是 Claude Code env 配置中的上下文标记，**非模型 id 一部分**（API 调用级 id 仍是 `deepseek-v4-pro`）

---

## 全量模型清单

### Claude 系（Anthropic Messages API，官方 8 折）
| id | Input/M | Output/M | Cache Write 5m/M | Cache Write 1h/M | Cache Read/M | Web Search/1K |
|---|---|---|---|---|---|---|
| claude-opus-4-8 | 4 | 20 | 5 | 8 | 0.4 | 10 |
| claude-opus-4-7 | 4 | 20 | 5 | 8 | 0.4 | 10 |
| claude-opus-4-6 | 4 | 20 | 5 | 8 | 0.4 | 10 |
| claude-sonnet-4-6 | 2.4 | 12 | 3 | 4.8 | 0.24 | 10 |
| claude-haiku-4-5 | 0.8 | 4 | 1 | 1.6 | 0.08 | 10 |

> 注：opus-4-8 / 4-7 / 4-6 同档同价（最新为 4-8，pricing 表头列示）。

### OpenAI 系（Responses API，官方 8 折）
| id | Input short/M (<272K) | Input long/M (≥272K) | Output short/M | Output long/M | Cache Read/M | Web Search/1K |
|---|---|---|---|---|---|---|
| gpt-5.5 | 4 | 8 | 24 | 36 | 0.4 / 0.8 | 10 |
| gpt-5.3-codex | 1.4 | — | 11.2 | — | 0.14 | — |

### DeepSeek 系（Anthropic Messages API，官方 8 折）
| id | Input/M | Output/M | Cache Read/M |
|---|---|---|---|
| deepseek-v4-pro | 0.3528 | 0.7056 | 0.0032 |
| deepseek-v4-flash | 0.1176 | 0.2352 | 0.0024 |

### Qwen 系（Anthropic Messages API，官方 8 折，长/短档分界 256K）
| id | Input short/M | Input long/M | Output short/M | Output long/M | Cache Write 5m/M (短/长) | Cache Read/M (短/长) | Web Search/1K |
|---|---|---|---|---|---|---|---|
| qwen3.7-max | 1.412 | 0.2352 | 4.2352 | 1.412 | 1.7648 / 0.2936 | 0.1408 / 0.0232 | 0.588 |
| qwen3.6-plus | 0.9408 | — | 5.6472 | — | 1.176 | 0.0944 | 0.588 |

### GLM 系（Anthropic Messages API，官方 8 折，长/短档分界 32K）
| id | Input short/M | Input long/M | Output short/M | Output long/M | Cache Read/M (短/长) |
|---|---|---|---|---|---|
| glm-5.1 | 0.7056 | 0.9416 | 2.8232 | 3.2944 | 0.1528 / 0.2352 |
| glm-5 | 0.4704 | 0.7056 | 2.1176 | 2.588 | 0.1176 / 0.1768 |

### 其他
- **无** Google Gemini / xAI Grok / Moonshot Kimi / MiniMax / Doubao / Qianfan 等其他国产
- grep `gemini|google|grok|kimi|minimax` 全部 docs 页面零命中（2026-07-09 实测）

---

## 三档默认推荐（供 `models.default`）

按 aidog 平台默认模型惯例（sonnet 档 / gpt 档 / 国产档）：

```json
"models": {
  "default": {
    "claude": "claude-sonnet-4-6",
    "claude_opus": "claude-opus-4-8",
    "claude_haiku": "claude-haiku-4-5",
    "openai": "gpt-5.3-codex",
    "openai_flagship": "gpt-5.5",
    "deepseek": "deepseek-v4-pro",
    "deepseek_lite": "deepseek-v4-flash",
    "qwen": "qwen3.7-max",
    "qwen_lite": "qwen3.6-plus",
    "glm": "glm-5.1",
    "glm_lite": "glm-5"
  }
}
```
> key 命名仅参考，最终以 aidog 现有同类 preset（packycode/cherryin）字段约定为准；main agent 决策。

---

## 现有 7 模型核对

当前 `platform-presets.json` pateway.model_list.default：
```
claude-opus-4-8      ✅ 当前最新 opus，保留
claude-sonnet-4-6    ✅ 当前最新 sonnet，保留
claude-haiku-4-5     ✅ 当前最新 haiku，保留
claude-opus-4-7      ✅ pricing 同档，保留
claude-opus-4-6      ✅ pricing 同档，保留
claude-opus-4-5      ⚠️ 全部官方文档（pricing/api-ref/integration/faq）零命中 → 已下架，建议删除
claude-sonnet-4-5    ⚠️ 同上零命中 → 已下架，建议删除
```

应补增（pricing/integration 双重确认在售）：
```
gpt-5.5, gpt-5.3-codex,
deepseek-v4-pro, deepseek-v4-flash,
qwen3.7-max, qwen3.6-plus,
glm-5.1, glm-5
```

补全后 model_list 共 **13 个**（删 2 旧 + 加 8 新 + 留 5 现 = 13；等价于 5 Claude + 2 OpenAI + 2 DeepSeek + 2 Qwen + 2 GLM）。

---

## Caveats / Not Found

- `GET /v1/models` 全量模型 id 列表**未能动态拉取**（需用户控制台 API Key，401 阻塞）。本次模型清单基于 `pricing.html` 定价表 + `integration.html` 国产模型配置示例 + `api-reference.html` `/v1/models` 响应示例 三方静态交叉核对，覆盖完整，但**无法排除官方未在文档列示的隐藏/灰度模型**。建议 main 实施时让用户用真实 Key 跑一次 `/v1/models` 终检。
- `claude-opus-4-5` / `claude-sonnet-4-5` 在现有 preset 但官方文档全无 → **推测** 已随 opus-4-6/sonnet-4-6 上线而下架；若用户实测仍能调通可保留，否则删除。
- 模型 id 是否支持带日期别名（如 `claude-haiku-4-5-20251001`）入 model_list：pricing 页统一用裸 id，建议 preset 沿用裸 id 形式以与同站点（packycode/cherryin）一致。
- 上下文窗口：官方 pricing 仅给价格档位分界（272K/256K/32K/1M 标记），未显式声明每模型 max context；aidog preset 当前未存 context 字段，不影响。

---

## 数据来源（URL + 2026-07-09 实测）

| URL | 状态 | 用途 |
|---|---|---|
| https://pateway.ai/ | HTTP 200 | 首页 SEO 静态文本，首份模型范围声明（Claude + OpenAI Codex） |
| https://pateway.ai/docs/ | HTTP 200 | docs 索引（**真 docs 入口**，`docs.pateway.ai` 子域已弃用，返 404） |
| https://pateway.ai/docs/pricing.html | HTTP 200 | 全量模型 + 定价（核心数据源） |
| https://pateway.ai/docs/api-reference.html | HTTP 200 | endpoint 路径 / 鉴权 / `/v1/models` 响应格式 |
| https://pateway.ai/docs/integration.html | HTTP 200（15s 部分下载，已含全部国产模型配置段） | Claude Code / Codex CLI / SDK base_url 与工具兼容矩阵 |
| https://pateway.ai/docs/quickstart.html | HTTP 200 | 入门 curl 示例，二次核对 endpoint |
| https://pateway.ai/docs/faq.html | HTTP 200 | 协议范围二次确认 |
| https://pateway.ai/sitemap.xml | HTTP 200 | 仅根 URL，无深链 |
| https://api.pateway.ai/v1/models | HTTP 401 `Missing API Key` | 实测确认需 Key |
| https://api.pateway.ai/api/pricing | HTTP 404 | 实测无免鉴权 pricing API（非 new-api/one-api 系，与 packycode/cherryin 不同） |
| https://docs.pateway.ai/* | HTTP 404 | 子域已弃用，请改用 `/docs/` 路径 |

> 现有 preset 的 `"source_urls": {"docs":"https://docs.pateway.ai/", "pricing":"https://pateway.ai/pricing"}` **两个 URL 均失效**（404），建议更新为 `"docs":"https://pateway.ai/docs/", "pricing":"https://pateway.ai/docs/pricing.html"`。
