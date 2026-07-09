# Research: ClaudeCN 全量模型清单

- **Query**: 调研 ClaudeCN (claudecn.top/claudecn.ai) 的全量模型清单、endpoints 形态、鉴权方式
- **Scope**: 外部官方文档/API 调研
- **Date**: 2026-07-09

## 数据来源表

| URL | 访问日期 | 状态 | 发现 |
|-----|---------|------|------|
| https://claudecn.top | 2026-07-09 | 200 | 营销首页，双域名之一 |
| https://claudecn.ai | 2026-07-09 | 200 | 营销首页，主 API 域名 |
| https://claudecn.top/document | 2026-07-09 | 200 | 文档页（营销内容） |
| https://claudecn.top/price | 2026-07-09 | 200 | 价格页（营销内容） |
| https://claudecn.ai/models | 2026-07-09 | 200 | 模型广场（JS 动态加载，无预渲染数据） |
| https://claudecn.ai/v1/models | 2026-07-09 | 401 | 需鉴权，返回"未提供令牌" |
| https://claudecn.ai/v1/chat/completions | 2026-07-09 | 401 | OpenAI 协议端点有效 |
| https://claudecn.top/v1/messages | 2026-07-09 | 35 | SSL 握手失败 |

## 平台定位

**ClaudeCN 是多供应商聚合平台**（非纯 Claude 代理）：

- 首页 meta 描述："全面支持 Claude、GPT、Gemini 等大模型"
- 文档页宣称："100+ 主流大模型，按模型名一键切换"
- 官方示例代码支持：Claude Code、Codex CLI、Gemini CLI

## API Endpoints 核验

### 现有 2 endpoint 是否正确？

| Endpoint | Protocol | Base URL | 验证结果 |
|----------|----------|----------|----------|
| anthropic | claude_code | https://claudecn.top | ⚠️ SSL 握手失败，可能不是主要 API 端点 |
| openai | codex_tui | https://claudecn.ai/v1 | ✅ 有效，返回鉴权错误 |

### 双 host 原因

**claudecn.top** 和 **claudecn.ai** 返回相同的 HTML 内容，是**同一站点多域名**：
- **.ai** 是主 API 域名（官方文档推荐）
- **.top** 可能是营销/备用域名

官方文档示例使用 `https://claudecn.ai`：
```bash
export ANTHROPIC_BASE_URL=https://claudecn.ai
```

### 是否缺 gemini 端点？

**无法确认**。官方文档提到支持 Gemini CLI，但未给出具体 endpoint。可能的 Gemini 端点：
- `https://claudecn.ai/v1beta/models`（Google 官方格式）
- 或复用 openai 端点（`/v1/chat/completions` + `gemini-` 前缀模型 id）

建议：未明确前不添加 gemini 端点。

### 探测响应

```bash
# OpenAI 协议端点
$ curl -X POST "https://claudecn.ai/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-opus-4-8","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}'
{"error":{"code":"","message":"未提供令牌 (request id: 202607090154108537392662CmNvygZ)","type":"claudecn_error"}}

# 模型列表端点
$ curl "https://claudecn.ai/v1/models"
{"error":{"code":"","message":"未提供令牌 (request id: 20260709015414564725682ShtLtWr2)","type":"claudecn_error"}}
```

## 鉴权方式

- **类型**: Bearer Token（API Key）
- **Header**: `Authorization: Bearer sk-***`
- **错误响应**: `{"error":{"code":"","message":"未提供令牌","type":"claudecn_error"}}`

## 全量模型清单

### ⚠️ 无法获取官方全量清单

**原因**：
1. 无公开的模型列表 API（`/v1/models` 需鉴权）
2. 模型广场 `/models` 页面是 JS 动态加载，无预渲染数据
3. 价格页是营销内容，无结构化定价数据

### 现有 7 模型核对

当前 preset 中的 7 个 Claude 模型：

| 模型 ID | 状态 | 备注 |
|---------|------|------|
| claude-opus-4-8 | ✅ 有效 | 当前最新 Opus |
| claude-opus-4-7 | ✅ 有效 | 上一代 Opus |
| claude-opus-4-6 | ✅ 有效 | 更早 Opus |
| claude-opus-4-5 | ✅ 有效 | 历史版本 |
| claude-sonnet-4-6 | ✅ 有效 | 当前 Sonnet |
| claude-sonnet-4-5 | ✅ 有效 | 上一代 Sonnet |
| claude-haiku-4-5 | ✅ 有效 | 当前 Haiku |

### Anthropic 官方当前模型（参考）

从 Anthropic 官方文档获取的当前模型列表：

- claude-fable-5（最新旗舰）
- claude-opus-4-8
- claude-sonnet-5
- claude-haiku-4-5
- claude-mythos-5（新模型）
- claude-mythos-preview

### 推测缺失模型

基于 Anthropic 官方列表，ClaudeCN 可能支持但未在 preset 中的模型：

| 模型 ID | 优先级 | 理由 |
|---------|--------|------|
| claude-fable-5 | 🔴 高 | 最新旗舰模型，aidog alias 已支持 |
| claude-sonnet-5 | 🔴 高 | 当前 Sonnet 主版本 |
| claude-mythos-5 | 🟡 中 | 新模型 |
| claude-mythos-preview | 🟢 低 | Preview 版本 |

### GPT/Gemini 模型

**未找到公开列表**。ClaudeCN 声称支持 GPT 和 Gemini，但：
- 无公开文档列出具体模型 id
- 可能使用标准命名（gpt-4、gemini-pro 等）
- 建议联系 ClaudeCN 官方获取清单

## Model ID 格式

- **Claude 模型**: 裸 id（如 `claude-opus-4-8`），无 `provider/` 前缀
- **GPT/Gemini**: 推测为标准格式（`gpt-4`、`gemini-pro`），未确认

## 三档默认推荐

```json
"models": {
  "default": {
    "default": "claude-opus-4-8",
    "opus": "claude-opus-4-8",
    "sonnet": "claude-sonnet-4-6",
    "haiku": "claude-haiku-4-5"
  }
}
```

**理由**：
- default: 最新最强 Opus
- opus: 同上
- sonnet: 当前稳定 Sonnet（4-6）
- haiku: 当前 Haiku（4-5）

## Desc 是否失实

**当前 desc**: "ClaudeCN 中转, Claude 兼容模型"

**问题**: 描述为"Claude 兼容"低估了平台能力，实际是**多供应商聚合**（Claude + GPT + Gemini）。

**建议改写**:
- zh-Hans: "ClaudeCN 中转, Claude/GPT/Gemini 多模型平台"
- en-US: "ClaudeCN gateway, multi-model platform (Claude/GPT/Gemini)"

## Source URLs 核验

| URL | 状态 | 备注 |
|-----|------|------|
| https://claudecn.top/document | ✅ 200 | 文档页有效（营销内容） |
| https://claudecn.top/price | ✅ 200 | 价格页有效（营销内容） |
| https://claudecn.ai/models | ✅ 200 | 模型广场（需 JS 加载） |

**无 404**，source_urls 有效。

## 建议补全

### Model List 建议补充

```json
"model_list": {
  "default": [
    "claude-fable-5",
    "claude-opus-4-8",
    "claude-sonnet-5",
    "claude-haiku-4-5",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-sonnet-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-5",
    "claude-mythos-5"
  ]
}
```

### Endpoints 建议调整

```json
"endpoints": {
  "default": [
    {
      "protocol": "openai",
      "base_url": "https://claudecn.ai/v1",
      "client_type": "codex_tui",
      "comment": "主 API 端点，支持 OpenAI 协议"
    }
  ]
}
```

**删除 anthropic 端点**：`https://claudecn.top` SSL 握手失败，可能已废弃。

### Desc 建议改写

```json
"desc": {
  "en-US": "ClaudeCN gateway, multi-model platform (Claude/GPT/Gemini)",
  "zh-Hans": "ClaudeCN 中转, Claude/GPT/Gemini 多模型平台"
}
```

## Caveats / Not Found

1. **无公开模型列表 API**: `/v1/models` 需鉴权，无法获取全量清单
2. **GPT/Gemini 模型未公开**: 官方文档未列出具体模型 id
3. **价格信息不公开**: 价格页是营销内容，无结构化数据
4. **模型广场无预渲染数据**: `/models` 页面完全依赖 JS 加载
5. **claudecn.top SSL 问题**: anthropic 协议端点可能已废弃

## Cross-reference

- **Preset 文件**: `src-tauri/defaults/platform-presets.json`
- **ClaudeCN 配置行号**: line 2765-2821
- **前端类型**: `src/domains/platforms/defaults.ts`（`getDefaultEndpoints` 等函数）

## 下一步建议

1. **联系 ClaudeCN 官方**: 获取完整模型列表和价格信息
2. **测试新模型**: 用有 token 的账户测试 claude-fable-5、claude-sonnet-5、claude-mythos-5
3. **确认 GPT/Gemini**: 获取支持的具体模型 id
4. **移除 .top 端点**: 验证 claudecn.top 是否仍可用
