---
title: api-payload-optimization
layer: recall
category: optimization
keywords: [api,payload,ipc,distinct,set-deduplication,query-optimization]
status: active
---

## 后端 DISTINCT 替代前端集合去重降低 IPC payload

后端改为返回去重后的单列（如 DISTINCT model），而非拉全字段摘要行数组到前端，再用集合去重。

**收益**（logs-query-ipc-slimming s4）：
- 改前：拉 200 行 ProxyLogSummary（15 字段，单行 ~250-350 字节）→ 50-70 KB，前端 Set 去重
- 改后：后端 SELECT DISTINCT model LIMIT 200，返 Vec<String>（典型 10-30 模型值）→ 0.1-1 KB
- **降幅约两个数量级**；且改后字段窄到与用途完全对齐

**改法**：
1. 后端新增 query 命令（如 `distinct_models_proxy_log(db, filter, actual, limit)`）
2. 前端改为调该新命令，而非用通用 `listFiltered` 后前端 Set 去重
3. 验收：下拉选项内容与改前一致；EXPLAIN 无劣化（内层子查询同旧实现，外层对已 LIMIT 截断的行集再 DISTINCT，代价可忽略）

**适用场景**：
- 下拉列表取去重单列
- 聚合统计查询（SUM/COUNT BY 维度）
- 任何「前端集合操作作用于后端返回的大数组」的场景

**度量**：比对改前后 EXPLAIN QUERY PLAN 与实测耗时，确认无 I/O 退化
