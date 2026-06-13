# Research: proxy_log.group_name 现状

- **Query**: proxy_log.group_name 字段定义 + 写入时机 + 空值情况
- **Scope**: internal
- **Date**: 2026-06-13

## 结论速览

- `group_name` 列**早已存在**（migration 001，非后加列），有专用索引 `idx_proxy_log_group`。
- 正常成功请求**会填充** group_name（在解析分组后、调用上游前写入）。
- **存在空值情况**：请求体读取失败、无匹配分组（404）等早退路径会留下 `group_name = ''`。

## Findings

### Files Found

| File Path | Description |
|---|---|
| `src-tauri/migrations/001_init.sql:68-96` | proxy_log 表定义，第 70 行 `group_name` 列 |
| `src-tauri/migrations/001_init.sql:97` | `idx_proxy_log_group` 索引 |
| `src-tauri/src/gateway/proxy.rs:494-523` | log 初始化，group_name 初值空串 |
| `src-tauri/src/gateway/proxy.rs:586-614` | resolve_group → 写入 group_name |
| `src-tauri/src/gateway/db.rs:1001-1052` | proxy_log 列序常量 + INSERT |

### 字段定义（migration 001）

`src-tauri/migrations/001_init.sql:68-70`
```sql
CREATE TABLE IF NOT EXISTS proxy_log (
    id                        TEXT PRIMARY KEY,
    group_name                TEXT NOT NULL DEFAULT '',
```

注意：group_name 不是后加的 ALTER 列。db.rs `init_tables()` 的 migration 004–010 全是 ALTER（est_cost / is_stream 等），**没有**针对 group_name 的迁移，说明它从建表起就在（`src-tauri/src/gateway/db.rs:82-110`）。

### 索引（migration 001:97）

```sql
CREATE INDEX IF NOT EXISTS idx_proxy_log_group ON proxy_log(group_name) WHERE deleted_at = 0;
```

部分索引（`WHERE deleted_at = 0`），正好匹配 `get_group_usage_stats` 的查询条件 `group_name = ?1 AND deleted_at = 0`。**按 group 聚合的索引已现成。**

### 写入时机（proxy.rs）

1. **初始化为空串** — `src-tauri/src/gateway/proxy.rs:494-496`
```rust
let mut log = ProxyLog {
    id: request_id,
    group_name: String::new(),
    ...
```

2. **Upsert #1（请求刚收到，group 未解析）** — `proxy.rs:583-584`，此刻 group_name 仍为 `''`。

3. **解析分组后写入** — `proxy.rs:587-614`
```rust
let group = {
    match resolve_group(&state.db, auth_header.as_deref(), &path).await {
        Some(g) => g,
        None => { /* 404 早退，group_name 保持 '' */ }
    }
};
// Upsert #2: group resolved
log.group_name = group.name.clone();    // proxy.rs:609
...
upsert_log(&state, &log, &log_settings).await;  // proxy.rs:614
```

group_name 来源 = **router 匹配到的 group 的 name**（`group.name`），由 `resolve_group(db, auth_header, path)` 决定。鉴权令牌即 group_name（CLAUDE.md: Authorization Bearer `<group_name>`）。

### 空值（group_name = ''）情况

凡在 `resolve_group` 返回前就早退的请求，行被持久化但 group_name 留空：

- **请求体读取失败** — `proxy.rs:563-571`，status 400，upsert 后返回，group_name 仍 `''`。
- **无匹配分组** — `proxy.rs:590-604`，status 404（token 不匹配任何 group / path 无法匹配），upsert 后返回，group_name 仍 `''`。

成功路径在 `proxy.rs:609` 已设 group_name，后续上游调用、平台路由（`proxy.rs:693` 设 platform_id）都在此之后，故**正常完成的请求 group_name 必有值**。

## Caveats / Not Found

- 未实测线上库是否有历史空 group_name 行；逻辑上 400/404 早退行会是空。新需求若按 group_name 聚合，这些空行天然不计入任一 group（符合预期），但 Logs 页全量统计需注意。
- `resolve_group` 函数体未逐行读（仅确认其为 group_name 唯一来源）。如需确认"直连无 token"是否也能匹配默认 group，需进一步读 `resolve_group` 实现。
