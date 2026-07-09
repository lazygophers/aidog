# Research: Xiaomi MiMo 模型与 API 端点

- **Query**: 小米 MiMo（xiaomi_mimo 协议）官方信息研究
- **Scope**: 外部调研（官方文档 + GitHub 社区验证）
- **Date**: 2026-07-09

## 官方信息源

- **官方文档**: https://platform.xiaomimimo.com/docs
- **模型列表**: https://platform.xiaomimimo.com/docs/en-US/quick-start/summary/model
- **API 参考**: https://platform.xiaomimimo.com/docs/en-US/api/guidance/rate-limit
- **官方主页**: https://mimo.xiaomi.com
- **开放平台**: https://platform.xiaomimimo.com
- **AI Studio**: https://aistudio.xiaomimimo.com

## 现状核实（preset 各字段对照官方）

### 模型列表 (model_list.default)

**当前 preset 配置**:
```json
["mimo-v2.5-pro", "mimo-v2-pro", "mimo-v2.5", "mimo-v2-omni", "mimo-v2-flash"]
```

**官方文档（文本生成模型）**:
| Model ID | 能力支持 | 状态 |
|----------|----------|------|
| `mimo-v2.5-pro` | 文本生成、深度思考、流式输出、Function Call、结构化输出、Web Search | **在售** |
| `mimo-v2.5` | 文本生成、全模态理解、深度思考、流式输出、Function Call、结构化输出、Web Search | **在售** |
| `mimo-v2-pro` | - | **已弃用** (2026-06-30) |
| `mimo-v2-omni` | - | **已弃用** (2026-06-30) |
| `mimo-v2-flash` | - | **已弃用** (2026-06-30) |

**官方弃用公告**:
> `mimo-v2-pro`, `mimo-v2-omni`, `mimo-v2-flash` 和 `mimo-v2-tts` **已在 2026 年 6 月 30 日正式弃用**。请尽快切换到新模型。
> 来源: https://platform.xiaomimimo.com/docs/en-US/quick-start/summary/model

### 默认模型 (models.default.default)

**当前 preset**: `mimo-v2.5-pro`
**官方推荐**: 复杂推理、深度分析、长文档处理场景推荐 `mimo-v2.5-pro`
**结论**: ✅ 配置正确

### API 端点 (endpoints.default)

**当前 preset 配置**:
```json
[
  {"protocol": "anthropic", "base_url": "https://api.xiaomimimo.com/anthropic", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://api.xiaomimimo.com/v1", "client_type": "codex_tui"}
]
```

**官方文档（First API Call）**:

**Pay-as-you-go（按量付费）**:
| 用途 | BASE_URL | 认证 |
|------|----------|------|
| OpenAI 兼容 | `https://api.xiaomimimo.com/v1` | API Key (`sk-xxxxx`) |
| Anthropic 兼容 | `https://api.xiaomimimo.com/anthropic` | API Key (`sk-xxxxx`) |

**Token Plan（订阅计划）**:
| 用途 | BASE_URL | 认证 |
|------|----------|------|
| OpenAI 兼容 | `https://token-plan-cn.xiaomimimo.com/v1` | API Key (`tp-xxxxx`) |
| Anthropic 兼容 | `https://token-plan-cn.xiaomimimo.com/anthropic` | API Key (`tp-xxxxx`) |

来源: https://platform.xiaomimimo.com/docs/en-US/quick-start/summary/first-api-call

**结论**: ✅ 按 Pay-as-you-go 模式，端点配置正确

### 认证方式

**官方文档**:
- **Pay-as-you-go**: API Key 格式 `sk-xxxxx`，从 [API Keys](https://platform.xiaomimimo.com/#/console/api-keys) 创建
- **Token Plan**: API Key 格式 `tp-xxxxx`，订阅后在 [Token Plan](https://platform.xiaomimimo.com/#/console/plan-manage) 获取

**请求头示例**:
```bash
# OpenAI 格式
curl 'https://api.xiaomimimo.com/v1/chat/completions' \
  --header "api-key: $MIMO_API_KEY"

# Anthropic 格式
curl 'https://api.xiaomimimo.com/anthropic/v1/messages' \
  --header "api-key: $MIMO_API_KEY"
```

**结论**: 使用 `api-key` 请求头（非 `Authorization`）

## model_list 补全建议

### 需删除项（已弃用）

| 模型 ID | 删除理由 | 官方文档引用 |
|---------|----------|---------------|
| `mimo-v2-pro` | 2026-06-30 已正式弃用 | https://platform.xiaomimimo.com/docs/en-US/quick-start/summary/model |
| `mimo-v2-omni` | 2026-06-30 已正式弃用 | https://platform.xiaomimimo.com/docs/en-US/quick-start/summary/model |
| `mimo-v2-flash` | 2026-06-30 已正式弃用 | https://platform.xiaomimimo.com/docs/en-US/quick-start/summary/model |

### 需新增项（可选）

| 模型 ID | 用途 | 是否建议新增 | 理由 |
|---------|------|--------------|------|
| `mimo-v2.5-asr` | 语音识别 | ❌ | ASR 专用，非对话模型 |
| `mimo-v2.5-tts` | 语音合成 | ❌ | TTS 专用，非对话模型 |
| `mimo-v2.5-tts-voiceclone` | 语音克隆 | ❌ | TTS 专用，非对话模型 |
| `mimo-v2.5-tts-voicedesign` | 音色设计 | ❌ | TTS 专用，非对话模型 |
| `mimo-v2.5-pro-ultraspeed` | 超速版（内测） | ❌ | 内测阶段，未公开 |

### 建议的 model_list.default

```json
["mimo-v2.5-pro", "mimo-v2.5"]
```

**理由**:
- 仅保留官方在售的文本生成模型
- 移除已弃用的 V2 系列
- ASR/TTS 模型非对话用途，不列入

## endpoints 核实

### 路径正确性

| 端点 | 当前 preset | 官方文档 | 结论 |
|------|------------|----------|------|
| Anthropic 兼容 | `/anthropic` | `/anthropic` | ✅ 正确 |
| OpenAI 兼容 | `/v1` | `/v1` | ✅ 正确 |

### 域名变体

**存在两套域名**:

1. **按量付费（默认）**:
   - `api.xiaomimimo.com` - 当前 preset 使用

2. **Token Plan（订阅）**:
   - `token-plan-cn.xiaomimimo.com` - Token Plan 专属域名

**结论**: 当前 preset 配置的 `api.xiaomimimo.com` 正确（按量付费模式）。如需支持 Token Plan，可考虑新增分支，但通常用户使用按量付费的 API Key 即可。

**无国际版域名**: 官方文档仅显示国内域名（`token-plan-cn.xiaomimimo.com` 中的 `-cn`），未发现类似 `mimo.xiaomi.com` 国际版域名。

## models.default 建议

**当前配置**: `mimo-v2.5-pro`
**官方推荐**: "复杂推理、深度分析、长文档处理" → `mimo-v2.5-pro`

**结论**: ✅ 配置正确，无需修改

## 认证方式

| 项目 | 说明 |
|------|------|
| **认证方式** | API Key |
| **请求头** | `api-key: <key>` （非 `Authorization`） |
| **Key 格式** | `sk-xxxxx`（按量付费）或 `tp-xxxxx`（Token Plan） |
| **获取途径** | https://platform.xiaomimimo.com/#/console/api-keys |

## 社区验证（GitHub）

从 GitHub 项目 [rong6/mimo-2api](https://github.com/rong6/mimo-2api) 获取的模型列表与官方文档一致：

| ID | 说明 |
|------|------|
| mimo-v2.5-pro | 开源性能旗舰模型 |
| mimo-v2.5 | 全模态理解大模型 |

该项目将 Xiaomi MiMo Studio 网页端 API 转为 OpenAI 格式，侧面验证了端点的正确性。

## 结论摘要

**一句话给 PRD 用**:
> Xiaomi MiMo 官方已弃用 V2 系列（v2-pro/v2-omni/v2-flash），需移除；当前端点配置正确，保留 `mimo-v2.5-pro` 和 `mimo-v2.5` 即可。

**核心改动**:
1. **model_list.default**: 移除已弃用的 `mimo-v2-pro`, `mimo-v2-omni`, `mimo-v2-flash`
2. **endpoints**: 无需修改（已正确）
3. **models.default**: 无需修改（`mimo-v2.5-pro` 正确）

**修改后的 model_list.default**:
```json
["mimo-v2.5-pro", "mimo-v2.5"]
```
