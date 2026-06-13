# Research: 今日统计数据源

- **Query**: 今日已用金额 / 缓存率 / token 总量 / 各平台当日使用 的现有 query/command；缺哪些；"只展示已用平台"如何过滤
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### 需求 4 项 vs 现有数据

| 需求项 | 现有？ | 来源 |
|---|---|---|
| ① 今日已用金额 | ✅ 有 | `db::today_stats().cost` = `SUM(est_cost)` 今日 |
| ② 缓存率 | ✅ 有 | `db::today_stats().cache_rate` = `cache_tokens/input_tokens*100` 今日 |
| ③ Token 总量 | ✅ 有 | `db::today_stats().tokens` = `SUM(input+output)` 今日 |
| ④ 各平台当日使用（只展示已用） | ❌ **缺** | 无「按 platform_id 分组的今日聚合」查询 |

### ①②③ — 全局今日统计（已有）

`db::today_stats(db)` — `src-tauri/src/gateway/db.rs:524-575`：
```rust
// 今日本地 00:00 起点 start_ms
SELECT COALESCE(SUM(input_tokens + output_tokens), 0),   // tokens
       COALESCE(SUM(cache_tokens), 0),
       COALESCE(SUM(input_tokens), 0),
       COUNT(*)                                          // total_requests
FROM proxy_log WHERE created_at >= ?1 AND deleted_at = 0
// cache_rate = cache_tokens / input_tokens * 100
// cost = SELECT COALESCE(SUM(est_cost),0.0) ... 同 WHERE
```
返回 `TodayStats { tokens, cache_rate, cost, total_requests }`（`db.rs:512-521`）。

Command 暴露：
- `tray_today_stats`（`lib.rs:277`）→ 前端 `trayConfigApi.todayStats()`（`api.ts:426`）。
- `popover_data`（`lib.rs:311`）内部直接 `db::today_stats(&db)`。

→ **①②③ 直接复用 `today_stats`，无需新增。** popover.tsx 已展示此三项 + reqs（`src/popover.tsx:122-142`）。

### ④ — 各平台当日使用（缺，需新增）

现有平台维度聚合 `db::get_platform_usage_stats(db, platform_id)` — `db.rs:1307-1318`，底层 `usage_stats()`（`db.rs:1259-1305`）：
```rust
SELECT COUNT(*), SUM(success...), SUM(input_tokens), SUM(output_tokens),
       SUM(cache_tokens), COALESCE(SUM(est_cost),0.0)
FROM proxy_log WHERE deleted_at = 0 AND (platform_id = ?1 OR ...auto group回溯)
```
返回 `PlatformUsageStats { total_requests, success_count, total_input_tokens, total_output_tokens, total_cache_tokens, cache_rate, recent_failures, recent_total, total_cost }`（`models.rs:719-733`）。

**两个缺口**：
1. **无时间过滤** — `get_platform_usage_stats` 是**累计全时段**，`where_clause` 无 `created_at >= today`。要「当日」需加日期条件。
2. **无一次性按平台分组** — 现状是「单平台一次查询」，要列「所有当日已用平台」需 `GROUP BY platform_id`（一次取回多平台），否则要 N 次调用 + 需先知道平台列表。`grep "GROUP BY platform_id"` → 无匹配（确认不存在）。

→ **需新增** 一个 query，如 `today_platform_stats()`：`WHERE created_at >= start_ms AND deleted_at = 0 GROUP BY platform_id`，返回 `Vec<{platform_id, tokens, cost, requests, cache_rate}>`。

### 「只展示已用的平台」如何过滤

天然由 `GROUP BY platform_id` + `created_at >= 今日` 得到——只有今日有日志的 platform_id 才出现在结果里，即「已用」。无需额外平台启用判断。

注意 `platform_id=0`（自动分组日志）回溯逻辑见 `get_platform_usage_stats`（`db.rs:1310-1311`）：自动分组日志 platform_id 可能为 0，需经 `group.auto_from_platform` 回溯到真实平台。新增今日按平台聚合时若要精确归属需复用此回溯（否则 platform_id=0 会聚成一团「未知平台」）。平台名需 JOIN/查 platform 表（`get_platform_usage_stats` 仅返回数值，平台名由前端 `platformApi.list()` 映射，见 `TrayConfigTab.tsx:217` `platforms.find`）。

## Caveats / Not Found

- 余额/coding 配额（`quota.rs` / `platform.est_balance_remaining` / `platform.est_coding_plan`）是**平台维度的预估余额/利用率**，不是「当日使用量」。tray 平台列用的是这个（`TrayConfigTab.tsx:106-115`），与需求④「当日已用」语义不同——需求④要的是当日 token/花费/请求数，应走 proxy_log 聚合而非 quota。
- 时区：所有今日查询用本地时区 00:00（`chrono::Local`），与 `today_stats` 一致即可。
