---
title: pagination-offset
layer: recall
category: db
keywords: [pagination,limit,offset,has_more,count,full-table-scan]
status: active
---

## LIMIT+1 探测分页无精确总数

当分页 UI 仅需「有无下一页」而不需精确总数时，改用 LIMIT offset+pageSize+1 探测有下一页，而非 COUNT(*)。

**改法**：
1. 后端查询改为 LIMIT limit+1；多取 1 行用于判定 has_more，再 truncate(limit) 不下发到前端
2. 前端分页组件接收 has_more boolean 而非 total，UI 展示「第 N 页 / 有更多/已到底」代替精确总数
3. 页码按钮列表改为只保留首页/上页/下页（下页在 !hasMore 时 disabled）

**优势**：
- 消除每次分页都对大表 COUNT(*) 的全表扫描（如 8GB proxy_log 表，18k+ 行，每 500ms 轮询一次）
- 查询成本恒定（LIMIT 有界），与表增长无关

**示例**（logs-query-ipc-slimming s2）：
- 改前：Promise.all(listFiltered + countFiltered)，COUNT 每 500ms 执行一次
- 改后：仅 listFiltered(LIMIT 21)，has_more = rows.len() > 20；返 ProxyLogPage { items, has_more }
- 验收：EXPLAIN 证明不含 SCAN proxy_log 全表扫；转发期 COUNT 查询次数为 0；payload 略增（+1 bool 字段）但代价可忽略

**约束**：
- 仅适用于不需精确总数的场景；如需精确数字，退到近似计数（或缓存 COUNT 结果直到某个时间窗口）
- 深分页（offset 很大）成为新瓶颈时可考虑游标分页
