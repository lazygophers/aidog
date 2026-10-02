# 默认模型预设

校对于 2026-10-02（registry 化后布局）。

## 机制

默认模型住 **registry `platform.json` 的 `models` 字段**，不再是前端硬编码 map：

```json
"models": {
  "default": { "default": "glm-5.2" },          // 外层 default/peak = 时段分支；内层 key = ModelSlot
  "peak":   { "default": "glm-5.2-turbo" }       // 可选：高峰时段切换分支（PRD 07-11，现仅 glm_coding 带）
},
"model_list": { "default": ["glm-5.3", "glm-5.2", ...] }   // 下拉冷启动全候选清单
```

- **slot key** ∈ `ModelSlot`（default / sonnet / opus / haiku / gpt / jev；TS `src/services/api/types/manual.ts`，Rust `aidog_db` models）。
- **前端读取**：`getDefaultModels(protocol, isPeak?)`（`src/domains/platforms/defaults.ts`，async，走 `pickModelsBranch` 选 default/peak 分支）——**加平台时 0 前端改动**，写 JSON 即生效。
- **不变量：`model_list.default` 必须是 `models` 各分支（default / peak）值集的超集**（check-registry 门禁）。
- **路由消费**：`resolve_model`（`aidog_core/src/gateway/router/`）按槽位名匹配；请求模型名含 opus/sonnet/haiku/gpt → 对应槽位，否则 `default`，无 default → 透传（去 `[budget]` 后缀）。
- **高峰模型切换**：后端 `gateway/router/candidates.rs::resolve_effective_models` 三层级联（用户 `time_windows` → `preset.models.peak` → `platform.models`），peak 为 preset 级硬约束（覆盖用户自定义，需保留请配 time_windows）。

## 写什么

- 取该平台**当前主力型号**填 `default` 槽；官方主力分档（如 Anthropic 的 opus/sonnet/haiku）才填其它槽。
- 型号名以官方文档为准（memory `registry-verify-before-create`：推测 id 禁写入）；过时由 `fetchModels` 上游拉取覆盖。
- peak 分支仅当平台有官方峰时换模机制才加，加了必须同步 `peak` 时段窗口 + `models.peak`，且 model_list 超集不变量仍成立。
- 改完跑 `node scripts/check-registry.mjs` + `node scripts/bump-registry-last-updated.mjs` 盖戳。
