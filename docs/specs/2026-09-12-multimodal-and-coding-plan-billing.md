# Spec：多模态转发 + coding plan 计费准确性

> 来源：wayfinder 地图 [#11](https://github.com/lazygophers/aidog/issues/11)，决策全集见其 Decisions so far 与各票 resolution（#12–#19）。
> 本文档只汇编，决策真值在各票内；冲突时以票为准。

## A. 多模态转发

### A1. tool_result 内图片 → openai 系目标：提升到相邻 user message（决策 #15）

- `aidog_adapter/src/protocols/openai/request.rs` 的 `has_tool_result` 分支：tool message 保持文本占位（现行为），同时把 `ToolResult.content_blocks` 里的 image block 追加为**紧随 tool message 的 user message** 的 `image_url` 段（同批次合成，不虚构新轮次；多条 tool_result 的图片合并进一条 user message）
- openai_responses 出站补 `input_image` 时同样处理
- →gemini 维持占位（上游官方拒多模态 functionResponse，`protocols/gemini/convert.rs` 注释已引）；→anthropic 已保真不动
- 验收：roundtrip 测试——anthropic 源 tool_result 含 image，转 openai 后图片出现在相邻 user message，tool message 仍带 `[image: …]` 占位

### A2. openai_responses 双向 image（决策 #12 缺口 + #15）

- 入站 `from_responses`：解析 `input_image`（image_url / file_id / detail）→ 中立 image block（与 openai chat 的 parse.rs:104 同构：data URL 拆 base64，其余作 url source）
- 出站 `to_responses`：中立 image block → `{"type":"input_image","image_url":…}`（convert.rs:48 的 `Unknown(_) => None` 增加 image 分支）
- 官方形状：https://developers.openai.com/api/docs/guides/images-vision

### A3. audio/video 全链路 typed 块（决策 #15）

- `ContentBlock` 新增 typed 变体（建议 `Media { media_type: String, source: MediaSource }`，覆盖 audio/video；image 是否并型由实现取舍——并型 diff 大，不并型则 audio/video 与 image 走不同路径，均可接受，见地图雾区遗留）
- 转换矩阵（源→目标）：
  - openai `input_audio{data,format}` ↔ 中立 Media ↔ openai `input_audio`（chat 协议）
  - gemini `inlineData{mimeType≠image/*}` / `fileData` ↔ 中立 Media ↔ gemini `inlineData`
  - anthropic 目标：**丢弃 + `tracing::warn`**（官方无 audio 输入形状，anthropic-sdk-python#1198 未落地）
  - anthropic 源：不存在 audio/video block，无需处理
- 修 bug：gemini 入站 inlineData 不分 mimeType 全标 image（`protocols/gemini/convert.rs:581`）→ 按 mimeType 前缀分流 image/audio/video
- 官方形状：Gemini https://ai.google.dev/gemini-api/docs/tokens（inlineData mimeType 任意）
- 验收：openai 源 input_audio → gemini 目标 inlineData(audio/wav) roundtrip；gemini 源 audio → openai 目标 input_audio；→anthropic 丢弃有 warn 日志

### A4. 图片 token 计费：零改动（决策 #13）

- 三家官方（OpenAI/Anthropic/Gemini）图片 token 均计入 input/prompt tokens 按文本价计费，est_cost 现路径（上游 usage × resolve_price）已正确
- registry **不加**图片计价字段；本地估算公式（(w×h)/750、512-tile）范围外（pre-flight 估算未做）

## B. coding plan 计费

### B1. EstTier 计费类型模型（决策 #16 + #14）

`aidog_core/src/gateway/estimate/model.rs` 的 `EstTier` 增加 `unit` 字段（serde default 向后兼容），枚举：

| unit | 语义 | 增量行为 | 平台 |
|---|---|---|---|
| `prompt_count` | 按次扣 | `has_base`（有绝对 limit）→ 每请求 +`100/limit`；无绝对 → 拟合 `coef_per_request`（真查样本 Δutil/Δ请求数），替代现 `coef_per_token` | GLM（无绝对数）、Kimi（有）、MiniMax（有）、百炼（静态 plan_quotas 当 has_base） |
| `mcp_time` | MCP 使用时长 | 不增量，只真查（现 algo.rs:33 特判转正为类型） | GLM mcp_monthly |
| `tokens` | 按 token 扣 | 现 coef_per_token 行为原样保留（兜底；qianfan 新 Token Plan 属此型） | qianfan、未标注平台 |
| ~~`response_inline`~~ | ~~响应头/体带配额真值~~ | **2026-09-16 删除**：只实现了「不增量」半边，提取端从未写过。速率限制余量改由独立列 `platform.rate_limit` 承载（`gateway::estimate::rate_limit`），与本表的「周期内套餐额度」不是一个维度 | — |

- 数据流：registry quota 脚本返回值（`QuotaTier`，`aidog_adapter/src/quota/http.rs:51`）增加 `unit` 透传 → `calibrate_from_quota` 写入 EstTier
- 调用点：`apply_tier_delta`（algo.rs:28）签名从 tokens 改为（请求数, tokens）按 unit 分派
- 校准阈值不变（5min/100 次，model.rs:6）
- 验收：单测覆盖四类型增量 + 拟合 coef_per_request + serde 向后兼容（旧 EstCodingPlan JSON 无 unit 字段可读）

### B2. 无脚本平台的配额锚点（决策 #18）

| 平台 | 处理 |
|---|---|
| bailian_coding | 无查询 API（官方仅控制台）；`plan_quotas` 静态额度（6000/5h、45000/周、90000/月，platform.json 已有）→ EstTier `has_base=true, limit=amount, unit=prompt_count`，纯本地 per-request 精确预估，无需真查 |
| qianfan_coding | 无查询 API；产品已迁移 Token Plan（按 token）→ tokens 兜底型 |
| compshare_coding | 社区验证接口 `GET cloud.infini-ai.com/maas/coding/usage`（Bearer sk-cp-，返回 5_hour/7_day/30_day {quota,used,remain}，源：Vncntvx/codingplan-status `cli/providers/infini-provider.js`）；**cp.compshare.cn 域名未实测** → 需要: 套餐 key 实测同构后写 quota_scripts（registry 先验证后写入红线） |
| xiaomi_mimo_coding(_en) | endpoint 存在但 cookie 鉴权（cc-switch #5031），API key 不可查 → 无脚本无预估，维持现状；registry 可留 `plan_quotas` 元数据位 |

### B3. API 价折算花费统计（决策 #17）

est_cost 口径不动（上游 usage × API 价，billing.rs）。新增三处展示：

1. **PlatformCard / Groups 卡片**：coding plan 平台显示「本周期折算 $X · 套餐 ¥Y/月」。折算 $ = `EstTier.window_start` 起 `proxy_log.est_cost` 聚合；套餐价读 `platform.extra.plan_price`（新约定键，手填，registry 不内置）
2. **Stats 页**：CostTrendChart 数据链已聚合 est_cost，补 `is_coding_plan` 单列筛选维度（JSON 顶层标记已有）
3. **statusline**：`/api/group-info` 已有累计预估花费（group_info.rs:8），statusline 脚本模板加一行折算 $

- 验收：PlatformCard 对 coding plan 平台显示折算行；Stats 可筛 coding plan；statusline 模板含折算变量

## 遗留（实现时确认，不阻塞开工）

- audio/video typed 块是否与 image Unknown 路径并型（实现取舍）
- ~~response_inline 各平台可用性~~ → 2026-09-16 结案：改为独立的 `platform.rate_limit` 列，认 Anthropic / OpenAI / OpenRouter 三家响应头
- compshare `cp.compshare.cn` endpoint 实测
- Kimi 扣费单位官方原文未取到（行为按条数，第 3 类证据）

## Out of scope（地图裁决）

- 图片生成模型协议转发（2026-09-12 用户排除）
- 响应前预估花费（pre-flight cost estimate）
