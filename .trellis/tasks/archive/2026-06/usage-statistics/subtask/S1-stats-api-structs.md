---
id: S1
slug: stats-api-structs
deliverable: D1
parent-task: usage-statistics
status: planned
execution-layer: main
isolation: none
depends-on: []
blocks: [S2]
estimated-tokens: 2000
---

# S1 · 定义统计 API 数据结构与 Tauri command

## 目标

在 Rust 端定义统计查询的输入/输出结构体，注册 Tauri command，编译通过。

## 产出

- `src-tauri/src/gateway/models.rs`: 新增 `StatsQuery`, `StatsBucket`, `StatsResult`
- `src-tauri/src/lib.rs`: 新增 `stats_query` command + 注册到 `invoke_handler`

## 验证

```bash
cd src-tauri && cargo check
```

期望输出: 编译通过，无 error

## 资源

- 独占文件: `src-tauri/src/gateway/models.rs` (新增部分), `src-tauri/src/lib.rs` (新增 command)

## 依赖

无上游依赖。

## 执行细节

1. 在 `models.rs` 新增：
   ```rust
   pub struct StatsQuery {
       pub start: Option<String>,    // ISO 8601
       pub end: Option<String>,      // ISO 8601
       pub granularity: Option<String>, // "hourly" | "daily"
       pub group_by: Option<String>,    // "platform" | "model" | "group"
       pub filter_group: Option<String>,
       pub filter_model: Option<String>,
       pub filter_protocol: Option<String>,
   }
   pub struct StatsBucket {
       pub time_bucket: String,
       pub total_requests: i32,
       pub success_count: i32,
       pub error_count: i32,
       pub input_tokens: i32,
       pub output_tokens: i32,
       pub avg_duration_ms: i32,
   }
   pub struct StatsOverview {
       pub total_requests: i32,
       pub success_rate: f64,
       pub total_input_tokens: i32,
       pub total_output_tokens: i32,
       pub avg_duration_ms: i32,
   }
   pub struct StatsResult {
       pub overview: StatsOverview,
       pub buckets: Vec<StatsBucket>,
       pub dimension: Option<String>,
       pub dimension_data: Vec<DimensionEntry>,
   }
   pub struct DimensionEntry {
       pub name: String,
       pub total_requests: i32,
       pub success_count: i32,
       pub input_tokens: i32,
       pub output_tokens: i32,
       pub avg_duration_ms: i32,
   }
   ```
2. 在 `lib.rs` 新增 `stats_query` command 桩（先返回空结果，S2 实现 DB 查询）
3. 注册到 `invoke_handler`

## 回滚

```bash
git checkout -- src-tauri/src/gateway/models.rs src-tauri/src/lib.rs
```

## 风险

无显著风险。

## 历史

- 2026-06-10: created
