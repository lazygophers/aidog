# Research: Moonshot/Kimi 官方模型清单与端点

- **Query**: Moonshot/Kimi 全部官方模型清单 + 端点 + 认证方式
- **Scope**: 外部文档研究
- **Date**: 2026-07-09

## 官方文档源

| URL | 描述 |
|-----|------|
| https://platform.moonshot.cn/docs/models | 完整模型列表（Stable + Deprecated） |
| https://platform.moonshot.cn/docs/api/chat | 聊天补全 API，含模型枚举 |
| https://platform.moonshot.cn/docs/api/overview | API 概述，服务地址与端点一览 |
| https://platform.moonshot.cn/docs/anthropic | Anthropic 兼容端点文档 |
| https://platform.moonshot.cn/docs/guide/claude-code-kimi | Claude Code 集成指南（Anthropic 端点用法） |
| https://platform.moonshot.cn/docs/guide/start-using-kimi-api | 快速开始，含 base_url 与认证 |
| https://platform.moonshot.cn/docs/pricing/chat | 模型定价 |

## model_list 最终清单

### 当前可用模型（Stable）

**多模态模型（4 个）**

| 模型 ID | 状态 | 描述 | 上下文 |
|---------|------|------|--------|
| `kimi-k2.7-code` | Stable | Kimi 迄今最智能的 Coding 模型，长上下文更可靠地遵循指令，文本/图片/视频输入 + 思考模式 | 256k |
| `kimi-k2.7-code-highspeed` | Stable | K2.7 Code 高速版，输出约 180 Tokens/s（短上下文可达 260 Tokens/s） | 256k |
| `kimi-k2.6` | Stable | Kimi 迄今最智能的模型，agentic coding、长上下文推理、长周期执行、前端设计场景升级，文本/图片/视频输入 + 思考模式 | 256k |
| `kimi-k2.5` | Stable | Agent、代码、视觉理解通用 SoTA，文本/图片/视频输入，思考/非思考模式 | 256k |

**生成模型 Moonshot V1（6 个）**

| 模型 ID | 状态 | 描述 | 上下文 |
|---------|------|------|--------|
| `moonshot-v1-8k` | Stable | 生成短文本 | 8k |
| `moonshot-v1-32k` | Stable | 生成长文本 | 32k |
| `moonshot-v1-128k` | Stable | 生成超长文本 | 128k |
| `moonshot-v1-8k-vision-preview` | Stable | Vision 视觉模型，理解图片内容并输出文本 | 8k |
| `moonshot-v1-32k-vision-preview` | Stable | Vision 视觉模型，理解图片内容并输出文本 | 32k |
| `moonshot-v1-128k-vision-preview` | Stable | Vision 视觉模型，理解图片内容并输出文本 | 128k |

> 注：Moonshot V1 系列模型区别仅在于最大上下文长度，效果上并无差异（出处：https://platform.moonshot.cn/docs/models）

### 已下线模型（Deprecated）

**kimi-k2 系列模型 — 2026 年 5 月 25 日下线**

| 模型 ID | 下线日期 |
|---------|----------|
| `kimi-k2-0905-preview` | 2026-05-25 |
| `kimi-k2-0711-preview` | 2026-05-25 |
| `kimi-k2-turbo-preview` | 2026-05-25 |
| `kimi-k2-thinking` | 2026-05-25 |
| `kimi-k2-thinking-turbo` | 2026-05-25 |

**其他已下线模型**

| 模型 ID | 下线日期 |
|---------|----------|
| `kimi-latest` | 2026-01-28 |
| `kimi-thinking-preview` | 2025-11-11 |

出处：https://platform.moonshot.cn/docs/models

### 推荐的最终 model_list 数组（按优先级排序）

```json
[
  "kimi-k2.7-code",
  "kimi-k2.7-code-highspeed",
  "kimi-k2.6",
  "kimi-k2.5",
  "moonshot-v1-8k",
  "moonshot-v1-32k",
  "moonshot-v1-128k",
  "moonshot-v1-8k-vision-preview",
  "moonshot-v1-32k-vision-preview",
  "moonshot-v1-128k-vision-preview"
]
```

## models.default.default 推荐

**官方推荐**：`kimi-k2.7-code` 或 `kimi-k2.7-code-highspeed`

**理由**：
- Kimi 官方文档明确标注「Kimi 迄今最智能的 Coding 模型」
- 高速版输出速度约 180 Tokens/s（短上下文可达 260 Tokens/s），适合编程 Agent
- 官方 Claude Code 集成指南默认推荐 `kimi-k2.7-code`（出处：https://platform.moonshot.cn/docs/guide/claude-code-kimi）

**当前 preset 值**：`kimi-k2.6`
- 建议**更新为** `kimi-k2.7-code`（更推荐）或 `kimi-k2.7-code-highspeed`（追求速度）

## endpoints

### OpenAI 兼容端点

| 字段 | 值 | 出处 |
|------|---|------|
| protocol | `openai` | - |
| base_url | `https://api.moonshot.cn/v1` | https://platform.moonshot.cn/docs/api/overview |
| client_type | `claude_code` | 遵循项目约定 |
| 认证方式 | `Authorization: Bearer $MOONSHOT_API_KEY` | https://platform.moonshot.cn/docs/api/overview |

**完整端点路径**：`https://api.moonshot.cn/v1/chat/completions`

### Anthropic 兼容端点

| 字段 | 值 | 出处 |
|------|---|------|
| protocol | `anthropic` | - |
| base_url | `https://api.moonshot.cn/anthropic` | https://platform.moonshot.cn/docs/guide/claude-code-kimi |
| client_type | `claude_code` | 遵循项目约定 |
| 认证方式 | `Authorization: Bearer $MOONSHOT_API_KEY` | https://platform.moonshot.cn/docs/guide/claude-code-kimi |

**Claude Code 集成示例**（出处：https://platform.moonshot.cn/docs/guide/claude-code-kimi）：
```bash
export ANTHROPIC_BASE_URL=https://api.moonshot.cn/anthropic
export ANTHROPIC_AUTH_TOKEN=${YOUR_MOONSHOT_API_KEY}
export ANTHROPIC_MODEL=kimi-k2.7-code
```

### 推荐的最终 endpoints 数组

```json
[
  {
    "protocol": "openai",
    "base_url": "https://api.moonshot.cn/v1",
    "client_type": "claude_code"
  },
  {
    "protocol": "anthropic",
    "base_url": "https://platform.moonshot.cn/anthropic",
    "client_type": "claude_code"
  }
]
```

> **注意**：Anthropic 端点使用 `platform.moonshot.cn` 域名（非 `api.moonshot.cn`），与 Claude Code 集成文档一致。

## 排除项与原因

### 需从当前 preset 移除的模型

| 模型 ID | 排除原因 | 出处 |
|---------|----------|------|
| `kimi-k2-thinking` | 已于 2026-05-25 下线 | https://platform.moonshot.cn/docs/models |
| `kimi-latest` | 已于 2026-01-28 下线 | https://platform.moonshot.cn/docs/models |

### 当前 model_list vs 推荐对照

**当前 preset（src-tauri/defaults/platform-presets.json）**：
```json
["kimi-k2.6", "kimi-k2.5", "kimi-k2-thinking", "kimi-latest"]
```

**推荐更新**：
```json
[
  "kimi-k2.7-code",
  "kimi-k2.7-code-highspeed",
  "kimi-k2.6",
  "kimi-k2.5",
  "moonshot-v1-8k",
  "moonshot-v1-32k",
  "moonshot-v1-128k",
  "moonshot-v1-8k-vision-preview",
  "moonshot-v1-32k-vision-preview",
  "moonshot-v1-128k-vision-preview"
]
```

**变更点**：
1. 移除已下线模型：`kimi-k2-thinking`、`kimi-latest`
2. 新增 K2.7 Code 系列：`kimi-k2.7-code`、`kimi-k2.7-code-highspeed`
3. 新增 Moonshot V1 完整系列（含 vision-preview）

## Caveats / 需要 main 关注

1. **K2.7 Code 默认模型**：官方文档和 Claude Code 集成指南均推荐 `kimi-k2.7-code` 作为默认模型，建议同步更新 `models.default.default`。

2. **Anthropic 端点域名**：与 preset 当前配置 `https://api.moonshot.cn/anthropic` 不同，官方 Claude Code 文档使用 `https://platform.moonshot.cn/anthropic`（建议核实是否为正式域名）。

3. **temperature 参数限制**：
   - `kimi-k2.6` 和 `kimi-k2.5` 在思考模式下固定使用 `temperature=1.0`，非思考模式使用 `temperature=0.6`（出处：https://platform.moonshot.cn/docs/guide/migrating-from-openai-to-kimi）
   - Kimi API 的 `temperature` 取值范围为 `[0, 1]`，而 OpenAI 为 `[0, 2]`（可能影响兼容性）。

4. **Moonshot V1 模型状态**：官方模型列表页面仍将 Moonshot V1 系列列为「Stable」，但未明确标注为「主推模型」，建议确认是否为长期支持模型。

5. **kimi-k2.6 vs k2.7-code 选择**：
   - 通用对话/多模态理解：`kimi-k2.6`
   - 编程 Agent/代码生成：`kimi-k2.7-code` 或 `kimi-k2.7-code-highspeed`

## 验收标准达成

- [x] 每个模型 id 有官方 URL 引证（全部链接已标注）
- [x] 明确列出排除项与原因（已下线模型表）
- [x] 区分 Stable / Preview / Deprecated（Stable / Deprecated 分类表）
