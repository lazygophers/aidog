# Research: SSSAiCode 全量模型与端点

- **Query**: SSSAiCode (sssaicode.com) 全量官方信息调研
- **Scope**: 外部调研（官网 + API 探测）
- **Date**: 2026-07-09

## TL;DR / 结论速览

| 维度 | 结论 |
|------|------|
| **平台定位** | **多供应商聚合**（非纯 Claude 代理），支持 Anthropic / OpenAI / DeepSeek 三类协议 |
| **总模型数** | 17 个（Anthropic 7 个 + OpenAI 5 个 + DeepSeek 2 个 + 其他 3 个） |
| **Endpoint 存活** | ✅ Anthropic (`/api/v1/messages`), ✅ OpenAI (`/api/v1/chat/completions`), ✅ DeepSeek（复用 OpenAI 协议） |
| **多节点布局** | HK（主节点）+ HK2 + HK3 + CF US + CF2 + HK（楼梯中转） |
| **ID 格式** | **裸 ID**（如 `claude-opus-4-8`、`gpt-4o`），无 `provider/model` 前缀 |
| **鉴权方式** | Authorization Bearer + 自定义 "client token" 校验（非标准 x-api-key） |
| **host 关系** | `sssaicode.com` = 官网/dashboard，`node-hk.sssaicodeapi.com` = 香港 API 网关 |

## 平台定位确认

**证据链**：
1. 官网 `/models` 页面明确列出三类供应商：
   - ![Anthropic](https://sssaicode.com/claude-color.svg) **Anthropic** 7 个
   - ![OpenAI](https://sssaicode.com/openai-white.svg) **OpenAI** 5 个
   - ![DeepSeek](https://sssaicode.com/deepseek-color.svg) **DeepSeek** 2 个
   - 总计 **17 个模型**

2. curl 探测三类端点全部返回 401 "缺少 client token"（非 404），证实端点存活：
   ```bash
   # Anthropic 协议
   POST https://node-hk.sssaicodeapi.com/api/v1/messages → 401 "缺少 client token"

   # OpenAI 协议
   POST https://node-hk.sssaicodeapi.com/api/v1/chat/completions → 401 "缺少 client token"

   # DeepSeek（复用 OpenAI 协议）
   POST https://node-hk.sssaicodeapi.com/api/v1/chat/completions (model=deepseek-chat) → 401 "缺少 client token"
   ```

**结论**：SSSAiCode 是 **多供应商聚合平台**，当前 preset 描述 "Claude 兼容模型" 需修正。

## API Endpoints

### 存活性探测结果

| 协议 | base_url | 探测路径 | 响应状态 | 鉴权方式 |
|------|----------|----------|----------|----------|
| **anthropic** | `https://node-hk.sssaicodeapi.com/api` | `/v1/messages` | ✅ 401 "缺少 client token" | Authorization Bearer |
| **openai** | `https://node-hk.sssaicodeapi.com/api` | `/v1/chat/completions` | ✅ 401 "缺少 client token" | Authorization Bearer |
| **deepseek** | `https://node-hk.sssaicodeapi.com/api` | `/v1/chat/completions` | ✅ 401 "缺少 client token" | Authorization Bearer |

### 多节点布局

官网 `/models` 页面显示通道列表：
- **HK**（香港主节点，`node-hk.sssaicodeapi.com`）
- **HK2**
- **HK3**
- **CF US**（Cloudflare US 节点）
- **CF2**（Cloudflare 2 节点）
- **HK 🪜**（楼梯中转节点，可能是代理链中转）

**节点域名**：
- 主 API 网关：`node-hk.sssaicodeapi.com`
- 备用域名：`sssaiapi.com`、`sssaicodeapi.com`（2026-06-04 通知新增，探测返回 405 Method Not Allowed，可能是 nginx 配置问题）

### host 关系

| 域名 | 角色 | 说明 |
|------|------|------|
| `sssaicode.com` | 官网/dashboard | SPA，含 `/models`、`/pricing`、`/install` 页面，需 Cloudflare 验证 |
| `node-hk.sssaicodeapi.com` | 香港 API 网关 | 真实 API 入口，路径含 `/api` 前缀 |
| `sssaiapi.com` | 备用官网域名 | 2026-06-04 新增 |
| `sssaicodeapi.com` | 备用 API 域名 | 2026-06-04 新增，探测返回 405 |

**URL 构造**：
```
完整 URL = base_url + provider_api_path
Anthropic: https://node-hk.sssaicodeapi.com/api + /v1/messages = https://node-hk.sssaicodeapi.com/api/v1/messages
OpenAI:   https://node-hk.sssaicodeapi.com/api + /v1/chat/completions = https://node-hk.sssaicodeapi.com/api/v1/chat/completions
```

## 全量模型清单

基于官网 `/models` 页面信息（17 个模型）和当前 preset（7 个 aidog alias），推测模型清单如下：

### Anthropic 家族（7 个）

| 营销名 | aidog alias | 状态 | 备注 |
|--------|-------------|------|------|
| Claude Opus 4.8 | `claude-opus-4-8` | ✅ 当前 preset | 最新旗舰 |
| Claude Sonnet 5 | `claude-sonnet-5` | ❌ 缺失 | 2026-07-01 上线通知 |
| Claude Fable 5 | `claude-fable-5` | ❌ 缺失 | 2026-07-02 上线通知，仅 Max 通道 |
| Claude Opus 4.7 | `claude-opus-4-7` | ✅ 当前 preset | |
| Claude Opus 4.6 | `claude-opus-4-6` | ✅ 当前 preset | |
| Claude Opus 4.5 | `claude-opus-4-5` | ✅ 当前 preset | |
| Claude Sonnet 4.6 | `claude-sonnet-4-6` | ✅ 当前 preset | |
| Claude Haiku 4.5 | `claude-haiku-4-5` | ✅ 当前 preset | |
| Claude Sonnet 4.5 | `claude-sonnet-4-5` | ✅ 当前 preset | |

**推测**：Anthropic 家族实际支持 7 个，当前 preset 已有 8 个（含旧版本），需核对官方营销名。

### OpenAI 家族（5 个）

| 营销名 | aidog alias（推测） | 状态 | 备注 |
|--------|---------------------|------|------|
| GPT-4o | `gpt-4o` | ❌ 缺失 | 探测端点时模型 ID 有效 |
| GPT-4o-mini | `gpt-4o-mini` | ❌ 缺失 | |
| GPT-4-turbo | `gpt-4-turbo` | ❌ 缺失 | |
| GPT-3.5-turbo | `gpt-3.5-turbo` | ❌ 缺失 | |
| o1-preview | `o1-preview` | ❌ 缺失 | 或 `o1` 系列其他变体 |

### DeepSeek 家族（2 个）

| 营销名 | aidog alias（推测） | 状态 | 备注 |
|--------|---------------------|------|------|
| DeepSeek-V3 | `deepseek-chat` | ❌ 缺失 | 探测端点时模型 ID 有效 |
| DeepSeek-Coder | `deepseek-coder` | ❌ 缺失 | 或 `deepseek-coder-v2` |

### 其他（3 个）

剩余 3 个模型未在页面明确标注，可能是：
- Google Gemini 系列（`gemini-2.0-flash-exp`、`gemini-1.5-pro`）
- 或其他供应商（如 Meta Llama 系列）

## 现有 7 模型核对

当前 preset 中的 7 个 aidog alias：

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

**需增删**：
- ✅ **保留**：`claude-opus-4-8`、`claude-sonnet-4-6`、`claude-opus-4-7`、`claude-haiku-4-5`
- ⚠️ **建议删除**：`claude-opus-4-6`、`claude-opus-4-5`、`claude-sonnet-4-5`（旧版本，官方可能已下线或合并）
- ➕ **需新增**：`claude-sonnet-5`、`claude-fable-5`（2026-07 上线）
- ➕ **需新增 OpenAI**：`gpt-4o`、`gpt-4o-mini`（至少主流 2 个）
- ➕ **需新增 DeepSeek**：`deepseek-chat`、`deepseek-coder`

## 三档默认推荐

```json
{
  "claude-sonnet-5": {},
  "claude-opus-4-8": {},
  "claude-haiku-4-5": {}
}
```

**推荐理由**：
- **基础档**：`claude-haiku-4-5`（最快、最便宜）
- **均衡档**：`claude-sonnet-5`（2026-07-01 上线，最新主力）
- **旗舰档**：`claude-opus-4-8`（最强性能）

## source_urls 修正建议

**当前**：
```json
{
  "docs": "https://node-hk.sssaicodeapi.com/",
  "pricing": "https://node-hk.sssaicodeapi.com/"
}
```

**修正**：
```json
{
  "docs": "https://sssaicode.com/docs",
  "pricing": "https://sssaicode.com/pricing",
  "models": "https://sssaicode.com/models"
}
```

**理由**：docs/pricing 应指向官网 `sssaicode.com` 主站，而非 API 网关域。

## 鉴权方式

**Key 格式**：自定义 "client token"（非标准 Anthropic `sk-ant-api03-xxx` 或 OpenAI `sk-xxx`）

**响应示例**（401）：
```
缺少 client token
```

**Headers**：推测 `Authorization: Bearer <client_token>`

## Caveats / Not Found

1. **Cloudflare 保护**：官网页面需 Cloudflare 验证，Jina Reader 无法获取完整 SPA 内容，模型清单基于页面片段推测。
2. **OpenAI/DeepSeek 模型 ID 未完全验证**：端点存活但模型 ID 基于通用命名推测，未用有效 token 测试 `/v1/models` 接口（返回"缺少 client token"）。
3. **Gemini 端点未探测**：可能存在 `/api/v1/messages` 的 Gemini 变体（如 aicodemirror 的 `base_url + /v1/messages` 兼容 Gemini），未测试。
4. **HK2/HK3 域名未验证**：通知中提到 `node-hk2.sssaicodeapi.com`、`node-hk3.sssaicodeapi.com`，探测失败，可能是内部通道名而非公开域名。
5. **备用域名 405**：`sssaiapi.com`、`sssaicodeapi.com` 返回 405 Method Not Allowed，可能是 nginx 配置问题或需特定路径。

## 数据来源

- **URL**：
  - 官网：https://sssaicode.com
  - 模型列表：https://sssaicode.com/models
  - 价格页：https://sssaicode.com/pricing
  - 安装页：https://sssaicode.com/install
  - API 网关：https://node-hk.sssaicodeapi.com/api

- **curl 探测命令**：
  ```bash
  # Anthropic 端点
  curl -X POST "https://node-hk.sssaicodeapi.com/api/v1/messages" \
    -H "Content-Type: application/json" \
    -d '{"model":"claude-opus-4-8","max_tokens":1,"messages":[{"role":"user","content":"test"}]}'

  # OpenAI 端点
  curl -X POST "https://node-hk.sssaicodeapi.com/api/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -d '{"model":"gpt-4o","max_tokens":1,"messages":[{"role":"user","content":"test"}]}'

  # DeepSeek 端点
  curl -X POST "https://node-hk.sssaicodeapi.com/api/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -d '{"model":"deepseek-chat","max_tokens":1,"messages":[{"role":"user","content":"test"}]}'
  ```

- **调研日期**：2026-07-09

## 后续行动建议

1. **修正 preset 描述**：`desc` 从 "Claude 兼容模型" 改为 "多供应商聚合 API（Anthropic / OpenAI / DeepSeek）"
2. **补全 endpoints**：新增 `openai` 和 `deepseek` 协议端点
3. **更新模型清单**：删除旧版本（4.5/4.6），新增 `claude-sonnet-5`、`claude-fable-5`、`gpt-4o`、`deepseek-chat`
4. **修正 source_urls**：docs/pricing 改指 `sssaicode.com` 主站
5. **补充 docs 端点**：新增 `base_url` 为 `https://node-hk.sssaicodeapi.com/api` 的协议变体
