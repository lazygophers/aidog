# Research: APIKEY.FUN 全量模型清单调研

- **Query**: APIKEY.FUN 全量模型清单 + endpoints 形态 + 鉴权方式
- **Scope**: 外部官方文档/API 调研
- **Date**: 2026-07-09

## 数据来源表

| URL | 访问日期 | 状态 | 说明 |
|-----|---------|------|------|
| https://apikey.fun/ | 2026-07-09 | 200 OK | 首页（平台定位、支持模型） |
| https://apikey.fun/pricing | 2026-07-09 | 200 OK | 定价页（Claude 模型列表） |
| https://apikey.fun/docs | 2026-07-09 | 200 OK | Docs 主页（endpoint 配置） |
| https://api.apikey.fun | 2026-07-09 | 301 → 主页 | HEAD 请求重定向，实际为 web 域 |
| https://api.apikey.fun/v1/chat/completions | 2026-07-09 | 401 Unauthorized | OpenAI 协议端点存在（需鉴权） |
| https://api.apikey.fun/v1/messages | 2026-07-09 | 401 Unauthorized | Anthropic 协议端点存在（需鉴权） |
| https://api.apikey.fun/models | 2026-07-09 | 401 Unauthorized | Gemini 协议端点存在（需鉴权） |
| https://apikey.fun/docs/claude-code | 2026-07-09 | 302 → 登录页 | 需登录 |
| https://apikey.fun/pricing/gpt | 2026-07-09 | 302 → 登录页 | 需登录 |
| https://apikey.fun/pricing/gemini | 2026-07-09 | 302 → 登录页 | 需登录 |
| https://apikey.fun/faq | 2026-07-09 | 302 → 登录页 | 需登录 |

**注**：GPT/Gemini 定价页需登录，pricing 页仅展示 Claude 模型。所有 docs 子页面和 FAQ 需登录。

---

## 平台定位

**结论**：**多供应商聚合平台**（非 Claude-only）

**证据**：
1. 首页标题：`"The Universal AI Gateway"`（通用 AI 网关）
2. 首页描述：`"low-latency access to Claude, ChatGPT, Gemini, and more"`
3. 独立套餐页：`"Claude On-Demand"` + `"ChatGPT On-Demand"`（两个独立套餐）
4. 3 协议 endpoint 都存在（见下表）

**现有 preset 描述失实**：
- 当前 desc: `"APIKEY.FUN API, Claude 兼容模型"`
- 应改为: `"APIKEY.FUN API, 多供应商聚合平台（Claude/GPT/Gemini）"` 或类似表述

---

## API Endpoints 核验表

| 协议 | base_url | 验证方式 | 结果 | 来源 |
|-----|----------|---------|------|------|
| anthropic | `https://api.apikey.fun` | docs 页 + 401 探测 | ✅ 正确 | docs 主页 |
| openai | `https://api.apikey.fun/v1` | docs 页 + 401 探测 | ✅ 正确 | Codex 配置 |
| gemini | `https://api.apikey.fun` | 401 探测（含 x-goog-api-key 提示） | ✅ 正确 | API 响应 |

**探测响应示例**（OpenAI 端点）：
```json
{"code":"API_KEY_REQUIRED","message":"API key is required in Authorization header (Bearer scheme), x-api-key header, or x-goog-api-key header"}
```

**结论**：现有 preset 3 个 endpoint **全部正确，无需修改**。

---

## 全量模型清单

### Claude 模型（ pricing 页明文确认）

| Model ID | 输入价格 | 输出价格 | Cache Write | Cache Read | 来源 |
|----------|---------|---------|-------------|-------------|------|
| claude-opus-4-8 | ￥3.50 / 1M | ￥17.50 / 1M | ￥4.38 / 1M | ￥0.35 / 1M | pricing 页 |
| claude-opus-4-7 | ￥3.50 / 1M | ￥17.50 / 1M | ￥4.38 / 1M | ￥0.35 / 1M | pricing 页 |
| claude-opus-4-6 | ￥3.50 / 1M | ￥17.50 / 1M | ￥4.38 / 1M | ￥0.35 / 1M | pricing 页 |
| claude-sonnet-5 | ￥1.40 / 1M | ￥7.00 / 1M | ￥1.75 / 1M | ￥0.14 / 1M | pricing 页 |
| claude-sonnet-4-6 | ￥2.10 / 1M | ￥10.50 / 1M | ￥2.63 / 1M | ￥0.21 / 1M | pricing 页 |
| claude-haiku-4-5 | ￥0.70 / 1M | ￥3.50 / 1M | ￥0.88 / 1M | ￥0.07 / 1M | pricing 页 |

**总计 6 个 Claude 模型**。

### GPT/Gemini 模型（pricing 页未展示）

**状态**：pricing 页仅显示 Claude 模型，GPT/Gemini 定价需登录。

**推测**（基于多供应商定位）：
- OpenAI 端点存在（`/v1/chat/completions` 401 响应）
- Gemini 端点存在（`/models` 401 响应 + x-goog-api-key 提示）
- 模型列表可能与官方保持一致（类似其他聚合平台）

**需用户控制台核实**：
- GPT 模型列表（gpt-4.* / gpt-3.5-turbo / gpt-*）
- Gemini 模型列表（gemini-2.0-flash / gemini-1.5-pro / gemini-*）

---

## 现有 7 模型核对表

| Model ID | pricing 页状态 | preset 状态 | 是否需改 |
|----------|---------------|-------------|---------|
| claude-opus-4-8 | ✅ 存在 | ✅ 存在 | - |
| claude-sonnet-4-6 | ✅ 存在 | ✅ 存在 | - |
| claude-haiku-4-5 | ✅ 存在 | ✅ 存在 | - |
| claude-opus-4-7 | ✅ 存在 | ✅ 存在 | - |
| claude-opus-4-6 | ✅ 存在 | ✅ 存在 | - |
| claude-opus-4-5 | ❌ 不在 pricing 页 | ✅ 存在 | ⚠️ 可能旧 |
| claude-sonnet-4-5 | ❌ 不在 pricing 页 | ✅ 存在 | ⚠️ 可能旧 |

**新增模型**：
- `claude-sonnet-5`：pricing 页有，preset 缺失

**建议**：
1. **新增** `claude-sonnet-5`（pricing 页最新模型）
2. **保留** `claude-opus-4-5` 和 `claude-sonnet-4-5`（可能是旧版但仍可用，或未展示在 pricing 页）

---

## Model ID 格式与日期后缀

**格式**：裸 id（无 `provider/` 前缀），如 `claude-opus-4-8`

**日期后缀**：
- pricing 页模型：**无日期后缀**（`claude-opus-4-8`）
- aidog 官方 preset（anthropic 协议）：部分有日期后缀（如 `claude-opus-4-5-20251101`）

**结论**：APIKEY.FUN 使用 **无日期后缀格式**，与 aidog 对 Claude 代理平台的 alias 约定一致（见兄弟 preset `aicodemirror`/`apinebula`/`sudocode` 等 18+ 平台，同样 7 个 alias 无后缀）。

---

## 三档默认推荐

基于 pricing 页价格梯度：

```json
{
  "models": {
    "default": {
      "default": "claude-opus-4-8",
      "opus": "claude-opus-4-8",
      "sonnet": "claude-sonnet-5",
      "haiku": "claude-haiku-4-5"
    }
  }
}
```

**理由**：
- **default/opus**：`claude-opus-4-8`（最强能力，pricing 页首位）
- **sonnet**：`claude-sonnet-5`（最新 Sonnet，pricing 页显示）
- **haiku**：`claude-haiku-4-5`（最经济）

---

## source_urls 核验

| URL | 状态 |
|-----|------|
| https://apikey.fun/ | ✅ 200 OK |
| https://apikey.fun/pricing | ✅ 200 OK |

**结论**：source_urls 正确，无需修改。

---

## Caveats / 数据局限

### 已穷尽的渠道
✅ 官网首页
✅ pricing 页（Claude 模型）
✅ docs 主页（endpoint 配置）
✅ API 端点探测（3 协议 401 验证）

### 需用户控制台核实的项
⚠️ **GPT 模型列表**：pricing 页需登录，未公开
⚠️ **Gemini 模型列表**：pricing 页需登录，未公开
⚠️ **claude-opus-4-5** 和 **claude-sonnet-4-5** 是否仍可用（pricing 页未显示）

### 未尝试的渠道
- 登录后查看完整 pricing 页（GPT/Gemini 段）
- 登录后查看 docs 子页面（API 文档、模型列表）
- GitHub/Reddit/V2EX 社区讨论（Exa 搜索未返回结果）

---

## 建议补全 model_list

### 短期（基于 pricing 页确认数据）
```json
{
  "model_list": {
    "default": [
      "claude-opus-4-8",
      "claude-sonnet-5",
      "claude-haiku-4-5",
      "claude-opus-4-7",
      "claude-opus-4-6",
      "claude-sonnet-4-6"
    ]
  }
}
```

**变更**：
- **新增** `claude-sonnet-5`
- **移除** `claude-opus-4-5`、`claude-sonnet-4-5`（pricing 页未显示，可能已废弃）

### 长期（需控制台核实）
- 补全 GPT 模型列表（需登录查看 pricing/gpt 页）
- 补全 Gemini 模型列表（需登录查看 pricing/gemini 页）

---

## Cross-reference

### preset 路径
- 文件：`src-tauri/defaults/platform-presets.json`
- apikeyfun 起始行：2537
- 关键字段：
  - endpoints.default: line 2540-2556
  - model_list.default: line 2562-2570
  - models.default: line 2558-2559（当前为空）
  - desc: line 2582-2590

### 兄弟 preset 参考（Claude 代理平台）
- `aicodemirror`：纯 Claude 代理，7 alias 无后缀
- `apinebula`：3 endpoint，7 模型，结构几乎完全一致（line 2599-2660）
- `sudocode`：1 endpoint（anthropic），7 模型（line 2661-2685）

### 约定一致性
APIKEY.FUN 的 7 个 Claude alias 与 aidog 项目内 18+ Claude 代理平台保持一致，无需日期后缀。
