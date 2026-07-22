---
title: 跨库读两阶段 (禁同闭包跨库读/JOIN)
layer: core
category: arch
keywords: [db,sqlite,拆库,跨库,两阶段,聚合,handle,路由,粒度]
source: stats-agg-to-main-db
created: 1752940800
---

# 跨库读两阶段（禁同闭包跨库读/JOIN）

何时被读: 聚合/统计函数需读 A 库表（proxy_log 在 log.db）+ 写 B 库表（stats_agg_hourly 在主库）时
谁读: 拆库后 stats/agg 类跨库操作 sub-agent / main
不遵守的代价: 单 handle 闭包内跨库读 → `no such table`（编译/单测过，运行期崩）

---

## MUST 两阶段拆分

**禁同闭包跨库读**。聚合类（读源 + 写目标）拆两阶段：

```rust
// 阶段 1: 源库读池读 + 内存聚合
let agg = db.call_read_proxy_log_traced(.., |conn| {
    aggregate_proxy_logs(conn, &auto_map)  // 返 HashMap/Vec，不写
}).await?;
// 阶段 2: 目标库写槽批量写
db.call_traced(.., |conn| {
    upsert_aggregated(conn, &agg)
}).await?;
```

- `aggregate_*` 函数须「只读返内存结构」，禁内联写库
- 错误类型转换：`rusqlite::Error → tokio_rusqlite::Error::Other(e.into())`（call_*_traced 闭包要求 tokio_rusqlite::Result）

## 按粒度路由 handle（查询侧）

同文件查询不同粒度数据归属不同库时，按粒度 partition 路由：
- `stats_agg_hourly`（聚合，主库）→ `call_read_traced`
- `proxy_log` minute 粒度（原始，log.db）→ `call_read_proxy_log_traced`

批量混批按粒度 partition 保留原 idx 串行（≤2 次 IPC；纯批仍单次）。

## 内存库测试盲区

**单测用 in-memory conn 直接测，不走 Db handle 三池分离** → 生产文件库路由错（handle 配错库）单测不暴露。须 grep 残留 `call_*_traced` 逐点核对闭包读的表是否真归属该 handle 库，**禁只看调用文件名**（stats_agg.rs 里有 5 处 `call_read_proxy_log_traced` 是合法两阶段第①步读 proxy_log）。

## 反例

- ❌ `db.call_traced(|conn| { aggregate(conn) /* 读 proxy_log */ })` — 主库闭包读 log.db 表 → 运行期 no such table
- ❌ grep `call_*_proxy_log_traced` in stats_agg.rs 见命中就改 — 可能误改合法两阶段第①步
- ❌ 用 ATTACH 在运行期连接跨库 JOIN — 拆库禁 JOIN（[[sqlite-cross-db-no-join]]）
