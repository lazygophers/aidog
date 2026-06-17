# 补齐所有平台预设 base_url 与默认模型

## Goal

审计 aidog 全部平台预设，补齐缺失的默认 base_url（OpenAI 兼容系列尤其）与默认模型，使每个平台在下拉选中后能自动填入正确 base_url + 端点协议 + 默认模型。触发起因：小米平台缺默认 openai 系列 base_url 与默认模型。

## What I already know

- 平台预设住**前端** `src/pages/Platforms.tsx`（非 db.rs，见记忆 aidog-add-platform-skill）：
  - `getDefaultEndpoints(protocol, codingPlan?)` `:150` — 返回默认端点（含 base_url + protocol + client_type）。
  - `getDefaultModels(protocol, codingPlan?)` `:371` — 返回默认模型（落 CreatePlatform.models）。
- Protocol 枚举约 62 变体（Rust models.rs ↔ TS api.ts 双写）。并非每个变体都有完整预设。
- 模型名月级腐化，改默认模型值**必须 WebSearch 核官方**（记忆 platform-default-model）；base_url 含版本前缀，禁额外拼接（记忆 url-construction-rule）。
- 小米：当前代码库无 xiaomi/mimo 痕迹，需新增其 openai 系列 base_url + 默认模型。

## Decision (ADR-lite)

- **Context**: 小米缺默认 openai base_url/模型，泛化为全平台预设不完整。
- **Decision**: 范围 = **补缺 + 校正过时**（不止补空，已有值也核实更新）；值来源 = **逐平台 WebSearch 核官方文档**，每个改动带来源引用。
- **Consequences**: 工作量大（~62 Protocol 变体），按平台分组并行派 subagent 研究+改预设；模型名随时间腐化，本次以核查日期为准。

## Open Questions

- 小米平台的确切官方 base_url（OpenAI 兼容端点）与推荐默认模型？→ exec 阶段 WebSearch 解决。

## Requirements (evolving)

- [ ] 每个平台预设有非空默认 base_url（适用平台）与默认模型。
- [ ] 小米平台预设补齐（openai 系列 base_url + 默认模型）。

## Acceptance Criteria (evolving)

- [ ] `getDefaultEndpoints` / `getDefaultModels` 覆盖目标平台集，无缺失。
- [ ] 改动的 base_url/模型有官方来源引用。
- [ ] yarn build 绿；如涉 Protocol 枚举改动 cargo build/clippy 绿。

## Definition of Done

- 预设补齐 + 官方来源标注；build 绿；不破坏 url-construction-rule。

## Out of Scope (explicit)

- 小米 coding plan 配额查询（独立 task `06-17-xiaomi-coding-plan`）。
- 平台余额/coding plan quota 查询逻辑（quota.rs）。

## Technical Notes

- 触点：`src/pages/Platforms.tsx`（getDefaultEndpoints/getDefaultModels）；若新增 Protocol 变体则 `models.rs` + `api.ts` 双写。
- 参考记忆：aidog-add-platform-skill、platform-default-model、url-construction-rule、platform-protocol-design。
- 工具：skill `aidog-add-platform` 覆盖加/改平台全套触点。
