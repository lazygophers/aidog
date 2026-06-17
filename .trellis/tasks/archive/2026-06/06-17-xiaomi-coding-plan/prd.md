# 小米 coding plan 配额支持

## Goal

为 aidog 增加对「小米」平台 coding plan 订阅配额的查询与展示支持，对齐现有 Kimi / GLM / MiniMax 的 coding plan 能力（余额栏展示 5h/周等用量 tier）。

## What I already know

- coding plan 现有实现在 `src-tauri/src/gateway/quota.rs`：
  - 按 `base_url` 子串分派（`query_quota` `:373`），命中后调 `query_<platform>_coding_plan`，返回 `CodingPlanInfo { tiers, level }`。
  - 参考实现：Kimi（`:243` GET `api.kimi.com/coding/v1/usages`，绝对 limit/remaining）、GLM 智谱（`:287` GET `{base}/api/monitor/usage/quota/limit`，percentage + unit 分类 5h/weekly）。
- 当前代码库**无任何** xiaomi / 小米 / MiMo 痕迹（grep 全空）→ 小米平台预设很可能也未添加。
- 加平台 / 加 coding plan 全套触点见 skill `aidog-add-platform`（Protocol 枚举 Rust↔TS 双写、前端 Platforms.tsx 预设、quota 按 base_url 子串分派）。

## Open Questions

- [Blocking] 小米 coding plan 配额查询 API 规格（endpoint / 鉴权 header / 响应 JSON 结构）从哪获取？
- [Blocking/Preference] 小米平台预设是否需要一并添加（平台下拉 + base_url + 端点协议），还是仅补 coding plan 查询分支？
- 小米 coding plan 暴露哪些 tier（5h 窗口 / 周限额 / 月限额 / 绝对量 or 百分比）？

## Decision (ADR-lite)

- **Context**: research（`research/xiaomi-coding-plan-api.md`）证实小米 MiMo Token Plan 无 API-Key 可访问的配额查询接口（`/api/v1/usage` 走 SSO Cookie，拒 API Key），照 Kimi/GLM 模式接入不可行。
- **Decision**: 小米 coding plan **改走 manual_budget 手动月度额度兜底**（通用功能，已存在 `manual_budget.rs`），不新增配额查询 API。
- **Consequences**: 本 task 无（或极少）新代码——manual_budget 已对任意平台通用。小米平台预设修正（openai 端点/token-plan host/codingPlan 标记）归 `06-17-platform-presets-fill`。本 task 待确认无代码后归档。

## Requirements (evolving)

- [ ] 小米平台可用 manual_budget 设置月度额度并展示（验证现有通用功能对小米生效）。

## Acceptance Criteria (evolving)

- [ ] `query_quota` 命中小米 base_url → 返回 `CodingPlanInfo` 且 tier 正确。
- [ ] cargo test / clippy 绿。

## Out of Scope (explicit)

- 小米模型的定价估算接入（除非 coding plan 展示必需）。
- **小米平台预设（base_url + 默认模型）补齐**：已拆出到独立 task `06-17-platform-presets-fill`（含小米及所有平台），本 task 仅保留 coding plan 配额查询（quota.rs 分派 + query 函数）。

## Technical Notes

- 触点文件：`src-tauri/src/gateway/quota.rs`（dispatch + query fn）、可能 `models.rs`/前端 `Platforms.tsx`（若需平台预设）。
- 参考记忆：quota-service、coding-plan-client-type-whitelist、aidog-add-platform-skill。
