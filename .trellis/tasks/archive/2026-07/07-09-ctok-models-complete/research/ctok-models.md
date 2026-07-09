# Research: CTok.ai 全量模型研究

- **Query**: CTok.ai (ctok.ai) 平台定位、endpoints 核验、全量模型清单、鉴权方式
- **Scope**: external（官网/API 探测）+ internal（preset 对齐）
- **Date**: 2026-07-09

---

## 结论速览（TL;DR）

1. **平台定位**：CTok.ai 是**多供应商 API 网关**，支持 **Claude + GPT + Gemini** 三家族，非纯 Claude 代理。
2. **Endpoints 全部存活**（HTTP 401 鉴权拦截 = 路由有效）：
   - Anthropic: `https://api.ctok.ai/v1/messages`
   - OpenAI: `https://api.ctok.ai/v1/chat/completions`
   - Gemini: `https://api.ctok.ai/v1beta/models`
3. **多 host 布局**：
   - `ctok.ai` - Claude Code 教程站（中文内容站）
   - `api.ctok.ai` - API 网关首页（登录墙后）
   - `etok.ai` - 国际站（AI 编程教程）
4. **Health 端点**：`GET /health` 返回 `{"status":"ok"}`
5. **鉴权方式**：通用 `Authorization: Bearer <key>`，key 格式未公开（推测非 `sk-ant-api03-xxx` 专属格式）
6. **现有 7 模型核对**：当前 preset 仅含 Claude 系 7 个 id，与**多供应商定位不符**，建议扩展。
7. **Caveats**：无公开文档/模型清单 API，全部信息需登录后获取。

---

## API Endpoints

### 路由验证（curl 探测，2026-07-09）

| 协议 | base_url | 探测路径 | 响应状态 | 鉴权报错 |
|---|---|---|---|---|
| anthropic | `https://api.ctok.ai` | `POST /v1/messages` | **401** | `{"code":"INVALID_API_KEY","message":"Invalid API key"}` |
| openai | `https://api.ctok.ai` | `POST /v1/chat/completions` | **401** | `{"code":"INVALID_API_KEY","message":"Invalid API key"}` |
| gemini | `https://api.ctok.ai` | `GET /v1beta/models` | **401** | `{"error":{"code":401,"message":"Invalid API key","status":"UNAUTHENTICATED"}}` |
| health | `https://api.ctok.ai` | `GET /health` | **200** | `{"status":"ok"}` |

**判定**：
- 三协议 endpoint 全部存活（401 = 需有效 key，非 404/403 不存在）
- 报错格式通用，无 `sk-ant-api03-xxx` 专属格式要求（与 aicodemirror 不同）
- Gemini 报错略有不同（`status: "UNAUTHENTICATED"`），但核心一致

### 多 Host 布局确认

| 子域 | 用途 | 内容 |
|---|---|---|
| `ctok.ai` | 教程站 | Claude Code 安装指南、最佳实践（中文） |
| `api.ctok.ai` | API 网关 | 网关首页、登录、注册、订阅管理 |
| `etok.ai` | 国际站 | AI 编程教程（英文） |

**来源**：
- `https://api.ctok.ai/` 首页显示 "One API, Multiple Choices - Claude Supported, GPT Supported, Gemini Supported"
- `https://ctok.ai/` 首页显示 "CTok Claude Code 教程"
- `https://etok.ai/` 首页显示 "ETok — AI Coding Tutorials & Coding Plans"

---

## 鉴权方式

- **Header**：`Authorization: Bearer <key>`
- **Key 格式**：未公开（推测无特定前缀要求，通用 Bearer token）
- **获取方式**：需在 `https://api.ctok.ai/` 注册/登录后获取（`registration_enabled: false`，需邀请码）

**测试命令**：
```bash
# Anthropic endpoint
curl -X POST "https://api.ctok.ai/v1/messages" \
  -H "anthropic-version: 2023-06-01" \
  -H "Authorization: Bearer test-key" \
  -d '{"model":"claude-3-5-sonnet-20241022","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}'

# OpenAI endpoint
curl -X POST "https://api.ctok.ai/v1/chat/completions" \
  -H "Authorization: Bearer test-key" \
  -d '{"model":"gpt-4","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}'

# Gemini endpoint
curl -X GET "https://api.ctok.ai/v1beta/models" \
  -H "Authorization: Bearer test-key"
```

---

## 模型范围确认

### 证据链

1. **官网定位**：api.ctok.ai 首页显示 "Supported AI Models: Claude, GPT, Gemini"
2. **三协议端点全部存活**：证明多供应商路由存在
3. **与纯 Claude 代理的差异**：aicodemirror/ccsub/claudeapi 等纯 Claude 代理的 401 报错明确要求 `sk-ant-api03-xxx`，ctok 则通用报错

### 推测模型范围（**无官方确认**）

| 家族 | 可能支持的模型（推测） |
|---|---|
| Claude | claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5, claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5, claude-sonnet-5, claude-fable-5 |
| GPT | gpt-4o, gpt-4-turbo, gpt-4, gpt-3.5-turbo, o1-preview, o1-mini |
| Gemini | gemini-1.5-pro, gemini-1.5-flash, gemini-pro |

**注**：以上为基于多供应商定位的**推测**，非官方清单。ctok 可能仅支持特定子集。

---

## 全量模型清单

### 现有 7 模型核对

当前 preset 的 `model_list.default`：

```json
[
  "claude-opus-4-8",
  "claude-sonnet-4-6",
  "claude-haiku-4-5",
  "claude-opus-4-7",
  "claude-opus-4-6",
  "claude-opus-4-5",
  "claude-sonnet-4-5"
]
```

**分析**：
- 与 aidog 内 18+ 个纯 Claude 代理 preset（ccsub/claudeapi/claudecn/aicodemirror/…）完全一致
- 这是 aidog 项目级的 Claude alias 约定
- **但与 ctok 多供应商定位不符**——应包含 GPT/Gemini

### 需扩展的建议模型（**待验证**）

基于多供应商定位，建议添加：

```json
{
  "claude": [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-5",
    "claude-sonnet-5",
    "claude-fable-5"
  ],
  "gpt": [
    "gpt-4o",
    "gpt-4-turbo",
    "gpt-4",
    "gpt-3.5-turbo",
    "o1-preview",
    "o1-mini"
  ],
  "gemini": [
    "gemini-1.5-pro",
    "gemini-1.5-flash",
    "gemini-pro"
  ]
}
```

**警告**：以上为**推测**，实际支持的模型子集可能不同。需登录 dashboard 或获取有效 API key 后调用 `/v1/models` 确认。

---

## ID 格式判定

- **Claude 系**：裸 id（如 `claude-opus-4-8`），无 `anthropic/` 前缀
- **GPT 系**：推测裸 id（如 `gpt-4o`），无 `openai/` 前缀
- **Gemini 系**：推测裸 id（如 `gemini-1.5-pro`），无 `google/` 前缀

**依据**：aidog preset 约定，所有 model_list 使用裸 id，`base_url` 统一处理协议路由。

---

## 三档默认推荐（`models.default`）

**当前状态**：空 `{}`

**建议**（基于 aidog Claude 约定 + 多供应商定位）：

```json
{
  "claude-opus-4-8": {},
  "claude-sonnet-4-6": {},
  "claude-haiku-4-5": {}
}
```

**或考虑跨家族推荐**：

```json
{
  "claude-opus-4-8": {},
  "gpt-4o": {},
  "gemini-1.5-pro": {}
}
```

**注**：aidog 约定 `models.default` 的 key 是 model id（非档位名），value 为空 obj `{}`。

---

## Caveats / Not Found

1. **无公开文档**：`/pricing` 404，`docs.ctok.ai` 404，`/docs` 登录墙
2. **无模型清单 API**：`/v1/models` `/api/models` `/api/pricing` 均需鉴权
3. **无公开定价信息**：pricing 页面不存在
4. **需邀请注册**：`registration_enabled: false`
5. **模型范围不确定**：虽确认多供应商，但具体支持哪些模型（是否全家族/子集）无官方说明
6. **Key 格式不确定**：无 `sk-ant-api03-xxx` 专属要求，但实际发行格式未知

---

## 数据来源

| 来源 | URL/方法 | 日期 |
|---|---|---|
| API 探测 | curl `https://api.ctok.ai/v1/messages` | 2026-07-09 |
| API 探测 | curl `https://api.ctok.ai/v1/chat/completions` | 2026-07-09 |
| API 探测 | curl `https://api.ctok.ai/v1beta/models` | 2026-07-09 |
| 官网 | `https://api.ctok.ai/` (via Jina Reader) | 2026-07-09 |
| 教程站 | `https://ctok.ai/` (via Jina Reader) | 2026-07-09 |
| 国际站 | `https://etok.ai/` (via Jina Reader) | 2026-07-09 |
| Health | curl `https://api.ctok.ai/health` | 2026-07-09 |
| Preset 对齐 | `src-tauri/defaults/platform-presets.json` | 2026-07-09 |

---

## 与 aidog preset 的差异

| 项目 | 当前 preset | 研究发现 | 建议 |
|---|---|---|---|
| desc | "CTok.ai API, Claude 兼容模型" | 多供应商（Claude + GPT + Gemini） | 改为 "CTok.ai API 网关, 多模型支持" |
| endpoints | 仅 anthropic | 三协议全部存活 | 添加 openai/gemini endpoints |
| model_list | 仅 Claude 7 个 | 应支持 GPT/Gemini | 扩展（待官方确认） |
| models.default | 空 `{}` | - | 填充三档推荐 |
| source_urls | docs/pricing 存在 | 404 | 移除或改为 homepage |

---

## 下一步行动建议

1. **获取有效 API key**：联系 ctok.ai 或申请测试账号
2. **调用模型列表 API**：`GET /v1/models` 获取完整清单
3. **验证 GPT/Gemini 支持**：用有效 key 测试 gpt-4o/gemini-1.5-pro
4. **更新 preset**：根据官方清单补全 model_list + models.default
5. **更新 source_urls**：移除 404 的 docs/pricing，改为 homepage
