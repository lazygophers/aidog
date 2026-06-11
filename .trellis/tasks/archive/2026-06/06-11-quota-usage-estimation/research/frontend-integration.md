# Research: 前端接入

- **Query**: quotaMap 当前状态；预估值存 platform 列后前端如何拿回 + 展示预估 vs 真实
- **Scope**: internal
- **Date**: 2026-06-11

## 现状

- `quotaMap: Record<number, PlatformQuota>`（Platforms.tsx:789）—— **纯前端 state，不持久化**
- 填充：`load()`（:855）批量调 `quotaApi.query(baseUrl, api_key)`（:875，每平台一次真实上游查）；`refreshQuota(p)`（:887）单个手动刷新（关联刚做的 quota 刷新图标 `quotaRefreshing` state :893）
- 展示：`quotaMap[p.id]`（:1672）→ balance badge / coding_plan tiers 按 utilization% 渲染 StatBadge（:1681-1685，颜色按 <50/<80/≥80 分档）
- API 类型：`PlatformQuota`（api.ts:537）/ `quotaApi.query`（api.ts:546）→ command `platform_query_quota`
- platform_list 命令：`platformApi.list()`（Platforms.tsx:857）→ 返回 `Platform[]`，当前 **不含任何 quota 字段**（Platform struct models.rs:261 无 est_*）

## 接入方案（设计建议）

### 数据回传路径
预估值存 platform 列后，两条路：
- **路 A（推荐）**：扩展 `Platform` struct（models.rs:261）+ api.ts `Platform` 接口，加 `est_balance_remaining` / `est_coding_plan`（解析为 PlatformQuota 形态）/ `last_real_query_at` / `estimate_count`。`platformApi.list()` 天然带回预估值 → load() 不再需要每平台同步真查（去掉 :870-878 的批量 query，改用 list 带回的预估 + 仅在过期时刷新）。
- **路 B**：新增专用 command `platform_estimated_quota(id)` 返回预估 PlatformQuota。改动小但多一次 IPC。

### 预估 vs 真实标识
- `last_real_query_at` 回传前端 → 展示 "预估"/"实测" 标签（如 badge 加 `~` 前缀或副标 "预估，N 分钟前校准"）。
- 与现有 quota 刷新图标（refreshQuota / quotaRefreshing :893）整合：点刷新 = 强制真查 + 覆盖预估（即手动触发校准），刷新后标记转"实测"。
- coding plan 拟合平台（GLM/MiniMax）预估精度低，UI 宜明确标 "预估" 并允许一键真查。

### 改动点清单
1. `models.rs:261` Platform struct — 加 est_* 字段（serde）
2. `db.rs` row_to_platform（:82）+ get_group_platforms 内联 parser（:411）+ PLATFORM_COLUMNS（:74）同步（见 02 文档）
3. `src/services/api.ts` Platform 接口（搜 `interface Platform`）— 加字段
4. `src/pages/Platforms.tsx:855 load()` — 改为用 list 带回预估 + 按 last_real_query_at 决定是否刷新
5. `Platforms.tsx:1672-1685` 渲染 — 加预估/实测标识
6. `Platforms.tsx:887 refreshQuota` — 语义=手动校准，刷新后写回（若后端校准已落库，前端 refresh 可直接重 list）

## Caveats
- 当前 load() 每次进页面对所有平台真查上游（:870-878），频繁。本特性目标正是降低此频率——预估存库后 load 应优先读库预估，仅过期才真查。
- api.ts Platform 接口具体行号未定位（搜 `export interface Platform`），实现时确认。
