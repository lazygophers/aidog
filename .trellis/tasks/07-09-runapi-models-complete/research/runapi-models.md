# Research: RunAPI (runapi.co) 全量模型调研

- **Query**: 调研 RunAPI (runapi.co) 全量官方信息：① 是否纯 Claude 代理 vs 多供应商聚合 ② endpoints 全量 ③ model_list 全量清单 ④ models.default 三档推荐
- **Scope**: 外部调研（API 探测 + 官网分析）
- **Date**: 2026-07-09

## TL;DR / 结论速览

### 平台定位
**多供应商聚合平台**（非纯 Claude 代理）。官网自我定位为"国内 OpenRouter 替代"，支持 OpenAI、Claude、Gemini、DeepSeek、Grok 等 **204 个模型**，统一 API 接入，智能路由。

### Endpoints 存活表
| 协议 | Base URL | 探测路径 | 响应状态 | 鉴权方式 |
|------|----------|----------|----------|----------|
| anthropic | `https://runapi.co` | `POST /v1/messages` | 401 | Bearer Token |
| openai | `https://runapi.co` | `POST /v1/chat/completions` | 401 | Bearer Token |
| gemini | `https://runapi.co` | `GET /v1beta/models` | 401 | Bearer Token |
| models | `https://runapi.co` | `GET /v1/models` | 401 | Bearer Token |

**结论**：三类端点全部存活（401 = 路由存在但鉴权失败，非 404）。

### 模型范围
- **Claude 系列**：20 个模型（含 Opus 4.6/4.7/4.8、Sonnet 4.6/5、Haiku 4.5、Fable 5 等）
- **OpenAI 系列**：25 个模型（含 GPT-5.x、GPT-4.x 系列）
- **Gemini 系列**：13 个模型（含 2.5/3.x 系列）
- **国产系列**：DeepSeek (13)、Grok (12)、Qwen (12)、GLM (7)、Kimi (4)、MiniMax (5)、Doubao (15)
- **其他**：Flux、Jina、Kling、Veo、Sora 等

### ID 格式判定
**裸 id 格式**（无 `anthropic/` 或 `openai/` 前缀）。部分模型带日期后缀：
- 短 id：`claude-opus-4-8`、`claude-sonnet-4-6`、`claude-haiku-4-5`
- 完整 id：`claude-opus-4-5-20251101`、`claude-sonnet-4-5-20250929`、`claude-haiku-4-5-20251001`

**推测**：API 可能接受短 id 作为 alias（需实际请求验证），或需更新为完整 id。

---

## API Endpoints 核验

### curl 探测结果

```bash
# anthropic 协议端点
$ curl -X POST "https://runapi.co/v1/messages" \
  -H "Authorization: Bearer test" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-opus-4-8","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}'
Status: 401  # 端点存活

# openai 协议端点
$ curl -X POST "https://runapi.co/v1/chat/completions" \
  -H "Authorization: Bearer test" \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}'
Status: 401  # 端点存活

# gemini 协议端点
$ curl -X GET "https://runapi.co/v1beta/models" \
  -H "Authorization: Bearer test"
Status: 401  # 端点存活

# models 列表端点
$ curl -X GET "https://runapi.co/v1/models" \
  -H "Authorization: Bearer test"
Status: 401  # 端点存活
```

### 鉴权方式
- **Authorization Bearer**：所有端点统一使用 `Authorization: Bearer <token>`
- **Key 格式**：推测为 `sk-xxx` 或自定义格式（需注册后确认）

---

## 模型范围确认

### 证据链

#### 1. 官网首页 Meta
```html
<meta name="description" content="RunAPI | 高效稳定的AI模型API中转api供应商，国内OpenRouter替代。
支持OpenAI、Claude、Gemini、DeepSeek等150+模型，统一API接入，智能路由，低至70%折扣。" />
```

#### 2. Schema.org 结构化数据
```json
{
  "description": "支持OpenAI、Claude、Gemini、DeepSeek、Grok等150+模型，统一API接入，智能路由。"
}
```

#### 3. 公开 API `/api/pricing`
返回 204 个模型的完整列表（含 endpoint_types、价格、描述）。

### 按 endpoint_types 分组
| Endpoint Type | 模型数 |
|---------------|--------|
| openai | 150 |
| anthropic | 20 |
| gemini | 13 |
| image-generation | 15 |
| openai-video | 9 |
| kling | 4 |
| minimax | 2 |
| audio-speech | 3 |
| cohere-rerank | 3 |
| jina | 5 |
| suno | 2 |

---

## 全量模型清单

### Claude 系列（20 个）

| 模型 ID | Endpoint Types | 说明 |
|---------|----------------|------|
| claude-fable-5 | anthropic, openai | 最新 Fable 系列 |
| claude-opus-4-8 | openai, anthropic | Opus 4.8 |
| claude-sonnet-5 | openai, anthropic | Sonnet 5 |
| claude-haiku-4-5-20251001 | openai, anthropic | Haiku 4.5（完整 id） |
| claude-opus-4-7 | openai, anthropic | Opus 4.7 |
| claude-opus-4-6 | anthropic, openai | Opus 4.6 |
| claude-sonnet-4-6 | openai, anthropic | Sonnet 4.6 |
| claude-opus-4-5-20251101 | openai, anthropic | Opus 4.5（完整 id） |
| claude-sonnet-4-5-20250929 | openai, anthropic | Sonnet 4.5（完整 id） |
| claude-opus-4-1-20250805 | openai, anthropic | Opus 4.1 |
| claude-opus-4-20250514 | openai, anthropic | Opus 4 |
| claude-sonnet-4-20250514 | openai, anthropic | Sonnet 4 |
| claude-opus-4-6-thinking | anthropic, openai | Opus 4.6 Thinking |
| claude-sonnet-4-6-thinking | openai, anthropic | Sonnet 4.6 Thinking |
| claude-opus-4-5-20251101-thinking | openai, anthropic | Opus 4.5 Thinking |
| claude-sonnet-4-5-20250929-thinking | openai, anthropic | Sonnet 4.5 Thinking |
| claude-haiku-4-5-20251001-thinking | anthropic, openai | Haiku 4.5 Thinking |
| claude-opus-4-1-20250805-thinking | openai, anthropic | Opus 4.1 Thinking |
| claude-opus-4-20250514-thinking | openai, anthropic | Opus 4 Thinking |
| claude-sonnet-4-20250514-thinking | openai, anthropic | Sonnet 4 Thinking |

### OpenAI GPT 系列（25 个，部分）
| 模型 ID | Endpoint Types |
|---------|----------------|
| gpt-5.5 | openai |
| gpt-5.5-pro | openai-response |
| gpt-5.4 | openai, openai-response |
| gpt-5.4-pro | openai, openai-response |
| gpt-5.4-mini | openai, openai-response |
| gpt-5.4-nano | openai, openai-response |
| gpt-5.3-codex | openai |
| gpt-5.2 | openai, openai-response |
| gpt-5.2-pro | openai, openai-response |
| gpt-5.1 | openai, openai-response |
| gpt-5 | openai, openai-response |
| gpt-5-pro | openai-response |
| gpt-5-mini | openai, openai-response |
| gpt-5-nano | openai, openai-response |
| gpt-4.1 | openai, openai-response |
| gpt-4.1-mini | openai, openai-response |
| gpt-4.1-nano | openai, openai-response |
| gpt-4o-mini | openai, openai-response |
| o3 | openai |
| o3-pro | openai |

### Gemini 系列（13 个）
| 模型 ID | Endpoint Types |
|---------|----------------|
| gemini-2.5-pro | openai, gemini |
| gemini-2.5-flash | openai, gemini |
| gemini-2.5-flash-lite | openai, gemini |
| gemini-3.5-flash | gemini, openai |
| gemini-3-flash-preview | gemini, openai |
| gemini-3-pro-preview | openai, gemini |
| gemini-3.1-flash-lite | openai, gemini |
| gemini-3.1-flash-lite-preview | openai, gemini |
| gemini-3.1-pro-preview | openai, gemini |

### 国产系列（重点）
#### DeepSeek（13 个）
deepseek-v3、deepseek-v4-flash、deepseek-r1、deepseek-reasoner、deepseek-chat 等

#### Grok（12 个）
grok-3、grok-4、grok-4.1、grok-4.2、grok-4.3 等

#### Qwen（12 个）
qwen3-plus、qwen3.5-plus、qwen3-coder-plus、qwen3-vl-plus 等

#### GLM（7 个）
glm-4.5、glm-4.6、glm-4.7、glm-5、glm-5.1、glm-5.2 等

#### Kimi（4 个）
kimi-k2、kimi-k2.5、kimi-k2.6、kimi-k2-thinking

#### MiniMax（5 个）
MiniMax-M2.5、MiniMax-M2.7、MiniMax-M3、MiniMax-Hailuo-2.3 等

---

## 三档默认推荐（供 `models.default`）

### Claude 系列（推荐）
| 档位 | 模型 ID | 用途 |
|------|---------|------|
| 主力 | `claude-sonnet-5` | 通用推理、编码、对话（性价比最佳） |
| 重型 | `claude-opus-4-8` | 复杂推理、专业任务 |
| 轻量 | `claude-haiku-4-5-20251001` | 快速响应、简单任务 |

### 备选：混合三档
| 档位 | 模型 ID | 用途 |
|------|---------|------|
| 主力 | `claude-sonnet-5` 或 `gpt-5.4` | 通用任务 |
| 重型 | `claude-opus-4-8` 或 `gpt-5.5-pro` | 高难度任务 |
| 轻量 | `claude-haiku-4-5-20251001` 或 `gpt-5.4-mini` | 快速响应 |

---

## 现有 7 模型核对

### 当前 preset 模型列表
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

### 核对结果
| Preset ID | API 中的对应 ID | 状态 | 建议 |
|-----------|-----------------|------|------|
| claude-opus-4-8 | claude-opus-4-8 | ✓ 存在 | 保留 |
| claude-sonnet-4-6 | claude-sonnet-4-6 | ✓ 存在 | 保留 |
| claude-haiku-4-5 | claude-haiku-4-5-20251001 | ⚠ 短 id | 更新为完整 id 或确认 alias 支持 |
| claude-opus-4-7 | claude-opus-4-7 | ✓ 存在 | 保留 |
| claude-opus-4-6 | claude-opus-4-6 | ✓ 存在 | 保留 |
| claude-opus-4-5 | claude-opus-4-5-20251101 | ⚠ 短 id | 更新为完整 id 或确认 alias 支持 |
| claude-sonnet-4-5 | claude-sonnet-4-5-20250929 | ⚠ 短 id | 更新为完整 id 或确认 alias 支持 |

### 缺失的重要模型
- **claude-fable-5**：最新 Fable 系列，建议添加
- **claude-sonnet-5**：Sonnet 5 是最新主力模型，建议替换 claude-sonnet-4-6

### 建议的新模型列表（10 个）
```json
[
  "claude-opus-4-8",
  "claude-sonnet-5",
  "claude-haiku-4-5-20251001",
  "claude-opus-4-7",
  "claude-opus-4-6",
  "claude-opus-4-5-20251101",
  "claude-sonnet-4-6",
  "claude-sonnet-4-5-20250929",
  "claude-fable-5",
  "claude-sonnet-4-6-thinking"
]
```

---

## Endpoints 配置建议

### 当前 preset（仅 anthropic）
```json
{
  "endpoints": {
    "default": [
      {
        "protocol": "anthropic",
        "base_url": "https://runapi.co",
        "client_type": "claude_code"
      }
    ]
  }
}
```

### 建议的 endpoints（多协议）
```json
{
  "endpoints": {
    "default": [
      {
        "protocol": "anthropic",
        "base_url": "https://runapi.co",
        "client_type": "claude_code"
      },
      {
        "protocol": "openai",
        "base_url": "https://runapi.co",
        "client_type": "default"
      },
      {
        "protocol": "gemini",
        "base_url": "https://runapi.co",
        "client_type": "default"
      }
    ]
  }
}
```

**说明**：
- 所有协议共享同一 base_url（`https://runapi.co`）
- API 通过 `endpoint_types` 字段路由到不同上游
- Claude 模型同时支持 `anthropic` 和 `openai` 端点类型

---

## Desc 更新建议

### 当前 desc
```json
{
  "zh-Hans": "RunAPI 中转, Claude 兼容模型"
}
```

### 建议更新（反映多供应商定位）
```json
{
  "zh-Hans": "RunAPI 中转, 支持 150+ 模型（Claude / GPT / Gemini / DeepSeek / Grok 等）",
  "en-US": "RunAPI proxy for 150+ models (Claude / GPT / Gemini / DeepSeek / Grok, etc.)"
}
```

---

## Caveats / Not Found

### Dashboard 需登录
- 官网 docs 和 pricing 页面为 SPA，内容由 JS 动态加载
- 未找到免鉴权的方式获取用户级配置
- `/api/pricing` 为公开 API，无需登录

### ID 格式不确定性
- **推测**：API 可能接受短 id 作为 alias（如 `claude-haiku-4-5`）
- **需验证**：实际请求时短 id 是否有效
- **备选**：如不支持，需更新 preset 为完整 id（带日期后缀）

### 其他发现
- 无 `/api/models` 端点（只 `/api/pricing` 返回模型列表）
- `/v1/models` 端点存在但需鉴权（推测返回动态模型列表）
- 无 coding_plan 专用端点（基于 `/api/pricing` 数据）

---

## 数据来源

### URL 列表
- **首页**：https://runapi.co
- **Docs**：https://runapi.co/docs（SPA）
- **Pricing**：https://runapi.co/pricing（SPA）
- **公开 API**：https://runapi.co/api/pricing ✅（无需鉴权）

### curl 探测命令
```bash
# 端点存活探测
curl -X POST "https://runapi.co/v1/messages" -H "Authorization: Bearer test"
curl -X POST "https://runapi.co/v1/chat/completions" -H "Authorization: Bearer test"
curl -X GET "https://runapi.co/v1beta/models" -H "Authorization: Bearer test"

# 获取完整模型列表
curl -s "https://runapi.co/api/pricing" | python3 -m json.tool
```

### 调研日期
2026-07-09

---

## 对当前 preset 的修改建议

### 1. 更新 desc
```diff
- "zh-Hans": "RunAPI 中转, Claude 兼容模型"
+ "zh-Hans": "RunAPI 中转, 支持 150+ 模型（Claude / GPT / Gemini / DeepSeek / Grok 等）"
```

### 2. 添加多协议 endpoints
```diff
{
  "endpoints": {
    "default": [
+     {
+       "protocol": "openai",
+       "base_url": "https://runapi.co",
+       "client_type": "default"
+     },
+     {
+       "protocol": "gemini",
+       "base_url": "https://runapi.co",
+       "client_type": "default"
+     },
      {
        "protocol": "anthropic",
        "base_url": "https://runapi.co",
        "client_type": "claude_code"
      }
    ]
  }
}
```

### 3. 更新 model_list（完整 id）
```diff
{
  "model_list": {
    "default": [
-     "claude-opus-4-8",
-     "claude-sonnet-4-6",
-     "claude-haiku-4-5",
-     "claude-opus-4-7",
-     "claude-opus-4-6",
-     "claude-opus-4-5",
-     "claude-sonnet-4-5"
+     "claude-opus-4-8",
+     "claude-sonnet-5",
+     "claude-haiku-4-5-20251001",
+     "claude-opus-4-7",
+     "claude-opus-4-6",
+     "claude-opus-4-5-20251101",
+     "claude-sonnet-4-6",
+     "claude-sonnet-4-5-20250929",
+     "claude-fable-5",
+     "claude-sonnet-4-6-thinking"
    ]
  }
}
```

### 4. 添加 models.default（三档推荐）
```diff
{
  "models": {
-   "default": {}
+   "default": {
+     "default": "claude-sonnet-5",
+     "opus": "claude-opus-4-8",
+     "haiku": "claude-haiku-4-5-20251001"
+   }
  }
}
```

---

## 总结

RunAPI 是多供应商聚合平台（非纯 Claude 代理），支持 204 个模型，涵盖 Claude、GPT、Gemini、DeepSeek、Grok、Qwen、GLM、Kimi、MiniMax 等主流家族。当前 preset 需要更新 desc 反映多供应商定位，建议添加 openai/gemini endpoints，更新 model_list 为 API 返回的完整 id 格式，并添加 models.default 三档推荐。
