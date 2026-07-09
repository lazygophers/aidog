# Research: glm-coding（智谱 GLM Coding Plan — 新增独立协议）⭐ OQ3 关键

- **Query**: 确认 glm-coding base_url（同源 `/api/paas/v4` 还是独立 coding 域名）+ 计费规则 + 支持模型
- **Scope**: external（GLM Coding Plan 官方文档）
- **Date**: 2026-07-09
- **状态**: **OQ3 已解答**（base_url 不同源，coding plan 有独立 `/api/coding/paas/v4` 路径）

## design.md §2.1 草案 vs 实际

design.md 预填 base_url = `https://open.bigmodel.cn/api/paas/v4`（猜测「同源仅计费差异」）。**猜测错误**。

## 官方文档列出值（原文 + URL）

### Source
- 编码套餐概览（peak 规则）：https://docs.bigmodel.cn/cn/coding-plan/overview
- 最新模型：https://docs.bigmodel.cn/cn/coding-plan/latest-model
- 快速开始（**endpoint 表 — OQ3 答案出处**）：https://docs.bigmodel.cn/cn/coding-plan/quick-start
- FAQ：https://docs.bigmodel.cn/cn/coding-plan/faq

### OQ3 答案：endpoint base_url（quick-start 表格原文）

```
<tr><td>Anthropic Message 协议</td><td><code>https://open.bigmodel.cn/api/anthropic</code></td></tr>
<tr><td>OpenAI Chat Completion 协议</td><td><code>https://open.bigmodel.cn/api/coding/paas/v4</code></td></tr>
```

→ **Coding Plan OpenAI 兼容端点 = `https://open.bigmodel.cn/api/coding/paas/v4`**（注意 `/api/coding/paas/v4`，比普通版 `/api/paas/v4` 多一段 `/coding/`）。Anthropic 兼容端点 = `https://open.bigmodel.cn/api/anthropic`（与普通版共用路径，但 Coding Plan Key 走套餐计费）。

### Peak 规则原文（overview / faq 双重出处）
> **GLM-5.2/GLM-5-Turbo** 作为高阶模型，对标 Claude Opus，调用时将按照 "高峰期 3 倍，非高峰期 2 倍" 系数消耗额度。
> **（作为限时福利，GLM-5.2/GLM-5-Turbo 将在非高峰期仅作为 1 倍抵扣，持续到 9 月底。）**
> 注："高峰期"为每日的 14:00～18:00（UTC+8）。

→ UTC+8 14:00-18:00 = UTC+0 06:00-10:00。仅 GLM-5.2 / GLM-5-Turbo 受影响；其他模型（GLM-4.7 等）普通 1 倍。

### Coding Plan 推荐使用模型（overview + faq）
- 主力：**GLM-5.2**（HOT 标）、**GLM-5-Turbo**、**GLM-4.7**（普通任务节省额度）
- 历史模型：GLM-5.1, GLM-5
- FAQ 原文：「我们推荐您在复杂任务上切换至 GLM-5.2 处理，普通任务上继续使用 GLM-4.7」

## Diff（对照 design.md §2.1 草案）

| 字段 | design.md 草案 | 官方实际 | 是否需改 |
|---|---|---|---|
| OpenAI base_url | `https://open.bigmodel.cn/api/paas/v4` | `https://open.bigmodel.cn/api/coding/paas/v4` | **是，必须改** |
| Anthropic base_url | （草案未列） | `https://open.bigmodel.cn/api/anthropic` | 补 |
| peak_hours.start_hour/end_hour | 6 / 10（UTC+0） | UTC+8 14-18 = UTC+0 6-10 | ✅ 正确 |
| peak_hours.models | `["glm-5.2","glm-5-turbo"]` | 仅 5.2 / 5-Turbo 受影响 | ✅ 正确（注意官方写法 GLM-5-Turbo，模型 id 通常小写 `glm-5-turbo`） |
| peak_hours.multiplier | 3.0（高峰） | 高峰 3 倍 + 非高峰福利 1 倍（9 月底后 2 倍） | ✅ 当前态正确；R5 福利期截止待处理 |
| model_list | `["glm-5.2","glm-5-turbo"]`（草案） | 主力 5.2/5-Turbo + 4.7（普通任务）+ 历史 5.1/5 | **建议补 glm-4.7**（套餐内推荐主力），可选补 glm-5.1 / glm-5 |
| models slot 填充 | `default:glm-5.2, opus:glm-5.2, sonnet:glm-5.2, gpt:glm-5-turbo` | 5.2 对标 Opus，5-Turbo 次之，4.7 经济 | 合理；可考虑 `haiku: glm-4.7`（经济模型对应 haiku slot） |

## 补齐建议（具体改什么）— ST3 落盘用

```json
"glm-coding": {
  "client_type": "codex_tui",
  "endpoints": {
    "default": [
      { "protocol": "openai", "base_url": "https://open.bigmodel.cn/api/coding/paas/v4", "client_type": "codex_tui", "coding_plan": true },
      { "protocol": "anthropic", "base_url": "https://open.bigmodel.cn/api/anthropic", "client_type": "claude_code", "coding_plan": true }
    ]
  },
  "models": {
    "default": {
      "default": "glm-5.2",
      "opus": "glm-5.2",
      "sonnet": "glm-5.2",
      "gpt": "glm-5-turbo",
      "haiku": "glm-4.7"
    }
  },
  "model_list": ["glm-5.2", "glm-5-turbo", "glm-4.7", "glm-5.1", "glm-5"],
  "peak_hours": [
    { "start_hour": 6, "end_hour": 10, "multiplier": 3.0, "models": ["glm-5.2", "glm-5-turbo"] }
  ]
}
```

关键修正：
1. **base_url = `https://open.bigmodel.cn/api/coding/paas/v4`**（非 `/api/paas/v4`）
2. endpoint 加 `coding_plan: true` flag（D1 机制并存）
3. model_list 补 `glm-4.7`（套餐内推荐经济模型，FAQ 明示）
4. 加 `haiku: glm-4.7`（对标 haiku 经济 slot）
5. peak_hours.models 明确限 `["glm-5.2","glm-5-turbo"]`（D2 model scope）

## Caveats / Not Found

- **R5 福利期 9 月底截止**：截止后非高峰应转 2 倍。当前 JSON 只配高峰 3 倍窗口，非高峰 absent = 1.0（福利态）。OQ1 仍待用户拍板处理方式（硬编码截止日期 / 注释提醒 / 定时切换）。
- GLM-5.2 / GLM-5-Turbo 的精确模型 id 大小写（`glm-5-turbo` vs `GLM-5-Turbo`）：JSON 既有用小写 `glm-5-turbo`，官方文档行文用大写。维持小写 id。
- `api/anthropic` 端点是否对 coding plan key 鉴权后走套餐计费：`推测` 是（套餐 key 在两端点通用），`需要: 官方明示 coding plan key 在 /api/anthropic 的计费路径`。
