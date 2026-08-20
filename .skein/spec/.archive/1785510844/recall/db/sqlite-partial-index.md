---
title: sqlite-partial-index
layer: recall
category: db
keywords: [sqlite,partial-index,query-plan,parameter-binding,sargable]
status: active
---

## 参数化查询无法触发 partial index（字面量盲区）

SQLite 查询规划器对 partial index 的匹配仅识别 SQL 文本中的**字面量常量**谓词，不识别**参数化绑定**（?1, ?2...）。

**现象**：
```sql
-- 字面量版本（在 SQL 文本里）
SELECT ... WHERE source_protocol NOT IN ('test','quota')
-- 规划器选中对应 partial index
QUERY PLAN: SCAN ... USING INDEX idx_xxx

-- 参数化版本（即使 parameter 值等同）
SELECT ... WHERE source_protocol NOT IN (?1, ?2)
-- 规划器无法匹配 WHERE deleted_at=0 AND source_protocol NOT IN ('test','quota') 的 partial index
QUERY PLAN: SCAN ... （无 USING INDEX）
```

**测试复现**（logs-query-ipc-slimming s3）：
- 合成库 5 万行，2% 穿插 test/quota
- 建 `CREATE INDEX idx_try(source_protocol, created_at) WHERE deleted_at=0 AND source_protocol NOT IN ('test','quota')`
- 字面量查询：走新索引，0ms
- 参数化查询：不走新索引，仍使用旧索引或全表扫

**设计决策**：
- 若代码出于安全考虑（禁字符串拼接用户输入）必须走参数化，**不要寄希望 partial index 能优化该查询**
- 转而：(1) 依赖既有通用索引的提前终止（LIMIT 提高效率）；(2) 若确需过滤优化，改成应用层过滤（预查 id set 后批量 IN）

**不采纳建议**：覆盖索引（写放大成本）、字符串拼接（SQL 注入风险）
