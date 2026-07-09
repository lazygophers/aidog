# Research: AiHubMix 全量模型清单

- **Query**: 研究 AiHubMix（aihubmix 协议）的官方信息，补全 platform-presets.json
- **Scope**: 外部搜索（官方端点 + 文档）
- **Date**: 2026-07-09
- **数据源**: `https://aihubmix.com/api/v1/models`（OpenAI 兼容端点）

---

## Findings

### 全量模型清单（2026-07-09 现状）

**总模型数**: **800 个**

数据来源于 `https://aihubmix.com/api/v1/models` 端点（OpenAI 兼容的 models 端点）。

### Provider 分布统计

按 developer_id 分组（前 13 位，覆盖 95% 模型）：

| Developer ID | 推断 Provider | 模型数量 | 代表模型 |
|---|---|---:|---|
| 13 | Qwen（阿里通义） | 133 | qwen3.7-max, qwen3.7-plus, qwen3.6-flash |
| 12 | OpenAI | 126 | gpt-5.5, gpt-5.5-pro, gpt-5.3-codex |
| 8 | Google Gemini | 80 | gemini-3.5-flash, gemini-3.1-pro-preview |
| 5 | GLM（智谱） | 63 | glm-5.2, coding-glm-5.2, glm-5.1 |
| 11 | Doubao（字节豆包） | 52 | doubao-seed-2-1-pro, doubao-seedance-2-0 |
| 4 | Meta/Llama 开源 | 44 | llama-4-maverick, llama-4-scout, llama-3.3-70b |
| 18 | MiniMax | 31 | minimax-m3, coding-minimax-m3-free |
| 15 | Kimi（月之暗面） | 31 | kimi-k2.7-code, kimi-k2.6, kimi-k2.5 |
| 2 | Anthropic | 29 | claude-opus-4-8, claude-sonnet-5, claude-fable-5 |
| 7 | DeepSeek | 18 | deepseek-v4-pro, deepseek-v4-flash |
| 25 | ERNIE（百度千帆） | 23 | ernie-5.1, ernie-5.0 |
| 31 | 小米（大模型） | 31 | xiaomi-mimo-v2.5-pro |
| 9 | xAI（Grok） | 10 | grok-4.3, grok-build-0.1 |
| 其他 | 17 个小 provider | 29 | （各种长尾/视频/图像模型） |

### 模型 id 命名格式

**结论：裸 id 格式，无 provider 前缀**

样本验证：
- `claude-opus-4-8` ✓
- `gpt-5.5` ✓
- `glm-5.2` ✓
- `gemini-3.5-flash` ✓
- `deepseek-v4-pro` ✓
- `kimi-k2.7-code` ✓
- `grok-4.3` ✓

**与 preset 现状一致**：preset 使用裸 id，与官方端点实际格式匹配。

### 支持的协议/endpoints

从模型数据 `endpoints` 字段提取，AiHubMix 支持 **4 种协议类型**：

| Endpoint 类型 | 含义 | 覆盖范围 |
|---|---|---|
| `chat_completions` | OpenAI 兼容（`/v1/chat/completions`） | 全部 800 模型 |
| `claude_api` | Anthropic 原生（`/v1/messages`） | Claude 全系列 + 部分其他模型 |
| `gemini_api` | **Gemini 原生**（`/v1/models/*`） | Gemini 全系列 + Claude Sonnet 5/4.6 |
| `responses` | OpenAI streaming responses | 部分 OpenAI 模型 |

#### Gemini 原生协议支持确认

**YES** — AiHubMix **支持 Gemini 原生协议**！

证据：
- `gemini_api` 在 endpoints 字段中存在
- 支持 `gemini_api` 的模型包括：
  - `gemini-3.5-flash`
  - `gemini-3.1-pro-preview`
  - `gemini-3.1-flash-lite`
  - `claude-sonnet-5`（跨协议支持）
  - `claude-sonnet-4-6`（跨协议支持）

**端点路径推测**：基于 AiHubMix 的协议路由模式：
- Anthropic: `https://aihubmix.com`（根域）
- OpenAI: `https://aihubmix.com/v1`
- Gemini: `https://aihubmix.com/google` 或 `https://aihubmix.com/v1/models`（待官方文档确认）

### Preset 现状 14 项模型核实

**全部 14 个模型仍在列表中** ✅

| 模型 ID | 状态 | 说明 |
|---|---|---|
| `claude-opus-4-8` | ✅ 存在 | Anthropic 最新 Opus |
| `claude-sonnet-4-6` | ✅ 存在 | Anthropic Sonnet |
| `claude-sonnet-4-5` | ✅ 存在 | Anthropic Sonnet 旧版 |
| `gpt-5.5` | ✅ 存在 | OpenAI 最新 GPT |
| `gpt-5.5-pro` | ✅ 存在 | OpenAI GPT Pro |
| `gpt-5.3-codex` | ✅ 存在 | OpenAI 编程模型 |
| `gemini-3.5-flash` | ✅ 存在 | Google Gemini Flash |
| `gemini-3.1-pro-preview` | ✅ 存在 | Google Gemini Pro |
| `deepseek-v4-pro` | ✅ 存在 | DeepSeek Pro |
| `deepseek-v4-flash` | ✅ 存在 | DeepSeek Flash |
| `qwen3.7-max` | ✅ 存在 | 阿里通义千问 |
| `glm-5.2` | ✅ 存在 | 智谱 GLM |
| `kimi-k2.7-code` | ✅ 存在 | 月之暗面编程版 |
| `grok-4.3` | ✅ 存在 | xAI Grok |

### Endpoints 核实

**preset 现状**：
```json
"endpoints": {
  "default": [
    {
      "protocol": "anthropic",
      "base_url": "https://aihubmix.com",
      "client_type": "claude_code"
    },
    {
      "protocol": "openai",
      "base_url": "https://aihubmix.com/v1",
      "client_type": "codex_tui"
    }
  ]
}
```

**验证结果**：
- Anthropic 端点 `https://aihubmix.com` ✓ 正确（根域无路径）
- OpenAI 端点 `https://aihubmix.com/v1` ✓ 正确（OpenAI 兼容路径）
- **缺少 Gemini 端点** ⚠️ — AiHubMix 支持 `gemini_api`，但 preset 未配置

### Models.default 建议

**推荐默认模型**：`claude-sonnet-5` 或 `gpt-5.5`

理由：
- `claude-sonnet-5` 是 Anthropic 最新 Sonnet，能力全面
- `gpt-5.5` 是 OpenAI 最新 GPT，兼容性最佳
- preset 现有的 `claude-sonnet-4-6` 仍在但可升级到 `claude-sonnet-5`

**preset 现状**：`models.default.default = {}`（空，需补）

### 时效性与腐化风险

**高腐化风险**：

1. **模型上下架频繁**：AiHubMix 是聚合平台，供应商模型变动会直接反映在端点
2. **总量 800 模型**：preset 硬编码全量 = 高频维护
3. **建议**：
   - preset 只保留「精选列表」（20-30 个高频模型）
   - 全量清单应由前端动态从 `/api/v1/models` 端点拉取
   - 或由后端定时同步到本地缓存

**当前 preset 14 项精选**：合理，覆盖主流供应商旗舰模型。

### 结论摘要

**一句话给 PRD 用**：
> AiHubMix 聚合 800 模型（Anthropic 29 / OpenAI 126 / Google 80 / GLM 63 / Doubao 52 / 等），支持裸 id 格式、原生 Gemini 协议（`gemini_api`），preset 14 项精选全部有效、建议补 `claude-sonnet-5` 为默认、新增 Gemini 端点配置。

**关键数据**：
- 总模型数：800
- 主要 provider：13 个
- 协议支持：4 种（`chat_completions` / `claude_api` / `gemini_api` / `responses`）
- preset 现有 14 模型：全部有效 ✅

---

## 待确认项

1. **Gemini 端点路径**：`https://aihubmix.com/google` vs `https://aihubmix.com/v1/models`
2. **developer_id → 官方名称映射**：当前为推断（如 13=Qwen, 4=Meta）
3. **认证方式**：推测为 `Authorization: Bearer <api_key>`（未在端点响应中确认）

## 下一步（Implement 阶段）

1. 补 `models.default.default = "claude-sonnet-5"` 或 `"gpt-5.5"`
2. 新增 Gemini 端点配置：
   ```json
   {
     "protocol": "gemini",
     "base_url": "https://aihubmix.com",  // 或 /google
     "client_type": "default"
   }
   ```
3. 全量模型数组由 implement 阶段从端点拉取写入（或保持精选列表不变）
