# Research: CCSub 全量模型清单与 Endpoints 核验

- **Query**: CCSub 平台 (https://www.ccsub.net) 全量模型清单 + endpoints 形态 + 鉴权方式
- **Scope**: 外部官方文档/API 调研
- **Date**: 2026-07-09

## 数据来源

| URL | 访问日期 | 状态码 | 数据类型 |
|---|---|---|---|
| https://www.ccsub.net/docs | 2026-07-09 | 200 | 文档导航 + 平台定位 |
| https://www.ccsub.net/pricing | 2026-07-09 | 200 | 定价方案（无模型清单） |
| https://www.ccsub.net | 2026-07-09 | 200 | 首页（工具支持 + 模型预览） |
| https://www.ccsub.net/models | 2026-07-09 | 200 | **完整模型清单 + 价格** |
| https://www.ccsub.net/docs/install | 2026-07-09 | 200 | Endpoints 配置文档 |
| https://www.ccsub.net/docs/usage | 2026-07-09 | 200 | 模型推荐组合 |
| https://www.ccsub.net/v1/models | 2026-07-09 | 200 | **公开 API 端点（无鉴权）** |
| https://docs.anthropic.com/en/docs/about-claude/models | 2026-07-09 | 200 | Anthropic 官方模型 ID 命名参考 |

## 平台定位

**CCSub 是多供应商聚合 AI API 中转服务，非 Claude-only 平台。**

- **覆盖供应商**: Anthropic (Claude 全系列) + OpenAI (GPT-5.4/5/4o, o3/o3-pro, Codex) + Google (Gemini 3.5/2.5 系列) + 生图 (gpt-image-1, dall-e-3)
- **价值主张**: 国内直连、透明计费（1 RMB = 1 USD）、多工具兼容、故障切换
- **支持工具**: Claude Code, Codex CLI, Cursor, VS Code (Continue/Cline), JetBrains, Gemini CLI, OpenCode, OpenClaw, CherryStudio

**现有 preset desc 需改写**:
- 当前: "CCSub API for Claude-compatible models" / "CCSub API, Claude 兼容模型"
- 应改为: "多供应商聚合 AI API 中转服务 (Claude + GPT + Gemini)" / "Multi-vendor AI API relay (Claude/GPT/Gemini)"

## API Endpoints 核验

### 现有 preset endpoints

| 协议 | base_url | client_type | 状态 |
|---|---|---|---|
| anthropic | `https://www.ccsub.net` | claude_code | **正确** |
| openai | `https://www.ccsub.net/v1` | codex_tui | **正确** |

### 验证依据

**Anthropic 协议**:
- 文档明确: `ANTHROPIC_BASE_URL=https://www.ccsub.net` (无 `/v1` 后缀)
- 首页示例: `$curl -X POST https://www.ccsub.net/v1/messages` (注: `/v1/messages` 是路径，非 base_url)

**OpenAI 协议**:
- 文档明确: `OPENAI_BASE_URL=https://www.ccsub.net/v1` (有 `/v1` 后缀)
- Cursor/Continue 配置: `https://www.ccsub.net/v1`

**Gemini 协议**:
- 文档提及: "Gemini CLI ... 指向 `https://www.ccsub.net`" (同 anthropic，无 `/v1`)
- **建议新增**: preset 应加入 `gemini` 协议端点

**是否缺端点**: **是**，缺少 `gemini` 协议端点。

## 全量模型清单

**来源**: `https://www.ccsub.net/v1/models` 公开 API 端点（无鉴权，返回 19 个模型）

### Anthropic (7 个)

| Model ID | Display Name | 输入价格 | 输出价格 | 用途标签 |
|---|---|---|---|---|
| `claude-opus-4-8` | Claude Opus 4.8 | $5/MTok | $25/MTok | 推理 编码 最强 最新 |
| `claude-opus-4-6` | Claude Opus 4.6 | $5/MTok | $25/MTok | 推理 编码 最强 |
| `claude-opus-4-5` | Claude Opus 4.5 | $5/MTok | $25/MTok | 推理 编码 |
| `claude-sonnet-5` | Claude Sonnet 5 | $3/MTok | $15/MTok | 编码 高效 主力 最新 |
| `claude-sonnet-4-6` | Claude Sonnet 4.6 | $3/MTok | $15/MTok | 编码 高效 主力 |
| `claude-sonnet-4-5` | Claude Sonnet 4.5 | $3/MTok | $15/MTok | 编码 高效 |
| `claude-haiku-4-5` | Claude Haiku 4.5 | $0.8/MTok | $4/MTok | 快速 轻量 |

### OpenAI (8 个)

| Model ID | Display Name | 输入价格 | 输出价格 | 用途标签 |
|---|---|---|---|---|
| `gpt-5.4` | GPT-5.4 | $5/MTok | $15/MTok | 推理 编码 多模态 |
| `gpt-5` | GPT-5 | $5/MTok | $15/MTok | 推理 多模态 |
| `gpt-5-mini` | GPT-5 Mini | $1.5/MTok | $6/MTok | 快速 高效 |
| `gpt-4o` | GPT-4o | $2.5/MTok | $10/MTok | 多模态 |
| `o3` | o3 | $10/MTok | $40/MTok | 推理 编码 |
| `o3-pro` | o3-pro | $20/MTok | $80/MTok | 推理 最强 |
| `o4-mini` | o4-mini | $1.1/MTok | $4.4/MTok | 推理 快速 |
| `codex-mini-latest` | Codex Mini | $1.5/MTok | $6/MTok | 编码 |

### Google (4 个)

| Model ID | Display Name | 输入价格 | 输出价格 | 用途标签 |
|---|---|---|---|---|
| `gemini-3.5-flash` | Gemini 3.5 Flash | $0.15/MTok | $0.6/MTok | 推理 多模态 最新 |
| `gemini-2.5-pro` | Gemini 2.5 Pro | $1.25/MTok | ? | 推理 多模态 长上下文 |
| `gemini-2.5-flash` | Gemini 2.5 Flash | ? | ? | 推理 |
| `gemini-2.5-flash-lite` | Gemini 2.5 Flash Lite | ? | ? | 推理 |

**模型总数**: 19 个

## Model ID 格式

**格式**: 裸 ID，无 `provider/model` 前缀（如 `claude-opus-4-8` 非 `anthropic/claude-opus-4-8`）。

**与现有 preset 一致**: 是，现有 7 个 Claude 模型 ID 格式正确。

## 现有 7 个 Claude 模型核对

| preset 中 ID | API 返回 | Anthropic 官方状态 | 是否需改 |
|---|---|---|---|
| `claude-opus-4-8` | ✅ 返回 | 当前旗舰 | ✅ 保留 |
| `claude-sonnet-4-6` | ✅ 返回 | 可用 | ✅ 保留 |
| `claude-haiku-4-5` | ✅ 返回 | 可用 (alias of `claude-haiku-4-5-20251001`) | ✅ 保留 |
| `claude-opus-4-7` | ❌ 不返回 | 已被 4.8 取代 | ❌ **应移除** |
| `claude-opus-4-6` | ✅ 返回 | 可用 | ✅ 保留 |
| `claude-opus-4-5` | ✅ 返回 | 可用 | ✅ 保留 |
| `claude-sonnet-4-5` | ✅ 返回 | 可用 | ✅ 保留 |
| — | `claude-sonnet-5` | ✅ 返回 | **最新主力** | ❌ **应加入** |

**日期后缀核对**: 无需改动，CCSub 使用官方 alias 形式（如 `claude-haiku-4-5` 而非 `claude-haiku-4-5-20251001`）。

## 三档默认推荐

基于文档推荐组合 + 价格分层：

```json
{
  "models": {
    "default": {
      "economy": "claude-haiku-4-5",
      "balanced": "claude-sonnet-4-6",
      "flagship": "claude-opus-4-8"
    }
  }
}
```

**依据**:
- **经济档** (`claude-haiku-4-5`): $0.8/$4 per MTok，文档推荐"大量简单查询/代码补全"
- **均衡档** (`claude-sonnet-4-6`): $3/$15 per MTok，文档明确"日常编码主力"（性价比最高）
- **旗舰档** (`claude-opus-4-8`): $5/$25 per MTok，文档明确"最新旗舰推理模型"

**替代方案** (如需覆盖 OpenAI):
```json
{
  "anthropic": "claude-sonnet-4-6",
  "openai": "gpt-5.4",
  "gemini": "gemini-3.5-flash"
}
```

## 鉴权方式

- **API Key 格式**: `sk-xxx` (在 [/keys](https://www.ccsub.net/keys) 页面创建)
- **Header**: `Authorization: Bearer sk-xxx`
- **环境变量**:
  - Anthropic: `ANTHROPIC_AUTH_TOKEN=sk-xxx`
  - OpenAI: `OPENAI_API_KEY=sk-xxx`
- **免鉴权端点**: `/v1/models` 可直接调用返回模型清单（实测 2026-07-09）

## 建议补全 model_list

**现有 preset model_list.default** (7 个 Claude):
```json
[
  "claude-opus-4-8",
  "claude-sonnet-4-6",
  "claude-haiku-4-5",
  "claude-opus-4-7",    // 应移除
  "claude-opus-4-6",
  "claude-opus-4-5",
  "claude-sonnet-4-5"
]
```

**建议补全为** (19 个全量):
```json
[
  // Anthropic (7)
  "claude-opus-4-8",
  "claude-opus-4-6",
  "claude-opus-4-5",
  "claude-sonnet-5",
  "claude-sonnet-4-6",
  "claude-sonnet-4-5",
  "claude-haiku-4-5",
  // OpenAI (8)
  "gpt-5.4",
  "gpt-5",
  "gpt-5-mini",
  "gpt-4o",
  "o3",
  "o3-pro",
  "o4-mini",
  "codex-mini-latest",
  // Google (4)
  "gemini-3.5-flash",
  "gemini-2.5-pro",
  "gemini-2.5-flash",
  "gemini-2.5-flash-lite"
]
```

## Caveats / Not Found

- **Gemini 2.5 系列价格**: 模型页仅显示输入价格 $1.25/MTok (Pro)，输出价格未标注；Flash/Lite 价格完全未标注
- **生图模型**: `gpt-image-1`, `dall-e-3` 在首页提及，但 `/v1/models` 未返回（可能是独立端点或需特殊配置）
- **动态性**: 模型清单可能随上游变化，建议定期同步 `/v1/models` 端点
- **prefix 模式**: 无 `provider/model` 前缀，与 preset 现有格式一致

## Cross-reference

- **preset 路径**: `src-tauri/defaults/platform-presets.json`
- **当前 ccsub 块**: 约 line 200-250 (需 grep 确认确切行号)
- **相关前端**: `src/domains/platforms/defaults.ts` 的 `getDefaultEndpoints` / `getDefaultModels` 等 async 函数

## 关键发现总结

1. **desc 需改写**: "Claude 兼容" 严重低估，应改为"多供应商聚合 (Claude/GPT/Gemini)"
2. **模型需更新**: 移除 `claude-opus-4-7`，加入 `claude-sonnet-5`
3. **endpoints 正确**: 现有 2 个端点验证无误
4. **缺端点**: 应新增 `gemini` 协议端点 (`https://www.ccsub.net`)
5. **模型总数**: 19 个（7 Claude + 8 OpenAI + 4 Google）
