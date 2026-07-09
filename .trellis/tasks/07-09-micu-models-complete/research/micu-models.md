# Research: Micu (micuapi.ai) 全量模型调研

- **Query**: Micu (micuapi.ai) 平台定位、endpoints、模型清单、鉴权方式
- **Scope**: 外部调研（micuapi.ai 官网 + docs + curl 探测）
- **Date**: 2026-07-09

## TL;DR / 结论速览

| 项目 | 结论 | 证据 |
|------|------|------|
| **平台定位** | **多供应商聚合平台**（非纯 Claude 代理） | 支持 Claude + GPT-5.x + Gemini + Grok + 国产，13 个令牌分组 |
| **Endpoint 存活** | 4/4 存活（anthropic/openai/models/gemini） | curl 探测 401 统一返回格式 |
| **鉴权方式** | `Authorization: Bearer sk-xxx` | 401 报错格式 |
| **模型范围** | 5 大类：Claude 4.x + GPT-5.x + Gemini 2.5/3/3.1 + Grok 4.20 + 国产（GLM/DeepSeek/Kimi/Qwen/MiniMax） | 官方文档令牌分组速查 |
| **id 格式** | 裸 id（无 provider 前缀） | 官网配置示例 |
| **现有 7 模型** | 仅 Claude alias，**缺 GPT/Gemini/Grok/国产** | 当前 preset 对比 |

## API Endpoints

### curl 探测结果

| 协议 | Base URL | 探测路径 | 响应状态 | 鉴权方式 | 401 报错格式 |
|------|----------|----------|----------|----------|--------------|
| anthropic | `https://www.micuapi.ai` | `POST /v1/messages` | 401 | Bearer sk-xxx | `{"error":{"code":"","message":"Invalid token (request id: ...)","type":"new_api_error"}}` |
| openai | `https://www.micuapi.ai` | `POST /v1/chat/completions` | 401 | Bearer sk-xxx | 同上 |
| models | `https://www.micuapi.ai` | `GET /v1/models` | 401 | Bearer sk-xxx | 同上 |
| gemini | `https://www.micuapi.ai` | `GET /v1beta/models` | 401 | Bearer sk-xxx | 同上 |

### 探测命令

```bash
# anthropic
curl -s -X POST "https://www.micuapi.ai/v1/messages" \
  -H "Authorization: Bearer test-key-123" \
  -H "User-Agent: claude-cli/2.0.76 (external, cli)" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-opus-4-8","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}'

# openai
curl -s -X POST "https://www.micuapi.ai/v1/chat/completions" \
  -H "Authorization: Bearer test-key-123" \
  -H "User-Agent: codex_cli_rs/0.77.0" \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-5.5","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}'

# models list
curl -s "https://www.micuapi.ai/v1/models" \
  -H "Authorization: Bearer test-key-123"

# gemini
curl -s "https://www.micuapi.ai/v1beta/models" \
  -H "Authorization: Bearer test-key-123"
```

### 系统特征

返回格式 `{"error":{"type":"new_api_error"}}` 是典型的 **new-api / one-api 系统**特征，非原生 Anthropic/OpenAI 端点。

## 模型范围确认

### 官网自我定位

- 首页 tagline：「米醋工作室出品 · Claude Code · Codex CLI · OpenClaw 中文配置手册」
- 核心服务：「令牌分组速查」列 13 个分组，覆盖 5 大模型家族
- 不限制 Claude，支持多供应商聚合

### 13 个令牌分组（按用途分类）

| 分组 | 说明 | 可用模型 | model 填写建议 |
|------|------|----------|----------------|
| **Claude — 官方/号池** ||||
| `default` | 富哥分组（路由到 vip_1_max_enterprise） | Claude 全系 | 不建议手动选择 |
| `vip_1_api_mix` | Claude AWS + Azure API 混合 | Claude Opus / Sonnet / Haiku 4.x | 留空 |
| `vip_1_max_cheap` | ClaudeMAX 号池（仅 CC 内部，限时） | Claude Opus / Sonnet / Haiku 4.x | 留空 |
| `vip_1_max_enterprise` | ClaudeMAX 号池（仅 CC 内部） | Claude Opus / Sonnet / Haiku 4.x | 留空 |
| `vip_1_max_ext` | ClaudeMAX 号池，可外接 | Claude Opus / Sonnet / Haiku 4.x（含 `-thinking`） | 留空或填 Claude 模型名 |
| **Claude — 逆向** ||||
| `free_2` | AWSQ（Kiro）逆向 Claude，**不支持 Thinking** | Claude Opus / Sonnet / Haiku 4.x | 留空 |
| **Codex / OpenAI** ||||
| `vip_2` | Codex Pro 号池 | `gpt-5.5` / `gpt-5.4` / `gpt-5.4-mini` / `gpt-5.3-codex-spark` / `codex-auto-review` | 推荐 `gpt-5.5` |
| `vip_2_cc` | Codex → ClaudeCode（CC 调 OAI） | 同 vip_2 + `gpt-5.3-codex` / `gpt-5.2` | **必须改为 GPT 模型**（如 `gpt-5.5`） |
| `vip_2_remap` | 临时重映射（gpt-5.4 → gpt-5.5） | `gpt-5.4` → `gpt-5.5` | 填 `gpt-5.4` 自动映射 |
| `vip_2_image` | GPT Image 图像生成 | `gpt-image-2` / `gpt-image-2-pro` | 按图像模型 ID 填写 |
| **Gemini / Grok** ||||
| `vip_3` | Gemini API | Gemini 2.5 / 3 / 3.1 全系（含 image/tts/veo），**40+ 个** | 按模型广场中的 Gemini ID 填写 |
| `grok` | Grok 专用 | Grok 4.20 全系 + `grok-imagine` 图像/视频 | 按模型广场中的 Grok ID 填写 |
| **国产** ||||
| `vip_4` | 国产模型混合 | GLM-5.x / DeepSeek-V4 / Kimi-K2.x / Qwen3.x / MiniMax-M2.7·M3 | 按模型广场中的对应 ID 填写 |

### User-Agent 要求（外接调用）

| 场景 | 推荐分组 | User-Agent 类型 |
|------|----------|-----------------|
| Claude 外接 | `vip_1_max_ext` / `free_2` | `claude-cli/2.0.76 (external, cli)` |
| Codex 外接 | `vip_2` | `codex_cli_rs/0.77.0 ...` |
| 国产模型外接 | `vip_4` | 浏览器型 UA（`Mozilla/5.0 ...`） |

## 全量模型清单

### Claude 4.x 系列（确认）

| 营销名 | aidog alias | 状态 |
|--------|-------------|------|
| Claude Opus 4.8 | `claude-opus-4-8` | ✅ 现有 |
| Claude Sonnet 4.6 | `claude-sonnet-4-6` | ✅ 现有 |
| Claude Haiku 4.5 | `claude-haiku-4-5` | ✅ 现有 |
| Claude Opus 4.7 | `claude-opus-4-7` | ✅ 现有 |
| Claude Opus 4.6 | `claude-opus-4-6` | ✅ 现有 |
| Claude Opus 4.5 | `claude-opus-4-5` | ✅ 现有 |
| Claude Sonnet 4.5 | `claude-sonnet-4-5` | ✅ 现有 |
| Claude Thinking 变体（含 `-thinking` 后缀） | 待确认 | ⚠️ vip_1_max_ext 支持 |

### GPT-5.x 系列（缺）

| 营销名 | aidog alias | 状态 |
|--------|-------------|------|
| GPT 5.5 | `gpt-5.5` | ❌ 缺失 |
| GPT 5.4 | `gpt-5.4` | ❌ 缺失 |
| GPT 5.4 Mini | `gpt-5.4-mini` | ❌ 缺失 |
| GPT 5.3 Codex Spark | `gpt-5.3-codex-spark` | ❌ 缺失 |
| Codex Auto Review | `codex-auto-review` | ❌ 缺失 |
| GPT Image 2 | `gpt-image-2` | ❌ 缺失 |
| GPT Image 2 Pro | `gpt-image-2-pro` | ❌ 缺失 |

### Gemini 系列（缺）

- Gemini 2.5 / 3 / 3.1 全系（**40+ 个**模型）
- 具体模型 ID 参考「模型广场」（需 dashboard 登录查看）

### Grok 系列（缺）

- Grok 4.20 全系
- `grok-imagine` 图像/视频

### 国产模型系列（缺）

- GLM-5.x
- DeepSeek-V4
- Kimi-K2.x
- Qwen3.x
- MiniMax-M2.7·M3

## 现有 7 模型核对

| aidog alias | 状态 | 备注 |
|-------------|------|------|
| `claude-opus-4-8` | ✅ 保留 | 最新 Opus |
| `claude-sonnet-4-6` | ✅ 保留 | 最新 Sonnet |
| `claude-haiku-4-5` | ✅ 保留 | 最新 Haiku |
| `claude-opus-4-7` | ⚠️ 降级 | 旧版 Opus，可保留作兼容 |
| `claude-opus-4-6` | ⚠️ 降级 | 旧版 Opus，可保留作兼容 |
| `claude-opus-4-5` | ⚠️ 降级 | 旧版 Opus，可保留作兼容 |
| `claude-sonnet-4-5` | ⚠️ 降级 | 旧版 Sonnet，可保留作兼容 |

**建议**：7 个 alias 全部保留（向后兼容），但需补充 GPT/Gemini/Grok/国产模型。

## 三档默认推荐

**aidog 约定**：`models.default` 用模型 id 直接作 key，值为空对象 `{}`。

```json
{
  "claude-sonnet-4-6": {},
  "claude-opus-4-8": {},
  "claude-haiku-4-5": {}
}
```

**可选 GPT 档**（若需补充）：
```json
{
  "gpt-5.5": {},
  "gpt-5.4": {},
  "gpt-5.4-mini": {}
}
```

## id 格式判定

- **裸 id**（无 provider 前缀）：`claude-opus-4-8` / `gpt-5.5`
- 非 `provider/model` 格式（如 `openai/gpt-5.5`）

## Base URL 变体

| 用途 | Base URL |
|------|----------|
| Claude Code / OpenClaw (Anthropic Messages) | `https://www.micuapi.ai` |
| Codex CLI (OpenAI Responses) | `https://www.micuapi.ai/v1` |

**Codex 的 Base URL 需要 `/v1` 后缀**，与 Claude Code 不同。

## 当前 preset 现状

```json
{
  "protocols": {
    "micu": {
      "desc": "Micu API, Claude 兼容模型",
      "source_urls": {
        "docs": "https://docs.micuapi.ai/",
        "pricing": "https://www.micuapi.ai/pricing"
      },
      "endpoints": {
        "default": [
          {
            "protocol": "anthropic",
            "base_url": "https://www.micuapi.ai",
            "client_type": "claude_code"
          }
        ]
      },
      "models": {
        "default": {}
      },
      "model_list": {
        "default": [
          "claude-opus-4-8",
          "claude-sonnet-4-6",
          "claude-haiku-4-5",
          "claude-opus-4-7",
          "claude-opus-4-6",
          "claude-opus-4-5",
          "claude-sonnet-4-5"
        ]
      }
    }
  }
}
```

### 需修改点

1. **desc**：「Micu API, Claude 兼容模型」→ 改为「米醋 API — 多供应商聚合平台（Claude + GPT-5.x + Gemini + Grok + 国产）」
2. **endpoints**：可补充 openai/gemini 协议端点（可选）
3. **models.default**：补充三档推荐
4. **model_list.default**：补充 GPT/Gemini/Grok/国产模型（若需要）

## Caveats / Not Found

1. **模型广场需登录**：`https://www.micuapi.ai/pricing` 页面是动态渲染的，完整模型清单需 dashboard 登录。本调研基于官方文档「令牌分组速查」列出的可用模型概览。
2. **Gemini 具体模型 ID**：文档仅说「Gemini 2.5 / 3 / 3.1 全系，40+ 个」，未列完整 id。如需精确清单，需访问模型广场或尝试 `/v1/models` 端点（需有效 API Key）。
3. **国产模型具体版本**：GLM-5.x / DeepSeek-V4 / Kimi-K2.x / Qwen3.x / MiniMax-M2.7·M3 为家族名，具体子版本需查阅模型广场。
4. **现有 7 模型版本核对**：claude-opus-4-7/4-6/4-5 / claude-sonnet-4-5 是否仍可用未验证，建议保留作向后兼容。

## 数据来源

| 来源 | URL | 调研日期 |
|------|-----|----------|
| 官网首页 | https://www.micuapi.ai | 2026-07-09 |
| 官方文档 | https://docs.micuapi.ai/ | 2026-07-09 |
| 定价页（模型广场） | https://www.micuapi.ai/pricing | 2026-07-09 |
| 状态监测 | https://www.micuapi.ai/monitoring | 2026-07-09 |
| curl 探测 | 见「API Endpoints」章节 | 2026-07-09 |
