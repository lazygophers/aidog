# Research: DeepSeek 模型清单与端点

- **Query**: DeepSeek 全部官方模型清单 + 端点 + 认证方式
- **Scope**: external
- **Date**: 2026-07-09

---

## 官方文档源

| URL | 说明 |
|-----|------|
| https://api-docs.deepseek.com/ | 英文首页（首次调用 API） |
| https://api-docs.deepseek.com/zh-cn/ | 中文首页 |
| https://api-docs.deepseek.com/quick_start/pricing | 计费页（在售模型 + 价格 + 弃用公告） |
| https://api-docs.deepseek.com/zh-cn/quick_start/pricing | 中文计费页 |
| https://api-docs.deepseek.com/guides/thinking_mode | 思考模式指南 |
| https://api-docs.deepseek.com/zh-cn/guides/anthropic_api | Anthropic 格式指南 |
| https://www.deepseek.com/ | DeepSeek 官网（含 GitHub 开源模型链接） |
| https://github.com/deepseek-ai | DeepSeek GitHub（开源模型仓库） |

---

## model_list 最终清单

### Stable（在售）

| Model ID | 状态 | 并发限制 | 价格（¥/M tokens） | 出处 URL |
|----------|------|----------|---------------------|----------|
| `deepseek-v4-flash` | Stable | 2500 | 输入 1.0（缓存未命中）/ 0.02（缓存命中）；输出 2.0 | [pricing](https://api-docs.deepseek.com/quick_start/pricing) |
| `deepseek-v4-pro` | Stable | 500 | 输入 3.0（缓存未命中）/ 0.025（缓存命中）；输出 6.0 | [pricing](https://api-docs.deepseek.com/quick_start/pricing) |

**说明**：
- 两模型均支持 non-thinking 与 thinking（默认）模式
- 上下文长度：1M；最大输出：384K
- 功能：Json Output、Tool Calls、Chat Prefix Completion（Beta）、FIM Completion（Beta，仅 non-thinking 模式）

### Deprecated（待弃用）

| Model ID | 弃用时间（北京时间） | 别名映射 | 出处 URL |
|----------|---------------------|----------|----------|
| `deepseek-chat` | 2026/07/24 23:59 | → `deepseek-v4-flash` 的 non-thinking 模式 | [pricing](https://api-docs.deepseek.com/quick_start/pricing) |
| `deepseek-reasoner` | 2026/07/24 23:59 | → `deepseek-v4-flash` 的 thinking 模式 | [pricing](https://api-docs.deepseek.com/quick_start/pricing) |

**注意**：弃用后兼容性映射将失效，请直接使用 `deepseek-v4-flash`。

---

## models.default.default 推荐

**推荐值**：`deepseek-v4-flash`

**理由**：
1. **官方主推**：首页文档示例默认使用 `deepseek-v4-flash`
2. **高并发**：并发限制 2500 vs v4-pro 的 500，适合高 QPS 场景
3. **低价格**：价格为 v4-pro 的 1/3（输入）/ 1/3（输出）
4. **性能平衡**：支持思考模式，上下文 1M，最大输出 384K

**替代选择**：`deepseek-v4-pro` 用于需要更高质量（但接受较低并发、更高价格）的场景。

---

## endpoints

| Protocol | base_url | Client Type | 认证方式 | 出处 URL |
|----------|----------|-------------|----------|----------|
| OpenAI | `https://api.deepseek.com/v1` | codex_tui | Authorization Bearer `${DEEPSEEK_API_KEY}` | [首页](https://api-docs.deepseek.com/) |
| Anthropic | `https://api.deepseek.com/anthropic` | claude_code | Authorization Bearer `${DEEPSEEK_API_KEY}` | [首页](https://api-docs.deepseek.com/) |
| OpenAI（根路径） | `https://api.deepseek.com` | — | 同上 | [首页](https://api-docs.deepseek.com/) |

**路径说明**：
- 官方文档示例使用 `https://api.deepseek.com/chat/completions`（不含 `/v1`）
- preset 中 `/v1` 路径经实测有效（`/v1/models` 返回认证错误而非 404），符合 OpenAI SDK 惯例
- 推荐 preset 保持 `/v1` 以兼容 OpenAI SDK

**API Key 申请**：https://platform.deepseek.com/api_keys

---

## 排除项与原因

| 模型 | 状态 | 原因 | 出处 |
|------|------|------|------|
| DeepSeek V3（deepseek-v3） | 开源，不在 API 销售 | V3 系列已被 V4 取代，API 计费页仅列 V4 模型 | [pricing](https://api-docs.deepseek.com/quick_start/pricing) |
| DeepSeek R1（deepseek-r1） | 开源，不在 API 销售 | 推理模型已整合进 V4 系列的思考模式，无独立 API 端点 | [R1 GitHub](https://github.com/deepseek-ai/DeepSeek-R1) |
| DeepSeek Coder V2 | 开源，不在 API 销售 | 代码专用模型已整合进 V4 通用模型，无独立 API 端点 | [Coder V2 GitHub](https://github.com/deepseek-ai/DeepSeek-Coder-V2) |
| DeepSeek Math | 开源，不在 API 销售 | 数学专用模型已整合进 V4 通用模型，无独立 API 端点 | [Math GitHub](https://github.com/deepseek-ai/DeepSeek-Math) |
| DeepSeek V2 / LLM / MoE | 开源，不在 API 销售 | 历史模型，已被 V3/V4 取代，无 API 端点 | [官网研究页](https://www.deepseek.com/) |

**结论**：DeepSeek API 平台仅提供 V4 系列模型（flash/pro），历史模型（V3/R1/Coder/Math）均为开源 GitHub 仓库，不在 API 销售。

---

## 别名映射表

| 别名 | 映射目标 | 生效时间 | 失效时间 |
|------|----------|----------|----------|
| `deepseek-chat` | `deepseek-v4-flash` (non-thinking 模式) | 当前 | 2026/07/24 23:59（北京时间） |
| `deepseek-reasoner` | `deepseek-v4-flash` (thinking 模式) | 当前 | 2026/07/24 23:59（北京时间） |

**说明**：
- 两个别名将在 2026/07/24 23:59 UTC+8 弃用
- 弃用后 API 将不再接受这些别名，需直接使用 `deepseek-v4-flash` 并通过 `thinking` 参数控制模式

---

## caveats / 需要 main 关注

1. **即将到来的模型别名弃用**（2026/07/24）
   - `deepseek-chat` / `deepseek-reasoner` 将停止接受
   - 需提前通知用户迁移至 `deepseek-v4-flash`
   - 迁移时需设置 `thinking: {"type": "enabled"}` 以启用思考模式

2. **思考模式参数不兼容**
   - 思考模式下不支持 `temperature`、`top_p`、`presence_penalty`、`frequency_penalty`
   - 设置这些参数不会报错，但会无效
   - 文档：[thinking_mode](https://api-docs.deepseek.com/guides/thinking_mode)

3. **OpenAI 路径规范**
   - 官方示例不含 `/v1`（`https://api.deepseek.com/chat/completions`）
   - 但 `/v1` 路径实测有效，符合 OpenAI SDK 惯例
   - 建议 preset 保持 `/v1`

4. **并发限制差异**
   - `deepseek-v4-flash`：2500 并发
   - `deepseek-v4-pro`：500 并发
   - 高并发场景应优先选 flash

5. **无国际端点**
   - 仅 `api.deepseek.com` 单一端点
   - 无类似其他平台的国际版/国内版区分

---

## preset 现状评估

当前 `platform-presets.json` deepseek 配置：

```json
{
  "endpoints": {
    "default": [
      {
        "protocol": "openai",
        "base_url": "https://api.deepseek.com/v1",
        "client_type": "codex_tui"
      },
      {
        "protocol": "anthropic",
        "base_url": "https://api.deepseek.com/anthropic",
        "client_type": "claude_code"
      }
    ]
  },
  "models": {
    "default": {
      "default": "deepseek-v4-flash"
    }
  },
  "model_list": {
    "default": [
      "deepseek-v4-flash",
      "deepseek-v4-pro",
      "deepseek-chat",
      "deepseek-reasoner"
    ]
  }
}
```

**评估**：
- ✅ endpoints 正确（含 `/v1` 与 anthropic 路径）
- ✅ models.default.default 为 `deepseek-v4-flash`（推荐值）
- ⚠️ model_list 含待弃用别名（`deepseek-chat`、`deepseek-reasoner`）

**建议**：
- 暂保留别名（弃用前仍可用），但可考虑添加弃用标记提示
- 弃用后（2026/07/24 后）需从 model_list 移除别名
