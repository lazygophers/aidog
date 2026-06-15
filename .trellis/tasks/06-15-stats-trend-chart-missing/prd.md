# 请求趋势图不显示

## 现象
Stats 页「请求趋势」区域完全不显示。前端条件 `buckets.length > 0`（Stats.tsx:347），buckets 空 = 图不渲染。

## 诊断
- 用户能看「今日缓存率 2025%」= `today_stats()` 独立 SQL 路径正常，proxy_log 有今日数据。
- stats 页用 `query_stats()` → `query_stats_inner`。回归 commit 6c358f2 的 dimension platform `LEFT JOIN platform` 致 `ambiguous column: deleted_at` → query_stats 报错 → 前端 data=null → buckets=[] → 图不渲染。
- 回归已修（commit 9a43436，cargo test 292 passed），**但 Rust 改动需重启 `yarn tauri dev`**，用户跑的可能是回归版本。

## 方案
1. 加 query_stats buckets 非空断言到测试（防回归 + 证后端 OK）。
2. 指引用户重启 dev。

## 验证
- cargo test 新断言过（buckets 非空）。
