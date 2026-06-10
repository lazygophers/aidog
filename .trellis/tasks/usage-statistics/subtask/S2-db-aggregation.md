---
id: S2
slug: db-aggregation
deliverable: D1
parent-task: usage-statistics
status: planned
execution-layer: main
isolation: none
depends-on: [S1]
blocks: [S3]
estimated-tokens: 4000
---

# S2 · 实现 DB 聚合查询

## 目标

在 `db.rs` 实现聚合查询函数，`lib.rs` 的 `stats_query` command 调用该函数返回完整统计数据。

## 产出

- `src-tauri/src/gateway/db.rs`: 新增 `query_stats` 方法
- `src-tauri/src/lib.rs`: 完善 `stats_query` command 实现

## 验证

```bash
cd src-tauri && cargo check
```

期望输出: 编译通过

## 资源

- 独占文件: `src-tauri/src/gateway/db.rs` (新增方法)
- 共享文件: `src-tauri/src/lib.rs` (修改 stats_query command)

## 依赖

| 上游 | 需要的产出 | 等待方式 |
| --- | --- | --- |
| S1 | StatsQuery / StatsResult 结构体存在 | 代码存在 |

## 执行细节

1. `db.rs` 新增 `Db::query_stats(&self, query: &StatsQuery) -> Result<StatsResult, String>`:
   - **时间范围**: 默认近 7 天。`start` / `end` 传入则用 `WHERE created_at BETWEEN ? AND ?`
   - **时间桶**: `granularity = hourly` → `strftime('%Y-%m-%d %H:00', created_at)`；`daily` → `strftime('%Y-%m-%d', created_at)`
   - **聚合 SQL**:
     ```sql
     SELECT
       strftime(?, created_at) AS time_bucket,
       COUNT(*) AS total_requests,
       SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) AS success_count,
       SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END) AS error_count,
       SUM(input_tokens) AS input_tokens,
       SUM(output_tokens) AS output_tokens,
       AVG(duration_ms) AS avg_duration_ms
     FROM proxy_logs
     WHERE created_at BETWEEN ? AND ?
       AND (? IS NULL OR group_name = ?)
       AND (? IS NULL OR model = ? OR actual_model = ?)
       AND (? IS NULL OR target_protocol = ?)
     GROUP BY time_bucket
     ORDER BY time_bucket
     ```
   - **overview**: 单独一条无 GROUP BY 的聚合查询
   - **dimension_data**: 按 `group_by` 维度做另一条 GROUP BY 查询
2. `lib.rs` 中 `stats_query` 调用 `db.query_stats()`

## 回滚

```bash
git checkout -- src-tauri/src/gateway/db.rs src-tauri/src/lib.rs
```

## 风险

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 大量日志时聚合慢 | 页面加载慢 | 限制默认时间范围 7 天；已有 created_at 索引 |

## 历史

- 2026-06-10: created
