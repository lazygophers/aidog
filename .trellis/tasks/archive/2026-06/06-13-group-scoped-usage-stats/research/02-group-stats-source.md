# Research: Groups 页卡片使用情况来源

- **Query**: Groups 卡片 usage stats 现在怎么算 — 平台求和 vs 按 group 查 proxy_log
- **Scope**: internal
- **Date**: 2026-06-13

## 结论速览

- **现状（Groups 卡片实际用的）= 前端对关联 platforms 的 `PlatformUsageStats` 求和**，与 CLAUDE.md / memory 记载一致，是**平台级聚合**。
- **后端其实已有按 group 查 proxy_log 的 query：`get_group_usage_stats` / command `group_usage_stats` / api `groupApi.usageStats`，但 Groups.tsx 当前没用它。**
- 差异根源：平台级聚合把该平台的**全部**请求（含被其他 group 使用的）都算进每个引用它的 group。

## Findings

### Files Found

| File Path | Description |
|---|---|
| `src/pages/Groups.tsx:251-296` | `load()`：拉 platform usageStats 后前端求和成 group stats |
| `src/pages/Groups.tsx:299-344` | `refreshStats()`：同款求和逻辑（请求完成后轻量刷新） |
| `src/services/api.ts:362` | `groupApi.usageStats(groupName)` → invoke `group_usage_stats`（**已存在但 Groups.tsx 未调用**） |
| `src-tauri/src/lib.rs:1155-1157,2490` | command `group_usage_stats` 已注册 |
| `src-tauri/src/gateway/db.rs:1320-1328` | `get_group_usage_stats` 按 group_name 查 proxy_log |
| `src-tauri/src/gateway/db.rs:1307-1318` | `get_platform_usage_stats` 平台维度 |
| `src-tauri/src/gateway/db.rs:1259-1305` | `usage_stats` 共用聚合 helper |

### Groups 卡片现状：前端对平台 stats 求和

`src/pages/Groups.tsx:251-292`（`load()`，`refreshStats()` 同构）：
```ts
// Load per-platform usage stats
const pStatsMap: Record<string, PlatformUsageStats> = {};
await Promise.all((p || []).map(async (plat) => {
  const s = await platformApi.usageStats(plat.id);   // 平台维度查询
  if (s && s.total_requests > 0) pStatsMap[plat.id] = s;
}));
// Aggregate group stats ... from associated platform stats
for (const g of d || []) {
  let total_requests = 0, ...;
  for (const gp of g.platforms) {
    const ps = pStatsMap[gp.platform.id];
    if (ps) {
      total_requests += ps.total_requests;   // ← 直接累加平台全量
      success_count  += ps.success_count;
      total_input_tokens += ps.total_input_tokens;
      ...
    }
  }
  statsMap[g.group.name] = { total_requests, ... };  // Groups.tsx:283
}
```

数据源是 `platformApi.usageStats(plat.id)`，即 `get_platform_usage_stats`。**Groups.tsx 不调用 `groupApi.usageStats`。**

### 平台维度 query（被求和的源）

`src-tauri/src/gateway/db.rs:1307-1318`：
```rust
pub async fn get_platform_usage_stats(db: &Db, platform_id: u64) -> Result<PlatformUsageStats, String> {
    let where_clause = "deleted_at = 0 AND (platform_id = ?1 OR (platform_id = 0 AND group_name IN (SELECT name FROM \"group\" WHERE auto_from_platform = ?2 AND deleted_at = 0)))";
    ...
}
```
按 `platform_id`（含 auto 分组回溯）统计该平台**所有** proxy_log，不区分发起的 group。

### 已存在的 group 维度 query（现成可用，Groups.tsx 未接）

`src-tauri/src/gateway/db.rs:1320-1328`：
```rust
pub async fn get_group_usage_stats(db: &Db, group_name: &str) -> Result<PlatformUsageStats, String> {
    let group_name = group_name.to_string();
    db.0.call(move |conn| {
        Ok(usage_stats(conn, "group_name = ?1 AND deleted_at = 0", &[&group_name])?)
    }).await...
}
```
command + TS 绑定也都齐：
- `src-tauri/src/lib.rs:1155-1157`（command），`lib.rs:2490`（注册到 invoke_handler）
- `src/services/api.ts:362`：`usageStats: (groupName: string) => invoke<PlatformUsageStats>("group_usage_stats", { groupName })`

### 差异根源（"平台全部数据 vs 分组请求数据"）

一个平台可被多个 group 引用。平台级求和下，平台 X 被 group A、B 同用时，X 的全部请求会**重复**计入 A 卡片和 B 卡片（各自把 X 全量加一遍）。新需求要的是"卡片只含该 group 发起的请求" → 应改用 `group_usage_stats`（按 group_name 过滤 proxy_log），各 group 只数自己 group_name 的行，互不重叠。

## Caveats / Not Found

- `get_group_usage_stats` / command / api 三层都已就绪，**新需求后端基本无需新增 query，主要是前端切换数据源**（见 03）。
- balance（余额）聚合仍是平台 `est_balance_remaining` 求和（Groups.tsx:270-271, 290），与 usage stats 是两条独立链路；新需求只针对 usage stats，余额是否一并改需产品确认。
